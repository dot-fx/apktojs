use std::collections::{HashMap, HashSet};
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, TaggedStmt};
use crate::translator::dalvik::interpreter::ir::RegId;

pub fn rename(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut current: HashMap<u8, usize> = HashMap::new();
    let mut next: HashMap<u8, usize> = HashMap::new();
    let locked: HashSet<u8> = HashSet::new();
    // Top-level statements aren't inside any loop, so any stray `Break`
    // here (shouldn't normally happen) has nowhere meaningful to report
    // its state to; this collector is discarded.
    let mut break_states: Vec<HashMap<u8, usize>> = Vec::new();

    rename_block(stmts, &mut current, &mut next, &locked, &mut break_states)
}

fn collect_assigned_regs(stmts: &[JsStmt], out: &mut HashSet<u8>) {
    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, .. } => { out.insert(reg.reg); }
            JsStmt::If { then_body, else_body, .. } => {
                collect_assigned_regs(then_body, out);
                collect_assigned_regs(else_body, out);
            }
            JsStmt::Loop { body } | JsStmt::While { body, .. } | JsStmt::DoWhile { body, .. } => {
                collect_assigned_regs(body, out);
            }
            JsStmt::Switch { cases, default, .. } => {
                for (_, body) in cases { collect_assigned_regs(body, out); }
                if let Some(body) = default { collect_assigned_regs(body, out); }
            }
            _ => {}
        }
    }
}

fn pre_declare_regs(
    stmts: &[JsStmt],
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
) {
    let mut assigned = HashSet::new();
    collect_assigned_regs(stmts, &mut assigned);
    for r in assigned {
        // Only reserve a version if this register already has a value
        // from before the branch. If it doesn't, there's no legitimate
        // "current" value to speak of yet -- pre-declaring one here just
        // creates a phantom version that nothing ever assigns, and any
        // read-before-write inside the branch would reference it.
        // Let the branch's real first assignment claim its own version
        // via `bump()` instead.
        if current.contains_key(&r) {
            continue;
        }
        // no-op: do not insert a synthetic version for registers with no prior value
    }
}


fn branch_falls_through(body: &[JsStmt]) -> bool {
    !stmts_always_terminate(body)
}

/// Whether every runtime path through `body` ends in `return`/`break`/
/// `continue` -- i.e. control never falls off the end of it. This has to
/// be recursive: checking only whether the *last statement* is literally
/// `Return`/`Break`/`Continue` misses the extremely common case where the
/// last statement is an `if`/`else` (or exhaustive `switch`) whose *every*
/// arm terminates. Missing that made `merge_parallel` think such a branch
/// "falls through" when it doesn't, causing it to compute a bogus merge
/// version and inject a synchronizing assignment that reads a register
/// version nothing on that path ever actually produced.
fn stmts_always_terminate(body: &[JsStmt]) -> bool {
    match body.last() {
        Some(JsStmt::Return(_)) | Some(JsStmt::Break) | Some(JsStmt::Continue) => true,

        // An `if` only guarantees termination if it has a real `else`
        // (an absent/empty else is an implicit "do nothing" path that
        // falls through) and *both* arms terminate.
        Some(JsStmt::If { then_body, else_body, .. }) => {
            !else_body.is_empty()
                && stmts_always_terminate(then_body)
                && stmts_always_terminate(else_body)
        }

        // A `switch` only guarantees termination if every case terminates
        // AND there's a `default` (no default means "nothing matched"
        // falls through) that also terminates.
        Some(JsStmt::Switch { cases, default, .. }) => match default {
            Some(default_body) => {
                stmts_always_terminate(default_body)
                    && cases.iter().all(|(_, case_body)| stmts_always_terminate(case_body))
            }
            None => false,
        },

        // `Loop`/`While`/`DoWhile` are left conservative (not treated as
        // always-terminating) -- their own exit handling is covered
        // separately, and getting this wrong here would only cause an
        // unnecessary (harmless) fixup attempt, not a bogus one.
        _ => false,
    }
}

fn merge_parallel(
    entry: &HashMap<u8, usize>,
    branch_finals: &[HashMap<u8, usize>],
    branch_bodies: &mut [&mut Vec<JsStmt>],
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
) {
    debug_assert_eq!(branch_finals.len(), branch_bodies.len());
    if branch_finals.is_empty() {
        return;
    }

    let mut all_regs: HashSet<u8> = HashSet::new();
    for m in branch_finals {
        for &r in m.keys() {
            all_regs.insert(r);
        }
    }
    for &r in entry.keys() {
        all_regs.insert(r);
    }

    for r in all_regs {
        let entry_ver = entry.get(&r).copied();

        // Determine each branch's version for r. If a branch doesn't write
        // r, it keeps whatever was live at entry (if any). If there's no
        // entry value AND this branch doesn't write it, this register has
        // no real value on this path -- bail out on merging it entirely
        // rather than fabricating a version.
        let mut finals: Vec<usize> = Vec::with_capacity(branch_finals.len());
        let mut has_gap = false;
        for m in branch_finals {
            match m.get(&r).copied().or(entry_ver) {
                Some(v) => finals.push(v),
                None => { has_gap = true; break; }
            }
        }
        if has_gap {
            // Not defined on every path -- any post-merge read of this
            // register would itself be reading something undefined in the
            // original bytecode too (i.e. genuinely dead/unreachable on
            // whichever path lacks it), so don't touch `current`/`next`
            // for it. Leave it exactly as it was before this branch.
            continue;
        }

        let first = finals[0];
        if finals.iter().all(|&v| v == first) {
            current.insert(r, first);
            let candidate = first + 1;
            let nv = next.get(&r).copied().unwrap_or(candidate).max(candidate);
            next.insert(r, nv);
            continue;
        }

        let merge_ver = finals.iter().copied().max().unwrap_or(entry_ver.unwrap_or(0)) + 1;

        for (branch_ver, body) in finals.iter().zip(branch_bodies.iter_mut()) {
            if *branch_ver != merge_ver && branch_falls_through(body.as_slice()) {
                body.push(JsStmt::Assign {
                    reg: RegId { reg: r, version: merge_ver },
                    expr: JsExpr::Reg(RegId { reg: r, version: *branch_ver }),
                });
            }
        }

        current.insert(r, merge_ver);
        next.insert(r, merge_ver + 1);
    }
}

fn rename_stmt(
    stmt: JsStmt,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
    locked: &HashSet<u8>,
    // Every register-version snapshot at the point of a `Break` reachable
    // from here without crossing into a nested loop's own body -- i.e.
    // the actual set of post-loop predecessors for whichever loop
    // encloses this statement. Populated by the `Break` case below and
    // consumed by `JsStmt::Loop`'s handling once the whole body has been
    // walked.
    break_states: &mut Vec<HashMap<u8, usize>>,
) -> JsStmt {
    match stmt {
        JsStmt::Break => {
            break_states.push(current.clone());
            JsStmt::Break
        }

        JsStmt::Assign { reg, expr } => {
            let expr = rename_expr(expr, current);
            let new_ver = bump(reg.reg, current, next, locked);
            JsStmt::Assign {
                reg: RegId { reg: reg.reg, version: new_ver },
                expr,
            }
        }

        JsStmt::Param { reg, name } => {
            current.entry(reg).or_insert(0);
            next.entry(reg).or_insert(1);
            JsStmt::Param { reg, name }
        }

        JsStmt::Return(e) =>
            JsStmt::Return(e.map(|e| rename_expr(e, current))),

        JsStmt::Expr(e) =>
            JsStmt::Expr(rename_expr(e, current)),

        JsStmt::FieldSet { receiver, field, value } => JsStmt::FieldSet {
            receiver: rename_expr(receiver, current),
            field,
            value: rename_expr(value, current),
        },

        JsStmt::ArraySet { arr, idx, value } => JsStmt::ArraySet {
            arr:   rename_expr(arr,   current),
            idx:   rename_expr(idx,   current),
            value: rename_expr(value, current),
        },

        JsStmt::StaticSet { class, field, value } => JsStmt::StaticSet {
            class,
            field,
            value: rename_expr(value, current),
        },

        JsStmt::CondGoto { cond, target } => JsStmt::CondGoto {
            cond: rename_expr(cond, current),
            target,
        },

        JsStmt::If { cond, then_body, else_body } => {
            let cond = rename_expr(cond, current);

            pre_declare_regs(&then_body, current, next);
            pre_declare_regs(&else_body, current, next);

            // An if/else isn't a back-edge -- each arm runs at most once per
            // visit, so registers reassigned inside a branch are free to
            // take on fresh SSA versions. We don't lock anything here
            // ourselves; we just forward whatever `locked` set we inherited
            // from an enclosing loop, if any.
            let entry = current.clone();

            let mut then_curr = current.clone();
            let mut then_next = next.clone();
            let mut then_body = rename_block(then_body, &mut then_curr, &mut then_next, locked, break_states);

            let mut else_curr = current.clone();
            let mut else_next = next.clone();
            let mut else_body = rename_block(else_body, &mut else_curr, &mut else_next, locked, break_states);

            {
                let finals = [then_curr, else_curr];
                let mut bodies: [&mut Vec<JsStmt>; 2] = [&mut then_body, &mut else_body];
                merge_parallel(&entry, &finals, &mut bodies, current, next);
            }

            JsStmt::If { cond, then_body, else_body }
        }

        JsStmt::Loop { body } => {
            // Loops *are* back-edges: a register reassigned inside the body
            // must keep the same identity across iterations (and be visible
            // to the next iteration's top), which a simple per-branch merge
            // can't express. Keeping the old "lock everything live on
            // entry" behavior here is intentional, not a bug -- it's what
            // makes the loop body share one ordinary mutable variable across
            // iterations, matching real loop semantics.
            //
            // But `Loop` (unlike `While`/`DoWhile`) has no implicit
            // condition-based exit at all -- every way out is an explicit
            // `Break`. That means the snapshots collected below ARE the
            // complete set of post-loop predecessors, and merging them
            // (like `merge_parallel` already does for if/else and switch)
            // is what correctly threads a register whose value depends on
            // *which* break fired -- e.g. a found-vs-not-found result --
            // through to whatever follows the loop. Previously nothing did
            // this merge at all: `current` was just left holding whatever
            // the final linear walk over the body happened to produce,
            // regardless of which break actually runs at execution time.
            pre_declare_regs(&body, current, next);
            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            let entry = current.clone();
            let mut body_curr = current.clone();
            let mut body_next = next.clone();
            let mut inner_breaks: Vec<HashMap<u8, usize>> = Vec::new();
            let mut body = rename_block(body, &mut body_curr, &mut body_next, &inner_locked, &mut inner_breaks);

            *next = body_next;

            if inner_breaks.is_empty() {
                // No break reaches here at all -- the loop never exits
                // (body always returns/throws), so nothing after it is
                // reachable. Leave `current` as the pre-loop state.
            } else {
                // One placeholder Vec per break, so merge_parallel has
                // somewhere to write a synchronizing assignment when a
                // register's version differs across exits; we then splice
                // each placeholder's contents in right before its
                // corresponding `Break`.
                let mut placeholders: Vec<Vec<JsStmt>> =
                    inner_breaks.iter().map(|_| Vec::new()).collect();
                {
                    let mut body_refs: Vec<&mut Vec<JsStmt>> =
                        placeholders.iter_mut().collect();
                    merge_parallel(&entry, &inner_breaks, &mut body_refs, current, next);
                }
                let mut adjustments = placeholders.into_iter();
                inject_before_breaks(&mut body, &mut adjustments);
            }

            JsStmt::Loop { body }
        }

        JsStmt::While { cond, body } => {
            let cond = rename_expr(cond, current);
            pre_declare_regs(&body, current, next);

            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            // NOTE: unlike `Loop`, a `While` has an *implicit* exit (the
            // condition going false) alongside any explicit `break`s
            // inside the body, and both land on the exact same place in
            // JS (the statement right after the loop) with no way to
            // tell them apart structurally. That means we can't attach a
            // per-exit fixup the way `Loop` does above without either
            // restructuring this into an explicit-break `Loop` or
            // accepting a possible mismatch for registers that genuinely
            // differ between the natural exit and a `break` exit. This
            // is the known remaining gap from the original bug report --
            // not fixed here, just not made worse. A `break` in this body
            // belongs to this `While` (not some enclosing loop), so its
            // own (discarded) collector is correct here regardless.
            let mut inner_breaks: Vec<HashMap<u8, usize>> = Vec::new();

            JsStmt::While {
                cond,
                body: rename_block(body, current, next, &inner_locked, &mut inner_breaks),
            }
        }

        JsStmt::DoWhile { body, cond } => {
            pre_declare_regs(&body, current, next);
            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            // See the note in `While` above -- same limitation applies.
            let mut inner_breaks: Vec<HashMap<u8, usize>> = Vec::new();
            let body = rename_block(body, current, next, &inner_locked, &mut inner_breaks);
            let cond = rename_expr(cond, current);
            JsStmt::DoWhile { body, cond }
        }

        JsStmt::Switch { expr, cases, default } => {
            let expr = rename_expr(expr, current);

            for (_, body) in &cases { pre_declare_regs(body, current, next); }
            if let Some(body) = &default { pre_declare_regs(body, current, next); }

            let entry = current.clone();

            let mut keys: Vec<_> = Vec::with_capacity(cases.len());
            let mut finals: Vec<HashMap<u8, usize>> = Vec::with_capacity(cases.len() + 1);
            let mut bodies: Vec<Vec<JsStmt>> = Vec::with_capacity(cases.len() + 1);

            for (key, body) in cases {
                let mut c_curr = current.clone();
                let mut c_next = next.clone();
                // A `break` inside a switch case breaks the switch, not
                // any enclosing loop, so it gets its own (discarded)
                // collector rather than the loop's `break_states`.
                let mut case_breaks: Vec<HashMap<u8, usize>> = Vec::new();
                let rb = rename_block(body, &mut c_curr, &mut c_next, locked, &mut case_breaks);
                keys.push(key);
                finals.push(c_curr);
                bodies.push(rb);
            }

            let has_default = default.is_some();
            if let Some(body) = default {
                let mut c_curr = current.clone();
                let mut c_next = next.clone();
                let mut case_breaks: Vec<HashMap<u8, usize>> = Vec::new();
                let rb = rename_block(body, &mut c_curr, &mut c_next, locked, &mut case_breaks);
                finals.push(c_curr);
                bodies.push(rb);
            }
            // NOTE: if there's no `default` and no case matches at runtime,
            // JS falls straight through the switch untouched -- that
            // implicit "nothing happened" path isn't modeled as a branch
            // here, so a register reassigned consistently by every present
            // case but lacking a default arm is still merged as if all
            // paths were covered. This mirrors a pre-existing limitation
            // (the same gap exists for any phi-style analysis without a
            // default/else) rather than introducing a new one, and matches
            // what Kotlin's exhaustive `when` produces in practice.

            {
                let mut body_refs: Vec<&mut Vec<JsStmt>> = bodies.iter_mut().collect();
                merge_parallel(&entry, &finals, &mut body_refs, current, next);
            }

            let default_body = if has_default { bodies.pop() } else { None };
            let cases: Vec<_> = keys.into_iter().zip(bodies.into_iter()).collect();

            JsStmt::Switch { expr, cases, default: default_body }
        }

        other => other,
    }
}

fn rename_block(
    stmts: Vec<JsStmt>,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
    locked: &HashSet<u8>,
    break_states: &mut Vec<HashMap<u8, usize>>,
) -> Vec<JsStmt> {
    stmts
        .into_iter()
        .map(|s| rename_stmt(s, current, next, locked, break_states))
        .collect()
}

/// Splices `adjustments[i]` in immediately before the i-th `Break`
/// encountered in `stmts` (in the same order `rename_block` visited
/// them), recursing into `If` bodies but deliberately not descending
/// into a nested `Loop`/`While`/`DoWhile`/`Switch` -- a `Break` in one of
/// those belongs to that construct, not the one whose exits we're
/// patching up here, and was never counted among `adjustments` in the
/// first place.
fn inject_before_breaks(
    stmts: &mut Vec<JsStmt>,
    adjustments: &mut impl Iterator<Item = Vec<JsStmt>>,
) {
    let mut i = 0;
    while i < stmts.len() {
        let is_break = matches!(stmts[i], JsStmt::Break);
        if is_break {
            if let Some(adj) = adjustments.next() {
                let n = adj.len();
                for (k, s) in adj.into_iter().enumerate() {
                    stmts.insert(i + k, s);
                }
                i += n;
            }
        } else if let JsStmt::If { then_body, else_body, .. } = &mut stmts[i] {
            inject_before_breaks(then_body, adjustments);
            inject_before_breaks(else_body, adjustments);
        }
        i += 1;
    }
}

fn rename_expr(expr: JsExpr, current: &mut HashMap<u8, usize>) -> JsExpr {
    match expr {
        JsExpr::Reg(id) => {
            let ver = *current.entry(id.reg).or_insert(0);
            JsExpr::Reg(RegId { reg: id.reg, version: ver })
        }

        JsExpr::MethodCall { receiver, method, args, is_static } =>
            JsExpr::MethodCall {
                receiver: Box::new(rename_expr(*receiver, current)),
                method,
                args: rename_exprs(args, current),
                is_static,
            },

        JsExpr::StaticCall { class, method, args } =>
            JsExpr::StaticCall {
                class,
                method,
                args: rename_exprs(args, current),
            },

        JsExpr::New { class, args } =>
            JsExpr::New { class, args: rename_exprs(args, current) },

        JsExpr::SuperCall { args } =>
            JsExpr::SuperCall { args: rename_exprs(args, current) },

        JsExpr::ThisCtorCall { args } =>
            JsExpr::ThisCtorCall { args: rename_exprs(args, current) },

        JsExpr::FieldGet { receiver, field } =>
            JsExpr::FieldGet {
                receiver: Box::new(rename_expr(*receiver, current)),
                field,
            },

        JsExpr::BinOp { op, left, right } =>
            JsExpr::BinOp {
                op,
                left:  Box::new(rename_expr(*left,  current)),
                right: Box::new(rename_expr(*right, current)),
            },

        JsExpr::UnaryOp { op, expr } =>
            JsExpr::UnaryOp {
                op,
                expr: Box::new(rename_expr(*expr, current)),
            },

        JsExpr::BitMask { expr, mask } =>
            JsExpr::BitMask {
                expr: Box::new(rename_expr(*expr, current)),
                mask,
            },

        JsExpr::Index { arr, idx } =>
            JsExpr::Index {
                arr: Box::new(rename_expr(*arr, current)),
                idx: Box::new(rename_expr(*idx, current)),
            },

        JsExpr::ArrayLiteral(exprs) =>
            JsExpr::ArrayLiteral(rename_exprs(exprs, current)),

        JsExpr::StringConcat(exprs) =>
            JsExpr::StringConcat(rename_exprs(exprs, current)),

        other => other,
    }
}

fn rename_exprs(exprs: Vec<JsExpr>, current: &mut HashMap<u8, usize>) -> Vec<JsExpr> {
    exprs.into_iter().map(|e| rename_expr(e, current)).collect()
}

fn bump(r: u8, current: &mut HashMap<u8, usize>, next: &mut HashMap<u8, usize>, locked: &HashSet<u8>) -> usize {
    if locked.contains(&r) {
        return *current.get(&r).unwrap();
    }

    let ver = *next.entry(r).or_insert(1);
    current.insert(r, ver);
    next.insert(r, ver + 1);
    ver
}
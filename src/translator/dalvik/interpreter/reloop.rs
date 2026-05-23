use std::collections::{HashMap, HashSet};
use crate::translator::dalvik::interpreter;
use crate::translator::dalvik::interpreter::{cfg, BasicBlock, JsExpr, JsStmt, TaggedStmt, Terminator};

const MAX_DEPTH: usize = 64;

pub fn structure_cfg(tagged: Vec<TaggedStmt>) -> Vec<JsStmt> {
    let tagged: Vec<TaggedStmt> = tagged
        .into_iter()
        .filter(|ts| {
            !(ts.offset == i32::MIN && matches!(&ts.stmt, JsStmt::Comment(c) if c.is_empty()))
        })
        .collect();

    if tagged.is_empty() { return vec![]; }

    let (prologue, body_tagged): (Vec<_>, Vec<_>) =
        tagged.into_iter().partition(|ts| ts.offset < 0);

    let mut out: Vec<JsStmt> = prologue.into_iter().map(|ts| ts.stmt).collect();

    if body_tagged.is_empty() { return out; }

    let blocks = cfg::build_blocks(body_tagged);
    if blocks.is_empty() { return out; }


    let b2i: HashMap<i32, usize> = blocks.iter().enumerate()
        .map(|(i, b)| (b.offset, i))
        .collect();

    let loop_headers: HashSet<i32> = {
        let mut h = HashSet::new();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        fn dfs(
            u_idx: usize,
            blocks: &[BasicBlock],
            b2i: &HashMap<i32, usize>,
            visiting: &mut HashSet<usize>,
            visited: &mut HashSet<usize>,
            h: &mut HashSet<i32>
        ) {
            visiting.insert(u_idx);

            for t in cfg::block_successors(&blocks[u_idx]) {
                if let Some(&v_idx) = b2i.get(&t) {
                    if visiting.contains(&v_idx) {
                        // True back-edge: target is currently on the recursion stack!
                        h.insert(t);
                    } else if !visited.contains(&v_idx) {
                        dfs(v_idx, blocks, b2i, visiting, visited, h);
                    }
                }
            }

            visiting.remove(&u_idx);
            visited.insert(u_idx);
        }

        if !blocks.is_empty() {
            for i in 0..blocks.len() {
                if !visited.contains(&i) {
                    dfs(i, &blocks, &b2i, &mut visiting, &mut visited, &mut h);
                }
            }
        }
        h
    };

    let preds = cfg::build_predecessors(&blocks, &b2i);

    reloop(
        &blocks,
        &b2i,
        &loop_headers,
        &preds,
        0,
        i32::MAX,
        None,
        None,
        &mut HashSet::new(),
        &mut out,
        0,
        None
    );

    out
}

#[allow(clippy::too_many_arguments)]
fn reloop(
    blocks:       &[BasicBlock],
    b2i:          &HashMap<i32, usize>,
    loop_headers: &HashSet<i32>,
    preds:        &[Vec<usize>],
    start_idx:    usize,
    until:        i32,
    loop_exit:    Option<i32>,
    current_loop: Option<i32>,
    visited:      &mut HashSet<usize>,
    out:          &mut Vec<JsStmt>,
    depth:        usize,
    consumed_pred_offset: Option<i32>,
){
    if depth > MAX_DEPTH { return; }

    let mut idx = start_idx;

    while idx < blocks.len() {
        let block = &blocks[idx];

        if block.offset >= until {
            break;
        }
        if visited.contains(&idx) {
            idx += 1;
            break;
        }

        if loop_headers.contains(&block.offset)
            && Some(block.offset) != current_loop
        {
            let loop_end = cfg::find_loop_end(blocks, idx, block.offset, b2i);

            let mut loop_visited = HashSet::new();
            loop_visited.insert(idx);

            let (header_cond, consumed_stmt, consumed_pred_offset) =
                match block_is_loop_guard(block, loop_end) {
                    Some(c) => {
                        let (cond, stmt, pred_off) =
                            resolve_loop_condition(block, c);
                        (Some(cond), stmt, pred_off)
                    }
                    None => (None, None, None),
                };

            let mut body = Vec::new();
            for (i, stmt) in block.stmts.iter().cloned().enumerate() {
                if Some(i) != consumed_stmt {
                    body.push(stmt);
                }
            }

            reloop(
                blocks,
                b2i,
                loop_headers,
                preds,
                idx + 1,
                loop_end,
                Some(loop_end),
                Some(block.offset),
                &mut loop_visited,
                &mut body,
                depth + 1,
                consumed_pred_offset
            );

            if matches!(body.last(), Some(JsStmt::Continue)) {
                body.pop();
            }

            if let Some(cond) = header_cond {
                let safe_to_hoist =
                    consumed_stmt.is_some()
                        || block.stmts.is_empty();

                if safe_to_hoist && !cond_regs_written_in_body(&cond, &body) {
                    out.push(JsStmt::While { cond, body });
                } else {
                    let mut safe_body = vec![
                        JsStmt::If {
                            cond: interpreter::negate(cond),
                            then_body: vec![JsStmt::Break],
                            else_body: vec![],
                        },
                    ];

                    safe_body.extend(body);

                    out.push(JsStmt::Loop {
                        body: safe_body,
                    });
                }
            } else if let Some((cond, break_idx)) = recover_loop_condition(&body) {
                let mut body = body;
                body.remove(break_idx);

                out.push(JsStmt::DoWhile { cond, body });
            } else {
                out.push(JsStmt::Loop { body });
            }

            idx = b2i.get(&loop_end).copied().unwrap_or(blocks.len());

            continue;
        }

        visited.insert(idx);

        let emit_stmts = if consumed_pred_offset == Some(block.offset) {
            &block.stmts[..block.stmts.len().saturating_sub(1)]
        } else {
            &block.stmts[..]
        };
        out.extend(emit_stmts.iter().cloned());

        match &block.term {
            Terminator::Return(e) => {
                out.push(JsStmt::Return(e.clone()));
                idx += 1;
                continue;
            }
            Terminator::ImplicitReturn | Terminator::Throw => {
                idx += 1;
                continue;
            }
            Terminator::Goto(t) => {
                let t = *t;
                if Some(t) == current_loop {
                    out.push(JsStmt::Continue);
                    break;
                }
                if loop_exit.map_or(false, |e| t >= e) {
                    out.push(JsStmt::Break);
                    break;
                }
                idx = b2i.get(&t).copied().unwrap_or(blocks.len());
                continue;
            }
            Terminator::FallThrough(next) => {
                idx = b2i.get(next).copied().unwrap_or(idx + 1);
                continue;
            }

            Terminator::CondGoto { cond, if_true, if_false } => {
                let (cond, if_true, if_false) = (cond.clone(), *if_true, *if_false);

                if Some(if_true) == loop_exit && if_false < if_true{
                    out.push(JsStmt::If {
                        cond,
                        then_body: vec![JsStmt::Break],
                        else_body: vec![],
                    });

                    idx = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                    continue;
                }

                if Some(if_false) == loop_exit && if_true < if_false {
                    out.push(JsStmt::If {
                        cond: interpreter::negate(cond),
                        then_body: vec![JsStmt::Break],
                        else_body: vec![],
                    });

                    idx = b2i.get(&if_true).copied().unwrap_or(blocks.len());
                    continue;
                }

                if Some(if_true) == current_loop {
                    out.push(JsStmt::If {
                        cond: cond.clone(),
                        then_body: vec![JsStmt::Continue],
                        else_body: vec![],
                    });

                    let fall_idx = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                    idx = fall_idx;
                    continue;
                }

                if Some(if_false) == current_loop {
                    out.push(JsStmt::If {
                        cond: interpreter::negate(cond.clone()),
                        then_body: vec![JsStmt::Continue],
                        else_body: vec![],
                    });

                    let branch_idx = b2i.get(&if_true).copied().unwrap_or(blocks.len());
                    idx = branch_idx;
                    continue;
                }

                let fall_idx   = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                let branch_idx = b2i.get(&if_true).copied().unwrap_or(blocks.len());

                let join = find_join_relooper(blocks, b2i, fall_idx, branch_idx, until, block.offset);

                let mut then_body = Vec::new();
                if branch_idx < blocks.len() {
                    let mut then_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           branch_idx, join, loop_exit, current_loop,
                           &mut then_visited, &mut then_body, depth + 1, None);
                }

                let mut else_body = Vec::new();
                let mut else_visited = visited.clone();
                reloop(blocks, b2i, loop_headers, preds,
                       fall_idx, join, loop_exit, current_loop,
                       &mut else_visited, &mut else_body, depth + 1, None);

                if !then_body.is_empty() || !else_body.is_empty() {
                    out.push(JsStmt::If {
                        cond,
                        then_body,
                        else_body,
                    });
                }

                idx = b2i.get(&join).copied().unwrap_or(blocks.len());
                continue;
            }

            Terminator::Switch { expr, cases, default } => {
                let (expr, cases, default) = (expr.clone(), cases.clone(), *default);

                let switch_end = cfg::find_switch_end(blocks, b2i, &cases, default, until);

                let mut target_order: Vec<i32> = Vec::new();
                let mut target_keys: HashMap<i32, Vec<i32>> = HashMap::new();
                for &(key, target) in &cases {
                    target_keys.entry(target).or_insert_with(|| {
                        target_order.push(target);
                        Vec::new()
                    }).push(key);
                }
                let mut resolved_cases: Vec<(i32, Vec<JsStmt>)> = Vec::new();
                let mut post_switch_visited: HashSet<usize> = HashSet::new();

                target_order.sort_by_key(|&t| {
                    target_keys[&t].iter().copied().min().unwrap_or(i32::MAX)
                });

                let mut offset_order = target_order.clone();
                offset_order.sort_by_key(|&t| b2i.get(&t).copied().unwrap_or(usize::MAX));
                let offset_stops: HashMap<i32, i32> = offset_order.iter().enumerate().map(|(ti, &t)| {
                    let next_t = offset_order.get(ti + 1).copied().unwrap_or(switch_end);
                    (t, next_t.min(switch_end))
                }).collect();

                for &t in target_order.iter() {
                    let stop = offset_stops[&t];

                    let case_start = b2i.get(&t).copied().unwrap_or(blocks.len());
                    let mut case_body = Vec::new();
                    let mut case_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           case_start, stop, loop_exit, current_loop,
                           &mut case_visited, &mut case_body, depth + 1, None);
                    post_switch_visited.extend(case_visited);

                    let mut keys = target_keys[&t].clone();
                    keys.sort();
                    for &k in &keys {
                        resolved_cases.push((k, case_body.clone())); // same body for all keys
                    }
                }

                let resolved_default = if default != -1 {
                    let default_start = b2i.get(&default).copied().unwrap_or(blocks.len());
                    let mut default_body = Vec::new();
                    let mut default_visited = visited.clone();

                    reloop(
                        blocks, b2i, loop_headers, preds,
                        default_start, switch_end, loop_exit, current_loop,
                        &mut default_visited, &mut default_body, depth + 1, None
                    );

                    post_switch_visited.extend(default_visited);
                    Some(default_body)
                } else {
                    None
                };

                visited.extend(post_switch_visited);
                out.push(JsStmt::Switch {
                    expr,
                    cases: resolved_cases,
                    default: resolved_default,
                });

                idx = b2i.get(&switch_end).copied().unwrap_or(blocks.len());
                continue;
            }
        }
    }
}

fn find_join_relooper(
    blocks:     &[BasicBlock],
    b2i:        &HashMap<i32, usize>,
    a_idx:      usize,
    b_idx:      usize,
    until:      i32,
    branch_off: i32,
) -> i32 {
    let forward_reachable = |start: usize| -> HashSet<i32> {
        let mut seen  = HashSet::new();
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let Some(b) = blocks.get(i) else { continue };
            if b.offset >= until  { continue; }
            if !seen.insert(b.offset) { continue; }
            for t in cfg::block_successors(b) {
                if t > b.offset {
                    if let Some(&ni) = b2i.get(&t) {
                        stack.push(ni);
                    }
                } else if t <= b.offset {
                    let header_idx = b2i.get(&t).copied().unwrap_or(blocks.len());
                    let loop_exit_off = cfg::find_loop_end(blocks, header_idx, t, b2i);
                    if loop_exit_off < until {
                        if let Some(&ei) = b2i.get(&loop_exit_off) {
                            stack.push(ei);
                        }
                    }
                }
            }
        }
        seen
    };

    let a_reach = forward_reachable(a_idx);
    let b_reach = forward_reachable(b_idx);

    let mut candidates: Vec<i32> = a_reach
        .intersection(&b_reach)
        .copied()
        .filter(|&o| o > branch_off && o <= until)
        .collect();
    candidates.sort_unstable();
    candidates.into_iter().next().unwrap_or(until)
}

fn block_is_loop_guard(
    block: &BasicBlock,
    loop_end: i32,
) -> Option<JsExpr> {
    match &block.term {
        Terminator::CondGoto { cond, if_true, if_false } => {
            let true_inside = *if_true < loop_end;
            let false_inside = *if_false < loop_end;

            match (true_inside, false_inside) {
                (true, false) => Some(cond.clone()),
                (false, true) => Some(interpreter::negate(cond.clone())),
                _ => None,
            }
        }
        _ => None,
    }
}

fn recover_loop_condition(body: &[JsStmt]) -> Option<(JsExpr, usize)> {
    if let Some((last_idx, stmt)) = body.iter().enumerate().last() {
        match stmt {
            JsStmt::If { cond, then_body, else_body }
            if else_body.is_empty()
                && then_body.len() == 1
                && matches!(then_body[0], JsStmt::Break)
            => {
                return Some((interpreter::negate(cond.clone()), last_idx));
            }
            _ => {}
        }
    }

    None
}

fn strip_double_negation(expr: JsExpr) -> JsExpr {
    match expr {
        JsExpr::UnaryOp { op, expr }
        if op == "!" =>
            {
                match *expr {
                    JsExpr::UnaryOp { op: inner_op, expr: inner_expr }
                    if inner_op == "!" =>
                        {
                            strip_double_negation(*inner_expr)
                        }

                    other => JsExpr::UnaryOp {
                        op,
                        expr: Box::new(strip_double_negation(other)),
                    },
                }
            }

        other => other,
    }
}

fn cond_regs_written_in_body(cond: &JsExpr, body: &[JsStmt]) -> bool {
    let mut cond_regs = HashSet::new();
    collect_expr_regs(cond, &mut cond_regs);
    if cond_regs.is_empty() { return false; }
    body_writes_any_reg(body, &cond_regs)
}

fn collect_expr_regs(expr: &JsExpr, out: &mut HashSet<u8>) {
    match expr {
        JsExpr::Reg(id) => { out.insert(id.reg); }
        JsExpr::BinOp { left, right, .. } => {
            collect_expr_regs(left, out);
            collect_expr_regs(right, out);
        }
        JsExpr::UnaryOp { expr, .. } => collect_expr_regs(expr, out),
        JsExpr::MethodCall { receiver, args, .. } => {
            collect_expr_regs(receiver, out);
            for a in args { collect_expr_regs(a, out); }
        }
        _ => {}
    }
}

fn body_writes_any_reg(stmts: &[JsStmt], regs: &HashSet<u8>) -> bool {
    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, .. } if regs.contains(&reg.reg) => return true,
            JsStmt::If { then_body, else_body, .. } => {
                if body_writes_any_reg(then_body, regs)
                    || body_writes_any_reg(else_body, regs) {
                    return true;
                }
            }
            JsStmt::Loop { body } | JsStmt::While { body, .. }
            | JsStmt::DoWhile { body, .. } => {
                if body_writes_any_reg(body, regs) { return true; }
            }
            _ => {}
        }
    }
    false
}

fn substitute_reg(expr: &mut JsExpr, target: u8, replacement: &JsExpr) {
    match expr {
        JsExpr::Reg(id) => {
            if id.reg == target {
                *expr = replacement.clone();
            }
        }
        JsExpr::BinOp { left, right, .. } => {
            substitute_reg(&mut **left, target, replacement);
            substitute_reg(&mut **right, target, replacement);
        }
        JsExpr::UnaryOp { expr: inner, .. } => {
            substitute_reg(&mut **inner, target, replacement);
        }
        JsExpr::MethodCall { receiver, args, .. } => {
            substitute_reg(&mut **receiver, target, replacement);
            for a in args {
                substitute_reg(a, target, replacement);
            }
        }
        _ => {}
    }
}

fn resolve_loop_condition(
    block: &BasicBlock,
    cond: JsExpr,
) -> (JsExpr, Option<usize>, Option<i32>) {
    let mut cond = strip_double_negation(cond);

    let mut cond_regs = HashSet::new();
    collect_expr_regs(&cond, &mut cond_regs);

    if cond_regs.len() != 1 {
        return (cond, None, None);
    }

    let r = *cond_regs.iter().next().unwrap();

    for (i, stmt) in block.stmts.iter().enumerate().rev() {
        if let JsStmt::Assign { reg, expr } = stmt {
            if reg.reg == r {
                substitute_reg(&mut cond, r, expr);
                return (cond, Some(i), None);
            }
        }
    }

    (cond, None, None)
}
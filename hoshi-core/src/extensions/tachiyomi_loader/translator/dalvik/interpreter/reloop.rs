use std::collections::{HashMap, HashSet};
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter;
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{cfg, BasicBlock, JsExpr, JsStmt, TaggedStmt, Terminator};

const MAX_DEPTH: usize = 64;

fn next_block_offset(blocks: &[BasicBlock], b2i: &HashMap<i32, usize>, idx: usize) -> i32 {
    blocks.get(idx + 1).map(|b| b.offset).unwrap_or(i32::MAX)
}

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

    let loop_headers: HashSet<i32> = {
        let mut h = HashSet::new();
        for b in &blocks {
            for t in cfg::block_successors(b) {
                if t <= b.offset { h.insert(t); }
            }
        }
        h
    };
    let b2i: HashMap<i32, usize> = blocks.iter().enumerate()
        .map(|(i, b)| (b.offset, i))
        .collect();

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
){
    if depth > MAX_DEPTH { return; }

    let mut idx = start_idx;

    while idx < blocks.len() {
        let block = &blocks[idx];

        if block.offset >= until { break; }
        if visited.contains(&idx) { break; }

        if loop_headers.contains(&block.offset)
            && Some(block.offset) != current_loop
        {
            let loop_end = cfg::find_loop_end(blocks, idx, block.offset);

            for bi in idx..blocks.len() {
                let b = &blocks[bi];
                if b.offset >= loop_end {
                    break;
                }
            }

            let mut loop_visited = HashSet::new();
            loop_visited.insert(idx);

            let (header_cond, consumed_stmt): (Option<JsExpr>, Option<usize>) =
                match block_is_loop_guard(block, loop_end) {
                    Some(c) => {
                        let (cond, stmt) = resolve_loop_condition(block, c);
                        (Some(cond), stmt)
                    }
                    None => (None, None),
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
            );

            if matches!(body.last(), Some(JsStmt::Continue)) {
                body.pop();
            }

            if let Some(cond) = header_cond {
                out.push(JsStmt::While {
                    cond,
                    body,
                });
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

        out.extend(block.stmts.iter().cloned());

        match &block.term {
            Terminator::Return(e) => {
                out.push(JsStmt::Return(e.clone()));
                break;
            }
            Terminator::ImplicitReturn | Terminator::Throw => {
                break;
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
                        cond: interpreter::negate(cond),
                        then_body: vec![JsStmt::Break],
                        else_body: vec![],
                    });
                    out.push(JsStmt::Continue);
                    break;
                }

                let fall_idx   = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                let branch_idx = b2i.get(&if_true).copied().unwrap_or(blocks.len());

                let join = find_join_relooper(blocks, b2i, fall_idx, branch_idx, until, block.offset);

                let mut then_body = Vec::new();
                let mut then_visited = visited.clone();
                reloop(blocks, b2i, loop_headers, preds,
                       fall_idx, join, loop_exit, current_loop,
                       &mut then_visited, &mut then_body, depth + 1);

                let mut else_body = Vec::new();
                if branch_idx < blocks.len() {
                    let mut else_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           branch_idx, join, loop_exit, current_loop,
                           &mut else_visited, &mut else_body, depth + 1);
                    visited.extend(else_visited.into_iter());
                }
                visited.extend(then_visited.into_iter());

                if !then_body.is_empty() || !else_body.is_empty() {
                    out.push(JsStmt::If {
                        cond,
                        then_body: else_body,
                        else_body: then_body,
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
                target_order.sort_unstable();

                let mut resolved_cases: Vec<(i32, Vec<JsStmt>)> = Vec::new();

                for (ti, &t) in target_order.iter().enumerate() {
                    let next_t = target_order.get(ti + 1).copied().unwrap_or(switch_end);
                    let stop   = next_t.min(switch_end);

                    let case_start = b2i.get(&t).copied().unwrap_or(blocks.len());
                    let mut case_body = Vec::new();
                    let mut case_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           case_start, stop, loop_exit, current_loop,
                           &mut case_visited, &mut case_body, depth + 1);
                    visited.extend(case_visited.into_iter());

                    for &k in &target_keys[&t] {
                        resolved_cases.push((k, case_body.clone()));
                    }
                }
                resolved_cases.sort_by_key(|(k, _)| *k);

                out.push(JsStmt::Switch { expr, cases: resolved_cases });

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
                    let loop_exit_off = cfg::find_loop_end(blocks, header_idx, t);
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

fn resolve_loop_condition(
    block: &BasicBlock,
    cond: JsExpr,
) -> (JsExpr, Option<usize>) {
    let cond = strip_double_negation(cond);

    match cond {
        JsExpr::Reg(r) => {
            for (i, stmt) in block.stmts.iter().enumerate().rev() {
                if let JsStmt::Assign { reg, expr } = stmt {
                    if *reg == r {
                        return (expr.clone(), Some(i));
                    }
                }
            }

            (JsExpr::Reg(r), None)
        }

        other => (other, None),
    }
}
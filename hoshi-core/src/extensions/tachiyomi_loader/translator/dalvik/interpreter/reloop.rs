use std::collections::{HashMap, HashSet};
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter;
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{cfg, BasicBlock, JsStmt, TaggedStmt, Terminator};

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
    visited:      &mut HashSet<usize>,
    out:          &mut Vec<JsStmt>,
    depth:        usize,
) {
    if depth > MAX_DEPTH { return; }

    let mut idx = start_idx;

    while idx < blocks.len() {
        let block = &blocks[idx];

        if block.offset >= until { break; }
        if visited.contains(&idx) { break; }

        if loop_headers.contains(&block.offset) && block.offset >= blocks[start_idx].offset {
            let loop_end = cfg::find_loop_end(blocks, idx, block.offset);

            visited.insert(idx);
            let mut body = Vec::new();
            reloop(blocks, b2i, loop_headers, preds,
                   idx, loop_end, Some(loop_end),
                   visited, &mut body, depth + 1);

            if matches!(body.last(), Some(JsStmt::Continue)) {
                let prev_is_if = body.len() >= 2 &&
                    matches!(body[body.len() - 2], JsStmt::If { .. });
                if !prev_is_if {
                    body.pop();
                }
            }

            out.push(JsStmt::Loop { body });

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
                if loop_headers.contains(&t) && t <= block.offset {
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

                if loop_headers.contains(&if_true) && if_true <= block.offset {
                    out.push(JsStmt::If {
                        cond: interpreter::negate(cond),
                        then_body: vec![JsStmt::Break],
                        else_body: vec![],
                    });
                    out.push(JsStmt::Continue);
                    break;
                }

                if loop_exit.map_or(false, |e| if_true >= e) {
                    let fall_idx = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                    let mut body = Vec::new();
                    let mut branch_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           fall_idx, if_true, loop_exit,
                           &mut branch_visited, &mut body, depth + 1);
                    visited.extend(branch_visited.iter().copied());
                    if !body.is_empty() {
                        out.push(JsStmt::If { cond: interpreter::negate(cond), then_body: body, else_body: vec![] });
                    }
                    out.push(JsStmt::Break);
                    break;
                }
                
                let fall_idx   = b2i.get(&if_false).copied().unwrap_or(blocks.len());
                let branch_idx = b2i.get(&if_true).copied().unwrap_or(blocks.len());
                
                let join = find_join_relooper(blocks, b2i, fall_idx, branch_idx, until, block.offset);

                let mut then_body = Vec::new();
                let mut then_visited = visited.clone();
                reloop(blocks, b2i, loop_headers, preds,
                       fall_idx, join, loop_exit,
                       &mut then_visited, &mut then_body, depth + 1);

                let mut else_body = Vec::new();
                if branch_idx < blocks.len() && blocks[branch_idx].offset < join {
                    let mut else_visited = visited.clone();
                    reloop(blocks, b2i, loop_headers, preds,
                           branch_idx, join, loop_exit,
                           &mut else_visited, &mut else_body, depth + 1);
                    visited.extend(else_visited.into_iter());
                }
                visited.extend(then_visited.into_iter());

                if !then_body.is_empty() || !else_body.is_empty() {
                    out.push(JsStmt::If { cond: interpreter::negate(cond), then_body, else_body });
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
                           case_start, stop, loop_exit,
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

        idx += 1;
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
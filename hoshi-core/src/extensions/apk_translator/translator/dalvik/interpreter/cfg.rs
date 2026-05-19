use std::collections::{HashMap, HashSet};
use crate::extensions::apk_translator::translator::dalvik::interpreter::{BasicBlock, JsExpr, JsStmt, TaggedStmt, Terminator};

pub fn block_successors(b: &BasicBlock) -> Vec<i32> {
    match &b.term {
        Terminator::Goto(t)                          => vec![*t],
        Terminator::FallThrough(t)                   => vec![*t],
        Terminator::CondGoto { if_true, if_false, .. } => vec![*if_true, *if_false],
        Terminator::Switch { cases, default, .. } => {
            let mut v: Vec<i32> = cases.iter().map(|(_, t)| *t).collect();
            v.push(*default);
            v
        }
        _ => vec![],
    }
}

pub fn build_predecessors(blocks: &[BasicBlock], b2i: &HashMap<i32, usize>) -> Vec<Vec<usize>> {
    let mut preds = vec![Vec::<usize>::new(); blocks.len()];
    for (i, b) in blocks.iter().enumerate() {
        for t in block_successors(b) {
            if let Some(&j) = b2i.get(&t) {
                preds[j].push(i);
            }
        }
    }
    preds
}

pub fn build_blocks(tagged: Vec<TaggedStmt>) -> Vec<BasicBlock> {
    if tagged.is_empty() { return vec![]; }

    let mut leaders: HashSet<i32> = HashSet::new();
    leaders.insert(tagged[0].offset);
    let normalize_target = |target: i32| -> i32 {
        tagged.iter()
            .find(|ts| ts.offset >= target)
            .map(|ts| ts.offset)
            .unwrap_or(target)
    };

    for (i, ts) in tagged.iter().enumerate() {
        match &ts.stmt {
            JsStmt::Goto(t) => {
                leaders.insert(normalize_target(*t));
                if let Some(next) = tagged.get(i + 1) {
                    leaders.insert(next.offset);
                }
            }
            JsStmt::CondGoto { target, .. } => {
                leaders.insert(normalize_target(*target));
                if let Some(next) = tagged.get(i + 1) {
                    leaders.insert(next.offset);
                }
            }
            JsStmt::Switch { cases, .. } => {
                for (_, body) in cases {
                    if let Some(JsStmt::Goto(t)) = body.first() {
                        leaders.insert(normalize_target(*t));
                    }
                }
            }
            _ => {}
        }
    }

    let snap = |idx: usize| -> i32 {
        tagged.get(idx).map(|ts| ts.offset).unwrap_or(i32::MAX)
    };

    let mut blocks:    Vec<BasicBlock> = Vec::new();
    let mut cur_stmts: Vec<JsStmt>    = Vec::new();
    let mut cur_off                    = tagged[0].offset;

    let mut i = 0usize;
    while i < tagged.len() {
        let ts = &tagged[i];

        if leaders.contains(&ts.offset) && ts.offset != cur_off && !cur_stmts.is_empty() {
            blocks.push(BasicBlock {
                offset: cur_off,
                stmts:  std::mem::take(&mut cur_stmts),
                term:   Terminator::FallThrough(ts.offset),
            });
            cur_off = ts.offset;
        }

        match &ts.stmt {
            JsStmt::Goto(t) => {
                blocks.push(BasicBlock {
                    offset: cur_off,
                    stmts:  std::mem::take(&mut cur_stmts),
                    term: Terminator::Goto(normalize_target(*t)),
                });
                cur_off = snap(i + 1);
            }
            JsStmt::CondGoto { cond, target } => {
                let if_false = snap(i + 1);
                leaders.insert(if_false);
                blocks.push(BasicBlock {
                    offset: cur_off,
                    stmts:  std::mem::take(&mut cur_stmts),
                    term:   Terminator::CondGoto {
                        cond:     cond.clone(),
                        if_true: normalize_target(*target),
                        if_false,
                    },
                });
                cur_off = if_false;
            }
            JsStmt::Return(e) => {
                blocks.push(BasicBlock {
                    offset: cur_off,
                    stmts:  std::mem::take(&mut cur_stmts),
                    term:   Terminator::Return(e.clone()),
                });
                cur_off = snap(i + 1);
            }
            JsStmt::Expr(JsExpr::Raw(s)) if s.starts_with("throw") => {
                cur_stmts.push(JsStmt::Expr(JsExpr::Raw(s.clone())));
                blocks.push(BasicBlock {
                    offset: cur_off,
                    stmts:  std::mem::take(&mut cur_stmts),
                    term:   Terminator::Throw,
                });
                cur_off = snap(i + 1);
            }
            JsStmt::Switch { expr, cases, default } => {
                let fall_off = snap(i + 1);
                let resolved: Vec<(i32, i32)> = cases.iter().map(|(key, body)| {
                    let target = body.iter().find_map(|s| {
                        if let JsStmt::Goto(t) = s {
                            Some(normalize_target(*t))
                        } else {
                            None
                        }
                    }).unwrap_or(fall_off);
                    (*key, target)
                }).collect();
                leaders.insert(fall_off);
                for &(_, t) in &resolved { leaders.insert(t); }
                blocks.push(BasicBlock {
                    offset: cur_off,
                    stmts:  std::mem::take(&mut cur_stmts),
                    term:   Terminator::Switch {
                        expr:    expr.clone(),
                        cases:   resolved,
                        default: default.as_ref()
                            .and_then(|body| {
                                body.iter().find_map(|s| {
                                    if let JsStmt::Goto(t) = s {
                                        Some(normalize_target(*t))
                                    } else {
                                        None
                                    }
                                })
                            })
                            .unwrap_or(fall_off),
                    },
                });
                cur_off = fall_off;
            }
            other => {
                cur_stmts.push(other.clone());
            }
        }
        i += 1;
    }

    if !cur_stmts.is_empty() {
        blocks.push(BasicBlock {
            offset: cur_off,
            stmts:  cur_stmts,
            term:   Terminator::ImplicitReturn,
        });
    }

    blocks
}

pub fn find_switch_end(
    blocks:  &[BasicBlock],
    b2i:     &HashMap<i32, usize>,
    cases:   &[(i32, i32)],
    default: i32,
    until:   i32,
) -> i32 {
    let mut candidate_ends: Vec<i32> = Vec::new();
    let all_targets: Vec<i32> = cases.iter().map(|&(_, t)| t).chain(std::iter::once(default)).collect();

    for &t in &all_targets {
        if let Some(&ti) = b2i.get(&t) {
            let mut ci = ti;
            loop {
                let cb = match blocks.get(ci) {
                    Some(cb) => cb,
                    None => break,
                };

                if cb.offset >= until {
                    break;
                }

                match &cb.term {
                    Terminator::FallThrough(n) => {
                        ci = b2i.get(n).copied().unwrap_or(blocks.len());
                    }
                    Terminator::Goto(g) => {
                        if *g > cb.offset {
                            candidate_ends.push(*g);
                        }
                        break;
                    }
                    _ => {
                        if let Some(nb) = blocks.get(ci + 1) {
                            candidate_ends.push(nb.offset);
                        }
                        break;
                    }
                }
            }
        }
    }

    if candidate_ends.is_empty() {
        return until;
    }

    let mut freq: HashMap<i32, usize> = HashMap::new();
    for &e in &candidate_ends { *freq.entry(e).or_insert(0) += 1; }

    let best = freq.iter()
        .filter(|(_, &c)| c > 1)
        .map(|(&o, _)| o)
        .min()
        .unwrap_or_else(|| *candidate_ends.iter().min().unwrap());

    best.min(until)
}

pub fn find_loop_end(
    blocks: &[BasicBlock],
    start_idx: usize,
    header: i32,
    b2i: &HashMap<i32, usize>
) -> i32 {
    let mut max_back_idx = start_idx;
    let mut found_backedge = false;

    for (i, b) in blocks.iter().enumerate().skip(start_idx) {
        if block_successors(b).contains(&header) {
            max_back_idx = i;
            found_backedge = true;
        }
    }

    if !found_backedge {
        return i32::MAX;
    }

    if let Some(header_block) = blocks.get(start_idx) {
        if let Terminator::CondGoto { if_true, if_false, .. } = &header_block.term {
            let max_body_offset = blocks[max_back_idx].offset;

            let if_true_is_throw = b2i.get(if_true)
                .and_then(|&i| blocks.get(i))
                .map(|b| matches!(b.term, Terminator::Throw))
                .unwrap_or(false);

            if *if_true != header && !if_true_is_throw {
                return *if_true;
            }
            if *if_false > max_body_offset && *if_false != header {
                return *if_false;
            }
        }
    }

    blocks.get(max_back_idx + 1)
        .map(|b| b.offset)
        .unwrap_or(i32::MAX)
}
use std::collections::HashMap;
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId};
use crate::translator::remap::{max_reg_in_stmts, remap_stmts};

pub fn inline_bridge_ctors(
    lifted: &mut Vec<(Vec<JsStmt>, String, String, bool, u16)>,
) {
    let zero_inits: HashMap<String, Vec<JsStmt>> = lifted.iter()
        .filter(|(_, name, _, _, ins_size)| name == "<init>" && *ins_size == 1)
        .map(|(stmts, _, defined_in, _, _)| (defined_in.clone(), stmts.clone()))
        .collect();

    for (stmts, name, defined_in, _, _) in lifted.iter_mut() {
        if name != "<init>" { continue; }
        let Some(zero_stmts) = zero_inits.get(defined_in) else { continue; };
        if zero_stmts.is_empty() { continue; }
        inline_one(stmts, zero_stmts);
    }
}

fn inline_one(stmts: &mut Vec<JsStmt>, zero_stmts: &[JsStmt]) {
    let Some(pos) = stmts.iter().position(|s| {
        matches!(s, JsStmt::Expr(JsExpr::ThisCtorCall { .. }))
    }) else { return; };

    let ctor_args = match &stmts[pos] {
        JsStmt::Expr(JsExpr::ThisCtorCall { args }) => args.clone(),
        _ => unreachable!(),
    };

    let offset = max_reg_in_stmts(stmts) + 1;
    let mut remapped = remap_stmts(zero_stmts.to_vec(), offset);

    let n_params = remapped.iter().take_while(|s| matches!(s,
        JsStmt::Assign { expr: JsExpr::Raw(r), .. } if r.starts_with("arguments[")
    )).count();

    let param_regs: Vec<RegId> = remapped[..n_params].iter().map(|s| match s {
        JsStmt::Assign { reg, .. } => reg.clone(),
        _ => unreachable!(),
    }).collect();

    remapped.drain(..n_params);

    if matches!(remapped.last(), Some(JsStmt::Return(None))) {
        remapped.pop();
    }

    let mut replacement: Vec<JsStmt> = param_regs
        .into_iter()
        .zip(ctor_args)
        .map(|(reg, expr)| JsStmt::Assign { reg, expr })
        .collect();
    replacement.extend(remapped);

    let splice_pos = if pos > 0
        && matches!(stmts.get(pos - 1), Some(JsStmt::Return(None)))
    {
        stmts.remove(pos - 1);
        pos - 1
    } else {
        pos
    };

    stmts.splice(splice_pos..=splice_pos, replacement);
}
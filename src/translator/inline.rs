use std::collections::HashMap;
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId};
use crate::translator::remap::{max_reg_in_stmts, remap_stmts};

pub fn inline_bridge_ctors(lifted: &mut Vec<(Vec<JsStmt>, String, String, bool, u16)>) {
    let mut inits_by_class: HashMap<String, Vec<(Vec<JsStmt>, u16)>> = HashMap::new();
    for (stmts, name, defined_in, _, ins_size) in lifted.iter() {
        if name == "<init>" {
            inits_by_class.entry(defined_in.clone())
                .or_default()
                .push((stmts.clone(), *ins_size));
        }
    }

    let primary_inits: HashMap<String, (Vec<JsStmt>, u16)> = inits_by_class
        .into_iter()
        .filter(|(_, inits)| inits.len() >= 2)
        .filter_map(|(class, mut inits)| {
            inits.sort_by_key(|(_, ins_size)| *ins_size);
            let (stmts, ins_size) = inits.remove(0);
            Some((class, (stmts, ins_size)))
        })
        .collect();

    for (stmts, name, defined_in, _, ins_size) in lifted.iter_mut() {
        if name != "<init>" { continue; }
        let Some((primary_stmts, primary_ins_size)) = primary_inits.get(defined_in) else { continue; };
        if ins_size == primary_ins_size { continue; }
        inline_one(stmts, primary_stmts);
    }
}

fn inline_one(stmts: &mut Vec<JsStmt>, primary_stmts: &[JsStmt]) {
    let last = stmts.last_mut();
    if let Some(JsStmt::If { then_body, else_body, .. }) = last {
        inline_into_branch(then_body, primary_stmts);
        inline_into_branch(else_body, primary_stmts);
    }
}

fn inline_into_branch(branch: &mut Vec<JsStmt>, primary_stmts: &[JsStmt]) {
    let Some(pos) = branch.iter().position(|s| {
        matches!(s, JsStmt::Expr(JsExpr::ThisCtorCall { .. }))
    }) else { return; };

    let ctor_args = match &branch[pos] {
        JsStmt::Expr(JsExpr::ThisCtorCall { args }) => args.clone(),
        _ => unreachable!(),
    };

    let offset = max_reg_in_stmts(branch) + 1;
    let mut remapped = remap_stmts(primary_stmts.to_vec(), offset);

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

    let end = if matches!(branch.get(pos + 1), Some(JsStmt::Return(None))) {
        pos + 2
    } else {
        pos + 1
    };

    branch.splice(pos..end, replacement);
}
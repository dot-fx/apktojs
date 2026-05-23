use std::collections::{HashMap, HashSet};
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, TaggedStmt};
use crate::translator::dalvik::interpreter::ir::RegId;

pub fn rename(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut current: HashMap<u8, usize> = HashMap::new();
    let mut next: HashMap<u8, usize> = HashMap::new();
    let locked: HashSet<u8> = HashSet::new();

    rename_block(stmts, &mut current, &mut next, &locked)
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
        if !current.contains_key(&r) {
            let ver = *next.entry(r).or_insert(1);
            current.insert(r, ver);
            next.insert(r, ver + 1);
        }
    }
}

fn rename_stmt(
    stmt: JsStmt,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
    locked: &HashSet<u8>,
) -> JsStmt {
    match stmt {
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

            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            let mut then_curr = current.clone(); let mut then_next = next.clone();
            let then_body = rename_block(then_body, &mut then_curr, &mut then_next, &inner_locked);

            let mut else_curr = current.clone(); let mut else_next = next.clone();
            let else_body = rename_block(else_body, &mut else_curr, &mut else_next, &inner_locked);

            JsStmt::If { cond, then_body, else_body }
        }

        JsStmt::Loop { body } => {
            pre_declare_regs(&body, current, next);
            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }
            JsStmt::Loop { body: rename_block(body, current, next, &inner_locked) }
        }

        JsStmt::While { cond, body } => {
            let cond = rename_expr(cond, current);
            pre_declare_regs(&body, current, next);

            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            JsStmt::While {
                cond,
                body: rename_block(body, current, next, &inner_locked),
            }
        }

        JsStmt::DoWhile { body, cond } => {
            pre_declare_regs(&body, current, next);
            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            let body = rename_block(body, current, next, &inner_locked);
            let cond = rename_expr(cond, current);
            JsStmt::DoWhile { body, cond }
        }

        JsStmt::Switch { expr, cases, default } => {
            let expr = rename_expr(expr, current);

            for (_, body) in &cases { pre_declare_regs(body, current, next); }
            if let Some(body) = &default { pre_declare_regs(body, current, next); }

            let mut inner_locked = locked.clone();
            for &r in current.keys() { inner_locked.insert(r); }

            let cases = cases.into_iter().map(|(key, body)| {
                let mut c_curr = current.clone(); let mut c_next = next.clone();
                (key, rename_block(body, &mut c_curr, &mut c_next, &inner_locked))
            }).collect();

            let default = default.map(|body| {
                let mut c_curr = current.clone(); let mut c_next = next.clone();
                rename_block(body, &mut c_curr, &mut c_next, &inner_locked)
            });

            JsStmt::Switch { expr, cases, default }
        }

        other => other,
    }
}

fn rename_block(
    stmts: Vec<JsStmt>,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
    locked: &HashSet<u8>,
) -> Vec<JsStmt> {
    stmts
        .into_iter()
        .map(|s| rename_stmt(s, current, next, locked))
        .collect()
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
use std::collections::HashMap;
use crate::extensions::apk_translator::translator::dalvik::interpreter::{JsExpr, JsStmt, TaggedStmt};
use crate::extensions::apk_translator::translator::dalvik::interpreter::ir::RegId;

/// Linear SSA renaming pass.
///
/// Walks the flat `Vec<TaggedStmt>` produced by the lifter (before reloop),
/// and rewrites every `RegId` so that each write to a register gets a fresh
/// version number and all subsequent reads use that version — eliminating the
/// "same `var` name reused" problem that breaks JS output.
///
/// Parameters are seeded at version 0 by the lifter and are left alone;
/// any write inside the method body starts at version 1.
pub fn rename(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut current: HashMap<u8, usize> = HashMap::new();
    let mut next: HashMap<u8, usize> = HashMap::new();

    // Directly process the blocks using your existing recursive logic
    rename_block(stmts, &mut current, &mut next)
}

// ---------------------------------------------------------------------------
// Statement renaming
// ---------------------------------------------------------------------------

fn rename_stmt(
    stmt: JsStmt,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
) -> JsStmt {
    match stmt {
        JsStmt::Assign { reg, expr } => {
            let expr = rename_expr(expr, current);
            let new_ver = bump(reg.reg, current, next);
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

            let mut then_current = current.clone();
            let mut then_next = next.clone();
            let then_body = rename_block(then_body, &mut then_current, &mut then_next);

            let mut else_current = current.clone();
            let mut else_next = next.clone();
            let else_body = rename_block(else_body, &mut else_current, &mut else_next);

            // Only merge state back if the branch actually survives to rejoin the main trunk
            if !is_terminal(&then_body) {
                for (r, v) in then_next { let e = next.entry(r).or_insert(0); if v > *e { *e = v; } }
                for (r, v) in then_current { let e = current.entry(r).or_insert(0); if v > *e { *e = v; } }
            }
            if !is_terminal(&else_body) {
                for (r, v) in else_next { let e = next.entry(r).or_insert(0); if v > *e { *e = v; } }
                for (r, v) in else_current { let e = current.entry(r).or_insert(0); if v > *e { *e = v; } }
            }

            JsStmt::If { cond, then_body, else_body }
        }

        JsStmt::Loop { body } =>
            JsStmt::Loop { body: rename_block(body, current, next) },

        JsStmt::While { cond, body } => JsStmt::While {
            cond: rename_expr(cond, current),
            body: rename_block(body, current, next),
        },

        JsStmt::DoWhile { body, cond } => {
            let body = rename_block(body, current, next);
            let cond = rename_expr(cond, current);
            JsStmt::DoWhile { body, cond }
        }

        JsStmt::Switch { expr, cases, default } => {
            let expr = rename_expr(expr, current);
            let mut merged_current = current.clone();
            let mut merged_next = next.clone();

            let cases = cases
                .into_iter()
                .map(|(key, body)| {
                    let mut c_curr = current.clone();
                    let mut c_next = next.clone();
                    let body = rename_block(body, &mut c_curr, &mut c_next);

                    if !is_terminal(&body) {
                        for (r, v) in c_next { let e = merged_next.entry(r).or_insert(0); if v > *e { *e = v; } }
                        for (r, v) in c_curr { let e = merged_current.entry(r).or_insert(0); if v > *e { *e = v; } }
                    }
                    (key, body)
                })
                .collect();

            let default = default.map(|body| {
                let mut c_curr = current.clone();
                let mut c_next = next.clone();
                let body = rename_block(body, &mut c_curr, &mut c_next);

                if !is_terminal(&body) {
                    for (r, v) in c_next { let e = merged_next.entry(r).or_insert(0); if v > *e { *e = v; } }
                    for (r, v) in c_curr { let e = merged_current.entry(r).or_insert(0); if v > *e { *e = v; } }
                }
                body
            });

            *current = merged_current;
            *next = merged_next;

            JsStmt::Switch { expr, cases, default }
        }

        other => other,
    }
}

fn rename_block(
    stmts: Vec<JsStmt>,
    current: &mut HashMap<u8, usize>,
    next: &mut HashMap<u8, usize>,
) -> Vec<JsStmt> {
    stmts
        .into_iter()
        .map(|s| rename_stmt(s, current, next))
        .collect()
}

// ---------------------------------------------------------------------------
// Expression renaming
// ---------------------------------------------------------------------------

fn rename_expr(expr: JsExpr, current: &mut HashMap<u8, usize>) -> JsExpr {
    match expr {
        // A read: rewrite to whatever version is current for this register.
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

        // Leaves — no registers inside.
        other => other,
    }
}

fn rename_exprs(exprs: Vec<JsExpr>, current: &mut HashMap<u8, usize>) -> Vec<JsExpr> {
    exprs.into_iter().map(|e| rename_expr(e, current)).collect()
}

// ---------------------------------------------------------------------------
// Version bookkeeping
// ---------------------------------------------------------------------------

/// Bump register `r` to its next write version, update `current`, return it.
fn bump(r: u8, current: &mut HashMap<u8, usize>, next: &mut HashMap<u8, usize>) -> usize {
    let ver = *next.entry(r).or_insert(1);
    current.insert(r, ver);
    next.insert(r, ver + 1);
    ver
}

fn is_terminal(stmts: &[JsStmt]) -> bool {
    stmts.iter().any(|stmt| {
        matches!(stmt, JsStmt::Return(_) | JsStmt::Throw | JsStmt::Goto(_)) ||
            matches!(stmt, JsStmt::Expr(JsExpr::UnaryOp { op, .. }) if *op == "throw ") ||
            matches!(stmt, JsStmt::Expr(JsExpr::Raw(s)) if s.starts_with("throw"))
    })
}
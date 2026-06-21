use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId};

pub fn remap_stmts(stmts: Vec<JsStmt>, offset: u8) -> Vec<JsStmt> {
    stmts.into_iter().map(|s| remap_stmt(s, offset)).collect()
}

pub fn max_reg_in_stmts(stmts: &[JsStmt]) -> u8 {
    stmts.iter().map(max_reg_in_stmt).max().unwrap_or(0)
}

fn remap_expr(expr: JsExpr, offset: u8) -> JsExpr {
    match expr {
        JsExpr::Reg(r) => JsExpr::Reg(RegId { reg: r.reg + offset, version: r.version }),
        JsExpr::BinOp { op, left, right } => JsExpr::BinOp {
            op,
            left: Box::new(remap_expr(*left, offset)),
            right: Box::new(remap_expr(*right, offset)),
        },
        JsExpr::UnaryOp { op, expr } => JsExpr::UnaryOp {
            op,
            expr: Box::new(remap_expr(*expr, offset)),
        },
        JsExpr::MethodCall { receiver, method, args, is_static } => JsExpr::MethodCall {
            receiver: Box::new(remap_expr(*receiver, offset)),
            method,
            args: args.into_iter().map(|e| remap_expr(e, offset)).collect(),
            is_static,
        },
        JsExpr::StaticCall { class, method, args } => JsExpr::StaticCall {
            class,
            method,
            args: args.into_iter().map(|e| remap_expr(e, offset)).collect(),
        },
        JsExpr::New { class, args } => JsExpr::New {
            class,
            args: args.into_iter().map(|e| remap_expr(e, offset)).collect(),
        },
        JsExpr::FieldGet { receiver, field } => JsExpr::FieldGet {
            receiver: Box::new(remap_expr(*receiver, offset)),
            field,
        },
        JsExpr::Index { arr, idx } => JsExpr::Index {
            arr: Box::new(remap_expr(*arr, offset)),
            idx: Box::new(remap_expr(*idx, offset)),
        },
        JsExpr::ArrayLiteral(items) => JsExpr::ArrayLiteral(
            items.into_iter().map(|e| remap_expr(e, offset)).collect()
        ),
        JsExpr::StringConcat(items) => JsExpr::StringConcat(
            items.into_iter().map(|e| remap_expr(e, offset)).collect()
        ),
        JsExpr::BitMask { expr, mask } => JsExpr::BitMask {
            expr: Box::new(remap_expr(*expr, offset)),
            mask,
        },
        JsExpr::SuperCall { args } => JsExpr::SuperCall {
            args: args.into_iter().map(|e| remap_expr(e, offset)).collect(),
        },
        JsExpr::ThisCtorCall { args } => JsExpr::ThisCtorCall {
            args: args.into_iter().map(|e| remap_expr(e, offset)).collect(),
        },
        JsExpr::StaticFieldGet { class, field } => JsExpr::StaticFieldGet { class, field },
        // primitives and non-reg exprs are unchanged
        other => other,
    }
}

fn remap_stmt(stmt: JsStmt, offset: u8) -> JsStmt {
    match stmt {
        JsStmt::Assign { reg, expr } => JsStmt::Assign {
            reg: RegId { reg: reg.reg + offset, version: reg.version },
            expr: remap_expr(expr, offset),
        },
        JsStmt::Param { reg, name } => JsStmt::Param {
            reg: reg + offset,
            name,
        },
        JsStmt::Expr(e) => JsStmt::Expr(remap_expr(e, offset)),
        JsStmt::Return(Some(e)) => JsStmt::Return(Some(remap_expr(e, offset))),
        JsStmt::Return(None) => JsStmt::Return(None),
        JsStmt::FieldSet { receiver, field, value } => JsStmt::FieldSet {
            receiver: remap_expr(receiver, offset),
            field,
            value: remap_expr(value, offset),
        },
        JsStmt::StaticSet { class, field, value } => JsStmt::StaticSet {
            class,
            field,
            value: remap_expr(value, offset),
        },
        JsStmt::StaticGet { class, field, dst } => JsStmt::StaticGet {
            class,
            field,
            dst: RegId { reg: dst.reg + offset, version: dst.version },
        },
        JsStmt::ArraySet { arr, idx, value } => JsStmt::ArraySet {
            arr: remap_expr(arr, offset),
            idx: remap_expr(idx, offset),
            value: remap_expr(value, offset),
        },
        JsStmt::If { cond, then_body, else_body } => JsStmt::If {
            cond: remap_expr(cond, offset),
            then_body: remap_stmts(then_body, offset),
            else_body: remap_stmts(else_body, offset),
        },
        JsStmt::Loop { body } => JsStmt::Loop {
            body: remap_stmts(body, offset),
        },
        JsStmt::While { cond, body } => JsStmt::While {
            cond: remap_expr(cond, offset),
            body: remap_stmts(body, offset),
        },
        JsStmt::DoWhile { body, cond } => JsStmt::DoWhile {
            body: remap_stmts(body, offset),
            cond: remap_expr(cond, offset),
        },
        JsStmt::Switch { expr, cases, default } => JsStmt::Switch {
            expr: remap_expr(expr, offset),
            cases: cases.into_iter()
                .map(|(k, body)| (k, remap_stmts(body, offset)))
                .collect(),
            default: default.map(|body| remap_stmts(body, offset)),
        },
        JsStmt::CondGoto { cond, target } => JsStmt::CondGoto {
            cond: remap_expr(cond, offset),
            target,
        },
        // these have no registers
        other => other,
    }
}

fn max_reg_in_stmt(stmt: &JsStmt) -> u8 {
    match stmt {
        JsStmt::Assign { reg, expr } => reg.reg.max(max_reg_in_expr(expr)),
        JsStmt::Expr(e) => max_reg_in_expr(e),
        JsStmt::Return(Some(e)) => max_reg_in_expr(e),
        JsStmt::FieldSet { receiver, value, .. } => max_reg_in_expr(receiver).max(max_reg_in_expr(value)),
        JsStmt::StaticSet { value, .. } => max_reg_in_expr(value),
        JsStmt::StaticGet { dst, .. } => dst.reg,
        JsStmt::ArraySet { arr, idx, value } => max_reg_in_expr(arr).max(max_reg_in_expr(idx)).max(max_reg_in_expr(value)),
        JsStmt::If { cond, then_body, else_body } => max_reg_in_expr(cond).max(max_reg_in_stmts(then_body)).max(max_reg_in_stmts(else_body)),
        JsStmt::Loop { body } => max_reg_in_stmts(body),
        JsStmt::While { cond, body } => max_reg_in_expr(cond).max(max_reg_in_stmts(body)),
        JsStmt::DoWhile { body, cond } => max_reg_in_stmts(body).max(max_reg_in_expr(cond)),
        JsStmt::Switch { expr, cases, default } => {
            let mut m = max_reg_in_expr(expr);
            for (_, body) in cases { m = m.max(max_reg_in_stmts(body)); }
            if let Some(body) = default { m = m.max(max_reg_in_stmts(body)); }
            m
        }
        JsStmt::CondGoto { cond, .. } => max_reg_in_expr(cond),
        _ => 0,
    }
}

fn max_reg_in_expr(expr: &JsExpr) -> u8 {
    match expr {
        JsExpr::Reg(r) => r.reg,
        JsExpr::BinOp { left, right, .. } => max_reg_in_expr(left).max(max_reg_in_expr(right)),
        JsExpr::UnaryOp { expr, .. } => max_reg_in_expr(expr),
        JsExpr::MethodCall { receiver, args, .. } => {
            args.iter().map(max_reg_in_expr).max().unwrap_or(0).max(max_reg_in_expr(receiver))
        }
        JsExpr::StaticCall { args, .. } => args.iter().map(max_reg_in_expr).max().unwrap_or(0),
        JsExpr::New { args, .. } => args.iter().map(max_reg_in_expr).max().unwrap_or(0),
        JsExpr::FieldGet { receiver, .. } => max_reg_in_expr(receiver),
        JsExpr::Index { arr, idx } => max_reg_in_expr(arr).max(max_reg_in_expr(idx)),
        JsExpr::ArrayLiteral(items) => items.iter().map(max_reg_in_expr).max().unwrap_or(0),
        JsExpr::StringConcat(items) => items.iter().map(max_reg_in_expr).max().unwrap_or(0),
        JsExpr::BitMask { expr, .. } => max_reg_in_expr(expr),
        JsExpr::SuperCall { args } => args.iter().map(max_reg_in_expr).max().unwrap_or(0),
        JsExpr::ThisCtorCall { args } => args.iter().map(max_reg_in_expr).max().unwrap_or(0),
        _ => 0,
    }
}
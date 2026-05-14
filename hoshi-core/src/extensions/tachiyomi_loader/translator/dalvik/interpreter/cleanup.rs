use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{JsExpr, JsStmt};

pub fn cleanup(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let stmts = elide_redundant_assigns(stmts);
    let stmts = simplify_loops(stmts);
    let stmts = simplify_array_add(stmts);

    stmts
}

fn simplify_loops(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    stmts
        .into_iter()
        .map(simplify_stmt_loops)
        .collect()
}

fn simplify_stmt_loops(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::Loop { body } => {
            let body = simplify_loops(body);

            if let Some(while_stmt) = try_make_while(&body) {
                while_stmt
            } else {
                JsStmt::Loop { body }
            }
        }

        JsStmt::If {
            cond,
            then_body,
            else_body,
        } => JsStmt::If {
            cond,
            then_body: simplify_loops(then_body),
            else_body: simplify_loops(else_body),
        },

        JsStmt::Switch { expr, cases } => {
            JsStmt::Switch {
                expr,
                cases: cases
                    .into_iter()
                    .map(|(k, v)| (k, simplify_loops(v)))
                    .collect(),
            }
        }

        other => other,
    }
}

fn try_make_while(body: &[JsStmt]) -> Option<JsStmt> {
    if body.len() < 2 {
        return None;
    }

    let first = &body[0];
    let second = &body[1];

    let (reg, expr) = match first {
        JsStmt::Assign { reg, expr } => (*reg, expr.clone()),
        _ => return None,
    };

    match second {
        JsStmt::If {
            cond,
            then_body,
            else_body,
        } => {

            if !else_body.is_empty() {
                return None;
            }

            if then_body.len() != 1 {
                return None;
            }

            if !matches!(then_body[0], JsStmt::Break) {
                return None;
            }

            match cond {
                JsExpr::UnaryOp { op, expr: inner } if *op == "!" => {
                    match inner.as_ref() {
                        JsExpr::Reg(r) if *r == reg => {
                            let remaining = body[2..].to_vec();

                            Some(JsStmt::While {
                                cond: expr,
                                body: simplify_loops(remaining),
                            })
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        _ => None,
    }
}

fn simplify_array_add(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    stmts
        .into_iter()
        .map(rewrite_add_stmt)
        .collect()
}

fn rewrite_add_stmt(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::Expr(expr) => {
            JsStmt::Expr(rewrite_add_expr(expr))
        }

        JsStmt::Assign { reg, expr } => {
            JsStmt::Assign {
                reg,
                expr: rewrite_add_expr(expr),
            }
        }

        JsStmt::If {
            cond,
            then_body,
            else_body,
        } => JsStmt::If {
            cond: rewrite_add_expr(cond),
            then_body: simplify_array_add(then_body),
            else_body: simplify_array_add(else_body),
        },

        JsStmt::Loop { body } => {
            JsStmt::Loop {
                body: simplify_array_add(body),
            }
        }

        JsStmt::While { cond, body } => {
            JsStmt::While {
                cond: rewrite_add_expr(cond),
                body: simplify_array_add(body),
            }
        }

        other => other,
    }
}

fn rewrite_add_expr(expr: JsExpr) -> JsExpr {
    match expr {
        JsExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "add" => {
            JsExpr::MethodCall {
                receiver,
                method: "push".into(),
                args,
            }
        }

        other => other,
    }
}

pub fn elide_redundant_assigns(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut out: Vec<JsStmt> = Vec::with_capacity(stmts.len());
    let mut iter = stmts.into_iter().peekable();

    while let Some(stmt) = iter.next() {
        let skip = if let JsStmt::Expr(ref e) = stmt {
            matches!(iter.peek(), Some(JsStmt::Assign { expr: rhs, .. }) if rhs == e)
        } else {
            false
        };

        if !skip {
            let stmt = match stmt {
                JsStmt::If { cond, then_body, else_body } => JsStmt::If {
                    cond,
                    then_body: elide_redundant_assigns(then_body),
                    else_body: elide_redundant_assigns(else_body),
                },
                JsStmt::Loop { body } => JsStmt::Loop {
                    body: elide_redundant_assigns(body),
                },
                JsStmt::Switch { expr, cases } => JsStmt::Switch {
                    expr,
                    cases: cases.into_iter()
                        .map(|(k, b)| (k, elide_redundant_assigns(b)))
                        .collect(),
                },
                other => other,
            };
            out.push(stmt);
        }
    }
    out
}
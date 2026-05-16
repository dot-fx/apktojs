use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{JsExpr, JsStmt};

pub fn cleanup(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let stmts = elide_redundant_assigns(stmts);
    let stmts = simplify_loops(stmts);
    let stmts = simplify_array_add(stmts);
    let stmts = simplify_first_instance(stmts);

    stmts
}

fn simplify_first_instance(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < stmts.len() {
        if i + 2 < stmts.len() {
            if let (
                JsStmt::While { body, .. },
                JsStmt::Assign { .. },
                JsStmt::Expr(JsExpr::Raw(s)),
            ) = (&stmts[i], &stmts[i + 1], &stmts[i + 2])
            {
                if s.starts_with("throw ")
                    && try_rewrite_first_instance(body)
                {
                    out.push(JsStmt::Assign {
                        reg: 6,
                        expr: JsExpr::Raw(
                            "firstInstance(v5, (v6) => v6 instanceof s1)".into()
                        ),
                    });

                    i += 3;
                    continue;
                }
            }
        }

        out.push(rewrite_first_instance_stmt(stmts[i].clone()));
        i += 1;
    }

    out
}

fn rewrite_first_instance_stmt(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::If {
            cond,
            then_body,
            else_body,
        } => JsStmt::If {
            cond,
            then_body: simplify_first_instance(then_body),
            else_body: simplify_first_instance(else_body),
        },

        JsStmt::Loop { body } => JsStmt::Loop {
            body: simplify_first_instance(body),
        },

        JsStmt::While { cond, body } => JsStmt::While {
            cond,
            body: simplify_first_instance(body),
        },

        JsStmt::Switch { expr, cases, default } => JsStmt::Switch {
            expr,
            cases: cases
                .into_iter()
                .map(|(k, v)| (k, simplify_first_instance(v)))
                .collect(),
            default: default.map(simplify_first_instance),
        },

        other => other,
    }
}

fn try_rewrite_first_instance(body: &[JsStmt]) -> bool {
    if body.len() < 2 {
        return false;
    }

    let mut found_instanceof = false;

    for stmt in body {
        match stmt {
            JsStmt::Assign {
                expr: JsExpr::Raw(s),
                ..
            } if s.contains("instanceof") => {
                found_instanceof = true;
            }

            JsStmt::If {
                then_body,
                else_body,
                ..
            } => {
                if else_body.is_empty()
                    && then_body.len() == 1
                    && matches!(then_body[0], JsStmt::Break)
                    && found_instanceof
                {
                    return true;
                }
            }

            _ => {}
        }
    }

    false
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

        JsStmt::Switch { expr, cases, default } => {
            JsStmt::Switch {
                expr,
                cases: cases
                    .into_iter()
                    .map(|(k, v)| (k, simplify_loops(v)))
                    .collect(),
                default: default.map(simplify_loops),
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
                JsStmt::Switch { expr, cases, default } => JsStmt::Switch {
                    expr,
                    cases: cases.into_iter()
                        .map(|(k, b)| (k, elide_redundant_assigns(b)))
                        .collect(),
                    default: default.map(elide_redundant_assigns),
                },
                other => other,
            };
            out.push(stmt);
        }
    }
    out
}
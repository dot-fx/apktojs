use crate::extensions::apk_translator::translator::dalvik::interpreter::{JsExpr, JsStmt};

pub fn cleanup(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let stmts = elide_redundant_assigns(stmts);
    let stmts = simplify_array_add(stmts);
    let stmts = simplify_first_instance(stmts);
    let stmts = simplify_foreach(stmts);
    let stmts = elide_trailing_return(stmts);
    stmts
}

fn elide_trailing_return(mut stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    if matches!(stmts.last(), Some(JsStmt::Return(None))) {
        stmts.pop();
    }
    stmts
}

fn simplify_first_instance(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < stmts.len() {
        if i + 2 < stmts.len() {
            if let (
                JsStmt::While { cond, body },
                JsStmt::Assign { reg: result_reg, .. },
                JsStmt::Expr(JsExpr::Raw(s)),
            ) = (&stmts[i], &stmts[i + 1], &stmts[i + 2])
            {
                if s.starts_with("throw ") {
                    if let Some((_inst_reg, class_name)) = try_rewrite_first_instance(body) {
                        let iter_reg = extract_iter_reg(cond);
                        let result_reg = *result_reg;
                        let expr = format!(
                            "firstInstance(v{}, (v{}) => v{} instanceof {})",
                            iter_reg, iter_reg, iter_reg, class_name,
                        );
                        out.push(JsStmt::Assign {
                            reg: result_reg,
                            expr: JsExpr::Raw(expr),
                        });
                        i += 3;
                        continue;
                    }
                }
            }
        }

        if let JsStmt::While { cond, body } = &stmts[i] {
            if let Some((_inst_reg, class_name)) = try_rewrite_first_instance(body) {
                let iter_reg = extract_iter_reg(cond);
                let expr = format!(
                    "firstInstance(v{}, (v{}) => v{} instanceof {})",
                    iter_reg, iter_reg, iter_reg, class_name
                );
                out.push(JsStmt::Expr(JsExpr::Raw(expr)));
                i += 1;
                continue;
            }
        }

        out.push(rewrite_first_instance_stmt(stmts[i].clone()));
        i += 1;
    }

    out
}

fn extract_iter_reg(cond: &JsExpr) -> u8 {
    match cond {
        JsExpr::MethodCall { receiver, .. } => match receiver.as_ref() {
            JsExpr::Reg(r) => *r,
            _ => 5,
        },
        JsExpr::Reg(r) => *r,
        _ => 5,
    }
}

fn rewrite_first_instance_stmt(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::If { cond, then_body, else_body } => JsStmt::If {
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

fn try_rewrite_first_instance(body: &[JsStmt]) -> Option<(u8, String)> {
    let mut instanceof_reg: Option<u8> = None;
    let mut class_name: Option<String> = None;

    for stmt in body {
        match stmt {
            JsStmt::Assign { reg, expr: JsExpr::Raw(s) } if s.contains("instanceof") => {
                if let Some(cls) = s.split("instanceof").nth(1) {
                    instanceof_reg = Some(*reg);
                    class_name = Some(
                        cls.trim()
                            .trim_end_matches(|c| c == ')' || c == ';')
                            .trim()
                            .to_string()
                    );
                }
            }
            JsStmt::If { then_body, else_body, .. }
            if else_body.is_empty()
                && then_body.len() == 1
                && matches!(then_body[0], JsStmt::Break)
                && instanceof_reg.is_some() =>
                {
                    return Some((instanceof_reg.unwrap(), class_name.unwrap()));
                }
            _ => {}
        }
    }
    None
}

fn simplify_foreach(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < stmts.len() {
        if i + 2 < stmts.len() {
            if let Some(expr) = try_rewrite_foreach(&stmts[i], &stmts[i + 1], &stmts[i + 2]) {
                out.push(JsStmt::Expr(JsExpr::Raw(expr)));
                i += 3;
                continue;
            }
        }

        out.push(rewrite_foreach_stmt(stmts[i].clone()));
        i += 1;
    }

    out
}

fn try_rewrite_foreach(s0: &JsStmt, s1: &JsStmt, s2: &JsStmt) -> Option<String> {
    let (list_reg, list_expr) = match s0 {
        JsStmt::Assign {
            reg,
            expr: JsExpr::MethodCall { receiver, method, args },
        } if args.is_empty() && method != "iterator" => {
            (*reg, format!("{}.{}()", expr_to_str(receiver), method))
        }
        _ => return None,
    };

    match s1 {
        JsStmt::Assign {
            reg,
            expr: JsExpr::MethodCall { receiver, method, args },
        } if *reg == list_reg
            && method == "iterator"
            && args.is_empty()
            && matches!(receiver.as_ref(), JsExpr::Reg(r) if *r == list_reg) => {}
        _ => return None,
    }

    let (item_reg, callback_stmts) = match s2 {
        JsStmt::While { cond, body } => {
            match cond {
                JsExpr::MethodCall { receiver, method, args }
                if method == "hasNext"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), JsExpr::Reg(r) if *r == list_reg) => {}
                _ => return None,
            }

            if body.len() < 2 {
                return None;
            }

            let item_reg = match &body[0] {
                JsStmt::Assign {
                    reg,
                    expr: JsExpr::MethodCall { receiver, method, args },
                } if method == "next"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), JsExpr::Reg(r) if *r == list_reg) =>
                    {
                        *reg
                    }
                _ => return None,
            };

            (item_reg, &body[1..])
        }
        _ => return None,
    };

    let body_str = if callback_stmts.len() == 1 {
        callback_stmts.iter()
            .map(stmt_to_str)
            .collect::<Option<Vec<_>>>()?
            .join("; ")
    } else {
        let stmts_rendered = callback_stmts.iter()
            .map(stmt_to_str)
            .collect::<Option<Vec<_>>>()?;
        format!("{{ {} }}", stmts_rendered.join("; "))
    };

    Some(format!("{}.forEach(v{} => {})", list_expr, item_reg, body_str))
}

fn expr_to_str(expr: &JsExpr) -> String {
    crate::extensions::apk_translator::translator::emit::render::expr_to_js(expr, false)
}

fn stmt_to_str(stmt: &JsStmt) -> Option<String> {
    match stmt {
        JsStmt::Expr(e) => Some(expr_to_str(e)),
        JsStmt::Assign { reg, expr } => Some(format!("v{} = {}", reg, expr_to_str(expr))),
        JsStmt::FieldSet { receiver, field, value } => Some(format!(
            "{}.{} = {}",
            expr_to_str(receiver), field, expr_to_str(value)
        )),
        JsStmt::ArraySet { arr, idx, value } => Some(format!(
            "{}[{}] = {}",
            expr_to_str(arr), expr_to_str(idx), expr_to_str(value)
        )),
        JsStmt::Return(Some(e)) => Some(format!("return {}", expr_to_str(e))),
        JsStmt::Return(None) => Some("return".into()),
        // Anything structurally complex (if/while/switch) can't inline into forEach — bail out
        _ => None,
    }
}

fn rewrite_foreach_stmt(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::If { cond, then_body, else_body } => JsStmt::If {
            cond,
            then_body: simplify_foreach(then_body),
            else_body: simplify_foreach(else_body),
        },
        JsStmt::Loop { body } => JsStmt::Loop {
            body: simplify_foreach(body),
        },
        JsStmt::While { cond, body } => JsStmt::While {
            cond,
            body: simplify_foreach(body),
        },
        JsStmt::Switch { expr, cases, default } => JsStmt::Switch {
            expr,
            cases: cases
                .into_iter()
                .map(|(k, v)| (k, simplify_foreach(v)))
                .collect(),
            default: default.map(simplify_foreach),
        },
        other => other,
    }
}

fn simplify_array_add(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    stmts.into_iter().map(rewrite_add_stmt).collect()
}

fn rewrite_add_stmt(stmt: JsStmt) -> JsStmt {
    match stmt {
        JsStmt::Expr(expr) => JsStmt::Expr(rewrite_add_expr(expr)),
        JsStmt::Assign { reg, expr } => JsStmt::Assign {
            reg,
            expr: rewrite_add_expr(expr),
        },
        JsStmt::If { cond, then_body, else_body } => JsStmt::If {
            cond: rewrite_add_expr(cond),
            then_body: simplify_array_add(then_body),
            else_body: simplify_array_add(else_body),
        },
        JsStmt::Loop { body } => JsStmt::Loop {
            body: simplify_array_add(body),
        },
        JsStmt::While { cond, body } => JsStmt::While {
            cond: rewrite_add_expr(cond),
            body: simplify_array_add(body),
        },
        other => other,
    }
}

fn rewrite_add_expr(expr: JsExpr) -> JsExpr {
    match expr {
        JsExpr::MethodCall { receiver, method, args } if method == "add" => {
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
            let is_pure = matches!(e, JsExpr::Reg(_) | JsExpr::Int(_) | JsExpr::Str(_));
            is_pure && matches!(iter.peek(), Some(JsStmt::Assign { expr: rhs, .. }) if rhs == e)
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
                    cases: cases
                        .into_iter()
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
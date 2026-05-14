use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::JsStmt;

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
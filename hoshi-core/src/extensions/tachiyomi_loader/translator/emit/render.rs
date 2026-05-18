use std::collections::HashSet;
use crate::extensions::tachiyomi_loader::{ApkMeta, WalkedSource};
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter;
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{JsExpr, JsStmt};

pub struct JsMethod {
    pub name: String,
    pub body: String,
    pub defined_in: String,
}

pub fn stmts_to_js(stmts: &[JsStmt], indent: usize, _method_name: &str) -> String {
    let stmts = strip_dead_code(stmts);
    let mut declared: HashSet<u8> = HashSet::new();
    let mut lines = Vec::new();
    render_stmts(&stmts, indent, &mut declared, &mut lines);
    lines.join("\n")
}

fn is_valid_js_ident(s: &str) -> bool {
    let mut chars = s.chars();

    match chars.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| {
        c == '_' || c == '$' || c.is_ascii_alphanumeric()
    })
}

fn js_prop(obj: &str, prop: &str) -> String {
    if is_valid_js_ident(prop) {
        format!("{}.{}", obj, prop)
    } else {
        format!("{}[\"{}\"]", obj, escape_js_string(prop))
    }
}

fn escape_js_string(s: &str) -> String {
    let mut out = String::new();

    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"'  => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }

    out
}

fn render_method_name(name: &str) -> String {
    match name {
        "<init>"   => "constructor".to_string(),
        "<clinit>" => "__static_init__".to_string(),
        _ => {
            if is_valid_js_ident(name) {
                name.to_string()
            } else {
                format!("[\"{}\"]", escape_js_string(name))
            }
        }
    }
}

fn strip_dead_code(stmts: &[JsStmt]) -> Vec<JsStmt> {
    let mut out = Vec::new();
    for stmt in stmts {
        let is_terminal = matches!(
            stmt,
            JsStmt::Return(_) | JsStmt::Break | JsStmt::Continue
        ) || matches!(stmt, JsStmt::Expr(JsExpr::Raw(s)) if s.starts_with("throw"))
            || matches!(stmt, JsStmt::Expr(JsExpr::StaticCall { method, .. }) if method == "throw")
            || matches!(stmt, JsStmt::Throw);

        let stmt = match stmt {
            JsStmt::If {
                cond,
                then_body,
                else_body,
            } => JsStmt::If {
                cond: cond.clone(),
                then_body: strip_dead_code(then_body),
                else_body: strip_dead_code(else_body),
            },
            JsStmt::Loop { body } => JsStmt::Loop {
                body: strip_dead_code(body),
            },
            JsStmt::Switch { expr, cases, default } => JsStmt::Switch {
                expr: expr.clone(),
                cases: cases.iter()
                    .map(|(k, body)| (*k, strip_dead_code(body)))
                    .collect(),
                default: default.as_ref().map(|body| strip_dead_code(body)),
            },
            other => other.clone(),
        };

        out.push(stmt);
        if is_terminal { break; }
    }
    out
}

fn simplify_cond(expr: &JsExpr) -> String {
    if let JsExpr::UnaryOp { op: "!", expr: inner } = expr {
        if let JsExpr::UnaryOp { op: "!", expr: innermost } = inner.as_ref() {
            return expr_to_js(innermost);
        }
    }

    if let JsExpr::BinOp { op, left, right } = expr {
        return format!("{} {} {}", expr_to_js(left), op, expr_to_js(right));
    }

    expr_to_js(expr)
}

fn render_stmts(
    stmts:    &[JsStmt],
    indent:   usize,
    declared: &mut HashSet<u8>,
    lines:    &mut Vec<String>,
) {
    let pad = " ".repeat(indent);

    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, expr } => {
                if declared.insert(*reg) {
                    lines.push(format!("{}let v{} = {};", pad, reg, expr_to_js(expr)));
                } else {
                    lines.push(format!("{}v{} = {};", pad, reg, expr_to_js(expr)));
                }
            }

            JsStmt::StaticGet { class, field, dst } => {
                if declared.insert(*dst) {
                    lines.push(format!("{}let v{} = {}.{};", pad, dst, class, field));
                } else {
                    lines.push(format!("{}v{} = {}.{};", pad, dst, class, field));
                }
            }

            JsStmt::StaticSet { class, field, value } => {
                lines.push(format!("{}{}.{} = {};", pad, class, field, expr_to_js(value)));
            }

            JsStmt::FieldSet { receiver, field, value } => {
                lines.push(format!("{}{}.{} = {};",
                                   pad, expr_to_js(receiver), field, expr_to_js(value)));
            }

            JsStmt::ArraySet { arr, idx, value } => {
                lines.push(format!("{}{}[{}] = {};",
                                   pad, expr_to_js(arr), expr_to_js(idx), expr_to_js(value)));
            }

            JsStmt::Expr(e) => {
                lines.push(format!("{}{};", pad, expr_to_js(e)));
            }

            JsStmt::Return(None) => {
                lines.push(format!("{}return;", pad));
            }

            JsStmt::Return(Some(e)) => {
                lines.push(format!("{}return {};", pad, expr_to_js(e)));
            }

            JsStmt::If {
                cond,
                then_body,
                else_body,
            } => {
                let (cond, then_body, else_body) = if then_body.is_empty() && !else_body.is_empty() {
                    (interpreter::negate(cond.clone()), else_body, then_body)
                } else {
                    (cond.clone(), then_body, else_body)
                };

                lines.push(format!("{}if ({}) {{", pad, simplify_cond(&cond)));
                render_stmts(then_body, indent + 2, declared, lines);

                if else_body.is_empty() {
                    lines.push(format!("{}}}", pad));
                } else {
                    lines.push(format!("{}}} else {{", pad));

                    render_stmts(else_body, indent + 2, declared, lines);

                    lines.push(format!("{}}}", pad));
                }
            }

            JsStmt::Loop { body } => {
                lines.push(format!("{}while (true) {{", pad));
                render_stmts(body, indent + 2, declared, lines);
                lines.push(format!("{}}}", pad));
            }

            JsStmt::Break    => { lines.push(format!("{}break;",    pad)); }
            JsStmt::Continue => { lines.push(format!("{}continue;", pad)); }

            JsStmt::Comment(c) => {
                if !c.is_empty() {
                    lines.push(format!("{}{}", pad, c));
                }
            }

            JsStmt::Switch { expr, cases, default } => {
                lines.push(format!("{}switch ({}) {{", pad, expr_to_js(expr)));
                let mut i = 0;
                while i < cases.len() {
                    let (key, body) = &cases[i];
                    let mut j = i + 1;
                    while j < cases.len() {
                        let same = cases[j].1.len() == body.len() &&
                            cases[j].1.iter().zip(body.iter()).all(|(a, b)| {
                                format!("{:?}", a) == format!("{:?}", b)
                            });
                        if same { j += 1; } else { break; }
                    }

                    for k in i..j {
                        if k == j - 1 {
                            lines.push(format!("{}  case {}: {{", pad, cases[k].0));
                        } else {
                            lines.push(format!("{}  case {}:", pad, cases[k].0));
                        }
                    }
                    render_stmts(body, indent + 4, declared, lines);
                    let needs_break = !matches!(body.last(),
                        Some(JsStmt::Break | JsStmt::Return(_) | JsStmt::Continue)
                    ) && !matches!(body.last(),
                        Some(JsStmt::Expr(JsExpr::Raw(s))) if s.starts_with("throw")
                    );
                    if needs_break {
                        lines.push(format!("{}    break;", pad));
                    }
                    lines.push(format!("{}  }}", pad));
                    i = j;
                }
                if let Some(body) = default {
                    lines.push(format!("{}  default: {{", pad));

                    render_stmts(body, indent + 4, declared, lines);

                    let needs_break = !matches!(
        body.last(),
        Some(JsStmt::Break | JsStmt::Return(_) | JsStmt::Continue)
    ) && !matches!(
        body.last(),
        Some(JsStmt::Expr(JsExpr::Raw(s))) if s.starts_with("throw")
    );

                    if needs_break {
                        lines.push(format!("{}    break;", pad));
                    }

                    lines.push(format!("{}  }}", pad));
                }
                lines.push(format!("{}}}", pad));
            }

            JsStmt::CondGoto { cond, target } => {
                lines.push(format!("{}/* if ({}) goto {} */",
                                   pad, expr_to_js(cond), target));
            }

            JsStmt::While { cond, body } => {
                lines.push(format!(
                    "{}while ({}) {{",
                    pad,
                    simplify_cond(cond),
                ));

                render_stmts(body, indent + 2, declared, lines);

                lines.push(format!("{}}}", pad));
            }

            JsStmt::DoWhile { body, cond } => {
                lines.push(format!("{}do {{", pad));
                render_stmts(body, indent + 2, declared, lines);
                lines.push(format!("{}}} while ({});", pad, simplify_cond(cond)));
            }

            JsStmt::Goto(target) => {
                lines.push(format!("{}/* goto {} */", pad, target));
            }

            JsStmt::Throw => {
                lines.push(format!("{}throw;", pad));
            }
        }
    }
}

pub fn render_class(
    class_name: &str,
    base_class:  &str,
    meta:        &ApkMeta,
    methods:     &[JsMethod],
    walked:      &WalkedSource,
) -> String {
    let mut out = String::new();

    out.push_str("// AUTO-GENERATED - Tachiyomi extension translator\n");
    out.push_str(&format!("// Package  : {}\n", meta.package));
    out.push_str(&format!("// Name     : {} (lang: {})\n", meta.name, meta.lang));
    out.push_str(&format!("// Version  : {} ({})\n", meta.version_name, meta.version_code));
    out.push_str(&format!("// Hierarchy: {}\n", walked.hierarchy.join(" → ")));
    out.push_str(&format!("// Kind     : {:?}\n", walked.kind));
    out.push('\n');

    let mut groups: Vec<(String, Vec<&JsMethod>)> = Vec::new();
    for method in methods {
        if let Some(g) = groups.iter_mut().find(|(name, _)| name == &method.defined_in) {
            g.1.push(method);
        } else {
            groups.push((method.defined_in.clone(), vec![method]));
        }
    }

    let walked_class = walked.methods.first()
        .map(|m| m.defined_in.as_str())
        .unwrap_or("");

    let is_main = |owner: &str| -> bool {
        owner == class_name
            || owner == walked_class
            || walked.hierarchy.first().map(|h| h == owner).unwrap_or(false)
    };

    for (owner, group_methods) in &groups {
        if is_main(owner) { continue; }

        let simple = owner.split('.').last().unwrap_or(owner);
        out.push_str(&format!("class {} {{\n", simple));
        emit_methods(&mut out, group_methods);
        out.push_str("}\n\n");
    }

    let main_methods: Vec<&JsMethod> = groups.iter()
        .filter(|(owner, _)| is_main(owner))
        .flat_map(|(_, ms)| ms.iter().copied())
        .collect();

    out.push_str(&format!("class {} extends {} {{\n", class_name, base_class));
    emit_methods(&mut out, &main_methods);
    out.push_str("}\n");

    out
}

fn emit_methods(out: &mut String, methods: &[&JsMethod]) {
    out.push('\n');
    for method in methods {
        let mut max_arg: Option<usize> = None;
        for line in method.body.lines() {
            if let Some(pos) = line.find("arguments[") {
                let after = &line[pos + "arguments[".len()..];
                if let Some(end) = after.find(']') {
                    if let Ok(i) = after[..end].parse::<usize>() {
                        max_arg = Some(max_arg.map_or(i, |m: usize| m.max(i)));
                    }
                }
            }
        }

        let params = match max_arg {
            None => String::new(),
            Some(max) => (0..=max)
                .map(|i| format!("arg{}", i))
                .collect::<Vec<_>>()
                .join(", "),
        };

        let body = match max_arg {
            None => method.body.clone(),
            Some(max) => {
                let mut b = method.body.clone();
                for i in 0..=max {
                    b = b.replace(&format!("arguments[{}]", i), &format!("arg{}", i));
                }
                b
            }
        };

        out.push_str(&format!(
            "  {}({}) {{\n",
            render_method_name(&method.name),
            params
        ));
        if !body.is_empty() {
            out.push_str(&body);
            out.push('\n');
        } else {
            out.push_str("    // empty method body\n");
        }
        out.push_str("  }\n\n");
    }
}

pub fn expr_to_js(expr: &JsExpr) -> String {
    match expr {
        JsExpr::Null        => "null".into(),
        JsExpr::Bool(b)     => b.to_string(),
        JsExpr::Int(n)      => n.to_string(),
        JsExpr::Float(f) => {
            if f.is_infinite() {
                if f.is_sign_positive() {
                    "Infinity".into()
                } else {
                    "-Infinity".into()
                }
            } else if f.is_nan() {
                "NaN".into()
            } else {
                f.to_string()
            }
        }
        JsExpr::Str(s) => format!("\"{}\"", escape_js_string(s)),
        JsExpr::Reg(r)      => format!("v{}", r),
        JsExpr::This        => "this".into(),
        JsExpr::Raw(s)      => s.clone(),

        JsExpr::BitMask { expr, mask } => {
            format!("({} {})", expr_to_js(expr), mask)
        }

        JsExpr::MethodCall { receiver, method, args } => {
            let r = expr_to_js(receiver);
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            if r == "super" && method == "constructor" {
                format!("super({})", a)
            } else {
                format!("{}({})", js_prop(&r, method), a)
            }
        }

        JsExpr::StaticCall { class, method, args } => {
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            if method.is_empty() {
                format!("{}({})", class, a)
            } else {
                format!("{}.{}({})", class, method, a)
            }
        }
        JsExpr::New { class, args } => {
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            format!("new {}({})", class, a)
        }
        JsExpr::FieldGet { receiver, field } => {
            js_prop(&expr_to_js(receiver), field)
        }
        JsExpr::BinOp { op, left, right } => {
            format!("({} {} {})", expr_to_js(left), op, expr_to_js(right))
        }
        JsExpr::UnaryOp { op, expr } => {
            match expr.as_ref() {
                JsExpr::Reg(_) | JsExpr::MethodCall { .. } => {
                    format!("{}{}",  op, expr_to_js(expr))
                }
                _ => format!("({}{})", op, expr_to_js(expr))
            }
        }

        JsExpr::ArrayLiteral(items) => {
            let parts = items
                .iter()
                .map(expr_to_js)
                .collect::<Vec<_>>()
                .join(", ");

            format!("[{}]", parts)
        }

        JsExpr::StringConcat(items) => {
            items.iter()
                .map(|e| format!("String({})", expr_to_js(e)))
                .collect::<Vec<_>>()
                .join(" + ")
        }

        JsExpr::Index { arr, idx } => {
            format!("{}[{}]", expr_to_js(arr), expr_to_js(idx))
        }
    }
}
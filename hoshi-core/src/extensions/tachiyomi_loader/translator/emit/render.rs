use std::collections::HashSet;
use crate::extensions::tachiyomi_loader::{ApkMeta, WalkedSource};
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{JsExpr, JsStmt};
use crate::extensions::tachiyomi_loader::translator::emit::params::method_params;

pub struct JsMethod {
    pub name: String,
    pub body: String,
}

pub fn stmts_to_js(stmts: &[JsStmt], indent: usize, method_name: &str) -> String {
    let stmts = strip_dead_code(stmts);
    let mut declared: HashSet<u8> = HashSet::new();

    let params_str = method_params(method_name);
    let param_count = if params_str.is_empty() { 0 } else { params_str.split(',').count() };

    let _ = param_count;

    let mut lines = Vec::new();
    render_stmts(&stmts, indent, &mut declared, &mut lines);

    let param_names: Vec<&str> = if params_str.is_empty() {
        vec![]
    } else {
        params_str.split(", ").map(str::trim).collect()
    };

    lines.iter().map(|line| {
        if let Some(rest) = line.trim_start().strip_prefix("let v") {
            if let Some(eq_pos) = rest.find(" = arguments[") {
                let reg_str = &rest[..eq_pos];
                if let Some(arg_pos) = rest.find("arguments[") {
                    let after = &rest[arg_pos + "arguments[".len()..];
                    if let Some(end) = after.find(']') {
                        if let Ok(i) = after[..end].parse::<usize>() {
                            if let Some(&pname) = param_names.get(i) {
                                let pad = " ".repeat(line.len() - line.trim_start().len());
                                return format!("{}let v{} = {};", pad, reg_str, pname);
                            }
                        }
                    }
                }
            }
        }
        line.clone()
    }).collect::<Vec<_>>().join("\n")
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
    if is_valid_js_ident(name) {
        name.to_string()
    } else {
        format!("[\"{}\"]", escape_js_string(name))
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
                lines.push(format!("{}if ({}) {{", pad, simplify_cond(cond)));

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

    out.push_str(&format!("class {} extends {} {{\n", class_name, base_class));
    out.push_str(&format!("  get name()    {{ return {:?}; }}\n", meta.name));
    out.push_str(&format!("  get lang()    {{ return {:?}; }}\n", meta.lang));
    out.push_str("  get baseUrl() { return /* TODO: fill in baseUrl */\"\"; }\n");
    if meta.nsfw {
        out.push_str("  // nsfw = true\n");
    }
    out.push('\n');

    for method in methods {
        let params = method_params(&method.name);
        out.push_str(&format!(
            "  {}({}) {{\n",
            render_method_name(&method.name),
            params
        ));
        if !method.body.is_empty() {
            out.push_str(&method.body);
            out.push('\n');
        } else {
            out.push_str("    // empty method body\n");
        }
        out.push_str("  }\n\n");
    }

    out.push_str("}\n\n");

    match walked.kind {
        crate::extensions::tachiyomi_loader::EntryKind::Factory => {
            out.push_str("// SourceFactory: instantiate variants then call registerSources([...])\n");
            out.push_str(&format!("registerSources([new {}()]);\n", class_name));
        }
        crate::extensions::tachiyomi_loader::EntryKind::Direct => {
            out.push_str(&format!("registerSource(new {}());\n", class_name));
        }
    }

    out
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
            format!("{}({})", js_prop(&r, method), a)
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
            format!("({}{})", op, expr_to_js(expr))
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
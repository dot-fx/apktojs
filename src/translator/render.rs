use std::collections::HashSet;
use crate::apk_inspector::ApkMeta;
use crate::dex_walker::WalkedSource;
use crate::translator::dalvik::interpreter;
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId};
use crate::translator::resolver::pool::Pool;
use crate::translator::resolver::resolve::TypeNames;

#[derive(Clone)]
pub struct JsMethod {
    pub name: String,
    pub body: String,
    pub defined_in: String,
    pub is_static: bool,
    pub param_count: usize,
}

fn hoist_super(stmts: Vec<JsStmt>, has_super: bool, names: &TypeNames, pool: &Pool) -> Vec<JsStmt> {
    let super_pos = stmts.iter().position(|s| {
        matches!(s, JsStmt::Expr(JsExpr::SuperCall { .. }))
    });

    let Some(super_pos) = super_pos else {
        return stmts;
    };

    let mut result = vec![];
    let mut deferred = vec![];
    let mut tmp_counter = 0;

    for stmt in stmts[..super_pos].iter().cloned() {
        match stmt {
            // this.x = expr  before super() → temp it, defer the assignment
            JsStmt::FieldSet { receiver: JsExpr::This, ref field, ref value } => {
                let tmp_name = format!("__super_tmp_{}_{}", field, tmp_counter);
                tmp_counter += 1;

                let rendered_expr = expr_to_js(value, has_super, names, pool);
                let safe_expr = if rendered_expr.trim().is_empty() {
                    "undefined".to_string()
                } else {
                    rendered_expr
                };

                // declare the temp before super()
                result.push(JsStmt::Expr(JsExpr::Raw(
                    format!("var {} = {};", tmp_name, safe_expr)
                )));

                let field_name = {
                    let has_conflict = pool.type_info.values()
                        .any(|t| t.methods.iter().any(|m| m == field));
                    if has_conflict { format!("{}_val", field) } else { field.clone() }
                };

                // assign from temp after super()
                deferred.push(JsStmt::Expr(JsExpr::Raw(
                    format!("{} = {};", js_prop("this", &field_name), tmp_name)
                )));
            }

            // Any other this.x = expr (non-This receiver FieldSet) stays in place
            // All Assign, Expr, Comment, etc. stay before super() — they don't touch `this`
            other => {
                result.push(other);
            }
        }
    }

    result.push(stmts[super_pos].clone());   // super(...)
    result.extend(deferred);                  // this.x = __super_tmp_x_N
    result.extend(stmts[super_pos + 1..].iter().cloned());
    result
}

pub fn stmts_to_js(stmts: &[JsStmt], indent: usize, _method_name: &str, has_super: bool, names: &TypeNames, pool: &Pool) -> String {
    let stmts = strip_dead_code(stmts);
    let stmts = hoist_super(stmts, has_super, names, pool);

    let mut lines = Vec::new();

    let mut declared: HashSet<RegId> = HashSet::new();

    render_stmts(&stmts, indent, &mut declared, &mut lines, has_super, names, pool);

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
            let name = if let Some(pos) = name.find('-') {
                &name[..pos]
            } else {
                name
            };

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
            || matches!(stmt, JsStmt::Throw)
            || matches!(stmt, JsStmt::Expr(JsExpr::UnaryOp { op, .. }) if *op == "throw ")
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

fn simplify_cond(expr: &JsExpr, has_super: bool, names: &TypeNames, pool: &Pool) -> String {
    if let JsExpr::UnaryOp { op: "!", expr: inner } = expr {
        if let JsExpr::UnaryOp { op: "!", expr: innermost } = inner.as_ref() {
            return expr_to_js(innermost, has_super, names, pool);
        }
    }

    if let JsExpr::BinOp { op, left, right } = expr {
        return format!("{} {} {}", expr_to_js(left, has_super, names, pool), op, expr_to_js(right, has_super, names, pool));
    }

    expr_to_js(expr, has_super, names, pool)
}

fn render_stmts(
    stmts:    &[JsStmt],
    indent:   usize,
    declared: &mut HashSet<RegId>,
    lines:    &mut Vec<String>,
    has_super: bool,
    names: &TypeNames,
    pool: &Pool
) {
    let pad = " ".repeat(indent);

    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, expr } => {
                let rendered = expr_to_js(expr, has_super, names, pool);

                let clean_expr = if rendered.trim().is_empty() {
                    "undefined".to_string()
                } else if rendered.trim().starts_with("var ") || rendered.trim().starts_with("let ") {
                    rendered.trim().split('=').last().unwrap_or("undefined").replace(';', "").trim().to_string()
                } else {
                    rendered
                };

                let prefix = if declared.insert(reg.clone()) { "var " } else { "" };
                lines.push(format!(
                    "{}{}v{}_{} = {};",
                    pad,
                    prefix,
                    reg.reg,
                    reg.version,
                    clean_expr
                ));
            }

            JsStmt::StaticGet { class, field, dst } => {
                let resolved_class = names.resolve(class);
                let field_name = if pool.type_info.get(class)
                    .map(|t| t.methods.iter().any(|m| m == field))
                    .unwrap_or(false)
                    || pool.type_info.values().any(|t| {
                    (t.full_name == *class || t.simple_name == names.resolve(class))
                        && t.methods.iter().any(|m| m == field)
                })
                {
                    format!("{}_val", field)
                } else {
                    field.clone()
                };

                let prefix = if declared.insert(dst.clone()) {
                    "var "
                } else {
                    ""
                };

                lines.push(format!(
                    "{}{}v{}_{} = {}.{};",
                    pad,
                    prefix,
                    dst.reg,
                    dst.version,
                    resolved_class,
                    field_name
                ));
            }

            JsStmt::Param { name, .. } => {
            }

            JsStmt::StaticSet { class, field, value } => {
                let resolved_class = names.resolve(class);
                let field_name = if pool.type_info.get(class)
                    .map(|t| t.methods.iter().any(|m| m == field))
                    .unwrap_or(false)
                    || pool.type_info.values().any(|t| {
                    (t.full_name == *class || t.simple_name == names.resolve(class))
                        && t.methods.iter().any(|m| m == field)
                })
                {
                    format!("{}_val", field)
                } else {
                    field.clone()
                };
                lines.push(format!("{}{}.{} = {};", pad, resolved_class, field_name,
                                   expr_to_js(value, has_super, names, pool)));
            }

            JsStmt::FieldSet { receiver, field, value } => {
                let receiver_js = expr_to_js(receiver, has_super, names, pool);
                let value_js = expr_to_js(value, has_super, names, pool);

                let field_name = {
                    let has_conflict = pool.type_info.values().any(|t| {
                        t.methods.iter().any(|m| m == field)
                    });
                    if has_conflict {
                        format!("{}_val", field)
                    } else {
                        field.clone()
                    }
                };

                if value_js.trim().is_empty() {
                    lines.push(format!("{}/* Dropped empty FieldSet for {}.{} */", pad, receiver_js, field_name));
                } else {
                    lines.push(format!("{}{}.{} = {};", pad, receiver_js, field_name, value_js));
                }
            }

            JsStmt::ArraySet { arr, idx, value } => {
                let value_js = expr_to_js(value, has_super, names, pool);

                if value_js.trim().is_empty() {
                    lines.push(format!("{}/* Dropped empty ArraySet */", pad));
                } else {
                    lines.push(format!(
                        "{}{}[{}] = {};",
                        pad,
                        expr_to_js(arr, has_super, names, pool),
                        expr_to_js(idx, has_super, names, pool),
                        value_js
                    ));
                }
            }

            JsStmt::Expr(e) => {
                let rendered = expr_to_js(e, has_super, names, pool);

                if !rendered.is_empty() {
                    lines.push(format!("{}{};", pad, rendered));
                }
            }

            JsStmt::Return(None) => {
                lines.push(format!("{}return;", pad));
            }

            JsStmt::Return(Some(e)) => {
                lines.push(format!("{}return {};", pad, expr_to_js(e, has_super, names, pool)));
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

                lines.push(format!("{}if ({}) {{", pad, simplify_cond(&cond, has_super, names, pool)));
                render_stmts(then_body, indent + 2, declared, lines, has_super, names, pool);

                if else_body.is_empty() {
                    lines.push(format!("{}}}", pad));
                } else {
                    lines.push(format!("{}}} else {{", pad));
                    render_stmts(else_body, indent + 2, declared, lines, has_super, names, pool);
                    lines.push(format!("{}}}", pad));
                }
            }

            JsStmt::Loop { body } => {
                lines.push(format!("{}while (true) {{", pad));
                render_stmts(body, indent + 2, declared, lines, has_super, names, pool);
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
                lines.push(format!("{}switch ({}) {{", pad, expr_to_js(expr, has_super, names, pool)));
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
                    render_stmts(body, indent + 4, declared, lines, has_super, names, pool);
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

                    render_stmts(body, indent + 4, declared, lines, has_super, names, pool);

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
                                   pad, expr_to_js(cond, has_super, names, pool), target));
            }

            JsStmt::While { cond, body } => {
                lines.push(format!(
                    "{}while ({}) {{",
                    pad,
                    simplify_cond(cond, has_super, names, pool),
                ));

                render_stmts(body, indent + 2, declared, lines, has_super, names, pool);

                lines.push(format!("{}}}", pad));
            }

            JsStmt::DoWhile { body, cond } => {
                lines.push(format!("{}do {{", pad));
                render_stmts(body, indent + 2, declared, lines, has_super, names, pool);
                lines.push(format!("{}}} while ({});", pad, simplify_cond(cond, has_super, names, pool)));
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
    meta:        &ApkMeta,
    methods:     &[JsMethod],
    walked:      &WalkedSource,
    pool: &Pool,
    names: &TypeNames,
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

    let ordered = topo_sort_classes(&groups, pool);

    let mut static_inits = Vec::new();

    for owner in ordered {
        let (_, group_methods) = groups.iter()
            .find(|(o, _)| *o == owner)
            .unwrap();

        let mut assigned_fields = HashSet::new();
        let mut read_fields = HashSet::new();

        for method in group_methods {
            for line in method.body.lines() {
                if let Some(pos) = line.find("this.") {
                    let sub = &line[pos + 5..];
                    if let Some(end) = sub.find(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                        let field_name = sub[..end].to_string();
                        if method.name == "<init>" {
                            assigned_fields.insert(field_name);
                        } else {
                            read_fields.insert(field_name);
                        }
                    }
                }
            }
        }

        let mut missing_fields: Vec<String> = read_fields
            .into_iter()
            .filter(|f| !assigned_fields.contains(f) && f.ends_with("_val"))
            .collect();
        missing_fields.sort();

        let simple = names.resolve(&*owner);
        let super_name: Option<&str> = pool.type_info
            .get(&*owner)
            .and_then(|t| t.superclass.as_deref())
            .filter(|s| {
                *s != "java.lang.Object"
                    && !s.ends_with(".Object")
                    && *s != "Object"
            });

        let is_entry_class = owner == walked.class_name
            || names.resolve(&*owner) == names.resolve(&walked.class_name);

        let extends_clause = if is_entry_class && walked.kind == crate::dex_walker::EntryKind::Factory {
            " extends SourceFactory".to_string()
        } else {
            super_name
                .filter(|&s|
                    s != "Object"
                        && s != "java.lang.Object"
                        && !s.ends_with(".Object")
                )
                .map(|s| format!(" extends {}", names.resolve(s)))
                .unwrap_or_default()
        };

        out.push_str(&format!(
            "class {}{} {{\n",
            simple,
            extends_clause
        ));

        let has_super = !extends_clause.is_empty();
        emit_methods(&mut out, group_methods, &*owner, is_main(&owner), has_super);

        out.push_str("}\n\n");
        if group_methods.iter().any(|m| m.name == "<clinit>") {
            static_inits.push(simple.clone());
        }
    }

    let re = regex::Regex::new(r"new (\w+)\(").unwrap();
    let re2 = regex::Regex::new(r"(\w+)\.Companion").unwrap();
    let re3 = regex::Regex::new(r"(\w+)\.\w+").unwrap();

    let static_init_set: HashSet<String> = static_inits.iter().cloned().collect();
    let mut si_deps: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for cls in &static_inits {
        let body = methods.iter()
            .find(|m| m.name == "<clinit>" && names.resolve(&m.defined_in) == *cls)
            .map(|m| m.body.as_str())
            .unwrap_or("");

        let ctor_body = methods.iter()
            .find(|m| m.name == "<init>" && names.resolve(&m.defined_in) == *cls)
            .map(|m| m.body.as_str())
            .unwrap_or("");

        let combined = format!("{}\n{}", body, ctor_body);

        let deps: Vec<String> = re.captures_iter(&combined)
            .map(|c| names.resolve(&c[1]))
            .chain(re2.captures_iter(&combined).map(|c| names.resolve(&c[1])))
            .chain(re3.captures_iter(&combined).map(|c| names.resolve(&c[1])))
            .filter(|resolved_c| static_init_set.contains(resolved_c) && resolved_c != cls)
            .collect();

        si_deps.insert(cls.clone(), deps);
    }
    fn si_visit(cls: &str, deps: &std::collections::HashMap<String, Vec<String>>, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        if !seen.insert(cls.to_string()) { return; }
        if let Some(d) = deps.get(cls) {
            for dep in d.clone() { si_visit(&dep, deps, seen, out); }
        }
        out.push(cls.to_string());
    }
    let mut si_seen = HashSet::new();
    let mut sorted_inits = Vec::new();
    for cls in &static_inits {
        si_visit(cls, &si_deps, &mut si_seen, &mut sorted_inits);
    }
    for cls in sorted_inits {
        out.push_str(&format!(
            "if (typeof {}.__static_init__ === 'function') {}.__static_init__();\n",
            cls, cls
        ));
    }

    out
}

fn emit_methods(
    out: &mut String,
    methods: &[&JsMethod],
    owner_class: &str,
    is_self: bool,
    has_super: bool,
) {
    let inits: Vec<&JsMethod> = methods.iter()
        .filter(|m| m.name == "<init>")
        .map(|m| *m)
        .collect();

    let merged_init: Option<JsMethod> = inits.iter()
        .max_by_key(|m| m.param_count)
        .map(|m| (*m).clone());

    let mut seen_names: HashSet<String> = HashSet::new();
    let mut deduped: Vec<JsMethod> = Vec::new();

    if let Some(init) = merged_init {
        seen_names.insert("<init>".to_string());
        deduped.push(init);
    }

    for method in methods.iter().map(|m| *m) {
        if method.name == "<init>" {
            continue;
        }
        if seen_names.insert(method.name.clone()) {
            deduped.push(method.clone());
        }
    }
    deduped.reverse();

    out.push('\n');
    for method in deduped {
        let max_arg = if method.param_count > 0 {
            Some(method.param_count - 1)
        } else {
            None
        };

        let params_list: Vec<String> = match max_arg {
            None => vec![],
            Some(max) => (0..=max).map(|i| format!("arg{}", i)).collect(),
        };

        let mut body = method.body.clone();
        if has_super && method.name == "<init>" && !body.contains("super(") {
            body = format!("    super();\n{}", body);
        }

        if let Some(max) = max_arg {
            for i in 0..=max {
                body = body.replace(
                    &format!("arguments[{}]", i),
                    &format!("arg{}", i),
                );
            }
        }

        body = fix_self_refs(&body, owner_class, is_self);

        let params = params_list.join(", ");

        out.push_str(&format!(
            "  {}{}({}) {{\n",
            if method.is_static { "static " } else { "" },
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

pub fn expr_to_js(expr: &JsExpr, has_super: bool, names: &TypeNames, pool: &Pool) -> String {
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
        JsExpr::StaticFieldGet { class, field } => {
            let resolved = names.resolve(class);
            let field_name = if pool.type_info.get(class)
                .map(|t| t.methods.iter().any(|m| m == field))
                .unwrap_or(false)
                || pool.type_info.values().any(|t| {
                (t.full_name == *class || t.simple_name == names.resolve(class))
                    && t.methods.iter().any(|m| m == field)
            })
            {
                format!("{}_val", field)
            } else {
                field.clone()
            };
            format!("{}.{}", resolved, field_name)
        }
        JsExpr::Str(s) => format!("\"{}\"", escape_js_string(s)),
        JsExpr::Reg(r) => format!("v{}_{}", r.reg, r.version),
        JsExpr::This        => "this".into(),
        JsExpr::Raw(s)      => s.clone(),

        JsExpr::BitMask { expr, mask } => {
            format!("({} {})", expr_to_js(expr, has_super, names, pool), mask)
        }

        JsExpr::MethodCall { receiver, method, args, is_static } => {
            let a = args.iter()
                .map(|e| expr_to_js(e, has_super, names, pool))
                .collect::<Vec<_>>()
                .join(", ");

            if *is_static {

                let class_name = match receiver.as_ref() {
                    JsExpr::Raw(s) => names.resolve(s),
                    _ => expr_to_js(receiver, has_super, names, pool),
                };
                format!("{}.{}({})", class_name, method, a)
            } else {
                let r = expr_to_js(receiver, has_super, names, pool);
                format!("{}({})", js_prop(&r, method), a)
            }
        }

        JsExpr::SuperCall { args } => {
            let a = args.iter()
                .map(|e| expr_to_js(e, has_super, names, pool))
                .collect::<Vec<_>>()
                .join(", ");

            if has_super {
                format!("super({})", a)
            } else {
                String::new()
            }
        }

        JsExpr::ThisCtorCall { args } => String::new(),

        JsExpr::StaticCall { class, method, args } => {
            let a = args.iter().map(|e| expr_to_js(e, has_super, names, pool)).collect::<Vec<_>>().join(", ");
            if method.is_empty() {
                format!("{}({})", names.resolve(class), a)
            } else {
                format!("{}.{}({})", names.resolve(class), method, a)
            }
        }
        JsExpr::New { class, args } => {
            let a = args.iter().map(|e| expr_to_js(e, has_super, names, pool)).collect::<Vec<_>>().join(", ");
            format!("new {}({})", names.resolve(class), a)
        }
        JsExpr::FieldGet { receiver, field } => {
            let has_conflict = pool.type_info.values().any(|t| {
                t.methods.iter().any(|m| m == field)
            });
            let field_name = if has_conflict {
                format!("{}_val", field)
            } else {
                field.clone()
            };
            js_prop(&expr_to_js(receiver, has_super, names, pool), &field_name)
        }
        JsExpr::BinOp { op, left, right } => {
            format!("({} {} {})", expr_to_js(left, has_super, names, pool), op, expr_to_js(right, has_super, names, pool))
        }
        JsExpr::UnaryOp { op, expr } => {
            match expr.as_ref() {
                JsExpr::Reg(_) | JsExpr::MethodCall { .. } => {
                    format!("{}{}",  op, expr_to_js(expr, has_super, names, pool))
                }
                _ => format!("({}{})", op, expr_to_js(expr, has_super, names, pool))
            }
        }

        JsExpr::ArrayLiteral(items) => {
            let parts = items
                .iter()
                .map(|e| expr_to_js(e, has_super, names, pool))
                .collect::<Vec<_>>()
                .join(", ");

            format!("[{}]", parts)
        }

        JsExpr::StringConcat(items) => {
            items.iter()
                .map(|e| format!("String({})", expr_to_js(e, has_super, names, pool)))
                .collect::<Vec<_>>()
                .join(" + ")
        }

        JsExpr::Index { arr, idx } => {
            format!("{}[{}]", expr_to_js(arr, has_super, names, pool), expr_to_js(idx, has_super, names, pool))
        }
    }
}

fn fix_self_refs(
    body: &str,
    owner_class: &str,
    is_self: bool
) -> String {
    let mut out = body.to_string();
    if is_self {
        let from = format!("new {}(", owner_class);

        out = out.replace(
            &from,
            "new this.constructor("
        );
    }

    out
}

fn topo_sort_classes(
    groups: &[(String, Vec<&JsMethod>)],
    pool: &Pool,
) -> Vec<String> {
    fn visit(
        cls: &str,
        pool: &Pool,
        seen: &mut HashSet<String>,
        out: &mut Vec<String>,
        existing: &HashSet<String>,
    ) {
        if !seen.insert(cls.to_string()) {
            return;
        }

        if let Some(super_cls) = pool.type_info.get(cls)
            .and_then(|t| t.superclass.as_deref())
        {
            if existing.contains(super_cls) {
                visit(super_cls, pool, seen, out, existing);
            }
        }

        out.push(cls.to_string());
    }

    let existing: HashSet<String> =
        groups.iter().map(|(c, _)| c.clone()).collect();

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for (cls, _) in groups {
        visit(cls, pool, &mut seen, &mut out, &existing);
    }

    out
}
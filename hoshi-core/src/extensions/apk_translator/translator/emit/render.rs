use std::collections::HashSet;
use crate::extensions::apk_translator::{ApkMeta, WalkedSource};
use crate::extensions::apk_translator::translator::dalvik::interpreter;
use crate::extensions::apk_translator::translator::dalvik::interpreter::{JsExpr, JsStmt};
use crate::extensions::apk_translator::translator::resolver::pool::Pool;
use crate::extensions::apk_translator::translator::resolver::resolve::TypeNames;

pub struct JsMethod {
    pub name: String,
    pub body: String,
    pub defined_in: String,
    pub is_static: bool,
}

fn hoist_super(stmts: Vec<JsStmt>) -> Vec<JsStmt> {
    let super_pos = stmts.iter().position(|s| {
        matches!(s, JsStmt::Expr(JsExpr::SuperCall { .. }))
    });

    let Some(super_pos) = super_pos else {
        return stmts;
    };

    let mut before: Vec<JsStmt> = vec![];
    let mut deferred: Vec<JsStmt> = vec![];

    for stmt in stmts[..super_pos].iter().cloned() {
        match &stmt {
            JsStmt::FieldSet { receiver: JsExpr::This, .. } => deferred.push(stmt),
            _ => before.push(stmt),
        }
    }

    let mut result = before;
    result.push(stmts[super_pos].clone());  // super(...)
    result.extend(deferred);                // this.x = ... now safe
    result.extend(stmts[super_pos + 1..].iter().cloned());
    result
}

pub fn stmts_to_js(stmts: &[JsStmt], indent: usize, _method_name: &str, has_super: bool, names: &TypeNames) -> String {
    let stmts = strip_dead_code(stmts);
    let stmts = hoist_super(stmts);

    let mut all_regs: Vec<u8> = {
        let mut set = HashSet::new();
        collect_assigned_regs(&stmts, &mut set);
        collect_read_regs(&stmts, &mut set);
        let mut v: Vec<u8> = set.into_iter().collect();
        v.sort();
        v
    };

    let mut lines = Vec::new();

    if !all_regs.is_empty() {
        let pad = " ".repeat(indent);
        let decls = all_regs.iter().map(|r| format!("v{}", r)).collect::<Vec<_>>().join(", ");
        lines.push(format!("{}let {};", pad, decls));
    }

    let mut declared: HashSet<u8> = all_regs.into_iter().collect(); // all pre-declared
    render_stmts(&stmts, indent, &mut declared, &mut lines, has_super, names);

    lines.join("\n")
}

fn collect_assigned_regs(stmts: &[JsStmt], out: &mut HashSet<u8>) {
    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, .. } => { out.insert(*reg); }
            JsStmt::StaticGet { dst, .. } => { out.insert(*dst); }
            JsStmt::If { then_body, else_body, .. } => {
                collect_assigned_regs(then_body, out);
                collect_assigned_regs(else_body, out);
            }
            JsStmt::Loop { body } | JsStmt::While { body, .. } => {
                collect_assigned_regs(body, out);
            }
            JsStmt::Switch { cases, default, .. } => {
                for (_, body) in cases { collect_assigned_regs(body, out); }
                if let Some(d) = default { collect_assigned_regs(d, out); }
            }
            _ => {}
        }
    }
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

fn simplify_cond(expr: &JsExpr, has_super: bool, names: &TypeNames) -> String {
    if let JsExpr::UnaryOp { op: "!", expr: inner } = expr {
        if let JsExpr::UnaryOp { op: "!", expr: innermost } = inner.as_ref() {
            return expr_to_js(innermost, has_super, names);
        }
    }

    if let JsExpr::BinOp { op, left, right } = expr {
        return format!("{} {} {}", expr_to_js(left, has_super, names), op, expr_to_js(right, has_super, names));
    }

    expr_to_js(expr, has_super, names)
}

fn render_stmts(
    stmts:    &[JsStmt],
    indent:   usize,
    declared: &mut HashSet<u8>,
    lines:    &mut Vec<String>,
    has_super: bool,
    names: &TypeNames
) {
    let pad = " ".repeat(indent);

    for stmt in stmts {
        match stmt {
            JsStmt::Assign { reg, expr } => {
                lines.push(format!(
                    "{}v{} = {};",
                    pad,
                    reg,
                    expr_to_js(expr, has_super, names)
                ));
            }

            JsStmt::Param { name, .. } => {
            }

            JsStmt::StaticGet { class, field, dst } => {
                lines.push(format!("{}v{} = {}.{};", pad, dst, class, field));
            }

            JsStmt::StaticSet { class, field, value } => {
                lines.push(format!("{}{}.{} = {};", pad, class, field, expr_to_js(value, has_super, names)));
            }

            JsStmt::FieldSet { receiver, field, value } => {
                lines.push(format!("{}{}.{} = {};",
                                   pad, expr_to_js(receiver, has_super, names), field, expr_to_js(value, has_super, names)));
            }

            JsStmt::ArraySet { arr, idx, value } => {
                lines.push(format!("{}{}[{}] = {};",
                                   pad, expr_to_js(arr, has_super, names), expr_to_js(idx, has_super, names), expr_to_js(value, has_super, names)));
            }

            JsStmt::Expr(e) => {
                let rendered = expr_to_js(e, has_super, names);

                if !rendered.is_empty() {
                    lines.push(format!("{}{};", pad, rendered));
                }
            }

            JsStmt::Return(None) => {
                lines.push(format!("{}return;", pad));
            }

            JsStmt::Return(Some(e)) => {
                lines.push(format!("{}return {};", pad, expr_to_js(e, has_super, names)));
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

                lines.push(format!("{}if ({}) {{", pad, simplify_cond(&cond, has_super, names)));
                render_stmts(then_body, indent + 2, declared, lines, has_super, names);

                if else_body.is_empty() {
                    lines.push(format!("{}}}", pad));
                } else {
                    lines.push(format!("{}}} else {{", pad));

                    render_stmts(else_body, indent + 2, declared, lines, has_super, names);

                    lines.push(format!("{}}}", pad));
                }
            }

            JsStmt::Loop { body } => {
                lines.push(format!("{}while (true) {{", pad));
                render_stmts(body, indent + 2, declared, lines, has_super, names);
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
                lines.push(format!("{}switch ({}) {{", pad, expr_to_js(expr, has_super, names)));
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
                    render_stmts(body, indent + 4, declared, lines, has_super, names);
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

                    render_stmts(body, indent + 4, declared, lines, has_super, names);

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
                                   pad, expr_to_js(cond, has_super, names), target));
            }

            JsStmt::While { cond, body } => {
                lines.push(format!(
                    "{}while ({}) {{",
                    pad,
                    simplify_cond(cond, has_super, names),
                ));

                render_stmts(body, indent + 2, declared, lines, has_super, names);

                lines.push(format!("{}}}", pad));
            }

            JsStmt::DoWhile { body, cond } => {
                lines.push(format!("{}do {{", pad));
                render_stmts(body, indent + 2, declared, lines, has_super, names);
                lines.push(format!("{}}} while ({});", pad, simplify_cond(cond, has_super, names)));
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

    let has_super =
        base_class != "Object"
            && base_class != "java.lang.Object"
            && !base_class.ends_with(".Object");

    let ordered = topo_sort_classes(&groups, pool);

    for owner in ordered {
        let (_, group_methods) = groups.iter()
            .find(|(o, _)| *o == owner)
            .unwrap();
        if is_main(&*owner) { continue; }

        let simple = names.resolve(&*owner);
        let super_name = pool.type_info.get(&owner)
            .and_then(|t| t.superclass.as_deref())
            .filter(|&s| s != "Object" && s != "java.lang.Object" && !s.ends_with(".Object"));


        let extends_clause = super_name
            .map(|s| format!(" extends {}", names.resolve(s)))
            .unwrap_or_default();

        out.push_str(&format!(
            "class {}{} {{\n",
            simple,
            extends_clause
        ));

        emit_methods(&mut out, group_methods, &*owner, false);

        out.push_str("}\n\n");
        if group_methods.iter().any(|m| m.name == "<clinit>") {
            out.push_str(&format!(
                "if (typeof {}.__static_init__ === 'function') {}.__static_init__();\n\n",
                simple, simple
            ));
        }
    }

    let main_methods: Vec<&JsMethod> = groups.iter()
        .filter(|(owner, _)| is_main(owner))
        .flat_map(|(_, ms)| ms.iter().copied())
        .collect();

    let extends_clause =
        if has_super {
            format!(" extends {}", names.resolve(base_class))
        } else {
            String::new()
        };

    out.push_str(&format!(
        "class {}{} {{\n",
        class_name,
        extends_clause
    ));
    emit_methods(&mut out, &main_methods, class_name, true);

    out.push_str("}\n");

    out
}

fn emit_methods(
    out: &mut String,
    methods: &[&JsMethod],
    owner_class: &str,
    is_self: bool
) {
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

        let mut body = match max_arg {
            None => method.body.clone(),
            Some(max) => {
                let mut b = method.body.clone();

                for i in 0..=max {
                    b = b.replace(
                        &format!("arguments[{}]", i),
                        &format!("arg{}", i),
                    );
                }

                b
            }
        };

        body = fix_self_refs(
            &body,
            owner_class,
            is_self,
        );

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

pub fn expr_to_js(expr: &JsExpr, has_super: bool, names: &TypeNames) -> String {
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
            format!("{}.{}", names.resolve(class), field)
        }
        JsExpr::Str(s) => format!("\"{}\"", escape_js_string(s)),
        JsExpr::Reg(r)      => format!("v{}", r),
        JsExpr::This        => "this".into(),
        JsExpr::Raw(s)      => s.clone(),

        JsExpr::BitMask { expr, mask } => {
            format!("({} {})", expr_to_js(expr, has_super, names), mask)
        }

        JsExpr::MethodCall { receiver, method, args, is_static } => {
            let r = expr_to_js(receiver, has_super, names);

            let a = args.iter()
                .map(|e| expr_to_js(e, has_super, names))
                .collect::<Vec<_>>()
                .join(", ");

            if *is_static {
                format!("{}.{}({})", names.resolve(&r), method, a)
            } else {
                format!("{}({})", js_prop(&r, method), a)
            }
        }

        JsExpr::SuperCall { args } => {
            let a = args.iter()
                .map(|e| expr_to_js(e, has_super, names))
                .collect::<Vec<_>>()
                .join(", ");

            if has_super {
                format!("super({})", a)
            } else {
                format!("/* super({}) -- no extends? */", a)
            }
        }

        JsExpr::ThisCtorCall { args } => {
            let a = args.iter()
                .map(|e| expr_to_js(e, has_super, names))
                .collect::<Vec<_>>()
                .join(", ");

            format!("this.constructor({})", a)
        }

        JsExpr::StaticCall { class, method, args } => {
            let a = args.iter().map(|e| expr_to_js(e, has_super, names)).collect::<Vec<_>>().join(", ");
            if method.is_empty() {
                format!("{}({})", names.resolve(class), a)
            } else {
                format!("{}.{}({})", names.resolve(class), method, a)
            }
        }
        JsExpr::New { class, args } => {
            let a = args.iter().map(|e| expr_to_js(e, has_super, names)).collect::<Vec<_>>().join(", ");
            format!("new {}({})", names.resolve(class), a)
        }
        JsExpr::FieldGet { receiver, field } => {
            js_prop(&expr_to_js(receiver, has_super, names), field)
        }
        JsExpr::BinOp { op, left, right } => {
            format!("({} {} {})", expr_to_js(left, has_super, names), op, expr_to_js(right, has_super, names))
        }
        JsExpr::UnaryOp { op, expr } => {
            match expr.as_ref() {
                JsExpr::Reg(_) | JsExpr::MethodCall { .. } => {
                    format!("{}{}",  op, expr_to_js(expr, has_super, names))
                }
                _ => format!("({}{})", op, expr_to_js(expr, has_super, names))
            }
        }

        JsExpr::ArrayLiteral(items) => {
            let parts = items
                .iter()
                .map(|e| expr_to_js(e, has_super, names))
                .collect::<Vec<_>>()
                .join(", ");

            format!("[{}]", parts)
        }

        JsExpr::StringConcat(items) => {
            items.iter()
                .map(|e| format!("String({})", expr_to_js(e, has_super, names)))
                .collect::<Vec<_>>()
                .join(" + ")
        }

        JsExpr::Index { arr, idx } => {
            format!("{}[{}]", expr_to_js(arr, has_super, names), expr_to_js(idx, has_super, names))
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

        if out.contains(&from) {
            eprintln!("self ctor replace hit: {:?}", from);
        }

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

fn collect_read_regs(stmts: &[JsStmt], out: &mut HashSet<u8>) {
    for stmt in stmts {
        match stmt {
            JsStmt::Assign { expr, .. } => collect_read_regs_expr(expr, out),
            JsStmt::Expr(e) => collect_read_regs_expr(e, out),
            JsStmt::Return(Some(e)) => collect_read_regs_expr(e, out),
            JsStmt::FieldSet { receiver, value, .. } => {
                collect_read_regs_expr(receiver, out);
                collect_read_regs_expr(value, out);
            }
            JsStmt::ArraySet { arr, idx, value } => {
                collect_read_regs_expr(arr, out);
                collect_read_regs_expr(idx, out);
                collect_read_regs_expr(value, out);
            }
            JsStmt::If { cond, then_body, else_body } => {
                collect_read_regs_expr(cond, out);
                collect_read_regs(then_body, out);
                collect_read_regs(else_body, out);
            }
            JsStmt::Loop { body } | JsStmt::While { body, .. } | JsStmt::DoWhile { body, .. } => {
                collect_read_regs(body, out);
            }
            JsStmt::Switch { expr, cases, default } => {
                collect_read_regs_expr(expr, out);
                for (_, body) in cases { collect_read_regs(body, out); }
                if let Some(d) = default { collect_read_regs(d, out); }
            }
            _ => {}
        }
    }
}

fn collect_read_regs_expr(expr: &JsExpr, out: &mut HashSet<u8>) {
    match expr {
        JsExpr::Reg(r) => { out.insert(*r); }
        JsExpr::MethodCall { receiver, args, .. } => {
            collect_read_regs_expr(receiver, out);
            for a in args { collect_read_regs_expr(a, out); }
        }
        JsExpr::StaticCall { args, .. } | JsExpr::New { args, .. } => {
            for a in args { collect_read_regs_expr(a, out); }
        }
        JsExpr::SuperCall { args } | JsExpr::ThisCtorCall { args } => {
            for a in args { collect_read_regs_expr(a, out); }
        }
        JsExpr::BinOp { left, right, .. } => {
            collect_read_regs_expr(left, out);
            collect_read_regs_expr(right, out);
        }
        JsExpr::UnaryOp { expr, .. } | JsExpr::BitMask { expr, .. } => {
            collect_read_regs_expr(expr, out);
        }
        JsExpr::FieldGet { receiver, .. } => collect_read_regs_expr(receiver, out),
        JsExpr::Index { arr, idx } => {
            collect_read_regs_expr(arr, out);
            collect_read_regs_expr(idx, out);
        }
        JsExpr::ArrayLiteral(items) | JsExpr::StringConcat(items) => {
            for i in items { collect_read_regs_expr(i, out); }
        }
        _ => {}
    }
}
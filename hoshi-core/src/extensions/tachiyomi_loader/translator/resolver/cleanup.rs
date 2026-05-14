use regex::Regex;

pub fn remove_duplicate_stmts(js: &str) -> String {
    let lines: Vec<&str> = js.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut depth = 0i32;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        for ch in trimmed.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }

        let cur = trimmed.trim_end_matches(';');

        let should_skip = depth == 0
            && i + 1 < lines.len()
            && {
            let next = lines[i + 1].trim();
            let rhs = next
                .strip_prefix("let ")
                .and_then(|s| s.find(" = ").map(|eq| s[eq + 3..].trim_end_matches(';')));
            rhs == Some(cur)
        };

        if should_skip {
            i += 1;
            continue;
        }

        out.push(lines[i]);
        i += 1;
    }

    out.join("\n")
}

pub fn remove_serializers_module_stmts(js: &str) -> String {
    let re = Regex::new(r"[ \t]*\w+\.getSerializersModule\(\);\n").unwrap();
    re.replace_all(js, "").into_owned()
}

pub fn collapse_companion_chains(js: &str) -> String {
    let lines: Vec<&str> = js.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        let companion = trimmed
            .strip_suffix(';')
            .and_then(|s| {
                let s = s.strip_prefix("let ").unwrap_or(s);
                // s is now: "vN = Foo.Companion"
                let (lhs, rhs) = s.split_once(" = ")?;
                let class = rhs.strip_suffix(".Companion")?;
                Some((lhs.trim(), class))
            });

        if let Some((var, class)) = companion {
            if let Some(next) = lines.get(i + 1) {
                let next_trimmed = next.trim();
                let prefix = format!("{} = {}.", var, var);
                if let Some(rest) = next_trimmed.strip_prefix(&prefix) {
                    let pad = " ".repeat(next.len() - next.trim_start().len());
                    let is_let = lines[i].trim().starts_with("let ");
                    let decl = if is_let { "let " } else { "" };
                    out.push(format!("{}{}{} = {}.{}", pad, decl, var, class, rest));
                    i += 2;
                    continue;
                }
            }
        }

        out.push(lines[i].to_string());
        i += 1;
    }

    out.join("\n")
}
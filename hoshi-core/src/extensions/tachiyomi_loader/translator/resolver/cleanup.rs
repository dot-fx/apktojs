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
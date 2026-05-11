use std::collections::HashMap;


use dex::{Dex, DexReader};
use regex::Regex;


use crate::extensions::tachiyomi_loader::ExtractedDex;


pub fn resolve(raw_js: &str, extracted: &ExtractedDex) -> String {
    let pool = Pool::build(&extracted.dex_files);
    let mut js = raw_js.to_string();

    js = apply_well_known(&js);
    js = resolve_strings(&js, &pool);
    js = resolve_static_methods(&js, &pool);
    js = resolve_fields(&js, &pool);
    js = resolve_sfields(&js, &pool);
    js = resolve_methods(&js, &pool);
    js = resolve_types(&js, &pool);
    js = remove_duplicate_stmts(&js);

    js
}

struct Pool {
    strings: HashMap<(usize, u32), String>,
    methods: HashMap<(usize, u32), MethodInfo>,
    fields:  HashMap<(usize, u32), FieldInfo>,
    types:   HashMap<(usize, u32), String>,
}


struct MethodInfo {
    class_name:  String,
    method_name: String,
    js_name:     Option<String>,
}


struct FieldInfo {
    class_name: String,
    field_name: String,
}


impl Pool {
    fn build(shards: &[Dex<Vec<u8>>]) -> Self {
        let mut strings = HashMap::new();
        let mut methods = HashMap::new();
        let mut fields  = HashMap::new();
        let mut types   = HashMap::new();


        for (shard_idx, shard) in shards.iter().enumerate() {
            for (idx, s) in shard.strings().enumerate() {
                if let Ok(s) = s {
                    strings.insert((shard_idx, idx as u32), s.to_string());
                }
            }


            // Types
            for (idx, t) in shard.types().enumerate() {
                if let Ok(t) = t {
                    types.insert((shard_idx, idx as u32), from_dex_type(t.to_string().as_str()));
                }
            }


            // Methods
            for (idx, item) in shard.method_ids().enumerate() {
                if let Ok(item) = item {
                    let class_name = shard.get_type(item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();
                    let method_name = shard.get_string(item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let js_name = well_known_method(&class_name, &method_name);
                    methods.insert((shard_idx, idx as u32), MethodInfo { class_name, method_name, js_name });
                }
            }

            // Fields
            for (idx, item) in shard.field_ids().enumerate() {
                if let Ok(item) = item {
                    let class_name = shard.get_type(*item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();
                    let field_name = shard.get_string(*item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    fields.insert((shard_idx, idx as u32), FieldInfo { class_name, field_name });
                }
            }
        }


        for s in 0..shards.len() {
            let max_idx = fields.keys().filter(|(si, _)| *si == s).map(|(_, i)| *i).max();
            eprintln!("Shard {} max field idx = {:?}", s, max_idx);
        }
        Pool { strings, methods, fields, types }
    }
}



fn resolve_strings(js: &str, pool: &Pool) -> String {
    // Replace /* string#N */ with the actual string literal
    let re = Regex::new(r"/\* string#(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_string(pool, idx){
            Some(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            None    => format!("/* string#{} */", idx),
        }
    }).into_owned()
}


fn resolve_methods(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"_meth(\d+)\(").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_method(pool, idx) {
            Some(m) => {
                if m.method_name == "<init>" || m.method_name.starts_with("<") {
                    return "/* ctor */(".to_string();
                }
                let name = m.js_name.as_deref().unwrap_or(&m.method_name);
                format!("{}(", name)
            }
            None => format!("_meth{}(", idx),
        }
    }).into_owned()
}

fn resolve_fields(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"_field(\d+)").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_field(pool, idx) {
            Some(f) => f.field_name.clone(),
            None    => format!("_field{}", idx),
        }
    }).into_owned()
}

fn resolve_types(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"/\* (?:type|class)#(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_type(pool, idx)  {
            Some(t) => kotlin_class_to_js(t),
            None    => format!("/* type#{} */", idx),
        }
    }).into_owned()
}

fn resolve_static_methods(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"/\* static_meth(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_method(pool, idx) {
            Some(m) => {
                if let Some(js_name) = &m.js_name {
                    js_name.clone()
                } else {
                    match (m.class_name.split('.').last().unwrap_or(""), m.method_name.as_str()) {
                        ("StringsKt", "trim")   => "/* call as: str.trim() */trim".into(),
                        ("StringsKt", "append") => "append".into(),
                        ("StringsKt", "isBlank")   => "/* str */.isBlank()".into(),
                        ("StringsKt", "trimStart") => "/* str */.trimStart()".into(),
                        ("StringsKt", "trimEnd")   => "/* str */.trimEnd()".into(),
                        _ => {
                            let simple = m.class_name.split('.').last().unwrap_or(&m.class_name);
                            format!("{}.{}", simple, m.method_name)
                        }
                    }
                }
            }
            None => format!("/* static_meth{} */", idx),
        }
    }).into_owned()
}

fn resolve_sfields(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"/\* static_field#(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        let known = match idx {
            8    => Some("c.Companion"),
            16   => Some("e0.Companion"),
            47   => Some("SManga.Companion"),
            72   => Some("k1.Companion"),
            87   => Some("n.Companion"),
            91   => Some("o0.Companion"),
            99   => Some("HttpUrl.Companion"),
            101  => Some("q1.Companion"),
            4206 => Some("HttpUrl.Companion"),
            _    => None,
        };
        if let Some(name) = known { return name.to_string(); }
        match lookup_field(pool, idx) {
            Some(f) => {
                let simple = f.class_name.split('.').last().unwrap_or(&f.class_name);
                // Companion objects are the common case for Kotlin serializers
                if f.field_name == "Companion" || f.field_name == "INSTANCE" {
                    format!("{}.{}", simple, f.field_name)
                } else {
                    format!("{}.{}", simple, f.field_name)
                }
            }
            None => format!("/* static_field#{} */", idx),
        }
    }).into_owned()
}

fn remove_duplicate_stmts(js: &str) -> String {
    let lines: Vec<&str> = js.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let cur = lines[i].trim().trim_end_matches(';');

        if i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            let rhs = next
                .strip_prefix("let ")
                .and_then(|s| s.find(" = ").map(|eq| s[eq + 3..].trim_end_matches(';')));

            if rhs == Some(cur) {
                i += 1;
                continue;
            }
        }

        out.push(lines[i]);
        i += 1;
    }

    out.join("\n")
}

fn lookup_string(pool: &Pool, idx: u32) -> Option<&String> {
    (0..16).find_map(|s| pool.strings.get(&(s, idx)))
}

fn lookup_method(pool: &Pool, idx: u32) -> Option<&MethodInfo> {
    (0..16).find_map(|s| pool.methods.get(&(s, idx)))
}

fn lookup_field(pool: &Pool, idx: u32) -> Option<&FieldInfo> {
    for s in 0..16 {
        if let Some(f) = pool.fields.get(&(s, idx)) {
            return Some(f);
        }
    }
    None
}

fn lookup_type(pool: &Pool, idx: u32) -> Option<&String> {
    (0..16).find_map(|s| pool.types.get(&(s, idx)))
}

fn well_known_method(class: &str, method: &str) -> Option<String> {
    let simple = class.split('.').last().unwrap_or(class);


    let mapped = match (simple, method) {
        // StringBuilder / buildString
        ("StringBuilder", "append")   => "append",
        ("StringBuilder", "toString") => "toString",


        // OkHttp Request.Builder
        ("Builder", "url")     => "url",
        ("Builder", "build")   => "build",
        ("Builder", "addHeader") => "addHeader",
        ("Builder", "header")  => "header",
        ("Builder", "post")    => "post",
        ("Builder", "get")     => "get",


        // HttpUrl.Builder
        ("Builder", "addQueryParameter") => "addQueryParameter",
        ("Builder", "addPathSegment")    => "addPathSegment",
        ("Builder", "setQueryParameter") => "setQueryParameter",
        ("Builder", "removeQueryParameter") => "removeQueryParameter",
        ("Builder", "fragment")          => "fragment",
        ("Builder", "newBuilder")        => "newBuilder",


        // String methods
        ("String", "format")   => "/* String.format → */ _fmt",
        ("String", "trim")     => "trim",
        ("String", "split")    => "split",
        ("String", "contains") => "includes",
        ("String", "isEmpty")  => "isBlank",
        ("String", "replace")  => "replace",
        ("String", "lowercase")=> "toLowerCase",
        ("String", "uppercase")=> "toUpperCase",


        // JSONObject / JSONArray
        ("JSONObject", "getString") => "/* json */ getString",
        ("JSONObject", "getInt")    => "/* json */ getInt",
        ("JSONObject", "optString") => "/* json */ optString",
        ("JSONArray",  "length")    => "length",
        ("JSONArray",  "getJSONObject") => "getJSONObject",


        // Jsoup
        ("Jsoup", "parse")  => "Jsoup.parse",
        ("Element", "select") => "select",
        ("Element", "selectFirst") => "selectFirst",
        ("Element", "text")  => "text",
        ("Element", "attr")  => "attr",
        ("Element", "html")  => "html",


        // Collections
        ("ArrayList", "add")   => "push",
        ("MutableList", "add") => "push",


        // Kotlin stdlib
        ("Regex", "find")       => "match",
        ("Regex", "findAll")    => "matchAll",
        ("Regex", "matches")    => "test",
        ("Regex", "containsMatchIn") => "test",
        ("MatchResult", "groupValues") => "/* groupValues */",

        // Kotlin Collections
        ("CollectionsKt", "toMutableList") => "Array.from",
        ("CollectionsKt", "addAll")                  => "push_all",
        ("CollectionsKt", "collectionSizeOrDefault") => "length",

        // Kotlin I/O & Streams
        ("CloseableKt",   "closeFinally")            => "_closeFinally",
        ("JvmStreamsKt",  "decodeFromStream")        => "_decodeFromStream",


        _ => return None,
    };
    Some(mapped.to_string())
}


fn kotlin_class_to_js(class: &str) -> String {
    match class {
        "java.lang.String"          => "String".into(),
        "java.lang.StringBuilder" => "StringBuilder".into(),
        "java.lang.Integer"         => "Number".into(),
        "java.lang.Boolean"         => "Boolean".into(),
        "kotlin.collections.ArrayList"
        | "java.util.ArrayList"     => "Array".into(),
        "okhttp3.HttpUrl"           => "HttpUrl".into(),
        "okhttp3.Request"         => "Request".into(),
        "okhttp3.FormBody"        => "FormBody".into(),
        "eu.kanade.tachiyomi.source.model.SManga" => "SManga".into(),
        "eu.kanade.tachiyomi.source.model.SChapter" => "SChapter".into(),
        "eu.kanade.tachiyomi.source.model.Page"   => "Page".into(),
        "eu.kanade.tachiyomi.source.model.MangasPage" => "MangasPage".into(),
        c => {
            // Return just the simple class name for unknown types
            c.split('.').last().unwrap_or(c).to_string()
        }
    }
}

fn from_dex_type(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}

fn apply_well_known(js: &str) -> String {
    let mut s = js.to_string();


    // `super._meth...` → just super call  (already handled by resolver mostly)
    // `(v0 + "")` is a Kotlin string concat pattern  → `String(v0)`
    let re_tostring = Regex::new(r#"\((\w+) \+ ""\)"#).unwrap();
    s = re_tostring.replace_all(&s, "String($1)").into_owned();


    // `HttpUrl.Builder()` constructor idiom
    s = s.replace("new Builder()", "new HttpUrlBuilder()");
    s = s.replace("HttpUrl.Builder()", "HttpUrl.Builder()");


    // tachiyomi.js already has `GET` / `POST` as top-level fns
    s = s.replace(".newCall(GET(", ".newCall(GET(");
    s = s.replace(".newCall(POST(", ".newCall(POST(");


    // Clean up `/* class */._staticN(` → `_staticN(` (class unknown, best we can do)
    let re_static = Regex::new(r"/\* class \*/\._static(\d+)\(").unwrap();
    s = re_static.replace_all(&s, "/* static_meth$1 */(").into_owned();

    s
}
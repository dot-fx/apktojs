use regex::Regex;
use crate::extensions::tachiyomi_loader::translator::resolver::cleanup::{collapse_companion_chains, remove_duplicate_stmts, remove_serializers_module_stmts};
use crate::extensions::tachiyomi_loader::translator::resolver::lookup::{lookup_field, lookup_method, lookup_string, lookup_type};
use crate::extensions::tachiyomi_loader::translator::resolver::mappings::{apply_well_known, kotlin_class_to_js};
use crate::extensions::tachiyomi_loader::translator::resolver::pool::Pool;

pub fn resolve(raw_js: &str, pool: &Pool) -> String {
    let mut js = raw_js.to_string();

    js = apply_well_known(&js);
    js = resolve_strings(&js, &pool);
    js = resolve_static_methods(&js, &pool);
    js = resolve_fields(&js, &pool);
    js = resolve_sfields(&js, &pool);
    js = resolve_methods(&js, &pool);
    js = resolve_types(&js, &pool);
    js = resolve_lambdas(&js, &pool);
    js = remove_getclass_stmts(&js);
    js = remove_serializers_module_stmts(&js);
    js = collapse_companion_chains(&js);
    js = remove_duplicate_stmts(&js);

    js
}

fn resolve_lambdas(js: &str, pool: &Pool) -> String {
    let re = Regex::new(
        r"new\s+([A-Za-z0-9_$.]+)\(([^)]*)\)"
    ).unwrap();

    re.replace_all(js, |caps: &regex::Captures| {

        let ty = &caps[1];
        let args = &caps[2];

        let Some(info) = pool.type_info.get(ty) else {
            return caps[0].to_string();
        };

        let is_lambda =
            info.superclass
                .as_deref()
                .map(|s| s.contains("kotlin.jvm.internal.Lambda"))
                .unwrap_or(false)
                ||
                info.interfaces.iter().any(|i| {
                    i.starts_with("kotlin.jvm.functions.Function")
                });

        if !is_lambda {
            return caps[0].to_string();
        }

        // locate invoke()
        let invoke_name = info.methods.iter()
            .find(|m| *m == "invoke");

        if invoke_name.is_none() {
            return caps[0].to_string();
        }

        format!(
            "((...args) => {}.invoke({}))",
            ty,
            if args.trim().is_empty() {
                "...args".to_string()
            } else {
                format!("{}, ...args", args)
            }
        )
    }).into_owned()
}

fn resolve_strings(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"/\* string#(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_string(pool, idx) {
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
                if m.method_name.starts_with('<') {
                    return format!("_meth{}(", idx);
                }
                let name = m.js_name.as_deref().unwrap_or(&m.method_name);
                format!("{}(", name)
            }
            None => format!("_meth{}(", idx),
        }
    }).into_owned()
}

fn resolve_fields(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"_field(\d+)_(\d+)").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let shard: usize = caps[1].parse().unwrap_or(0);
        let idx: u32     = caps[2].parse().unwrap_or(u32::MAX);
        match pool.fields.get(&(shard, idx)) {
            Some(f) => f.field_name.clone(),
            None    => format!("_field{}_{}", shard, idx),
        }
    }).into_owned()
}

fn resolve_types(js: &str, pool: &Pool) -> String {
    let re = Regex::new(r"_type(\d+)_(\d+)").unwrap();

    re.replace_all(js, |caps: &regex::Captures| {
        let shard: usize = caps[1].parse().unwrap_or(0);
        let idx: u32 = caps[2].parse().unwrap_or(u32::MAX);

        match pool.types.get(&(shard, idx)) {
            Some(t) => {
                kotlin_class_to_js(t)
                    .split('.')
                    .last()
                    .unwrap_or(t)
                    .to_string()
            }

            None => format!("_type{}_{}", shard, idx),
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
                    return js_name.clone();
                }
                match (m.class_name.split('.').last().unwrap_or(""), m.method_name.as_str()) {
                    _ => {
                        let simple = m.class_name.split('.').last().unwrap_or(&m.class_name);
                        format!("{}.{}", simple, m.method_name)
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
        match lookup_field(pool, idx) {
            Some(f) => {
                let simple = f.class_name.split('.').last().unwrap_or(&f.class_name);
                format!("{}.{}", simple, f.field_name)
            }
            None => format!("/* static_field#{} */", idx),
        }
    }).into_owned()
}

fn remove_getclass_stmts(js: &str) -> String {
    let re = Regex::new(r"[ \t]*\S+\.getClass\(\);\n").unwrap();
    re.replace_all(js, "").into_owned()
}
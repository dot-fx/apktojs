use regex::Regex;
use crate::translator::resolver::cleanup::{collapse_companion_chains, remove_duplicate_stmts, remove_serializers_module_stmts};
use crate::translator::resolver::lookup::{lookup_field, lookup_method, lookup_string, lookup_type};
use crate::translator::resolver::mappings::{apply_well_known, kotlin_class_to_js};
use crate::translator::resolver::pool::Pool;

use std::collections::{HashMap, HashSet};

pub struct TypeNames {
    pub full_to_js: HashMap<String, String>,
}

impl TypeNames {
    pub fn build(pool: &Pool) -> Self {
        let mut used = HashSet::new();
        let mut full_to_js = HashMap::new();

        let reserved = [
            ("java.lang.String", "String"),
            ("java.lang.StringBuilder", "StringBuilder"),
            ("java.lang.Integer", "Integer"),
            ("java.lang.Boolean", "Boolean"),
            ("java.lang.Object", "Object"),
            ("java.util.ArrayList", "ArrayList"),
            ("kotlin.collections.ArrayList", "ArrayList"),
        ];
        for (full, simple) in reserved {
            used.insert(simple.to_string());
            full_to_js.insert(full.to_string(), simple.to_string());
        }

        for ty in pool.type_info.keys() {
            if full_to_js.contains_key(ty.as_str()) { continue; }
            let simple = ty.split('.').last().unwrap_or(ty);

            let final_name = if used.insert(simple.to_string()) {
                simple.to_string()
            } else {
                let mut n = 2;

                loop {
                    let candidate = format!("{}_{}", simple, n);

                    if used.insert(candidate.clone()) {
                        break candidate;
                    }

                    n += 1;
                }
            };
            full_to_js.insert(ty.clone(), final_name);
        }
        Self { full_to_js }
    }

    pub fn resolve(&self, ty: &str) -> String {
        self.full_to_js
            .get(ty)
            .cloned()
            .unwrap_or_else(|| {
                ty.split('.').last().unwrap_or(ty).to_string()
            })
    }
}

pub fn resolve(raw_js: &str, pool: &Pool, names: &TypeNames) -> String {
    let mut js = raw_js.to_string();

    js = apply_well_known(&js);
    js = resolve_strings(&js, &pool);
    js = resolve_static_methods(&js, &pool, &names);
    js = resolve_fields(&js, &pool);
    js = resolve_sfields(&js, &pool, &names);
    js = resolve_methods(&js, &pool);
    js = resolve_types(&js, &pool, &names);
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

        let superclass = info.superclass.as_deref().unwrap_or("");

        let is_suspend_lambda =
            superclass.contains("SuspendLambda")
                || superclass.contains("ContinuationImpl");

        let is_lambda =
            !is_suspend_lambda
                && (
                superclass.contains("kotlin.jvm.internal.Lambda")
                    || info.interfaces.iter().any(|i| {
                    i.starts_with("kotlin.jvm.functions.Function")
                })
            );

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
            "((...args) => new {}({}).invoke(...args))",
            ty,
            args
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
    let re = Regex::new(r"_meth(\d+)_(\d+)\(").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let shard: usize = caps[1].parse().unwrap_or(0);
        let idx: u32     = caps[2].parse().unwrap_or(u32::MAX);
        match pool.methods.get(&(shard, idx)) {
            Some(m) => {
                let name = m.js_name.as_deref().unwrap_or(&m.method_name);
                if name == "<init>"   { return "constructor(".to_string(); }
                if name == "<clinit>" { return "__static_init__(".to_string(); }
                format!("{}(", name)
            }
            None => format!("_meth{}_{}", shard, idx),
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

fn resolve_types(js: &str, pool: &Pool, names: &TypeNames) -> String {
    let re = Regex::new(r"_type(\d+)_(\d+)").unwrap();

    re.replace_all(js, |caps: &regex::Captures| {
        let shard: usize = caps[1].parse().unwrap_or(0);
        let idx: u32 = caps[2].parse().unwrap_or(u32::MAX);

        match pool.types.get(&(shard, idx)) {
            Some(t) => {
                names.resolve(&kotlin_class_to_js(t))
            }

            None => format!("_type{}_{}", shard, idx),
        }
    }).into_owned()
}

fn resolve_static_methods(js: &str, pool: &Pool, names: &TypeNames) -> String {
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
                        let simple = names.resolve(&m.class_name);
                        format!("{}.{}", simple, m.method_name)
                    }
                }
            }
            None => format!("/* static_meth{} */", idx),
        }
    }).into_owned()
}

fn resolve_sfields(js: &str, pool: &Pool, names: &TypeNames) -> String {
    let re = Regex::new(r"/\* static_field#(\d+) \*/").unwrap();
    re.replace_all(js, |caps: &regex::Captures| {
        let idx: u32 = caps[1].parse().unwrap_or(u32::MAX);
        match lookup_field(pool, idx) {
            Some(f) => {
                let simple = names.resolve(&f.class_name);
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
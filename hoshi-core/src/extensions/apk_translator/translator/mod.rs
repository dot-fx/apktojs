pub mod dalvik;
pub mod emit;
pub mod resolver;

use crate::error::CoreError;
use crate::extensions::apk_translator::translator::dalvik::interpreter::{lift, JsStmt};
use crate::extensions::apk_translator::translator::resolver::infer::{rename_source_classes, InferCtx, SymKey};
use crate::extensions::apk_translator::translator::resolver::pool::Pool;
use crate::extensions::apk_translator::{ApkMeta, EntryKind, WalkedSource};

pub struct TranslatedSource {
    pub js: String,
    pub warnings: Vec<String>,
}

impl TranslatedSource {
    pub fn has_warnings(&self) -> bool { !self.warnings.is_empty() }
}

#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("translation error: {0}")]
    Internal(String),
}

impl From<TranslateError> for CoreError {
    fn from(e: TranslateError) -> Self {
        CoreError::Parse(e.to_string())
    }
}

pub fn translate(
    walked: &WalkedSource,
    meta:   &ApkMeta,
    pool:   &Pool,
) -> Result<TranslatedSource, TranslateError> {
    let mut warnings   = Vec::new();
    let mut js_methods = Vec::new();

    let mut lifted: Vec<(Vec<JsStmt>, String, String, bool)> = Vec::new();

    for method in &walked.methods {
        let decoded  = dalvik::decode(&method.insns);
        let insn_only: Vec<_> = decoded.iter().map(|d| d.insn.clone()).collect();

        let (stmts, mut w) = lift(
            &insn_only,
            &decoded,
            &method.name,
            &method.defined_in,
            method.registers_size,
            method.ins_size,
            method.is_static,
            walked.dex_shard,
            pool,
        );

        warnings.append(&mut w);
        lifted.push((
            stmts,
            method.name.clone(),
            method.defined_in.clone(),
            method.is_static,
        ));
    }

    let mut pool_mut = pool.clone();
    let mut infer_ctx = InferCtx::default();

    for (stmts, _, _, _) in &lifted {
        infer_ctx.scan_stmts(stmts, &pool_mut, walked.dex_shard);
    }
    let before: std::collections::HashMap<(usize, u32), Option<String>> = pool_mut.methods.iter()
        .map(|(k, m)| (*k, m.js_name.clone()))
        .collect();

    infer_ctx.apply(&mut pool_mut);

    let renames = rename_source_classes(&mut pool_mut, &meta.name);

    for ((s, idx), m) in &pool_mut.methods {
        let was = before.get(&(*s, *idx)).and_then(|v| v.as_deref());
        let now = m.js_name.as_deref();
        if now != was {
            eprintln!("INFER WROTE: ({},{}) {} → {}",
                      s, idx, m.method_name, now.unwrap_or("None"));
        }
    }

    let mut names = resolver::resolve::TypeNames::build(&pool_mut);
    for (full_name, new_name) in &renames {
        names.full_to_js.insert(full_name.clone(), new_name.clone());
    }

    for (stmts, method_name, defined_in, is_static) in &lifted {
        let has_super = pool.type_info.get(defined_in)
            .and_then(|t| t.superclass.as_deref())
            .map(|s|
                s != "Object"
                    && s != "java.lang.Object"
                    && !s.ends_with(".Object")
            )
            .unwrap_or(false);

        let body = emit::render::stmts_to_js(
            stmts,
            4,
            method_name,
            has_super,
            &names,
            &pool
        );

        js_methods.push(emit::render::JsMethod {
            name: method_name.clone(),
            body,
            defined_in: defined_in.clone(),
            is_static: *is_static,
        });
    }

    for ((s, idx), m) in &pool_mut.methods {
        if *s != walked.dex_shard { continue; }
        if let Some(ev) = infer_ctx.evidence.get(&SymKey::Method(*s, *idx)) {
            if infer_ctx.best_name(&SymKey::Method(*s, *idx)).is_none() && ev.entries.len() > 1 {
                eprintln!("NO WIN ({},{}) class={} method={} entries: {:?}",
                          s, idx, m.class_name, m.method_name,
                          ev.entries.iter().map(|e| format!("{:?}", e.kind)).collect::<Vec<_>>()
                );
            }
        }
    }

    let base_class = match walked.kind {
        EntryKind::Factory => "HttpSource",
        EntryKind::Direct  => {
            if walked.hierarchy.iter().any(|h| h.contains("ParsedHttpSource")) {
                "ParsedHttpSource"
            } else {
                "HttpSource"
            }
        }
    };

    let raw_js = emit::render::render_class(
        &meta.name,
        base_class,
        meta,
        &js_methods,
        walked,
        &pool_mut,
        &names,
    );

    for (i, line) in raw_js.lines().enumerate() {
        if line.contains("m0.a") {
            eprintln!("[pre-resolve] line {}: {}", i, line.trim());
        }
    }

    let resolved = resolver::resolve::resolve(
        &raw_js,
        &pool_mut,
        &names,
    );

    Ok(TranslatedSource { js: resolved, warnings })
}
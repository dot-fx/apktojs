pub mod dalvik;
pub mod resolver;
mod inline;
mod remap;
pub mod render;

use crate::apk_inspector::ApkMeta;
use crate::dex_walker::WalkedSource;
use crate::translator::dalvik::interpreter::{lift, JsStmt};
use crate::translator::inline::inline_bridge_ctors;
use crate::translator::resolver::infer::{rename_source_classes, InferCtx};
use crate::translator::resolver::pool::Pool;

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

pub fn translate(
    walked: &WalkedSource,
    meta:   &ApkMeta,
    pool:   &Pool,
) -> Result<TranslatedSource, TranslateError> {
    let mut warnings   = Vec::new();
    let mut js_methods = Vec::new();

    let mut lifted: Vec<(Vec<JsStmt>, String, String, bool, u16)> = Vec::new();

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
            method.ins_size,
        ));
    }

    inline_bridge_ctors(&mut lifted);

    let mut pool_mut = pool.clone();
    let mut infer_ctx = InferCtx::default();

    for (stmts, _, _, _, _) in &lifted {
        infer_ctx.scan_stmts(stmts, &pool_mut, walked.dex_shard);
    }

    infer_ctx.apply(&mut pool_mut);

    let renames = rename_source_classes(&mut pool_mut, &meta.name);
    for (_, _, defined_in, _, _) in &mut lifted {
        if let Some(new_name) = renames.get(defined_in.as_str()) {
            *defined_in = new_name.clone();
        }
    }

    let mut names = resolver::resolve::TypeNames::build(&pool_mut);
    for (full_name, new_name) in &renames {
        names.full_to_js.insert(full_name.clone(), new_name.clone());
    }

    for (stmts, method_name, defined_in, is_static, ins_size) in &lifted {
        let has_super = pool_mut.type_info.get(defined_in)
            .and_then(|t| t.superclass.as_deref())
            .map(|s|
                s != "Object"
                    && s != "java.lang.Object"
                    && !s.ends_with(".Object")
            )
            .unwrap_or(false);

        let body = render::stmts_to_js(
            stmts, 4, method_name, has_super, &names,
            &pool_mut
        );

        js_methods.push(render::JsMethod {
            name: method_name.clone(),
            body,
            defined_in: defined_in.clone(),
            is_static: *is_static,
            param_count: (*ins_size as usize).saturating_sub(if *is_static { 0 } else { 1 }),
        });
    }

    for js_method in &mut js_methods {
        if let Some(new_name) = renames.get(&js_method.defined_in) {
            js_method.defined_in = new_name.clone();
        }
    }

    let raw_js = render::render_class(
        &meta.name,
        meta,
        &js_methods,
        walked,
        &pool_mut,
        &names,
    );

    let resolved = resolver::resolve::resolve(
        &raw_js,
        &pool_mut,
        &names,
    );

    Ok(TranslatedSource { js: resolved, warnings })
}

pub fn from_dex_type(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}
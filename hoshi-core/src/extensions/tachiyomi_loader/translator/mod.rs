pub mod dalvik;
pub mod emit;
pub mod resolver;

use crate::extensions::tachiyomi_loader::{ApkMeta, EntryKind, WalkedSource};
use crate::error::CoreError;
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::lift;

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
) -> Result<TranslatedSource, TranslateError> {
    let mut warnings    = Vec::new();
    let mut js_methods  = Vec::new();

    for method in &walked.methods {
        let decoded = dalvik::decode(&method.insns);

        let insn_only: Vec<_> = decoded.iter()
            .map(|d| d.insn.clone())
            .collect();

        let (stmts, mut w) = lift(
            &insn_only,
            &decoded,
            &method.name,
            method.registers_size,
            method.ins_size,
            method.is_static,
            walked.dex_shard,
        );

        warnings.append(&mut w);

        let body = emit::render::stmts_to_js(&stmts, 4, &method.name);
        js_methods.push(emit::render::JsMethod {
            name: method.name.clone(),
            body,
        });
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
    );

    Ok(TranslatedSource { js: raw_js, warnings })
}
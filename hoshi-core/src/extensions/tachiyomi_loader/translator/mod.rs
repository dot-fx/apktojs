pub mod dalvik;
pub mod emit;
pub mod interpreter;
pub mod resolver;

use crate::extensions::tachiyomi_loader::{ApkMeta, WalkedSource, EntryKind};
use crate::error::CoreError;

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
        let insns = dalvik::decode(&method.insns);
        
        let (stmts, mut w) = interpreter::lift(
            &insns,
            &method.name,
            method.registers_size,
            method.ins_size,
            method.is_static,
        );

        if method.name == "popularMangaRequest" {
            for s in &stmts {
                eprintln!("STMT: {:?}", s);
            }
        }
        warnings.append(&mut w);

        let body = emit::stmts_to_js(&stmts, 4, &method.name);
        js_methods.push(emit::JsMethod {
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

    let raw_js = emit::render_class(
        &meta.name,
        base_class,
        meta,
        &js_methods,
        walked,
    );

    Ok(TranslatedSource { js: raw_js, warnings })
}
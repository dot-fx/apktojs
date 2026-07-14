pub mod apk_inspector;
pub mod dex_extractor;
pub mod dex_walker;
pub mod translator;

use std::io::{Cursor};

use apk_inspector::{ApkMeta, ApkError};
use dex_extractor::extract_dex;
use dex_walker::walk_source;
use translator::resolver::pool::Pool;

pub const SHIMS: &str = concat!(
    include_str!("../shims/00_prototypes.js"),
    include_str!("../shims/01_std.js"),
    include_str!("../shims/02_std_classes.js"),
    include_str!("../shims/03_locale_date.js"),
    include_str!("../shims/04_json.js"),
    include_str!("../shims/05_android.js"),
    include_str!("../shims/06_jsoup.js"),
    include_str!("../shims/07_network.js"),
);

use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub js: String,
    pub meta: ApkMeta,
}

pub fn apk_to_js(bytes: &[u8]) -> Result<TranslationResult, ApkError> {
    let meta = apk_inspector::inspect_apk_reader(Cursor::new(bytes))?;

    let mut zip = ZipArchive::new(Cursor::new(bytes))?;

    let extracted = extract_dex(&mut zip, &meta)?;

    let mut pool = Pool::build(&extracted.dex_files);

    let walked = walk_source(&extracted, &meta, &mut pool)
        .map_err(|e| ApkError::Axml(e.to_string()))?;

    let translated = translator::translate(&walked, &meta, &pool)
        .map_err(|e| ApkError::Axml(e.to_string()))?;

    Ok(TranslationResult {
        js: translated.js,
        meta,
    })
}
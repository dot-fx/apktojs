use dex::Dex;

use crate::error::CoreError;
use crate::extensions::tachiyomi_loader::{ApkMeta, ExtractedDex};
use crate::extensions::tachiyomi_loader::translator::dalvik::insn::Insn;

/// DEX type descriptor for HttpSource.
const HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/source/online/HttpSource;";
/// DEX type descriptor for ParsedHttpSource.
const PARSED_HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/source/online/ParsedHttpSource;";
/// DEX type descriptor for SourceFactory.
const SOURCE_FACTORY: &str = "Leu/kanade/tachiyomi/source/SourceFactory;";
const CREATE_SOURCES: &str = "createSources";


const SOURCE_METHODS: &[&str] = &[
    "popularMangaRequest",
    "popularMangaParse",
    "latestUpdatesRequest",
    "latestUpdatesParse",
    "searchMangaRequest",
    "searchMangaParse",
    "mangaDetailsParse",
    "chapterListRequest",
    "chapterListParse",
    "pageListParse",
    "imageUrlParse",
    "getFilterList",
];


#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    /// Directly extends HttpSource / ParsedHttpSource.
    Direct,
    /// Implements SourceFactory
    Factory,
}

#[derive(Debug, Clone)]
pub struct SourceMethod {
    pub name: String,
    pub defined_in: String,
    pub insns: Vec<u16>,
    pub registers_size: u16,
    pub ins_size: u16,
    pub is_static: bool,
}

#[derive(Debug)]
pub struct WalkedSource {
    pub class_name: String,
    pub kind: EntryKind,
    pub methods: Vec<SourceMethod>,
    pub hierarchy: Vec<String>,
    pub dex_shard: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum WalkError {
    #[error("ext_class '{0}' not found in any DEX shard")]
    ClassNotFound(String),

    #[error("ext_class '{0}' is not an HttpSource or SourceFactory")]
    NotASource(String),

    #[error("DEX error: {0}")]
    Dex(String),
}

impl From<WalkError> for CoreError {
    fn from(e: WalkError) -> Self {
        CoreError::Parse(e.to_string())
    }
}

pub fn walk_source(extracted: &ExtractedDex, meta: &ApkMeta) -> Result<WalkedSource, WalkError> {
    let fq_class = resolve_ext_class(&meta.package, &meta.ext_class);
    let descriptor = to_dex_descriptor(&fq_class);

    let (entry_class, dex_shard, shard) =
        find_class_in_shards(&extracted.dex_files, &descriptor)
            .ok_or_else(|| WalkError::ClassNotFound(fq_class.clone()))?;

    let kind = detect_kind(&entry_class)?;

    let mut hierarchy  = Vec::new();
    let mut methods    = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    match kind {
        EntryKind::Direct => {
            walk_hierarchy(
                &entry_class, shard, &extracted.dex_files,
                &mut hierarchy, &mut methods, &mut seen_names, 0,
            );
        }
        EntryKind::Factory => {
            let source_descriptors = find_factory_sources(
                &entry_class, shard, &extracted.dex_files,
            );

            hierarchy.push(fq_class.clone());

            if source_descriptors.is_empty() {
                return Err(WalkError::NotASource(fq_class.clone()));
            }

            for desc in &source_descriptors {
                if let Some((src_class, _, src_shard)) =
                    find_class_in_shards(&extracted.dex_files, desc)
                {
                    walk_hierarchy(
                        &src_class, src_shard, &extracted.dex_files,
                        &mut hierarchy, &mut methods, &mut seen_names, 0,
                    );
                }
            }
        }
    }

    if hierarchy.is_empty() {
        return Err(WalkError::NotASource(fq_class.clone()));
    }

    Ok(WalkedSource {
        class_name: fq_class,
        kind,
        methods,
        hierarchy,
        dex_shard,
    })
}

fn find_factory_sources(
    factory: &dex::class::Class,
    shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
) -> Vec<String> {
    use crate::extensions::tachiyomi_loader::translator::dalvik::{self};

    let create_method = factory
        .virtual_methods()
        .iter()
        .find(|m| m.name().to_string() == CREATE_SOURCES);

    let insns_raw = match create_method {
        Some(m) => extract_insns(&m).0,
        None => return vec![],
    };

    let insns = dalvik::decode(&insns_raw);
    let mut descriptors = Vec::new();

    for decoded in &insns {
        if let Insn::NewInstance(_, type_idx) = &decoded.insn {
            let desc = shard.get_type(*type_idx)
                .map(|t| t.to_string())
                .or_else(|_| {
                    for s in all_shards {
                        if let Ok(t) = s.get_type(*type_idx) {
                            return Ok(t.to_string());
                        }
                    }
                    Err(())
                })
                .unwrap_or_default();

            if desc.is_empty() {
                continue;
            }

            let fq = from_dex_descriptor(&desc);

            if fq.contains("kanade") || fq.contains("tachiyomi") {
                if !descriptors.contains(&desc) {
                    descriptors.push(desc);
                }
            }
        }
    }

    descriptors
}

fn resolve_ext_class(package: &str, ext_class: &str) -> String {
    if ext_class.starts_with('.') {
        format!("{}{}", package, ext_class)
    } else {
        ext_class.to_string()
    }
}

fn to_dex_descriptor(fq: &str) -> String {
    format!("L{};", fq.replace('.', "/"))
}

fn from_dex_descriptor(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}

fn find_class_in_shards<'a>(
    shards: &'a [Dex<Vec<u8>>],
    descriptor: &str,
) -> Option<(dex::class::Class, usize, &'a Dex<Vec<u8>>)> {
    for (idx, shard) in shards.iter().enumerate() {
        if let Ok(Some(class)) = shard.find_class_by_name(descriptor) {
            return Some((class, idx, shard));
        }
    }
    None
}

fn detect_kind(class: &dex::class::Class) -> Result<EntryKind, WalkError> {
    for iface in class.interfaces() {
        if iface.to_string() == SOURCE_FACTORY {
            return Ok(EntryKind::Factory);
        }
    }

    Ok(EntryKind::Direct)
}

fn walk_hierarchy(
    class: &dex::class::Class,
    shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
    hierarchy: &mut Vec<String>,
    methods: &mut Vec<SourceMethod>,
    seen: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }

    let class_name = from_dex_descriptor(&class.jtype().to_string());

    if class_name.starts_with("java.")
        || class_name.starts_with("kotlin.")
        || class_name.starts_with("android.")
        || class_name == "eu.kanade.tachiyomi.source.online.HttpSource"
        || class_name == "eu.kanade.tachiyomi.source.online.ParsedHttpSource"
    {
        return;
    }

    hierarchy.push(class_name.clone());

    for method in class.virtual_methods().iter().chain(class.direct_methods().iter()) {
        let name = method.name().to_string();

        if !SOURCE_METHODS.contains(&name.as_str()) {
            continue;
        }
        if seen.contains(&name) {
            continue;
        }

        let (insns, registers_size, ins_size) = extract_insns(&method);

        seen.insert(name.clone());
        methods.push(SourceMethod {
            name,
            defined_in: class_name.clone(),
            insns,
            registers_size,
            ins_size,
            is_static: (method.access_flags().bits() & 0x0008) != 0,
        });
    }

    if let Some(super_id) = class.super_class() {
        if let Ok(super_type) = shard.get_type(super_id) {
            let super_desc = super_type.to_string();
            let found = find_class_in_shards(
                std::slice::from_ref(shard),
                &super_desc,
            )
                .or_else(|| find_class_in_shards(all_shards, &super_desc));

            if let Some((super_class, _, super_shard)) = found {
                walk_hierarchy(
                    &super_class,
                    super_shard,
                    all_shards,
                    hierarchy,
                    methods,
                    seen,
                    depth + 1,
                );
            }
        }
    }
}

fn extract_insns(method: &dex::method::Method) -> (Vec<u16>, u16, u16) {
    method.code()
        .map(|code| (
            code.insns().clone(),
            code.registers_size(),
            code.ins_size(),
        ))
        .unwrap_or((vec![], 0, 0))
}
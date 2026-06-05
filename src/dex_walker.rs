use dex::Dex;
use crate::apk_inspector::ApkMeta;
use crate::dex_extractor::ExtractedDex;
use crate::translator::dalvik::decode;
use crate::translator::dalvik::insn::Insn;
use crate::translator::resolver::pool::Pool;

/// DEX type descriptor for HttpSource.
const HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/source/online/HttpSource;";
/// DEX type descriptor for ParsedHttpSource.
const PARSED_HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/source/online/ParsedHttpSource;";
/// DEX type descriptor for SourceFactory.
const SOURCE_FACTORY: &str = "Leu/kanade/tachiyomi/source/SourceFactory;";
const CREATE_SOURCES: &str = "createSources";

/// DEX type descriptor for AnimeHttpSource.
const ANIME_HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/animesource/online/AnimeHttpSource;";
/// DEX type descriptor for ParsedAnimeHttpSource.
const PARSED_ANIME_HTTP_SOURCE: &str = "Leu/kanade/tachiyomi/animesource/online/ParsedAnimeHttpSource;";
/// DEX type descriptor for AnimeSourceFactory.
const ANIME_SOURCE_FACTORY: &str = "Leu/kanade/tachiyomi/animesource/AnimeSourceFactory;";


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


pub fn walk_source(extracted: &ExtractedDex, meta: &ApkMeta, pool: &mut Pool) -> Result<WalkedSource, WalkError> {
    let fq_class = resolve_ext_class(&meta.package, &meta.ext_class);
    let descriptor = to_dex_descriptor(&fq_class);

    let (entry_class, dex_shard, shard) =
        find_class_in_shards(&extracted.dex_files, &descriptor)
            .ok_or_else(|| WalkError::ClassNotFound(fq_class.clone()))?;

    let kind = detect_kind(&entry_class, shard, &extracted.dex_files)?;

    let mut hierarchy  = Vec::new();
    let mut methods    = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    match kind {
        EntryKind::Direct => {
            walk_hierarchy(
                &entry_class, shard, &extracted.dex_files,
                &mut hierarchy, &mut methods, &mut seen_names, pool, 0,
            );
        }
        EntryKind::Factory => {
            let source_descriptors = find_factory_sources(
                &entry_class, shard, &extracted.dex_files,
            );

            if source_descriptors.is_empty() {
                return Err(WalkError::NotASource(fq_class.clone()));
            }

            walk_hierarchy(
                &entry_class, shard, &extracted.dex_files,
                &mut hierarchy, &mut methods, &mut seen_names, pool, 0,
            );

            for desc in &source_descriptors {
                if let Some((src_class, _, src_shard)) =
                    find_class_in_shards(&extracted.dex_files, desc)
                {
                    walk_hierarchy(
                        &src_class, src_shard, &extracted.dex_files,
                        &mut hierarchy, &mut methods, &mut seen_names, pool, 0,
                    );
                }
            }
        }
    }

    if hierarchy.is_empty() {
        return Err(WalkError::NotASource(fq_class.clone()));
    }

    let mut all_methods_to_scan = methods.clone();
    let mut helper_seen_classes: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        let mut referenced_descs: Vec<String> = Vec::new();

        for method in &all_methods_to_scan {
            let decoded = decode(&method.insns);
            for d in &decoded {
                match &d.insn {
                    Insn::SGet(_, field_idx)
                    | Insn::SGetObject(_, field_idx)
                    | Insn::SGetBoolean(_, field_idx)
                    | Insn::SPut(_, field_idx)
                    | Insn::SPutObject(_, field_idx)
                    | Insn::SPutBoolean(_, field_idx)
                    | Insn::SPutByte(_, field_idx)
                    | Insn::SPutChar(_, field_idx)
                    | Insn::SPutShort(_, field_idx) => {
                        for s in 0..16 {
                            if let Some(field_info) = pool.fields.get(&(s, *field_idx)) {
                                let desc = format!("L{};", field_info.class_name.replace('.', "/"));
                                referenced_descs.push(desc);
                                break;
                            }
                        }
                    }
                    Insn::InvokeStatic { method_idx, .. }
                    | Insn::InvokeStaticRange { method_idx, .. } => {
                        for s in 0..16 {
                            if let Some(m) = pool.methods.get(&(s, *method_idx)) {
                                let desc = format!("L{};", m.class_name.replace('.', "/"));
                                referenced_descs.push(desc);
                                break;
                            }
                        }
                    }
                    Insn::NewInstance(_, idx)
                    | Insn::ConstClass(_, idx) => {
                        for s in &extracted.dex_files {
                            if let Ok(t) = s.get_type(*idx) {
                                referenced_descs.push(t.to_string());
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut new_methods: Vec<SourceMethod> = Vec::new();
        let mut found_any = false;

        for desc in referenced_descs {
            let fq = from_dex_descriptor(&desc);
            if fq.starts_with("java.")
                || fq.starts_with("kotlin.")
                || fq.starts_with("android.")
                || fq.starts_with("androidx.")
                || hierarchy.contains(&fq)
                || helper_seen_classes.contains(&fq)
            {
                continue;
            }
            helper_seen_classes.insert(fq.clone());
            if let Some((helper_class, _, helper_shard)) =
                find_class_in_shards(&extracted.dex_files, &desc)
            {
                let mut helper_seen = std::collections::HashSet::new();
                walk_hierarchy(
                    &helper_class, helper_shard, &extracted.dex_files,
                    &mut hierarchy, &mut new_methods, &mut helper_seen, pool, 0,
                );
                found_any = true;
            }
        }

        if !found_any { break; }
        all_methods_to_scan = new_methods.clone();
        methods.extend(new_methods);
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
    use crate::translator::dalvik::{self};

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

            if desc.is_empty() { continue; }

            let fq = from_dex_descriptor(&desc);

            // Skip framework/stdlib classes
            if fq.starts_with("java.")
                || fq.starts_with("kotlin.")
                || fq.starts_with("android.")
                || fq.starts_with("androidx.")
            {
                continue;
            }

            // Accept anything else instantiated in createSources
            if !descriptors.contains(&desc) {
                descriptors.push(desc);
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
        for class in shard.classes() {
            if let Ok(class) = class {
                if class.jtype().to_string() == descriptor {
                    return Some((class, idx, shard));
                }
            }
        }
    }
    None
}

fn detect_kind(
    class: &dex::class::Class,
    shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
) -> Result<EntryKind, WalkError> {
    for iface in class.interfaces() {
        let iface_str = iface.to_string();
        if iface_str == SOURCE_FACTORY || iface_str == ANIME_SOURCE_FACTORY {
            return Ok(EntryKind::Factory);
        }
    }

    if extends_http_source(class, shard, all_shards, 0) {
        return Ok(EntryKind::Direct);
    }

    Err(WalkError::NotASource(
        from_dex_descriptor(&class.jtype().to_string())
    ))
}

fn extends_http_source(
    class: &dex::class::Class,
    shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
    depth: usize,
) -> bool {
    if depth > 8 { return false; }

    let super_id = match class.super_class() {
        Some(id) => id,
        None => return false,
    };

    let super_desc = match resolve_type_across_shards(super_id, shard, all_shards) {
        Some(s) => s,
        None => return false,
    };

    if super_desc == HTTP_SOURCE
        || super_desc == PARSED_HTTP_SOURCE
        || super_desc == ANIME_HTTP_SOURCE
        || super_desc == PARSED_ANIME_HTTP_SOURCE
    {
        return true;
    }

    let found = find_class_in_shards(std::slice::from_ref(shard), &super_desc)
        .or_else(|| find_class_in_shards(all_shards, &super_desc));

    match found {
        Some((super_class, _, super_shard)) =>
            extends_http_source(&super_class, super_shard, all_shards, depth + 1),
        None => false,
    }
}

fn walk_hierarchy(
    class: &dex::class::Class,
    shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
    hierarchy: &mut Vec<String>,
    methods: &mut Vec<SourceMethod>,
    seen: &mut std::collections::HashSet<String>,
    pool: &mut Pool,
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
        || class_name == "eu.kanade.tachiyomi.animesource.online.AnimeHttpSource"
        || class_name == "eu.kanade.tachiyomi.animesource.online.ParsedAnimeHttpSource"
    {
        return;
    }

    hierarchy.push(class_name.clone());


    let resolved_super = class
        .super_class()
        .and_then(|id| resolve_type_across_shards(id, shard, all_shards))
        .map(|t| from_dex_descriptor(&t))
        .filter(|s| !s.is_empty() && s != "java.lang.Object");

    let entry = pool.type_info
        .entry(class_name.clone())
        .or_insert_with(|| crate::translator::resolver::pool::TypeInfo {
            full_name: "".to_string(),
            simple_name: "".to_string(),
            superclass: None,
            interfaces: vec![],
            methods: vec![],
        });

    if entry.superclass.is_none() {
        entry.superclass = resolved_super;
    }

    for method in class.virtual_methods().iter().chain(class.direct_methods().iter()) {
        let name = method.name().to_string();
        let (insns, registers_size, ins_size) = extract_insns(&method);

        let key = format!("{}::{}#{}", class_name, name, ins_size);
        if seen.contains(&key) {
            continue;
        }

        seen.insert(key);
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
        if let Some(super_desc) = resolve_type_across_shards(super_id, shard, all_shards) {
            let found = find_class_in_shards(std::slice::from_ref(shard), &super_desc)
                .or_else(|| find_class_in_shards(all_shards, &super_desc));

            if let Some((super_class, _, super_shard)) = found {
                walk_hierarchy(
                    &super_class,
                    super_shard,
                    all_shards,
                    hierarchy,
                    methods,
                    seen,
                    pool,
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

fn resolve_type_across_shards(
    super_id: u32,
    primary_shard: &Dex<Vec<u8>>,
    all_shards: &[Dex<Vec<u8>>],
) -> Option<String> {
    if let Ok(t) = primary_shard.get_type(super_id) {
        let s = t.to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    for shard in all_shards {
        if std::ptr::eq(shard as *const _, primary_shard as *const _) {
            continue;
        }
        if let Ok(t) = shard.get_type(super_id) {
            let s = t.to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}
use std::io::{Read, Seek};
use std::path::Path;

use axmldecoder::{Node, XmlDocument};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct ApkMeta {
    pub package: String,
    pub name: String,
    pub version_name: String,
    pub version_code: u32,
    pub lang: String,
    pub ext_class: String,
    pub nsfw: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ApkError {
    #[error("not a valid ZIP/APK file: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("AndroidManifest.xml not found inside APK")]
    MissingManifest,

    #[error("failed to parse binary AndroidManifest.xml: {0}")]
    Axml(String),

    #[error("missing required attribute '{0}' in manifest")]
    MissingAttr(&'static str),

    #[error("not a Mihon/Tachiyomi extension (no tachiyomi.extension.class meta-data)")]
    NotAnExtension,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

fn get_attr<'a>(el: &'a axmldecoder::Element, key: &str) -> Option<&'a str> {
    el.get_attributes().iter()
        .find(|(k, _)| k.as_str() == key || k.ends_with(&format!(":{key}")))
        .map(|(_, v)| v.as_str())
}

pub fn inspect_apk(path: &Path) -> Result<ApkMeta, ApkError> {
    let file = std::fs::File::open(path)?;
    inspect_apk_reader(file)
}

pub fn inspect_apk_reader<R: Read + Seek>(reader: R) -> Result<ApkMeta, ApkError> {
    let mut archive = ZipArchive::new(reader)?;
    let manifest_bytes = read_manifest(&mut archive)?;
    parse_manifest(&manifest_bytes)
}

fn read_manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Vec<u8>, ApkError> {
    let mut entry = archive
        .by_name("AndroidManifest.xml")
        .map_err(|_| ApkError::MissingManifest)?;

    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

fn parse_manifest(bytes: &[u8]) -> Result<ApkMeta, ApkError> {
    let xml = axmldecoder::parse(bytes)
        .map_err(|e| ApkError::Axml(e.to_string()))?;

    let mut package      = None::<String>;
    let mut version_name = None::<String>;
    let mut version_code = None::<u32>;
    let mut ext_class    = None::<String>;
    let mut nsfw         = false;

    walk_xml(&xml, &mut |el: &axmldecoder::Element| {
        match el.get_tag() {
            "manifest" => {
                if package.is_none()      { package      = get_attr(el, "package").map(str::to_string); }
                if version_name.is_none() { version_name = get_attr(el, "versionName").map(str::to_string); }
                if version_code.is_none() { version_code = get_attr(el, "versionCode").and_then(|v| v.parse().ok()); }
            }
            "meta-data" => {
                match get_attr(el, "name") {
                    Some("tachiyomi.extension.class") => {
                        ext_class = get_attr(el, "value").map(str::to_string);
                    }
                    Some("tachiyomi.extension.nsfw") => {
                        nsfw = get_attr(el, "value")
                            .and_then(|v| v.parse::<u32>().ok())
                            .map(|n| n != 0)
                            .unwrap_or(false);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });

    let package      = package.ok_or(ApkError::MissingAttr("package"))?;
    let version_name = version_name.unwrap_or_else(|| "0.0.0".to_string());
    let version_code = version_code.unwrap_or(0);
    let ext_class    = ext_class.ok_or(ApkError::NotAnExtension)?;

    let (lang, name) = parse_package_suffix(&package);

    Ok(ApkMeta {
        package,
        name,
        version_name,
        version_code,
        lang,
        ext_class,
        nsfw,
    })
}

fn walk_node<F>(node: &Node, f: &mut F)
where
    F: FnMut(&axmldecoder::Element),
{
    if let Node::Element(el) = node {
        f(el);
        for child in el.get_children() {
            walk_node(child, f);
        }
    }
}

fn walk_xml<F>(xml: &XmlDocument, f: &mut F)
where
    F: FnMut(&axmldecoder::Element),
{
    for node in xml.get_root() {
        walk_node(node, f);
    }
}

fn parse_package_suffix(package: &str) -> (String, String) {
    let suffix = package
        .strip_prefix("eu.kanade.tachiyomi.extension.")
        .or_else(|| package.strip_prefix("eu.kanade.tachiyomi."))
        .unwrap_or(package);

    let mut parts = suffix.splitn(2, '.');
    let lang     = parts.next().unwrap_or("all").to_string();
    let raw_name = parts.next().unwrap_or(suffix);

    let name = raw_name
        .split(|c: char| c == '_' || c == '-')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None    => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join("");

    (lang, name)
}
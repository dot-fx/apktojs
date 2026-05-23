use std::io::{Read, Seek};

use dex::{Dex, DexReader};
use zip::ZipArchive;
use crate::apk_inspector::{ApkError, ApkMeta};

pub type ParsedDex = Dex<Vec<u8>>;

pub struct ExtractedDex {
    pub dex_files: Vec<ParsedDex>,
}

#[derive(Debug, thiserror::Error)]
pub enum DexError {
    #[error("no classes.dex found in APK")]
    NoDex,

    #[error("failed to parse DEX file '{name}': {source}")]
    Parse {
        name:   String,
        #[source]
        source: dex::Error,
    },

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<DexError> for ApkError {
    fn from(e: DexError) -> Self {
        match e {
            DexError::NoDex => ApkError::MissingManifest,
            other => ApkError::Axml(other.to_string()),
        }
    }
}

pub fn extract_dex<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    _meta: &ApkMeta,
) -> Result<ExtractedDex, DexError> {
    let mut dex_files = Vec::new();

    let entry_names = std::iter::once("classes.dex".to_string())
        .chain((2u8..=9).map(|n| format!("classes{}.dex", n)));

    for name in entry_names {
        match read_zip_entry(archive, &name) {
            Ok(bytes) => {
                let parsed = DexReader::from_vec(bytes)
                    .map_err(|e| DexError::Parse { name: name.clone(), source: e })?;
                dex_files.push(parsed);
            }
            Err(DexError::Zip(zip::result::ZipError::FileNotFound)) => break,
            Err(e) => return Err(e),
        }
    }

    if dex_files.is_empty() {
        return Err(DexError::NoDex);
    }

    Ok(ExtractedDex { dex_files })
}

fn read_zip_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, DexError> {
    let mut entry = archive.by_name(name)?;
    let mut buf   = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}
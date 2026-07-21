//! Fetch open WordNet data on demand and build a local index.
//!
//! Mirrors the research layer's public-data fetch pattern: download the open
//! WN-LMF distribution for a language, decompress it, parse it
//! ([`parse_lmf`](super::parse_lmf)), and persist the built index under the user
//! data directory. English (Open English WordNet, a plain `.xml.gz`) is
//! supported here; the OMW languages ship as `.tar.xz` and arrive in a later
//! release. Data is never bundled or redistributed — each user fetches it under
//! its own licence.

use super::{parse_lmf, WordNet};

/// A downloadable wordnet.
pub struct Source {
    pub lang: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub format: Format,
    pub license: &'static str,
}

#[derive(PartialEq)]
pub enum Format {
    /// A single gzipped WN-LMF document (Open English WordNet).
    XmlGz,
    /// A `.tar.xz` archive containing a WN-LMF document (OMW languages).
    TarXz,
}

/// The curated source list. English is fetchable today; the `.tar.xz` languages
/// are listed so `wordnet list` shows the roadmap and their licences.
pub const SOURCES: &[Source] = &[
    Source {
        lang: "en",
        name: "Open English WordNet 2025",
        url: "https://en-word.net/static/english-wordnet-2025-plus.xml.gz",
        format: Format::XmlGz,
        license: "CC BY 4.0",
    },
    Source {
        lang: "fr",
        name: "WOLF — WordNet Libre du Français",
        url: "https://github.com/omwn/omw-data/releases/download/v2.0/omw-fr-2.0.tar.xz",
        format: Format::TarXz,
        license: "CeCILL-C",
    },
    Source {
        lang: "de",
        name: "Open German WordNet (ODeNet)",
        url: "https://github.com/hdaSprachtechnologie/odenet/releases/download/v1.4/odenet-1.4.tar.xz",
        format: Format::TarXz,
        license: "CC BY-SA 4.0",
    },
    Source {
        lang: "es",
        name: "Spanish WordNet (OMW)",
        url: "https://github.com/omwn/omw-data/releases/download/v2.0/omw-es-2.0.tar.xz",
        format: Format::TarXz,
        license: "CC BY 3.0",
    },
];

/// The source record for a language code, if any.
pub fn source(lang: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.lang.eq_ignore_ascii_case(lang))
}

/// Download and build the wordnet for `lang`. Returns the parsed index (the
/// caller persists it). Async, using the shared reqwest stack.
pub async fn fetch(lang: &str) -> Result<WordNet, String> {
    let src = source(lang).ok_or_else(|| {
        let known = SOURCES.iter().map(|s| s.lang).collect::<Vec<_>>().join(", ");
        format!("no wordnet source for `{lang}` — known languages: {known}")
    })?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("inkhaven/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .get(src.url)
        .send()
        .await
        .map_err(|e| format!("download {}: {e}", src.url))?
        .error_for_status()
        .map_err(|e| format!("download {}: {e}", src.url))?;
    let bytes = resp.bytes().await.map_err(|e| format!("read body: {e}"))?;

    let xml = decompress(&bytes, &src.format)?;
    parse_lmf(&xml)
}

/// Build a wordnet from a local WN-LMF file — the escape hatch for languages
/// whose data isn't openly redistributable (Russian / RuWordNet): the user
/// obtains it under its own licence and imports the file here. Accepts a plain
/// `.xml`, a gzipped `.xml.gz`, or a `.tar.xz` archive.
pub fn import_file(path: &std::path::Path) -> Result<WordNet, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let name = path.to_string_lossy();
    let format = if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        Format::TarXz
    } else if name.ends_with(".gz") {
        Format::XmlGz
    } else {
        // Plain XML.
        return parse_lmf(&String::from_utf8_lossy(&bytes));
    };
    let xml = decompress(&bytes, &format)?;
    parse_lmf(&xml)
}

/// Decompress a downloaded/loaded blob to WN-LMF XML text.
fn decompress(bytes: &[u8], format: &Format) -> Result<String, String> {
    match format {
        Format::XmlGz => gunzip(bytes),
        Format::TarXz => untar_xz(bytes),
    }
}

fn gunzip(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut out = String::new();
    decoder.read_to_string(&mut out).map_err(|e| format!("gunzip: {e}"))?;
    Ok(out)
}

/// Decompress a `.tar.xz` and return the WN-LMF XML entry it contains. The OMW
/// archives hold a single `.xml` document; we take the first one.
fn untar_xz(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    // xz → raw tar bytes (pure-Rust decoder).
    let mut tar_bytes = Vec::new();
    lzma_rs::xz_decompress(&mut std::io::Cursor::new(bytes), &mut tar_bytes)
        .map_err(|e| format!("xz decompress: {e}"))?;
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    for entry in archive.entries().map_err(|e| format!("read tar: {e}"))? {
        let mut entry = entry.map_err(|e| format!("tar entry: {e}"))?;
        let is_xml = entry
            .path()
            .map(|p| p.extension().map(|e| e.eq_ignore_ascii_case("xml")).unwrap_or(false))
            .unwrap_or(false);
        if is_xml {
            let mut s = String::new();
            entry.read_to_string(&mut s).map_err(|e| format!("read xml from tar: {e}"))?;
            return Ok(s);
        }
    }
    Err("no .xml WN-LMF document found in the .tar.xz archive".to_string())
}

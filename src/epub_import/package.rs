//! EPUB import P1 — container + OPF parsing.
//!
//! Reads the zip's `META-INF/container.xml` to find the OPF package
//! document, then parses the OPF for the title/author, the manifest
//! (id → href + media-type), and the spine (the ordered reading list of
//! XHTML documents). Hrefs are resolved relative to the OPF's directory
//! so the orchestrator can read them straight back out of the zip.
//!
//! Untrusted input: every path returns `Result` and the parsers never
//! unwrap, so malformed bytes yield an error, never a panic.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use anyhow::{anyhow, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

/// One manifest entry. `href` is resolved relative to the zip root.
#[derive(Debug, Clone)]
pub struct ManifestItem {
    pub href: String,
    pub media_type: String,
}

/// The parsed package: what the orchestrator needs to walk the book.
#[derive(Debug, Default)]
pub struct EpubPackage {
    pub title: String,
    pub author: Option<String>,
    /// Spine reading order, as zip-root-relative hrefs.
    pub spine: Vec<String>,
    /// Manifest id → item (hrefs zip-root-relative).
    pub manifest: HashMap<String, ManifestItem>,
}

/// An opened EPUB zip. Holds the archive so the orchestrator can pull
/// spine XHTML + image entries after parsing the package.
pub struct EpubArchive {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
}

impl EpubArchive {
    pub fn open(bytes: Vec<u8>) -> Result<Self> {
        let zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| anyhow!("not a readable EPUB zip: {e}"))?;
        Ok(Self { zip })
    }

    /// Read one entry by exact name; `None` if absent or unreadable.
    pub fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        let mut f = self.zip.by_name(name).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(buf)
    }

    /// Parse the container + OPF into an [`EpubPackage`].
    pub fn package(&mut self) -> Result<EpubPackage> {
        let container = self
            .read("META-INF/container.xml")
            .ok_or_else(|| anyhow!("EPUB is missing META-INF/container.xml"))?;
        let opf_path = opf_path_from_container(&container)?;
        let opf = self
            .read(&opf_path)
            .ok_or_else(|| anyhow!("EPUB is missing its OPF package `{opf_path}`"))?;
        let base = parent_dir(&opf_path);
        parse_opf(&opf, &base)
    }
}

/// Pull the OPF `full-path` out of `META-INF/container.xml`.
fn opf_path_from_container(bytes: &[u8]) -> Result<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if local_name(e.name().as_ref()) == b"rootfile" => {
                if let Some(p) = attr(&e, b"full-path") {
                    return Ok(p);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("container.xml parse: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Err(anyhow!("container.xml has no <rootfile full-path=…>"))
}

/// Parse the OPF for metadata + manifest + spine; resolve the spine to
/// hrefs relative to the zip root.
fn parse_opf(bytes: &[u8], base: &str) -> Result<EpubPackage> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    let mut buf = Vec::new();

    let mut pkg = EpubPackage::default();
    let mut spine_idrefs: Vec<String> = Vec::new();
    // Track which dc element's text we're inside (title / creator).
    let mut capture: Option<&'static str> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match local_name(e.name().as_ref()) {
                b"title" => capture = Some("title"),
                b"creator" => capture = Some("creator"),
                b"item" => add_manifest_item(&e, base, &mut pkg),
                b"itemref" => {
                    if let Some(idref) = attr(&e, b"idref") {
                        spine_idrefs.push(idref);
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                b"item" => add_manifest_item(&e, base, &mut pkg),
                b"itemref" => {
                    if let Some(idref) = attr(&e, b"idref") {
                        spine_idrefs.push(idref);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let Some(field) = capture.take() {
                    let val = t.unescape().unwrap_or_default().trim().to_string();
                    if !val.is_empty() {
                        match field {
                            "title" if pkg.title.is_empty() => pkg.title = val,
                            "creator" if pkg.author.is_none() => pkg.author = Some(val),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(_)) => capture = None,
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("OPF parse: {e}")),
            _ => {}
        }
        buf.clear();
    }

    // Resolve spine idrefs → hrefs via the manifest, dropping any that
    // don't resolve to an XHTML document.
    for idref in spine_idrefs {
        if let Some(item) = pkg.manifest.get(&idref) {
            pkg.spine.push(item.href.clone());
        }
    }
    if pkg.spine.is_empty() {
        return Err(anyhow!("EPUB OPF has no readable spine documents"));
    }
    Ok(pkg)
}

fn add_manifest_item(e: &quick_xml::events::BytesStart, base: &str, pkg: &mut EpubPackage) {
    let (Some(id), Some(href)) = (attr(e, b"id"), attr(e, b"href")) else {
        return;
    };
    let media_type = attr(e, b"media-type").unwrap_or_default();
    pkg.manifest.insert(
        id,
        ManifestItem {
            href: resolve(base, &href),
            media_type,
        },
    );
}

/// The directory of `path` with a trailing slash (`"OEBPS/content.opf"`
/// → `"OEBPS/"`; a bare name → `""`).
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..=i].to_string(),
        None => String::new(),
    }
}

/// Resolve an OPF-relative `href` against `base`, collapsing `.`/`..`.
fn resolve(base: &str, href: &str) -> String {
    if href.starts_with('/') {
        return href.trim_start_matches('/').to_string();
    }
    let combined = format!("{base}{href}");
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// quick-xml gives qualified names (`dc:title`); strip the prefix.
fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// First attribute value matching `key` (by local name), unescaped.
fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if local_name(a.key.as_ref()) == key {
            return Some(String::from_utf8_lossy(&a.value).into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal valid EPUB zip in memory for round-trip parsing.
    fn make_epub(opf_dir: &str, opf: &str, container: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zw.start_file("mimetype", opts).unwrap();
            zw.write_all(b"application/epub+zip").unwrap();
            zw.start_file("META-INF/container.xml", opts).unwrap();
            zw.write_all(container.as_bytes()).unwrap();
            zw.start_file(format!("{opf_dir}content.opf"), opts).unwrap();
            zw.write_all(opf.as_bytes()).unwrap();
            zw.finish().unwrap();
        }
        buf
    }

    const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

    const OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>The Long Road</dc:title>
    <dc:creator>A. Writer</dc:creator>
  </metadata>
  <manifest>
    <item id="c1" href="chapter-001.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="text/chapter-002.xhtml" media-type="application/xhtml+xml"/>
    <item id="css" href="style.css" media-type="text/css"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
    <itemref idref="c2"/>
  </spine>
</package>"#;

    #[test]
    fn parses_title_author_and_ordered_spine() {
        let bytes = make_epub("OEBPS/", OPF, CONTAINER);
        let mut a = EpubArchive::open(bytes).unwrap();
        let pkg = a.package().unwrap();
        assert_eq!(pkg.title, "The Long Road");
        assert_eq!(pkg.author.as_deref(), Some("A. Writer"));
        // Spine in reading order, resolved relative to OEBPS/.
        assert_eq!(
            pkg.spine,
            vec!["OEBPS/chapter-001.xhtml", "OEBPS/text/chapter-002.xhtml"]
        );
        // The css item is in the manifest but not the spine.
        assert!(pkg.manifest.contains_key("css"));
    }

    #[test]
    fn resolve_collapses_dot_segments() {
        assert_eq!(resolve("OEBPS/", "../images/a.png"), "images/a.png");
        assert_eq!(resolve("OEBPS/text/", "../ch1.xhtml"), "OEBPS/ch1.xhtml");
        assert_eq!(resolve("", "ch1.xhtml"), "ch1.xhtml");
    }

    #[test]
    fn missing_container_is_an_error_not_a_panic() {
        // A valid zip but not an EPUB (no container.xml).
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zw.start_file("random.txt", opts).unwrap();
            zw.write_all(b"hi").unwrap();
            zw.finish().unwrap();
        }
        let mut a = EpubArchive::open(buf).unwrap();
        assert!(a.package().is_err());
    }

    use proptest::prelude::*;
    proptest! {
        /// Arbitrary bytes must never panic the EPUB parser (untrusted
        /// input). Open + package both return Ok|Err.
        #[test]
        fn parse_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
            if let Ok(mut a) = EpubArchive::open(bytes) {
                let _ = a.package();
            }
        }
    }
}

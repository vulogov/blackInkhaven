//! Parse a Scrivener `.scrivx` binder XML into a typed tree.
//!
//! The `.scrivx` lives at the root of a `.scriv` package and
//! describes the hierarchy via nested `<BinderItem>` elements.
//! Each item carries:
//!
//! * `UUID` attribute — the key into `Files/Docs/<UUID>.rtf`
//! * `Type` attribute — `DraftFolder` / `Folder` / `Text` /
//!   `Other` / etc.
//! * `<Title>` child element — display name.
//! * `<Children>` child wrapping more `<BinderItem>`s.
//!
//! We parse with `quick-xml`'s event reader so we never hold
//! the whole XML in memory at once. The output is a typed
//! `BinderItem` tree that's cheap to walk.

use anyhow::{anyhow, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

/// One node in the Scrivener binder. Kept loose (no enum for
/// `kind`) because Scrivener's type list is open-ended — new
/// versions add variants and we'd rather pass an unknown
/// through to the mapping layer than refuse the whole import.
#[derive(Debug, Clone)]
pub struct BinderItem {
    pub uuid: Uuid,
    pub kind: String,
    pub title: String,
    pub children: Vec<BinderItem>,
    /// 1.2.6+ — Scrivener keywords resolved against the
    /// project-level `<Keywords>` registry plus any inline
    /// `<Keywords>` text. Empty when this item has no
    /// keywords. The importer copies these to `Node.tags` on
    /// the corresponding inkhaven paragraph.
    pub keywords: Vec<String>,
}

#[cfg(test)]
impl BinderItem {
    /// Convenience: walk this subtree and call `visit(depth,
    /// &item)` on every item, depth-first, pre-order. Only used
    /// by tests in this file; production code walks via the
    /// orchestrator's own depth tracker in `WalkCtx`.
    fn walk(&self, depth: usize, visit: &mut dyn FnMut(usize, &BinderItem)) {
        visit(depth, self);
        for child in &self.children {
            child.walk(depth + 1, visit);
        }
    }
}

/// Parse `<scriv_root>/<Name>.scrivx`. `scriv_root` is the
/// directory ending in `.scriv`; the .scrivx filename matches
/// the directory's basename minus the extension.
pub fn parse_project(scriv_root: &Path) -> Result<Vec<BinderItem>> {
    let stem = scriv_root
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            anyhow!("scriv path `{}` has no stem", scriv_root.display())
        })?;
    let scrivx = scriv_root.join(format!("{stem}.scrivx"));
    if !scrivx.is_file() {
        return Err(anyhow!(
            ".scrivx not found at {} — is this a valid Scrivener project?",
            scrivx.display()
        ));
    }
    let bytes = std::fs::read(&scrivx)
        .with_context(|| format!("read {}", scrivx.display()))?;
    parse_scrivx(&bytes)
}

/// Pure parser over the .scrivx bytes. Exposed so tests can
/// feed fixtures directly.
pub fn parse_scrivx(bytes: &[u8]) -> Result<Vec<BinderItem>> {
    // Phase 1 — build a registry mapping keyword IDs to their
    // human-readable titles. Modern Scrivener stores keywords
    // at project level inside a `<Keywords>` block:
    //
    //   <Keywords>
    //     <Keyword ID="2"><Title>worldbuilding</Title>...</Keyword>
    //   </Keywords>
    //
    // and BinderItems reference them through
    // `<MetaData><KeywordsRefs><KeywordRef ID="2"/></KeywordsRefs>`.
    // Older / lighter exports use inline `<Keywords>foo;bar</Keywords>`
    // inside each item's `<MetaData>` — that path is handled
    // in phase 2.
    let registry = parse_keyword_registry(bytes)?;

    // Phase 2 — walk the binder tree.
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    // We track an explicit stack of "open BinderItem" entries.
    // Each entry holds its in-progress fields + a Vec of
    // already-parsed children. When we hit a `</BinderItem>`,
    // we pop the top of the stack and append it as a child of
    // the new top (or to the result if the stack becomes empty).
    let mut stack: Vec<PartialItem> = Vec::new();
    let mut result: Vec<BinderItem> = Vec::new();
    let mut current_text: Option<TextBuf> = None;
    // Per-item state for the inline-keywords path. Set when we
    // enter a BinderItem's MetaData/Keywords element, cleared
    // on close.
    let mut in_metadata: usize = 0;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .with_context(|| ".scrivx parse error".to_string())?;
        match event {
            Event::Start(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "BinderItem" => {
                        let mut uuid: Option<Uuid> = None;
                        let mut kind: String = "Unknown".into();
                        for attr in e.attributes().with_checks(false) {
                            let attr = attr
                                .map_err(|err| anyhow!("attr parse: {err}"))?;
                            let key = std::str::from_utf8(
                                attr.key.as_ref(),
                            )
                            .unwrap_or("");
                            let val_bytes = attr.value.as_ref();
                            let val = std::str::from_utf8(val_bytes)
                                .unwrap_or("")
                                .to_string();
                            match key {
                                "UUID" | "ID" => {
                                    if let Ok(u) = Uuid::parse_str(&val) {
                                        uuid = Some(u);
                                    } else {
                                        // Older Scrivener
                                        // projects use non-UUID
                                        // numeric IDs. Synthesise
                                        // a deterministic UUID
                                        // from the string so the
                                        // RTF lookup still works.
                                        uuid = Some(deterministic_uuid(&val));
                                    }
                                }
                                "Type" => kind = val,
                                _ => {}
                            }
                        }
                        stack.push(PartialItem {
                            uuid: uuid.unwrap_or_else(Uuid::nil),
                            kind,
                            title: String::new(),
                            children: Vec::new(),
                            keywords: Vec::new(),
                        });
                    }
                    "Title" => {
                        current_text = Some(TextBuf::Title);
                    }
                    "MetaData" => {
                        in_metadata += 1;
                    }
                    // Resolve `<KeywordRef ID="N"/>` (and the
                    // empty-element form handled by Event::Empty
                    // below). Only meaningful when we're inside
                    // a BinderItem AND inside its MetaData.
                    "KeywordRef" if in_metadata > 0 && !stack.is_empty() => {
                        if let Some(id) = extract_attr(&e, "ID") {
                            if let Some(title) = registry.get(&id) {
                                if let Some(top) = stack.last_mut() {
                                    push_unique_keyword(&mut top.keywords, title);
                                }
                            }
                        }
                    }
                    // Inline `<Keywords>...</Keywords>` form —
                    // only treat it as data when we're inside a
                    // BinderItem's MetaData. The top-level
                    // registry `<Keywords>` block (which holds
                    // `<Keyword>` children, NOT text) is
                    // handled in phase 1 and skipped here.
                    "Keywords" if in_metadata > 0 && !stack.is_empty() => {
                        current_text = Some(TextBuf::InlineKeywords);
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if name == "KeywordRef" && in_metadata > 0 && !stack.is_empty() {
                    if let Some(id) = extract_attr(&e, "ID") {
                        if let Some(title) = registry.get(&id) {
                            if let Some(top) = stack.last_mut() {
                                push_unique_keyword(&mut top.keywords, title);
                            }
                        }
                    }
                }
            }
            Event::Text(e) => {
                if let Some(top) = stack.last_mut() {
                    match current_text {
                        Some(TextBuf::Title) => {
                            let txt = e
                                .unescape()
                                .map_err(|err| anyhow!("title decode: {err}"))?;
                            top.title.push_str(&txt);
                        }
                        Some(TextBuf::InlineKeywords) => {
                            let txt = e
                                .unescape()
                                .map_err(|err| anyhow!("keywords decode: {err}"))?;
                            for piece in txt.split(|c| c == ',' || c == ';' || c == '\n')
                            {
                                let trimmed = piece.trim();
                                if !trimmed.is_empty() {
                                    push_unique_keyword(&mut top.keywords, trimmed);
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
            Event::End(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "Title" => {
                        if matches!(current_text, Some(TextBuf::Title)) {
                            current_text = None;
                        }
                    }
                    "Keywords" => {
                        if matches!(current_text, Some(TextBuf::InlineKeywords)) {
                            current_text = None;
                        }
                    }
                    "MetaData" => {
                        in_metadata = in_metadata.saturating_sub(1);
                    }
                    "BinderItem" => {
                        if let Some(p) = stack.pop() {
                            let item = BinderItem {
                                uuid: p.uuid,
                                kind: p.kind,
                                title: p.title,
                                children: p.children,
                                keywords: p.keywords,
                            };
                            if let Some(parent) = stack.last_mut() {
                                parent.children.push(item);
                            } else {
                                result.push(item);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

/// Walk the bytes once to extract the project-level keyword
/// registry: `<Keyword ID="N"><Title>name</Title></Keyword>`
/// entries (anywhere in the document — typically under a
/// top-level `<Keywords>` block in Scrivener 3.x).
fn parse_keyword_registry(bytes: &[u8]) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut out: HashMap<String, String> = HashMap::new();
    let mut current_id: Option<String> = None;
    let mut in_keyword: usize = 0;
    let mut in_keyword_title: bool = false;
    let mut current_title = String::new();
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .with_context(|| ".scrivx keyword-registry parse error".to_string())?;
        match event {
            Event::Start(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "Keyword" => {
                        in_keyword += 1;
                        current_id = extract_attr(&e, "ID");
                        current_title.clear();
                    }
                    "Title" if in_keyword > 0 => {
                        in_keyword_title = true;
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                if in_keyword_title {
                    let txt = e.unescape().map_err(|err| {
                        anyhow!("keyword-title decode: {err}")
                    })?;
                    current_title.push_str(&txt);
                }
            }
            Event::End(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "Title" if in_keyword_title => {
                        in_keyword_title = false;
                    }
                    "Keyword" => {
                        if let Some(id) = current_id.take() {
                            let title = current_title.trim();
                            if !title.is_empty() {
                                out.insert(id, title.to_owned());
                            }
                        }
                        in_keyword = in_keyword.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn extract_attr(e: &quick_xml::events::BytesStart, want: &str) -> Option<String> {
    for attr in e.attributes().with_checks(false).flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
        if key == want {
            let val = std::str::from_utf8(attr.value.as_ref())
                .unwrap_or("")
                .to_string();
            return Some(val);
        }
    }
    None
}

fn push_unique_keyword(into: &mut Vec<String>, candidate: &str) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if into.iter().any(|k| k == trimmed) {
        return;
    }
    into.push(trimmed.to_owned());
}

#[derive(Debug)]
struct PartialItem {
    uuid: Uuid,
    kind: String,
    title: String,
    children: Vec<BinderItem>,
    keywords: Vec<String>,
}

#[derive(Debug)]
enum TextBuf {
    Title,
    /// 1.2.6+ — inside a BinderItem's `<MetaData><Keywords>`
    /// inline text node (semicolon / comma / newline-separated
    /// keyword list).
    InlineKeywords,
}

/// Synthesise a deterministic UUID from a string. Used for
/// older Scrivener projects that use small integer IDs instead
/// of UUIDs. The output is stable across runs so re-import
/// against the same source produces the same node tree.
fn deterministic_uuid(s: &str) -> Uuid {
    // UUID v5 with the Scrivener namespace nil — same input
    // always produces the same output. Doesn't matter that
    // it's not a "real" v5 UUID — we only use it as a string
    // key into `Files/Docs/<id>.rtf`.
    let mut bytes = [0u8; 16];
    let src = s.as_bytes();
    for (i, b) in src.iter().enumerate() {
        bytes[i % 16] ^= *b;
    }
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_binder() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject>
  <Binder>
    <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="DraftFolder">
      <Title>Manuscript</Title>
      <Children>
        <BinderItem UUID="00000000-0000-0000-0000-000000000002" Type="Folder">
          <Title>Chapter One</Title>
          <Children>
            <BinderItem UUID="00000000-0000-0000-0000-000000000003" Type="Text">
              <Title>The Storm</Title>
            </BinderItem>
          </Children>
        </BinderItem>
      </Children>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "DraftFolder");
        assert_eq!(items[0].title, "Manuscript");
        assert_eq!(items[0].children.len(), 1);
        let ch1 = &items[0].children[0];
        assert_eq!(ch1.title, "Chapter One");
        assert_eq!(ch1.kind, "Folder");
        assert_eq!(ch1.children.len(), 1);
        let storm = &ch1.children[0];
        assert_eq!(storm.title, "The Storm");
        assert_eq!(storm.kind, "Text");
    }

    /// 1.2.6+ — Scrivener 3.x style: project-level
    /// `<Keywords>` registry plus per-item
    /// `<KeywordRef ID="N"/>` references. The parser should
    /// resolve refs to titles and attach them to the right
    /// BinderItem.
    #[test]
    fn keywords_via_registry_refs() {
        let xml = br#"<ScrivenerProject>
  <Keywords>
    <Keyword ID="1"><Title>worldbuilding</Title></Keyword>
    <Keyword ID="2"><Title>weather</Title></Keyword>
    <Keyword ID="3"><Title>character</Title></Keyword>
  </Keywords>
  <Binder>
    <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="DraftFolder">
      <Title>Manuscript</Title>
      <Children>
        <BinderItem UUID="00000000-0000-0000-0000-000000000002" Type="Text">
          <Title>The Storm</Title>
          <MetaData>
            <KeywordsRefs>
              <KeywordRef ID="2"/>
              <KeywordRef ID="1"/>
            </KeywordsRefs>
          </MetaData>
        </BinderItem>
      </Children>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        let storm = &items[0].children[0];
        assert_eq!(storm.title, "The Storm");
        // Order follows the order the refs appear in the source.
        assert_eq!(storm.keywords, vec!["weather".to_owned(), "worldbuilding".to_owned()]);
    }

    /// 1.2.6+ — older / lighter exports use inline
    /// `<MetaData><Keywords>foo, bar; baz</Keywords></MetaData>`.
    /// Splitter handles commas, semicolons, and newlines;
    /// trims whitespace; de-dupes.
    #[test]
    fn keywords_inline_split() {
        let xml = br#"<ScrivenerProject><Binder>
          <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="Text">
            <Title>Scene</Title>
            <MetaData>
              <Keywords>storm, weather; storm
character</Keywords>
            </MetaData>
          </BinderItem>
        </Binder></ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        assert_eq!(items.len(), 1);
        // `storm` appears twice in the source; de-duped to once.
        assert_eq!(
            items[0].keywords,
            vec![
                "storm".to_owned(),
                "weather".to_owned(),
                "character".to_owned(),
            ],
        );
    }

    /// Inline `<Keywords>` inside MetaData must not interfere
    /// with the top-level registry block (which holds
    /// `<Keyword>` children, not bare text).
    #[test]
    fn keywords_registry_does_not_double_count() {
        let xml = br#"<ScrivenerProject>
  <Keywords>
    <Keyword ID="1"><Title>foo</Title></Keyword>
  </Keywords>
  <Binder>
    <BinderItem UUID="00000000-0000-0000-0000-000000000002" Type="Text">
      <Title>Bare</Title>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        assert!(items[0].keywords.is_empty());
    }

    #[test]
    fn walks_in_preorder() {
        let xml = br#"<ScrivenerProject><Binder>
            <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="DraftFolder">
              <Title>Root</Title>
              <Children>
                <BinderItem UUID="00000000-0000-0000-0000-000000000002" Type="Text">
                  <Title>A</Title>
                </BinderItem>
                <BinderItem UUID="00000000-0000-0000-0000-000000000003" Type="Text">
                  <Title>B</Title>
                </BinderItem>
              </Children>
            </BinderItem>
        </Binder></ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        let mut titles: Vec<String> = Vec::new();
        for item in &items {
            item.walk(0, &mut |_d, i| titles.push(i.title.clone()));
        }
        assert_eq!(titles, vec!["Root", "A", "B"]);
    }
}

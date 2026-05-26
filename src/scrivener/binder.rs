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
    /// 1.2.8+ — Scrivener custom-metadata values resolved
    /// against the project-level `<CustomMetaDataSettings>`
    /// registry. Pairs of `(field_title, value)`. Empty when
    /// the item carries no `<CustomMetaData>`. The importer
    /// scans this list against `scrivener.date_fields` and
    /// attaches `EventData` to the paragraph for any matching
    /// pair whose value parses against the project's calendar.
    pub custom_meta: Vec<(String, String)>,
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

    // Phase 1.5 — build the custom-metadata-field registry so
    // per-item `<MetaDataItem ID=X>` references can resolve to a
    // field title (e.g. "Story Date"). Empty when the project
    // declared no CustomMeta fields.
    let custom_meta_registry = parse_custom_meta_registry(bytes)?;

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
    // 1.2.8+ — per-item CustomMeta state.
    //   in_custom_meta = depth of <CustomMetaData> nesting.
    //   current_custom_meta_id = ID attr of the current
    //     <MetaDataItem>, lookup-key into custom_meta_registry.
    //   current_custom_meta_value = text accumulator for the
    //     current <Value> child.
    let mut in_custom_meta: usize = 0;
    let mut current_custom_meta_id: Option<String> = None;
    let mut current_custom_meta_value = String::new();
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
                            custom_meta: Vec::new(),
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
                    // 1.2.8+ — open a <CustomMetaData> block.
                    // Only inside an open BinderItem's <MetaData>
                    // does this counter need to climb — the
                    // top-level <CustomMetaDataSettings> registry
                    // is handled in phase 1.5 and ignored here.
                    "CustomMetaData" if in_metadata > 0 && !stack.is_empty() => {
                        in_custom_meta += 1;
                    }
                    // 1.2.8+ — open a <MetaDataItem ID="X">
                    // inside the open <CustomMetaData>. Stash
                    // the ID; the inner <Value> text will be
                    // resolved against it on close.
                    "MetaDataItem" if in_custom_meta > 0 && !stack.is_empty() => {
                        current_custom_meta_id = extract_attr(&e, "ID");
                        current_custom_meta_value.clear();
                    }
                    // 1.2.8+ — open the <Value> child of the
                    // current <MetaDataItem>. Accumulates text
                    // into current_custom_meta_value until close.
                    "Value" if in_custom_meta > 0
                        && current_custom_meta_id.is_some()
                        && !stack.is_empty() =>
                    {
                        current_text = Some(TextBuf::CustomMetaValue);
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
                        Some(TextBuf::CustomMetaValue) => {
                            let txt = e.unescape().map_err(|err| {
                                anyhow!("custom-meta value decode: {err}")
                            })?;
                            current_custom_meta_value.push_str(&txt);
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
                    // 1.2.8+ — close of the current <Value>.
                    // Resolve current_custom_meta_id → field
                    // title via the project-level registry and
                    // push the (title, value) pair onto the
                    // top-of-stack item's custom_meta list. The
                    // value's surrounding whitespace is trimmed
                    // so calendar parsing on import doesn't have
                    // to deal with leading newlines.
                    "Value" if matches!(current_text, Some(TextBuf::CustomMetaValue)) => {
                        current_text = None;
                        if let Some(id) = current_custom_meta_id.as_ref() {
                            if let Some(title) = custom_meta_registry.get(id) {
                                let value = current_custom_meta_value.trim().to_string();
                                if !value.is_empty() {
                                    if let Some(top) = stack.last_mut() {
                                        top.custom_meta
                                            .push((title.clone(), value));
                                    }
                                }
                            }
                        }
                        current_custom_meta_value.clear();
                    }
                    "MetaDataItem" if in_custom_meta > 0 => {
                        current_custom_meta_id = None;
                        current_custom_meta_value.clear();
                    }
                    "CustomMetaData" => {
                        in_custom_meta = in_custom_meta.saturating_sub(1);
                    }
                    "BinderItem" => {
                        if let Some(p) = stack.pop() {
                            let item = BinderItem {
                                uuid: p.uuid,
                                kind: p.kind,
                                title: p.title,
                                children: p.children,
                                keywords: p.keywords,
                                custom_meta: p.custom_meta,
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

/// 1.2.8+ — walk the bytes once to extract the project-level
/// custom-metadata registry: `<MetaDataField ID="N"><Title>X</Title></MetaDataField>`
/// entries.  Modern Scrivener stores these under
/// `<CustomMetaDataSettings>` but the parser is tolerant — any
/// `<MetaDataField>` element with an `ID` attribute and a
/// `<Title>` child counts.  Returns ID → field-title map.
fn parse_custom_meta_registry(bytes: &[u8]) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut out: HashMap<String, String> = HashMap::new();
    let mut current_id: Option<String> = None;
    let mut in_field: usize = 0;
    let mut in_field_title: bool = false;
    let mut current_title = String::new();
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .with_context(|| ".scrivx custom-meta-registry parse error".to_string())?;
        match event {
            Event::Start(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "MetaDataField" => {
                        in_field += 1;
                        current_id = extract_attr(&e, "ID");
                        current_title.clear();
                    }
                    "Title" if in_field > 0 => {
                        in_field_title = true;
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                if in_field_title {
                    let txt = e.unescape().map_err(|err| {
                        anyhow!("metadata-field-title decode: {err}")
                    })?;
                    current_title.push_str(&txt);
                }
            }
            Event::End(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "Title" if in_field_title => {
                        in_field_title = false;
                    }
                    "MetaDataField" => {
                        if let Some(id) = current_id.take() {
                            let title = current_title.trim();
                            if !title.is_empty() {
                                out.insert(id, title.to_owned());
                            }
                        }
                        in_field = in_field.saturating_sub(1);
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
    /// 1.2.8+ — accumulated CustomMeta pairs for this item.
    custom_meta: Vec<(String, String)>,
}

#[derive(Debug)]
enum TextBuf {
    Title,
    /// 1.2.6+ — inside a BinderItem's `<MetaData><Keywords>`
    /// inline text node (semicolon / comma / newline-separated
    /// keyword list).
    InlineKeywords,
    /// 1.2.8+ — inside a `<MetaDataItem>` `<Value>` element,
    /// nested inside `<MetaData><CustomMetaData>`. The
    /// surrounding `MetaDataItem`'s `ID` attribute resolves
    /// to a field title via the project-level
    /// `<CustomMetaDataSettings>` registry built in Phase 1.5.
    CustomMetaValue,
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

    /// 1.2.8+ — CustomMeta extraction: project-level
    /// `<CustomMetaDataSettings>` defines fields; per-item
    /// `<MetaData><CustomMetaData><MetaDataItem ID=X>` carries
    /// values. The parser resolves IDs against the registry
    /// and attaches `(title, value)` pairs on the right item.
    #[test]
    fn custom_meta_resolves_field_titles() {
        let xml = br#"<ScrivenerProject>
          <CustomMetaDataSettings>
            <MetaDataField ID="field-1" FieldType="text">
              <Title>Story Date</Title>
            </MetaDataField>
            <MetaDataField ID="field-2" FieldType="text">
              <Title>POV</Title>
            </MetaDataField>
          </CustomMetaDataSettings>
          <Binder>
            <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="DraftFolder">
              <Title>Root</Title>
              <Children>
                <BinderItem UUID="00000000-0000-0000-0000-000000000002" Type="Text">
                  <Title>The Storm</Title>
                  <MetaData>
                    <CustomMetaData>
                      <MetaDataItem ID="field-1"><Value>1980-05-15</Value></MetaDataItem>
                      <MetaDataItem ID="field-2"><Value>Aerin</Value></MetaDataItem>
                    </CustomMetaData>
                  </MetaData>
                </BinderItem>
              </Children>
            </BinderItem>
          </Binder>
        </ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        let storm = &items[0].children[0];
        assert_eq!(storm.title, "The Storm");
        assert_eq!(
            storm.custom_meta,
            vec![
                ("Story Date".to_owned(), "1980-05-15".to_owned()),
                ("POV".to_owned(), "Aerin".to_owned()),
            ]
        );
    }

    /// 1.2.8+ — an unknown CustomMeta ID (field referenced by
    /// an item but missing from the project registry) should be
    /// silently skipped, not error out.
    #[test]
    fn custom_meta_unknown_id_skipped() {
        let xml = br#"<ScrivenerProject>
          <CustomMetaDataSettings>
            <MetaDataField ID="field-1"><Title>Date</Title></MetaDataField>
          </CustomMetaDataSettings>
          <Binder>
            <BinderItem UUID="00000000-0000-0000-0000-000000000001" Type="Text">
              <Title>Only known field is kept</Title>
              <MetaData>
                <CustomMetaData>
                  <MetaDataItem ID="field-1"><Value>2026-01-01</Value></MetaDataItem>
                  <MetaDataItem ID="field-99"><Value>orphan</Value></MetaDataItem>
                </CustomMetaData>
              </MetaData>
            </BinderItem>
          </Binder>
        </ScrivenerProject>"#;
        let items = parse_scrivx(xml).unwrap();
        let only = &items[0];
        assert_eq!(only.custom_meta.len(), 1);
        assert_eq!(only.custom_meta[0].0, "Date");
        assert_eq!(only.custom_meta[0].1, "2026-01-01");
    }
}

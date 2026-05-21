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
                        });
                    }
                    "Title" => {
                        current_text = Some(TextBuf::Title);
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                if let Some(TextBuf::Title) = current_text {
                    if let Some(top) = stack.last_mut() {
                        let txt = e
                            .unescape()
                            .map_err(|err| anyhow!("title decode: {err}"))?;
                        top.title.push_str(&txt);
                    }
                }
            }
            Event::End(e) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "Title" => {
                        current_text = None;
                    }
                    "BinderItem" => {
                        if let Some(p) = stack.pop() {
                            let item = BinderItem {
                                uuid: p.uuid,
                                kind: p.kind,
                                title: p.title,
                                children: p.children,
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

#[derive(Debug)]
struct PartialItem {
    uuid: Uuid,
    kind: String,
    title: String,
    children: Vec<BinderItem>,
}

#[derive(Debug)]
enum TextBuf {
    Title,
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

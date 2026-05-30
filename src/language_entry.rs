//! 1.2.13+ Phase B.2 — parser for Language-book dictionary entries.
//!
//! A dictionary entry's body lives as a fenced `​```hjson` block
//! sandwiched between a `= word` title line and a `# Free-form
//! notes` section.  The HJSON inside has four core fields
//! (`word`, `type`, `translation`, `example`) plus an optional
//! `inflection: { paradigm_name: form, ... }` map that drives
//! the lexicon's paradigm expansion (so "aiyo" lights up when
//! the lemma `aiya` has `inflection: { genitive: "aiyo" }`).
//!
//! Why a dedicated parser rather than reading the whole body
//! as HJSON: the body is markdown-ish — it has the `= word`
//! title line, the fenced HJSON block, and a free-form notes
//! section underneath.  serde_hjson chokes on the markdown.
//! We locate the fenced block, then feed the contents to
//! serde_hjson.
//!
//! Used by:
//!   * `tui::lexicon_build` — to expand paradigm forms into
//!     the lexicon so the overlay catches inflected words.
//!   * the editor footer (Phase B.2 chip) — to render
//!     `[word · POS · translation]` when the cursor lands on a
//!     Language hit.
//!   * Phase C — the AI translation flow's prompt envelope
//!     reads every entry from a language's Dictionary into a
//!     compact JSON block.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Parsed view of a dictionary entry's HJSON frontmatter.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)] // `example` is consumed by Phase C's translation prompt envelope
pub struct DictionaryEntry {
    #[serde(default)]
    pub word: String,
    /// Part of speech — the seed body writes this as `type` to
    /// match the `--type` CLI flag, but `type` is a Rust
    /// reserved word so we rename in the struct.
    #[serde(rename = "type", default)]
    pub pos: String,
    #[serde(default)]
    pub translation: String,
    #[serde(default)]
    pub example: String,
    /// Paradigm name → inflected form.  e.g.
    /// `{ "genitive": "aiyo", "dative": "aiyan" }`.  Drives the
    /// lexicon overlay's paradigm-form expansion: every value
    /// here is added as an extra lexicon name so the overlay
    /// lights up "aiyo" and "aiyan" the same way it lights up
    /// the lemma "aiya".
    #[serde(default)]
    pub inflection: BTreeMap<String, String>,
}

impl DictionaryEntry {
    /// Every form that should light up in prose for this
    /// entry: the lemma word plus every inflection value.
    /// Empty strings filtered out so a partially-populated
    /// HJSON ("inflection: { genitive: "" }") doesn't pollute
    /// the lexicon.
    pub fn surface_forms(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        let lemma = self.word.trim();
        if !lemma.is_empty() {
            out.push(lemma);
        }
        for form in self.inflection.values() {
            let trimmed = form.trim();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
        }
        out
    }
}

/// Parse the first fenced `​```hjson … ```` block out of a
/// paragraph body.  Returns the populated `DictionaryEntry`
/// when the block parses; returns `None` when there's no
/// hjson block at all (an entry written by hand before
/// Phase B that the author hasn't migrated yet).  Returns
/// `Err` when the block parses but the HJSON is malformed —
/// that's an actual schema error, surfaced upward so it can
/// be reported.
pub fn parse(body: &str) -> Result<Option<DictionaryEntry>, String> {
    parse_with::<DictionaryEntry>(
        body,
        |e| format!("dictionary entry HJSON parse failed: {e}"),
    )
}

/// Parsed view of a Language sub-book's `Meta/overview`
/// HJSON frontmatter.  Drives the alphabet-bucket
/// consultation in `inkhaven language add-word` for non-
/// Latin orthographies — the author's declared groupings
/// override the naive first-char uppercase fallback.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)] // remaining fields are consumed by Phase C translation envelope + Phase D doctor
pub struct MetaOverview {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub language_kind: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub iso_code: String,
    /// Alphabet entries in the order the author defined
    /// them.  Each entry is treated as a bucket name; a
    /// word's bucket is the first entry whose characters
    /// case-insensitively contain the word's first
    /// character.  Empty → no consultation (caller falls
    /// back to first-char uppercase).
    #[serde(default)]
    pub alphabet: Vec<String>,
    #[serde(default)]
    pub reading_direction: String,
    #[serde(default)]
    pub stemmer: String,
    #[serde(default)]
    pub example_corpus_ref: String,
}

/// Parse the Meta/overview body the same way as a
/// dictionary entry.  Identical fence semantics, different
/// schema.
pub fn parse_meta_overview(body: &str) -> Result<Option<MetaOverview>, String> {
    parse_with::<MetaOverview>(
        body,
        |e| format!("meta overview HJSON parse failed: {e}"),
    )
}

/// Shared parse path: try the whole body as HJSON first
/// (the pure-HJSON content_type=`"hjson"` format the
/// 1.2.13 Phase D.1 hotfix switched to); fall back to
/// the fenced extractor for legacy Typst-wrapped bodies
/// authored before the hotfix.  Empty / whitespace-only
/// bodies return `Ok(None)` so a freshly-stubbed
/// paragraph the author hasn't filled in yet doesn't
/// surface as a parse error.
fn parse_with<T: serde::de::DeserializeOwned>(
    body: &str,
    err: impl Fn(serde_hjson::Error) -> String,
) -> Result<Option<T>, String> {
    if body.trim().is_empty() {
        return Ok(None);
    }
    // Pure HJSON — try the whole body first.  Pure
    // HJSON paragraphs (the new format) parse cleanly;
    // Typst-wrapped paragraphs fail this attempt and
    // we fall through to fence extraction.
    if let Ok(v) = serde_hjson::from_str::<T>(body) {
        return Ok(Some(v));
    }
    // Legacy fenced format — locate the ```hjson block.
    let Some(block) = extract_hjson_block(body) else {
        return Ok(None);
    };
    let v: T = serde_hjson::from_str(block).map_err(err)?;
    Ok(Some(v))
}

impl MetaOverview {
    /// Find the alphabet bucket for a word according to
    /// the author's declared groupings.  Returns the
    /// matching alphabet entry verbatim (so an author
    /// writing `alphabet: ["Aleph", "Beth", ...]` gets
    /// bucket subchapters titled `Aleph`, `Beth`, …) or
    /// `None` if the word's first char isn't covered —
    /// signal to the caller to fall back to the naive
    /// first-char uppercase bucketing.
    pub fn bucket_for_word(&self, word: &str) -> Option<&str> {
        let first_char = word.chars().find(|c| !c.is_whitespace())?;
        let needle = first_char.to_lowercase().collect::<String>();
        self.alphabet
            .iter()
            .find(|entry| {
                entry
                    .to_lowercase()
                    .chars()
                    .any(|c| c.to_string() == needle)
            })
            .map(|s| s.as_str())
    }
}

/// Find the first `​```hjson` fenced block in `body` and
/// return the text between the opening and closing fence.
/// Both fences must sit at the start of a line (after any
/// indentation is trimmed for the open detection; the close
/// must be exactly `​```` after trim).  We're permissive
/// with the open ("```hjson" or "```hjson ") and strict
/// with the close (must be just "```", not "```rust" — we
/// don't want to grab the wrong fence in a body that opens
/// multiple code blocks).
fn extract_hjson_block(body: &str) -> Option<&str> {
    // Index by byte offsets so we can return a `&str`
    // slice without re-allocating.
    let mut cursor = 0usize;
    let mut open_end: Option<usize> = None;
    for line in body.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line.len();
        let trimmed = line.trim_start().trim_end_matches('\n').trim_end();
        if open_end.is_none() {
            if trimmed == "```hjson" || trimmed.starts_with("```hjson ") {
                open_end = Some(cursor); // start of content = end of fence line
            }
        } else {
            // Closing fence: must be exactly "```" so we
            // don't accidentally close on "```typst".
            if trimmed == "```" {
                let open = open_end.unwrap();
                // Content sits between open and line_start.
                return Some(&body[open..line_start]);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDED_BODY: &str = "\
= aiya

```hjson
{
  word:         \"aiya\"
  type:         \"interjection\"
  translation:  \"hail\"
  example:      \"Aiya Eärendil!\"
}
```

# Free-form notes

Greeting used by elves of Aman.
";

    const SEEDED_WITH_INFLECTION: &str = "\
= aiya

```hjson
{
  word:         \"aiya\"
  type:         \"interjection\"
  translation:  \"hail\"
  example:      \"Aiya Eärendil!\"
  inflection: {
    plural:    \"aiyar\"
    emphatic:  \"aiyala\"
  }
}
```
";

    /// 1.2.13+ Phase D.1 hotfix — new pure-HJSON seed
    /// shape (no Typst wrapper, no fence) authored by
    /// the hotfix's `seed_dictionary_entry_body` +
    /// stored under `content_type: "hjson"`.
    #[test]
    fn parses_pure_hjson_entry() {
        let body = r#"{
  word: "aiya"
  type: "interjection"
  translation: "hail"
  example: ""
}
"#;
        let entry = parse(body).unwrap().unwrap();
        assert_eq!(entry.word, "aiya");
        assert_eq!(entry.pos, "interjection");
        assert_eq!(entry.translation, "hail");
    }

    #[test]
    fn parses_pure_hjson_meta_overview() {
        let body = r#"{
  name: "Quenya"
  language_kind: "constructed"
  alphabet: ["A", "B", "C"]
}
"#;
        let meta = parse_meta_overview(body).unwrap().unwrap();
        assert_eq!(meta.name, "Quenya");
        assert_eq!(meta.language_kind, "constructed");
        assert_eq!(meta.alphabet, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    }

    #[test]
    fn parses_core_fields_from_seeded_body() {
        let entry = parse(SEEDED_BODY).unwrap().expect("hjson block present");
        assert_eq!(entry.word, "aiya");
        assert_eq!(entry.pos, "interjection");
        assert_eq!(entry.translation, "hail");
        assert_eq!(entry.example, "Aiya Eärendil!");
        assert!(entry.inflection.is_empty());
    }

    #[test]
    fn parses_inflection_map() {
        let entry = parse(SEEDED_WITH_INFLECTION).unwrap().unwrap();
        assert_eq!(entry.inflection.get("plural"), Some(&"aiyar".to_string()));
        assert_eq!(entry.inflection.get("emphatic"), Some(&"aiyala".to_string()));
    }

    #[test]
    fn surface_forms_includes_lemma_and_inflections() {
        let entry = parse(SEEDED_WITH_INFLECTION).unwrap().unwrap();
        let forms = entry.surface_forms();
        assert!(forms.contains(&"aiya"));
        assert!(forms.contains(&"aiyar"));
        assert!(forms.contains(&"aiyala"));
    }

    #[test]
    fn surface_forms_filters_empty_inflection_values() {
        // A partially-populated entry — author started the
        // paradigm but left a slot blank.  The blank must
        // not pollute the lexicon with an empty name.
        let body = "\
```hjson
{
  word: \"aiya\"
  type: \"interjection\"
  translation: \"hail\"
  example: \"\"
  inflection: { plural: \"aiyar\", dual: \"\" }
}
```";
        let entry = parse(body).unwrap().unwrap();
        let forms = entry.surface_forms();
        assert_eq!(forms.len(), 2, "got: {forms:?}");
        assert!(forms.contains(&"aiya"));
        assert!(forms.contains(&"aiyar"));
    }

    #[test]
    fn no_block_returns_none() {
        let body = "= aiya\n\nJust a free-form description, no HJSON.\n";
        assert!(parse(body).unwrap().is_none());
    }

    #[test]
    fn close_fence_must_be_bare() {
        // The closing fence is strict — `​```typst` after
        // the opening `​```hjson` doesn't close it.  If the
        // body had a second code block we'd want to make
        // sure we don't grab the wrong content; here we
        // verify that a malformed body (open fence with no
        // bare close) yields `None` cleanly.
        let body = "\
```hjson
{ word: \"aiya\" }
```typst
unmatched
";
        // No bare close → no block found → None.
        assert!(parse(body).unwrap().is_none());
    }

    #[test]
    fn meta_overview_alphabet_buckets_first_char() {
        let body = "\
```hjson
{
  name: \"Quenya\"
  alphabet: [\"Aa\", \"Bb\", \"Cc\"]
}
```";
        let meta = parse_meta_overview(body).unwrap().unwrap();
        assert_eq!(meta.bucket_for_word("aiya"), Some("Aa"));
        assert_eq!(meta.bucket_for_word("Bran"), Some("Bb"));
        assert_eq!(meta.bucket_for_word("zzz"), None,
            "word's first char not in the declared alphabet → None (signal fall-back)");
    }

    #[test]
    fn meta_overview_alphabet_multichar_buckets() {
        // Author transliterates Hebrew letter names —
        // each bucket entry is a multi-char string.  The
        // first char of the word ('a' for aleph, 'b' for
        // beth, etc.) drives the lookup.
        let body = "\
```hjson
{
  name: \"BiblicalHebrew\"
  alphabet: [\"Aleph\", \"Beth\", \"Gimel\"]
}
```";
        let meta = parse_meta_overview(body).unwrap().unwrap();
        assert_eq!(meta.bucket_for_word("Avraham"), Some("Aleph"));
        assert_eq!(meta.bucket_for_word("Beriah"), Some("Beth"));
    }

    #[test]
    fn meta_overview_empty_alphabet_returns_none() {
        let body = "\
```hjson
{
  name: \"BareBones\"
  alphabet: []
}
```";
        let meta = parse_meta_overview(body).unwrap().unwrap();
        assert_eq!(meta.bucket_for_word("anything"), None);
    }

    #[test]
    fn malformed_hjson_reports_error() {
        let body = "\
```hjson
{ word: \"aiya
```
";
        // Unterminated string → serde_hjson errors.  We
        // surface the error rather than silently dropping
        // the entry.
        assert!(parse(body).is_err());
    }
}

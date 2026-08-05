//! WordNet — a sense-based, multilingual thesaurus for manuscript prose (1.8.3+).
//!
//! A writing aid, not part of the constructed-language layer: point at a word in
//! real prose (English, Russian, French, German, Spanish) and get its *senses*,
//! each with synonyms, antonyms, and hypernyms/hyponyms drawn from WordNet. The
//! data is open, fetched on demand in the standard **WN-LMF** interchange format
//! and linked across languages by the interlingual index (ILI); see
//! [`fetch`](crate::wordnet::fetch).
//!
//! This module is the pure core: a compact in-memory model, a streaming WN-LMF
//! parser, sense/relation lookup, and (de)serialisation of the built index to
//! the user data directory. Network and CLI live in [`fetch`] and the CLI
//! handler.

pub mod fetch;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A parsed, indexed wordnet for one language, ready for lookup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WordNet {
    /// BCP-47 / ISO language code the lexicon declares.
    pub language: String,
    entries: Vec<Entry>,
    synsets: HashMap<String, Synset>,
    /// Lower-cased lemma → indices into `entries`.
    lemma_index: HashMap<String, Vec<u32>>,
    /// Sense id → index into `entries` (to resolve sense-relation targets to a lemma).
    sense_owner: HashMap<String, u32>,
    /// Synset id → the lemmas that are members of it.
    members: HashMap<String, Vec<String>>,
    /// Interlingual index → the local synset id carrying it (for pivot lookup).
    #[serde(default)]
    ili_to_synset: HashMap<String, String>,
    /// Interlingual index → this language's lemmas for that concept.
    #[serde(default)]
    ili_members: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    lemma: String,
    pos: String,
    senses: Vec<Sense>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Sense {
    id: String,
    synset: String,
    #[serde(default)]
    relations: Vec<Rel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Synset {
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    ili: Option<String>,
    pos: String,
    #[serde(default)]
    definition: Option<String>,
    #[serde(default)]
    relations: Vec<Rel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rel {
    rel_type: String,
    target: String,
}

/// One sense of a looked-up word, with its relations resolved to lemmas.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseView {
    pub pos: String,
    pub definition: Option<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub hypernyms: Vec<String>,
    pub hyponyms: Vec<String>,
}

/// The result of looking a word up: its senses (empty if unknown).
#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    pub word: String,
    pub senses: Vec<SenseView>,
}

/// SEMNET-P5 — one sense of a word as a graph node: its synset id, interlingual
/// index, and the target synset ids of its one-hop taxonomy relations.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseNode {
    pub synset: String,
    pub ili: Option<String>,
    pub hypernyms: Vec<String>,
    pub hyponyms: Vec<String>,
    pub antonyms: Vec<String>,
}

impl Lookup {
    pub fn is_empty(&self) -> bool {
        self.senses.is_empty()
    }

    /// Flatten the senses into a de-duplicated, ordered pick-list of candidate
    /// replacement words for the editor panel: synonyms first (the usual intent
    /// of a thesaurus), then hypernyms, hyponyms, and antonyms. The original
    /// word is never suggested. Each word appears once, tagged by the first
    /// relation that surfaced it.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let mut seen = std::collections::HashSet::new();
        seen.insert(self.word.to_lowercase());
        let mut out = Vec::new();
        for kind in ["synonym", "hypernym", "hyponym", "antonym"] {
            for sense in &self.senses {
                let words = match kind {
                    "synonym" => &sense.synonyms,
                    "hypernym" => &sense.hypernyms,
                    "hyponym" => &sense.hyponyms,
                    _ => &sense.antonyms,
                };
                for w in words {
                    if seen.insert(w.to_lowercase()) {
                        out.push(Suggestion { kind, word: w.clone() });
                    }
                }
            }
        }
        out
    }
}

/// One candidate replacement for the editor's WordNet panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// `synonym` | `antonym` | `hypernym` | `hyponym`.
    pub kind: &'static str,
    pub word: String,
}

impl WordNet {
    /// Look a word up in this wordnet alone (no cross-lingual expansion).
    pub fn lookup(&self, word: &str) -> Lookup {
        self.lookup_with_pivot(word, None)
    }

    /// Look a word up, optionally expanding its taxonomy relations through a
    /// `pivot` wordnet (English) by the interlingual index. A non-English
    /// wordnet often lists only lemmas per synset, not the hypernym/hyponym
    /// links; when the native synset has none, we follow its ILI into the
    /// pivot's rich relation graph and map the results back to this language's
    /// lemmas. Synonyms (synset co-members) are always native.
    pub fn lookup_with_pivot(&self, word: &str, pivot: Option<&WordNet>) -> Lookup {
        let key = word.to_lowercase();
        let mut senses = Vec::new();
        for &ei in self.lemma_index.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
            let entry = &self.entries[ei as usize];
            for sense in &entry.senses {
                let Some(synset) = self.synsets.get(&sense.synset) else { continue };
                let synonyms = self
                    .members
                    .get(&sense.synset)
                    .map(|ms| ms.iter().filter(|m| !m.eq_ignore_ascii_case(&entry.lemma)).cloned().collect())
                    .unwrap_or_default();
                let antonyms = sense
                    .relations
                    .iter()
                    .filter(|r| r.rel_type == "antonym")
                    .filter_map(|r| self.lemma_of_sense(&r.target))
                    .collect::<Vec<_>>();
                let hypernyms = self.relation_lemmas(synset, "hypernym", pivot);
                let hyponyms = self.relation_lemmas(synset, "hyponym", pivot);
                senses.push(SenseView {
                    pos: pos_label(&entry.pos).to_string(),
                    definition: synset.definition.clone(),
                    synonyms: dedup(synonyms),
                    antonyms: dedup(antonyms),
                    hypernyms: dedup(hypernyms),
                    hyponyms: dedup(hyponyms),
                });
            }
        }
        Lookup { word: word.to_string(), senses }
    }

    /// The lemmas reached by a synset-level relation — native if present, else
    /// expanded through the pivot wordnet by ILI.
    fn relation_lemmas(&self, synset: &Synset, rel_type: &str, pivot: Option<&WordNet>) -> Vec<String> {
        let native = self.related_members(synset, rel_type);
        if !native.is_empty() {
            return native;
        }
        // Expand via the pivot: this synset's ILI → the pivot's synset → its
        // relation targets' ILIs → this language's lemmas for those ILIs.
        let (Some(ili), Some(pivot)) = (synset.ili.as_ref(), pivot) else {
            return native;
        };
        let Some(psid) = pivot.ili_to_synset.get(ili) else { return native };
        let Some(psyn) = pivot.synsets.get(psid) else { return native };
        psyn.relations
            .iter()
            .filter(|r| r.rel_type == rel_type)
            .filter_map(|r| pivot.synsets.get(&r.target).and_then(|s| s.ili.as_ref()))
            .flat_map(|tili| self.ili_members.get(tili).cloned().unwrap_or_default())
            .collect()
    }

    /// The interlingual (ILI) codes of a word's synsets — the key to the same
    /// concept in another language's wordnet. Consumed by cross-lingual lookup
    /// when the OMW languages land (1.8.4); exercised now by the tests.
    #[allow(dead_code)]
    pub fn ili_of(&self, word: &str) -> Vec<String> {
        let key = word.to_lowercase();
        let mut out = Vec::new();
        for &ei in self.lemma_index.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
            for sense in &self.entries[ei as usize].senses {
                if let Some(ili) = self.synsets.get(&sense.synset).and_then(|s| s.ili.as_ref()) {
                    out.push(ili.clone());
                }
            }
        }
        dedup(out)
    }

    /// Number of lemmas indexed — a cheap health check after a fetch.
    pub fn lemma_count(&self) -> usize {
        self.lemma_index.len()
    }

    /// Number of synsets.
    pub fn synset_count(&self) -> usize {
        self.synsets.len()
    }

    fn lemma_of_sense(&self, sense_id: &str) -> Option<String> {
        self.sense_owner.get(sense_id).map(|&ei| self.entries[ei as usize].lemma.clone())
    }

    /// The synset a sense id belongs to (for resolving sense-level relation
    /// targets — e.g. antonym — to a synset endpoint).
    fn synset_of_sense(&self, sense_id: &str) -> Option<String> {
        let &ei = self.sense_owner.get(sense_id)?;
        self.entries[ei as usize]
            .senses
            .iter()
            .find(|s| s.id == sense_id)
            .map(|s| s.synset.clone())
    }

    /// SEMNET-P5 (lexical bridge) — a word's senses as graph nodes: each sense's
    /// synset id, its interlingual index (for cross-lingual `Translates`), and
    /// the target synset ids of its one-hop hypernym / hyponym / antonym
    /// relations (native links only; the ILI/`Translates` edge carries the
    /// cross-lingual pivot). Synonymy is left implicit — co-members share a
    /// synset, so two lemmas resolve to the same `Sense` endpoint.
    pub fn sense_nodes(&self, word: &str) -> Vec<SenseNode> {
        let key = word.to_lowercase();
        let mut out = Vec::new();
        for &ei in self.lemma_index.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
            for sense in &self.entries[ei as usize].senses {
                let Some(synset) = self.synsets.get(&sense.synset) else { continue };
                let targets = |rt: &str| -> Vec<String> {
                    synset.relations.iter().filter(|r| r.rel_type == rt).map(|r| r.target.clone()).collect()
                };
                let antonyms = sense
                    .relations
                    .iter()
                    .filter(|r| r.rel_type == "antonym")
                    .filter_map(|r| self.synset_of_sense(&r.target))
                    .collect();
                out.push(SenseNode {
                    synset: sense.synset.clone(),
                    ili: synset.ili.clone(),
                    hypernyms: targets("hypernym"),
                    hyponyms: targets("hyponym"),
                    antonyms,
                });
            }
        }
        out
    }

    fn related_members(&self, synset: &Synset, rel_type: &str) -> Vec<String> {
        synset
            .relations
            .iter()
            .filter(|r| r.rel_type == rel_type)
            .flat_map(|r| self.members.get(&r.target).cloned().unwrap_or_default())
            .collect()
    }

    /// Serialise the built index to `path`, behind a 4-byte version header so a
    /// stale index (from a build whose struct differed) is rejected loudly
    /// rather than misread — bincode is positional and not self-describing.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        let mut bytes = Vec::from(INDEX_MAGIC);
        bytes.push(INDEX_VERSION);
        bytes.extend(bincode::serialize(self).map_err(|e| format!("serialize wordnet: {e}"))?);
        // M6 (3.0.0 P2) — atomic write (temp + fsync + rename), so Ctrl-C or a
        // crash mid-write during `wordnet fetch` can't truncate the prior index.
        crate::io_atomic::write(path, &bytes).map_err(|e| format!("write {}: {e}", path.display()))
    }

    /// Load a previously-built index, verifying the version header.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if bytes.len() < 4 || &bytes[..3] != INDEX_MAGIC {
            return Err("not an Inkhaven wordnet index (or built by an older version) — re-run `wordnet fetch`".into());
        }
        if bytes[3] != INDEX_VERSION {
            return Err(format!(
                "wordnet index format v{} but this build expects v{INDEX_VERSION} — re-run `wordnet fetch` to rebuild it",
                bytes[3]
            ));
        }
        bincode::deserialize(&bytes[4..]).map_err(|e| format!("deserialize wordnet: {e}"))
    }
}

/// Magic prefix of a persisted index (`IWN`), followed by [`INDEX_VERSION`].
const INDEX_MAGIC: &[u8] = b"IWN";
/// Bumped whenever the [`WordNet`] on-disk layout changes, so old indexes are
/// rejected with a clear "re-fetch" message instead of silently misreading.
const INDEX_VERSION: u8 = 2;

fn dedup(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

/// Human-readable part of speech from the WN-LMF single-letter code.
fn pos_label(pos: &str) -> &'static str {
    match pos {
        "n" => "noun",
        "v" => "verb",
        "a" | "s" => "adjective",
        "r" => "adverb",
        _ => "other",
    }
}

/// The user data directory for fetched wordnet indexes
/// (`<data_dir>/inkhaven/wordnet/`), shared across projects.
pub fn data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "inkhaven", "inkhaven")
        .map(|d| d.data_dir().join("wordnet"))
}

/// The on-disk path of a language's built index.
pub fn index_path(language: &str) -> Option<PathBuf> {
    data_dir().map(|d| d.join(format!("{language}.wn")))
}

/// Parse a WN-LMF document (already decompressed) into an indexed [`WordNet`].
/// Streaming, so it handles the tens-of-MB OEWN file without a DOM. Reads
/// `LexicalEntry`/`Lemma`/`Sense`/`SenseRelation` and `Synset`/`Definition`/
/// `SynsetRelation`; everything else is ignored.
pub fn parse_lmf(xml: &str) -> Result<WordNet, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut language = String::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut synsets: HashMap<String, Synset> = HashMap::new();

    let mut cur_entry: Option<Entry> = None;
    let mut cur_synset: Option<Synset> = None;
    let mut in_def = false;
    let mut def_text = String::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("WN-LMF parse error at {}: {e}", reader.buffer_position()))?;
        match ev {
            Event::Eof => break,
            Event::Start(e) => on_open(
                &e,
                false,
                &mut language,
                &mut cur_entry,
                &mut cur_synset,
                &mut in_def,
                &mut def_text,
                &mut synsets,
            ),
            Event::Empty(e) => on_open(
                &e,
                true,
                &mut language,
                &mut cur_entry,
                &mut cur_synset,
                &mut in_def,
                &mut def_text,
                &mut synsets,
            ),
            Event::Text(t) => {
                if in_def {
                    def_text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Event::End(e) => {
                match e.name().as_ref() {
                    b"LexicalEntry" => {
                        if let Some(en) = cur_entry.take() {
                            if !en.lemma.is_empty() {
                                entries.push(en);
                            }
                        }
                    }
                    b"Synset" => {
                        if let Some(ss) = cur_synset.take() {
                            synsets.insert(ss.id.clone(), ss);
                        }
                    }
                    b"Definition" => {
                        in_def = false;
                        if let Some(ss) = cur_synset.as_mut() {
                            if ss.definition.is_none() {
                                ss.definition = Some(def_text.trim().to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(build_index(language, entries, synsets))
}

#[allow(clippy::too_many_arguments)]
fn on_open(
    e: &quick_xml::events::BytesStart,
    empty: bool,
    language: &mut String,
    cur_entry: &mut Option<Entry>,
    cur_synset: &mut Option<Synset>,
    in_def: &mut bool,
    def_text: &mut String,
    synsets: &mut HashMap<String, Synset>,
) {
    match e.name().as_ref() {
        b"Lexicon" => {
            if let Some(l) = attr(e, b"language") {
                *language = l;
            }
        }
        b"LexicalEntry" => {
            *cur_entry = Some(Entry { lemma: String::new(), pos: String::new(), senses: Vec::new() });
        }
        b"Lemma" => {
            if let Some(en) = cur_entry.as_mut() {
                if let Some(w) = attr(e, b"writtenForm") {
                    en.lemma = w;
                }
                if let Some(p) = attr(e, b"partOfSpeech") {
                    en.pos = p;
                }
            }
        }
        b"Sense" => {
            if let Some(en) = cur_entry.as_mut() {
                en.senses.push(Sense {
                    id: attr(e, b"id").unwrap_or_default(),
                    synset: attr(e, b"synset").unwrap_or_default(),
                    relations: Vec::new(),
                });
            }
        }
        b"SenseRelation" => {
            if let Some(sense) = cur_entry.as_mut().and_then(|en| en.senses.last_mut()) {
                if let (Some(t), Some(tgt)) = (attr(e, b"relType"), attr(e, b"target")) {
                    sense.relations.push(Rel { rel_type: t, target: tgt });
                }
            }
        }
        b"Synset" => {
            let ss = Synset {
                id: attr(e, b"id").unwrap_or_default(),
                ili: attr(e, b"ili").filter(|s| !s.is_empty()),
                pos: attr(e, b"partOfSpeech").unwrap_or_default(),
                definition: None,
                relations: Vec::new(),
            };
            if empty {
                synsets.insert(ss.id.clone(), ss);
            } else {
                *cur_synset = Some(ss);
            }
        }
        b"SynsetRelation" => {
            if let Some(ss) = cur_synset.as_mut() {
                if let (Some(t), Some(tgt)) = (attr(e, b"relType"), attr(e, b"target")) {
                    ss.relations.push(Rel { rel_type: t, target: tgt });
                }
            }
        }
        b"Definition" => {
            if !empty {
                *in_def = true;
                def_text.clear();
            }
        }
        _ => {}
    }
}

/// Read a single attribute (unescaped) from an element.
fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key {
            return Some(a.unescape_value().map(|c| c.into_owned()).unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned()));
        }
    }
    None
}

/// Build the derived lookup indexes from parsed entries + synsets.
fn build_index(language: String, entries: Vec<Entry>, synsets: HashMap<String, Synset>) -> WordNet {
    let mut lemma_index: HashMap<String, Vec<u32>> = HashMap::new();
    let mut sense_owner: HashMap<String, u32> = HashMap::new();
    let mut members: HashMap<String, Vec<String>> = HashMap::new();

    for (i, entry) in entries.iter().enumerate() {
        let ei = i as u32;
        lemma_index.entry(entry.lemma.to_lowercase()).or_default().push(ei);
        for sense in &entry.senses {
            sense_owner.insert(sense.id.clone(), ei);
            members.entry(sense.synset.clone()).or_default().push(entry.lemma.clone());
        }
    }
    for v in members.values_mut() {
        v.sort();
        v.dedup();
    }

    // ILI indexes: interlingual code → local synset, and → local lemmas.
    let mut ili_to_synset: HashMap<String, String> = HashMap::new();
    let mut ili_members: HashMap<String, Vec<String>> = HashMap::new();
    for (sid, synset) in &synsets {
        if let Some(ili) = &synset.ili {
            ili_to_synset.entry(ili.clone()).or_insert_with(|| sid.clone());
            if let Some(ms) = members.get(sid) {
                ili_members.entry(ili.clone()).or_default().extend(ms.iter().cloned());
            }
        }
    }
    for v in ili_members.values_mut() {
        v.sort();
        v.dedup();
    }

    WordNet { language, entries, synsets, lemma_index, sense_owner, members, ili_to_synset, ili_members }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A tiny hand-built WN-LMF document with two synsets, a synonym pair, an
    // antonym, and a hypernym link.
    const SAMPLE: &str = r#"
<LexicalResource>
  <Lexicon id="test" language="en">
    <LexicalEntry id="e-big">
      <Lemma writtenForm="big" partOfSpeech="a"/>
      <Sense id="s-big" synset="ss-large">
        <SenseRelation relType="antonym" target="s-small"/>
      </Sense>
    </LexicalEntry>
    <LexicalEntry id="e-large">
      <Lemma writtenForm="large" partOfSpeech="a"/>
      <Sense id="s-large" synset="ss-large"/>
    </LexicalEntry>
    <LexicalEntry id="e-small">
      <Lemma writtenForm="small" partOfSpeech="a"/>
      <Sense id="s-small" synset="ss-small"/>
    </LexicalEntry>
    <LexicalEntry id="e-dog">
      <Lemma writtenForm="dog" partOfSpeech="n"/>
      <Sense id="s-dog" synset="ss-dog"/>
    </LexicalEntry>
    <LexicalEntry id="e-animal">
      <Lemma writtenForm="animal" partOfSpeech="n"/>
      <Sense id="s-animal" synset="ss-animal"/>
    </LexicalEntry>
    <Synset id="ss-large" ili="i1" partOfSpeech="a">
      <Definition>above average in size</Definition>
    </Synset>
    <Synset id="ss-small" ili="i2" partOfSpeech="a"/>
    <Synset id="ss-dog" ili="i3" partOfSpeech="n">
      <Definition>a domesticated canine</Definition>
      <SynsetRelation relType="hypernym" target="ss-animal"/>
    </Synset>
    <Synset id="ss-animal" ili="i4" partOfSpeech="n"/>
  </Lexicon>
</LexicalResource>
"#;

    #[test]
    fn parses_language_and_counts() {
        let wn = parse_lmf(SAMPLE).unwrap();
        assert_eq!(wn.language, "en");
        assert_eq!(wn.lemma_count(), 5);
        assert_eq!(wn.synset_count(), 4);
    }

    #[test]
    fn synonyms_come_from_the_shared_synset() {
        let wn = parse_lmf(SAMPLE).unwrap();
        let l = wn.lookup("big");
        assert_eq!(l.senses.len(), 1);
        assert_eq!(l.senses[0].pos, "adjective");
        assert_eq!(l.senses[0].definition.as_deref(), Some("above average in size"));
        assert_eq!(l.senses[0].synonyms, vec!["large".to_string()]);
    }

    #[test]
    fn antonyms_resolve_through_sense_relations() {
        let wn = parse_lmf(SAMPLE).unwrap();
        let l = wn.lookup("big");
        assert_eq!(l.senses[0].antonyms, vec!["small".to_string()]);
    }

    #[test]
    fn hypernyms_resolve_through_synset_relations() {
        let wn = parse_lmf(SAMPLE).unwrap();
        let l = wn.lookup("dog");
        assert_eq!(l.senses[0].hypernyms, vec!["animal".to_string()]);
    }

    #[test]
    fn unknown_word_is_empty() {
        let wn = parse_lmf(SAMPLE).unwrap();
        assert!(wn.lookup("nonexistent").is_empty());
    }

    #[test]
    fn ili_links_the_concept() {
        let wn = parse_lmf(SAMPLE).unwrap();
        assert_eq!(wn.ili_of("dog"), vec!["i3".to_string()]);
    }

    // A tiny "French" wordnet: lemmas mapped to synsets by ILI, but with NO
    // relations of its own — the common OMW shape.
    const FR: &str = r#"
<LexicalResource>
  <Lexicon id="fr" language="fr">
    <LexicalEntry id="e-chien">
      <Lemma writtenForm="chien" partOfSpeech="n"/>
      <Sense id="s-chien" synset="fr-dog"/>
    </LexicalEntry>
    <LexicalEntry id="e-animal">
      <Lemma writtenForm="animal" partOfSpeech="n"/>
      <Sense id="s-animal" synset="fr-animal"/>
    </LexicalEntry>
    <Synset id="fr-dog" ili="i3" partOfSpeech="n"/>
    <Synset id="fr-animal" ili="i4" partOfSpeech="n"/>
  </Lexicon>
</LexicalResource>
"#;

    #[test]
    fn hypernyms_expand_through_the_english_pivot_by_ili() {
        let en = parse_lmf(SAMPLE).unwrap();
        let fr = parse_lmf(FR).unwrap();
        // French `chien` has no native relations; its synset shares ILI i3 with
        // English `dog`, whose hypernym (i4) is French `animal`.
        let native = fr.lookup("chien");
        assert!(native.senses[0].hypernyms.is_empty());
        let expanded = fr.lookup_with_pivot("chien", Some(&en));
        assert_eq!(expanded.senses[0].hypernyms, vec!["animal".to_string()]);
    }

    #[test]
    fn suggestions_are_ordered_and_deduped() {
        let wn = parse_lmf(SAMPLE).unwrap();
        let sug = wn.lookup("big").suggestions();
        // Synonyms come before antonyms; the word itself is never suggested.
        assert_eq!(sug[0], Suggestion { kind: "synonym", word: "large".into() });
        assert!(sug.iter().any(|s| s.kind == "antonym" && s.word == "small"));
        assert!(!sug.iter().any(|s| s.word.eq_ignore_ascii_case("big")));
    }

    #[test]
    fn round_trips_through_bincode() {
        let wn = parse_lmf(SAMPLE).unwrap();
        let bytes = bincode::serialize(&wn).unwrap();
        let back: WordNet = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.lookup("big").senses[0].synonyms, vec!["large".to_string()]);
    }
}

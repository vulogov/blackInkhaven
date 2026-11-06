//! The `--json` shapes of the language commands that used to print text only
//! (LANG-IMPROVE-1 LI-9), and the one `emit` that every handler routes through.
//!
//! Rule for the shapes: where the matching Bund word (`ink.lang.*`) already
//! returns a structure — a list, or a list / dict with named keys — the CLI
//! emits the same keys, so a script reads the same thing from either side.
//! Where the Bund word returns a scalar (one string), the CLI emits an object
//! carrying the fields its text output already shows.  Every shape derives
//! `Deserialize` too, so a reader can parse it back and the round-trip tests
//! below pin the field names.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// Print `value` as pretty JSON on stdout when `json` is set, otherwise run
/// the text renderer.  Data goes to stdout either way; handlers keep their
/// confirmations and counts on stderr.
pub(crate) fn emit<T: Serialize>(json: bool, value: &T, text: impl FnOnce(&T)) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(value)
                .map_err(|e| Error::Store(format!("serialising json: {e}")))?
        );
    } else {
        text(value);
    }
    Ok(())
}

/// `language list` — one row per language, the counts the text table shows
/// (paragraphs under each scaffold chapter).  `ink.lang.list` returns the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LanguageSummary {
    pub name: String,
    pub entries: usize,
    pub grammar: usize,
    pub phonology: usize,
    pub samples: usize,
}

/// `language ipa` — the derivation the text output prints on three lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IpaSurface {
    pub word: String,
    pub underlying: String,
    pub surface: String,
    pub romanized: String,
}

/// `language stress` — the syllables, which one carries primary stress, and
/// the marked rendering (`ˈka.ta`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StressedWord {
    pub word: String,
    pub syllables: Vec<String>,
    /// Index into `syllables`; `None` when the rule placed no stress.
    pub stressed: Option<usize>,
    pub marked: String,
}

/// `language romanize` — one conversion in either direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Romanized {
    pub input: String,
    pub scheme: String,
    /// `"ipa_to_text"` or `"text_to_ipa"`.
    pub direction: String,
    pub output: String,
}

/// `language tone` — underlying tone labels in, surface labels out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ToneSandhi {
    pub input: Vec<String>,
    pub surface: Vec<String>,
}

/// `language scan` — every line with its beats and metre, plus the stanza's
/// dominant metre when there is more than one line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Scansion {
    pub lines: Vec<ScannedLine>,
    pub dominant_meter: Option<MeterShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ScannedLine {
    pub words: Vec<ScannedWord>,
    pub syllables: usize,
    pub meter: Option<MeterShape>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ScannedWord {
    pub surface: String,
    pub syllables: Vec<ScannedSyllable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ScannedSyllable {
    pub text: String,
    /// `stressed` | `unstressed` | `flexible`.
    pub beat: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MeterShape {
    pub foot: String,
    pub feet: usize,
    pub name: String,
    pub conformance: f64,
}

impl From<&crate::conlang::scansion::Meter> for MeterShape {
    fn from(m: &crate::conlang::scansion::Meter) -> Self {
        MeterShape { foot: m.foot.to_string(), feet: m.feet, name: m.name.clone(), conformance: m.conformance }
    }
}

impl From<&crate::conlang::scansion::ScannedLine> for ScannedLine {
    fn from(l: &crate::conlang::scansion::ScannedLine) -> Self {
        use crate::conlang::scansion::Beat;
        ScannedLine {
            syllables: l.beats().len(),
            meter: l.meter.as_ref().map(MeterShape::from),
            words: l
                .words
                .iter()
                .map(|w| ScannedWord {
                    surface: w.surface.clone(),
                    syllables: w
                        .syllables
                        .iter()
                        .map(|s| ScannedSyllable {
                            text: s.text.clone(),
                            beat: match s.beat {
                                Beat::Stressed => "stressed",
                                Beat::Unstressed => "unstressed",
                                Beat::Flexible => "flexible",
                            }
                            .to_string(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// `language paradigm` — one cell; `ink.lang.paradigm` returns the same keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParadigmCell {
    pub form: String,
    pub gloss: String,
    pub features: BTreeMap<String, String>,
}

/// `language sentence` — the keys `ink.lang.sentence` returns plus the word
/// order and the per-word interlinear pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sentence {
    pub surface: String,
    pub gloss: String,
    pub literal: String,
    pub order: String,
    pub words: Vec<GlossPair>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GlossPair {
    pub surface: String,
    /// `None` when the word was not recognised (`gloss` prints `?`).
    pub gloss: Option<String>,
}

/// `language gloss` — every word with its gloss, and how many were recognised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Glossed {
    pub words: Vec<GlossPair>,
    pub matched: usize,
}

/// `language memory` — one remembered pair; `ink.lang.memory` returns the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MemoryPair {
    pub english: String,
    pub conlang: String,
}

/// `language varieties` — one declared variety; `ink.lang.varieties` returns
/// the same keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VarietySummary {
    pub id: String,
    pub kind: String,
    pub axis: String,
    pub prestige: Option<String>,
    pub sound_changes: usize,
    pub overrides: usize,
    pub note: Option<String>,
}

impl From<&crate::conlang::types::variety::Variety> for VarietySummary {
    fn from(v: &crate::conlang::types::variety::Variety) -> Self {
        VarietySummary {
            id: v.id.clone(),
            kind: v.kind.clone(),
            axis: v.axis.clone(),
            prestige: v.prestige.clone(),
            sound_changes: v.sound_changes.len(),
            overrides: v.lexicon.len(),
            note: v.note.clone(),
        }
    }
}

/// `language dialects` — the comparison table: the varieties compared and one
/// row per headword with each variety's form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DialectTable {
    pub language: String,
    pub varieties: Vec<String>,
    pub rows: Vec<DialectRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DialectRow {
    pub gloss: String,
    pub base: String,
    /// Keyed by variety id.
    pub forms: BTreeMap<String, DialectForm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DialectForm {
    pub form: String,
    /// `true` when a lexical override, not the sound changes, produced it
    /// (the `*` in the text table).
    pub overridden: bool,
}

/// `language cognates` — one daughter's reflex; `ink.lang.cognates` returns
/// the same keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Cognate {
    pub language: String,
    pub reflex: String,
}

/// `language cognates` — the whole set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CognateSet {
    pub proto: String,
    pub form: String,
    pub reflexes: Vec<Cognate>,
}

/// `language family-tree` — inheritance as `(language, proto)` pairs and the
/// horizontal contact edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FamilyTree {
    pub languages: Vec<Descent>,
    pub contact: Vec<ContactEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Descent {
    pub name: String,
    pub proto: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ContactEdge {
    pub a: String,
    pub b: String,
    pub region: String,
}

/// `language sound-change` — one proto-form evolved through the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SoundChanged {
    pub proto: String,
    pub form: String,
    pub result: String,
    pub rules: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(v: &T) -> serde_json::Value {
        let text = serde_json::to_string_pretty(v).unwrap();
        let back: T = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, v, "round-trip changed the value");
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn every_shape_round_trips_and_keeps_its_keys() {
        let v = roundtrip(&vec![LanguageSummary { name: "Avesha".into(), entries: 12, grammar: 3, phonology: 1, samples: 0 }]);
        assert_eq!(v[0]["entries"], 12);

        let v = roundtrip(&IpaSurface { word: "kaʃi".into(), underlying: "kaʃi".into(), surface: "kaʒi".into(), romanized: "kazhi".into() });
        assert_eq!(v["surface"], "kaʒi");

        let v = roundtrip(&StressedWord { word: "kata".into(), syllables: vec!["ka".into(), "ta".into()], stressed: Some(0), marked: "ˈka.ta".into() });
        assert_eq!(v["stressed"], 0);

        let v = roundtrip(&Romanized { input: "k a".into(), scheme: "default".into(), direction: "ipa_to_text".into(), output: "ka".into() });
        assert_eq!(v["direction"], "ipa_to_text");

        let v = roundtrip(&ToneSandhi { input: vec!["H".into(), "L".into()], surface: vec!["H".into(), "H".into()] });
        assert_eq!(v["surface"][1], "H");

        let v = roundtrip(&Scansion {
            lines: vec![ScannedLine {
                words: vec![ScannedWord { surface: "kata".into(), syllables: vec![ScannedSyllable { text: "ka".into(), beat: "stressed".into() }, ScannedSyllable { text: "ta".into(), beat: "unstressed".into() }] }],
                syllables: 2,
                meter: Some(MeterShape { foot: "trochee".into(), feet: 1, name: "trochaic monometer".into(), conformance: 1.0 }),
            }],
            dominant_meter: None,
        });
        assert_eq!(v["lines"][0]["words"][0]["syllables"][0]["beat"], "stressed");

        let v = roundtrip(&vec![ParadigmCell { form: "katat".into(), gloss: "stone-DAT".into(), features: BTreeMap::from([("case".to_string(), "dat".to_string())]) }]);
        assert_eq!(v[0]["features"]["case"], "dat");

        let v = roundtrip(&Sentence { surface: "kira nami".into(), gloss: "bird see".into(), literal: "the bird sees".into(), order: "SOV".into(), words: vec![GlossPair { surface: "kira".into(), gloss: Some("bird".into()) }] });
        assert_eq!(v["literal"], "the bird sees");

        let v = roundtrip(&Glossed { words: vec![GlossPair { surface: "zzz".into(), gloss: None }], matched: 0 });
        assert!(v["words"][0]["gloss"].is_null());

        let v = roundtrip(&vec![MemoryPair { english: "hello".into(), conlang: "sa".into() }]);
        assert_eq!(v[0]["conlang"], "sa");

        let v = roundtrip(&vec![VarietySummary { id: "north".into(), kind: "dialect".into(), axis: "region".into(), prestige: None, sound_changes: 1, overrides: 0, note: None }]);
        assert_eq!(v[0]["sound_changes"], 1);

        let v = roundtrip(&DialectTable { language: "Avesha".into(), varieties: vec!["north".into()], rows: vec![DialectRow { gloss: "stone".into(), base: "kata".into(), forms: BTreeMap::from([("north".to_string(), DialectForm { form: "kada".into(), overridden: false })]) }] });
        assert_eq!(v["rows"][0]["forms"]["north"]["form"], "kada");

        let v = roundtrip(&CognateSet { proto: "Proto".into(), form: "kata".into(), reflexes: vec![Cognate { language: "Tira".into(), reflex: "kada".into() }] });
        assert_eq!(v["reflexes"][0]["reflex"], "kada");

        let v = roundtrip(&FamilyTree { languages: vec![Descent { name: "Tira".into(), proto: Some("Proto".into()) }], contact: vec![ContactEdge { a: "Avesha".into(), b: "Tira".into(), region: "coast".into() }] });
        assert_eq!(v["contact"][0]["region"], "coast");

        let v = roundtrip(&SoundChanged { proto: "Proto".into(), form: "kata".into(), result: "kada".into(), rules: 2 });
        assert_eq!(v["rules"], 2);
    }

    #[test]
    fn emit_prints_json_or_runs_the_text_path() {
        let mut ran = false;
        emit(false, &MemoryPair { english: "a".into(), conlang: "b".into() }, |_| ran = true).unwrap();
        assert!(ran);
        emit(true, &MemoryPair { english: "a".into(), conlang: "b".into() }, |_| panic!("text path ran with --json")).unwrap();
    }
}

//! POEM-1 (PO-P1) — the `poem:` form declaration and the built-in forms library.
//!
//! A `poem:` block is the *declared form* the Inner Poet later measures a poem
//! against — its metre, foot count, rhyme scheme, and structural expectations.
//! [`PoemForm`] is that block; [`FormsLibrary`] is the bundled catalogue of
//! canonical forms (`assets/poetry/forms.hjson`, compiled in), each producible
//! for any project language via [`FormsLibrary::localized`].

use serde::{Deserialize, Serialize};

/// The bundled forms catalogue, compiled into the binary (zero runtime I/O).
pub const FORMS_HJSON: &str = include_str!("../../assets/poetry/forms.hjson");

/// A declared poetic form — the `poem:` HJSON block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PoemForm {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Canonical form name (`sonnet`, `haiku`, `blank_verse`, … or `custom`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub form: String,
    /// One-line description — for `poetry forms` listing, not the poem: block.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub desc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub metre: String,
    #[serde(default)]
    pub feet: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub metre_tradition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rhyme_scheme: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(default)]
    pub stanzas: u32,
    #[serde(default)]
    pub lines_per_stanza: u32,
    #[serde(default)]
    pub allow_pyrrhic: bool,
    #[serde(default)]
    pub require_final_stress: bool,
    #[serde(default)]
    pub elide_mute_e: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_word: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_while_drafting: Option<String>,
}

impl PoemForm {
    /// Parse a `poem:` block from HJSON — either a `{ poem: {...} }` wrapper (a
    /// sidecar or embedded leaf) or a bare `{...}` object. Returns `None` when
    /// no recognisable form is present. Consumed by the Inner Poet (PO-P3) when
    /// it reads a node's declared form; exercised now by the tests.
    #[allow(dead_code)]
    pub fn from_hjson(src: &str) -> Option<PoemForm> {
        #[derive(Deserialize)]
        struct Wrap {
            poem: PoemForm,
        }
        if let Ok(w) = serde_hjson::from_str::<Wrap>(src) {
            if !w.poem.is_empty() {
                return Some(w.poem);
            }
        }
        match serde_hjson::from_str::<PoemForm>(src) {
            Ok(f) if !f.is_empty() => Some(f),
            _ => None,
        }
    }

    /// Whether this carries any declared form information.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.form.is_empty() && self.metre.is_empty() && self.rhyme_scheme.is_empty()
    }

    /// Render as a pasteable `poem:` HJSON block (only the meaningful fields).
    pub fn to_poem_block(&self) -> String {
        let mut s = String::from("poem: {\n");
        if !self.desc.is_empty() {
            s.push_str(&format!("  // {}\n", self.desc));
        }
        let str_field = |s: &mut String, k: &str, v: &str, quote: bool| {
            if !v.is_empty() {
                if quote {
                    s.push_str(&format!("  {k}: \"{v}\"\n"));
                } else {
                    s.push_str(&format!("  {k}: {v}\n"));
                }
            }
        };
        str_field(&mut s, "title", &self.title, true);
        str_field(&mut s, "form", &self.form, false);
        str_field(&mut s, "metre", &self.metre, false);
        if self.feet > 0 {
            s.push_str(&format!("  feet: {}\n", self.feet));
        }
        str_field(&mut s, "metre_tradition", &self.metre_tradition, false);
        str_field(&mut s, "rhyme_scheme", &self.rhyme_scheme, true);
        str_field(&mut s, "language", &self.language, false);
        if self.stanzas > 0 {
            s.push_str(&format!("  stanzas: {}\n", self.stanzas));
        }
        if self.lines_per_stanza > 0 {
            s.push_str(&format!("  lines_per_stanza: {}\n", self.lines_per_stanza));
        }
        if self.allow_pyrrhic {
            s.push_str("  allow_pyrrhic: true\n");
        }
        if self.require_final_stress {
            s.push_str("  require_final_stress: true\n");
        }
        if self.elide_mute_e {
            s.push_str("  elide_mute_e: true\n");
        }
        if let Some(sw) = &self.signature_word {
            s.push_str(&format!("  signature_word: \"{sw}\"\n"));
        }
        s.push_str("}\n");
        s
    }
}

/// Per-language tuning of a form template. French is a syllabic tradition
/// (count syllables, elide mute-e); Russian accentual-syllabic verse allows
/// pyrrhic substitution and expects final stress. Everything else keeps the
/// accentual-syllabic defaults.
pub fn localize(f: &mut PoemForm, lang: &str) {
    f.language = lang.to_lowercase();
    match f.language.as_str() {
        "fr" => {
            if f.metre_tradition == "accentual_syllabic" {
                f.metre_tradition = "syllabic".into();
                f.metre = "syllabic".into();
            }
            f.elide_mute_e = true;
        }
        "ru" => {
            if f.metre_tradition == "accentual_syllabic" {
                f.allow_pyrrhic = true;
                f.require_final_stress = true;
            }
        }
        _ => {}
    }
}

/// The catalogue of built-in forms.
pub struct FormsLibrary {
    forms: Vec<PoemForm>,
}

impl FormsLibrary {
    /// Load the bundled catalogue (panics only on a corrupt bundled asset,
    /// which a test guards against).
    pub fn builtin() -> Self {
        #[derive(Deserialize)]
        struct FormsFile {
            forms: Vec<PoemForm>,
        }
        let file: FormsFile =
            serde_hjson::from_str(FORMS_HJSON).expect("bundled forms.hjson must parse");
        FormsLibrary { forms: file.forms }
    }

    pub fn all(&self) -> &[PoemForm] {
        &self.forms
    }

    pub fn get(&self, form: &str) -> Option<&PoemForm> {
        self.forms.iter().find(|f| f.form.eq_ignore_ascii_case(form))
    }

    /// A form tuned for `lang`, with `language` set — the `poetry forms
    /// --form X --language Y` template.
    pub fn localized(&self, form: &str, lang: &str) -> Option<PoemForm> {
        let mut f = self.get(form)?.clone();
        localize(&mut f, lang);
        Some(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bundled_forms_parse() {
        let lib = FormsLibrary::builtin();
        // The RFC's 18 canonical forms.
        assert_eq!(lib.all().len(), 18);
        // Every entry has a form name and a description.
        for f in lib.all() {
            assert!(!f.form.is_empty(), "form missing name");
            assert!(!f.desc.is_empty(), "form {} missing desc", f.form);
        }
        assert!(lib.get("sonnet").is_some());
        assert!(lib.get("SONNET").is_some()); // case-insensitive
        assert!(lib.get("nonesuch").is_none());
    }

    #[test]
    fn russian_localization_adds_pyrrhic_and_final_stress() {
        let lib = FormsLibrary::builtin();
        let ru = lib.localized("sonnet", "ru").unwrap();
        assert_eq!(ru.language, "ru");
        assert!(ru.allow_pyrrhic);
        assert!(ru.require_final_stress);
        assert_eq!(ru.metre_tradition, "accentual_syllabic");
    }

    #[test]
    fn french_localization_switches_to_syllabic() {
        let lib = FormsLibrary::builtin();
        let fr = lib.localized("sonnet", "fr").unwrap();
        assert_eq!(fr.metre_tradition, "syllabic");
        assert!(fr.elide_mute_e);
    }

    #[test]
    fn poem_block_round_trips_through_from_hjson() {
        let lib = FormsLibrary::builtin();
        let ru = lib.localized("shakespearean_sonnet", "ru").unwrap();
        let block = ru.to_poem_block();
        let back = PoemForm::from_hjson(&block).expect("parse emitted block");
        assert_eq!(back.form, "shakespearean_sonnet");
        assert_eq!(back.language, "ru");
        assert!(back.allow_pyrrhic);
        assert_eq!(back.rhyme_scheme, ru.rhyme_scheme);
    }

    // PO-P12 guard: the editor's form picker writes any built-in form's block as
    // a sidecar, and `poem_form_for` must be able to read it straight back — in
    // every project language. If a block a poet could attach failed to re-parse,
    // the Inner Poet would silently report "no poem: block" on a stanza that has
    // one. Assert the whole matrix round-trips.
    #[test]
    fn every_builtin_form_round_trips_in_every_language() {
        let lib = FormsLibrary::builtin();
        for pf in lib.all() {
            for lang in ["en", "ru", "fr", "de", "es"] {
                let localized = lib.localized(&pf.form, lang).unwrap_or_else(|| pf.clone());
                let block = localized.to_poem_block();
                let back = PoemForm::from_hjson(&block)
                    .unwrap_or_else(|| panic!("form `{}` ({lang}) block did not re-parse", pf.form));
                assert_eq!(back.form, pf.form, "form name lost round-tripping ({lang})");
            }
        }
    }

    #[test]
    fn from_hjson_reads_a_wrapper_and_rejects_junk() {
        // One field per line — HJSON quoteless strings run to end of line.
        let f = PoemForm::from_hjson("{ poem: {\n  form: haiku\n  metre_tradition: syllabic\n} }")
            .unwrap();
        assert_eq!(f.form, "haiku");
        assert!(PoemForm::from_hjson("{ unrelated: 3 }").is_none());
    }
}

//! Typological-feature catalog (LANG-1 P3.4).
//!
//! A curated, WALS-aligned set of the high-leverage typological questions a
//! conlanger answers. Each feature has an id, a question, and a set of
//! `(value, one-line consequence)` options. The catalog drives the
//! `inkhaven language grammar` questionnaire (validating answers) and the AI
//! grammar book later. Static + pure.

pub struct GrammarFeature {
    pub id: &'static str,
    pub question: &'static str,
    /// `(value, one-line consequence)`.
    pub options: &'static [(&'static str, &'static str)],
}

impl GrammarFeature {
    pub fn is_valid(&self, value: &str) -> bool {
        self.options.iter().any(|(v, _)| v.eq_ignore_ascii_case(value))
    }
    pub fn values(&self) -> String {
        self.options.iter().map(|(v, _)| *v).collect::<Vec<_>>().join(" | ")
    }
}

/// The full catalog.
pub fn catalog() -> &'static [GrammarFeature] {
    CATALOG
}

/// Look a feature up by id (case-insensitive).
pub fn feature(id: &str) -> Option<&'static GrammarFeature> {
    CATALOG.iter().find(|f| f.id.eq_ignore_ascii_case(id))
}

const CATALOG: &[GrammarFeature] = &[
    GrammarFeature {
        id: "word_order",
        question: "Basic order of subject, verb, object?",
        options: &[
            ("svo", "subject–verb–object (English, Mandarin)"),
            ("sov", "subject–object–verb (Japanese, Turkish, Latin)"),
            ("vso", "verb–subject–object (Welsh, Classical Arabic)"),
            ("vos", "verb–object–subject (Malagasy)"),
            ("osv", "object–subject–verb (rare)"),
            ("ovs", "object–verb–subject (rare)"),
            ("free", "free order with case marking (Russian, Sanskrit)"),
        ],
    },
    GrammarFeature {
        id: "adjective_order",
        question: "Where do adjectives sit relative to the noun?",
        options: &[("prenominal", "before the noun (English)"), ("postnominal", "after the noun (French, Spanish)")],
    },
    GrammarFeature {
        id: "genitive_order",
        question: "Where does the possessor sit relative to the possessed?",
        options: &[("possessor_first", "possessor–possessed (John's book)"), ("possessed_first", "possessed–possessor (book of John)")],
    },
    GrammarFeature {
        id: "adposition",
        question: "Prepositions or postpositions?",
        options: &[("preposition", "before the noun (to the house)"), ("postposition", "after the noun (the house to)"), ("none", "case marking instead")],
    },
    GrammarFeature {
        id: "alignment",
        question: "Morphosyntactic alignment?",
        options: &[
            ("nominative_accusative", "subject vs object (most European languages)"),
            ("ergative_absolutive", "agent vs patient/intransitive subject (Basque, Dyirbal)"),
            ("tripartite", "S, A, and P all distinct"),
            ("active_stative", "marking by volitionality"),
        ],
    },
    GrammarFeature {
        id: "case",
        question: "Morphological case system?",
        options: &[("none", "no case marking"), ("few", "a small set (2–4 cases)"), ("many", "a rich case system (5+)")],
    },
    GrammarFeature {
        id: "gender",
        question: "Grammatical gender / noun class?",
        options: &[("none", "no gender"), ("two", "two genders (masc/fem or common/neuter)"), ("three", "three genders"), ("many", "a noun-class system (Bantu)")],
    },
    GrammarFeature {
        id: "number",
        question: "Number marking?",
        options: &[("none", "no obligatory number"), ("singular_plural", "singular vs plural"), ("singular_dual_plural", "adds a dual"), ("rich", "trial / paucal / etc.")],
    },
    GrammarFeature {
        id: "definiteness",
        question: "How is definiteness marked?",
        options: &[("none", "no articles"), ("articles", "definite/indefinite words (a, the)"), ("affix", "a definite affix (Scandinavian, Arabic)")],
    },
    GrammarFeature {
        id: "tense",
        question: "Tense inflection?",
        options: &[("none", "no grammatical tense"), ("past_nonpast", "past vs non-past"), ("past_present_future", "three-way"), ("remoteness", "graded remoteness (today/yesterday/distant)")],
    },
    GrammarFeature {
        id: "aspect",
        question: "Aspect inflection?",
        options: &[("none", "no grammatical aspect"), ("perfective_imperfective", "perfective vs imperfective"), ("rich", "progressive/habitual/perfect/…")],
    },
    GrammarFeature {
        id: "mood",
        question: "Mood inflection?",
        options: &[("none", "mood only periphrastically"), ("indicative_subjunctive", "a realis/irrealis split"), ("rich", "many moods (optative, jussive, …)")],
    },
    GrammarFeature {
        id: "evidentiality",
        question: "Is the source of information grammaticalized?",
        options: &[("none", "no evidentials"), ("present", "marks witnessed / reported / inferred (Quechua, Tariana)")],
    },
    GrammarFeature {
        id: "negation",
        question: "How is clausal negation expressed?",
        options: &[("particle", "a negative word (not)"), ("affix", "a negative affix on the verb"), ("auxiliary", "a negative auxiliary verb (Finnish)")],
    },
    GrammarFeature {
        id: "question",
        question: "How are polar (yes/no) questions formed?",
        options: &[("intonation", "intonation only"), ("particle", "a question particle (Japanese -ka)"), ("word_order", "inversion (English)"), ("morphology", "verbal morphology")],
    },
    GrammarFeature {
        id: "relative_clause",
        question: "Relative-clause strategy?",
        options: &[("postnominal", "after the head noun (English)"), ("prenominal", "before the head noun (Japanese, Chinese)"), ("internally_headed", "head inside the clause"), ("correlative", "correlative (Hindi)")],
    },
    // 1.7 LING-1 L-P3 — six more WALS-aligned features. The word-order-correlated
    // pair (numeral/demonstrative) feed the harmony check; morphological_type +
    // head_marking drive the morphotype survey; voice + comparative round out the
    // 22-feature catalog.
    GrammarFeature {
        id: "numeral_order",
        question: "Where do cardinal numerals sit relative to the noun?",
        options: &[("numeral_first", "numeral–noun (three books; English)"), ("noun_first", "noun–numeral (books three; Khmer)")],
    },
    GrammarFeature {
        id: "demonstrative_order",
        question: "Where do demonstratives sit relative to the noun?",
        options: &[("demonstrative_first", "this book (English)"), ("noun_first", "book this (Indonesian)")],
    },
    GrammarFeature {
        id: "morphological_type",
        question: "Dominant morphological type?",
        options: &[
            ("isolating", "little/no inflection (Mandarin, Vietnamese)"),
            ("agglutinative", "clear morpheme boundaries (Turkish, Finnish)"),
            ("fusional", "portmanteau affixes (Latin, Russian)"),
            ("polysynthetic", "many morphemes per word (Inuktitut)"),
        ],
    },
    GrammarFeature {
        id: "head_marking",
        question: "Is agreement marked on the head or the dependent?",
        options: &[("head_marking", "on the head (Mayan)"), ("dependent_marking", "on the dependent / by case (Latin)"), ("double", "both"), ("none", "neither")],
    },
    GrammarFeature {
        id: "voice",
        question: "Valence-changing voice operations?",
        options: &[("none", "no voice morphology"), ("passive", "a passive"), ("passive_antipassive", "passive + antipassive"), ("rich", "applicatives / causatives / …")],
    },
    GrammarFeature {
        id: "comparative",
        question: "How is the standard of comparison marked?",
        options: &[("particle", "a comparative word (bigger than)"), ("locational", "a case/adposition ('big from Y')"), ("exceed", "a verb 'to exceed' (serial; many African/Asian langs)"), ("conjoined", "conjoined ('X is big, Y is small')")],
    },
];

// ── 1.7 typed grammar blocks ──────────────────────────────────────────────

/// A principles-and-parameters setting.
pub struct UgParameter {
    pub id: &'static str,
    /// The elicitation prompt — catalog metadata for the forthcoming UG-parameter
    /// questionnaire (parallel to `GrammarFeature::question`); read there, not by
    /// the validator.
    #[allow(dead_code)]
    pub question: &'static str,
    /// Valid values (all boolean here, but kept general).
    pub values: &'static [&'static str],
}

impl UgParameter {
    pub fn is_valid(&self, value: &str) -> bool {
        self.values.iter().any(|v| v.eq_ignore_ascii_case(value))
    }
}

/// The recognised UG parameters. `head_final` is cross-checked against the
/// `word_order` / `adposition` features by the grammar validator.
pub const UG_PARAMETERS: &[UgParameter] = &[
    UgParameter {
        id: "head_final",
        question: "Are heads final (the dependent precedes the head)?",
        values: &["true", "false"],
    },
    UgParameter {
        id: "pro_drop",
        question: "May subject pronouns be omitted (pro-drop)?",
        values: &["true", "false"],
    },
    UgParameter {
        id: "wh_movement",
        question: "Do question words move to the clause front?",
        values: &["true", "false"],
    },
    UgParameter {
        id: "configurational",
        question: "Is phrase structure fixed (vs free word order)?",
        values: &["true", "false"],
    },
    UgParameter {
        id: "polysynthesis",
        question: "Does the verb incorporate its arguments (polysynthesis)?",
        values: &["true", "false"],
    },
];

/// Look up a UG parameter by id (case-insensitive).
pub fn ug_parameter(id: &str) -> Option<&'static UgParameter> {
    UG_PARAMETERS.iter().find(|p| p.id.eq_ignore_ascii_case(id))
}

/// Valid verb valences (argument structures).
pub const VERB_VALENCES: &[&str] = &["intransitive", "transitive", "ditransitive", "impersonal"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookup_and_validation() {
        assert_eq!(catalog().len(), 22); // 1.7 L-P3 extended 16 → 22
        let wo = feature("word_order").unwrap();
        assert!(wo.is_valid("SOV")); // case-insensitive
        assert!(!wo.is_valid("backwards"));
        assert!(feature("nonexistent").is_none());
        assert!(wo.values().contains("svo"));
        // The L-P3 additions are present + validate.
        assert!(feature("morphological_type").unwrap().is_valid("agglutinative"));
        assert!(feature("numeral_order").unwrap().is_valid("noun_first"));
    }
}

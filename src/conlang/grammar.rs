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
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookup_and_validation() {
        assert!(catalog().len() >= 12);
        let wo = feature("word_order").unwrap();
        assert!(wo.is_valid("SOV")); // case-insensitive
        assert!(!wo.is_valid("backwards"));
        assert!(feature("nonexistent").is_none());
        assert!(wo.values().contains("svo"));
    }
}

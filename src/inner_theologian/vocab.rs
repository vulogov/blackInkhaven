//! INNER-THEOLOGIAN-1 (IT-P1) — per-language fast-track vocabulary, five
//! languages. Three tradition-neutral lists: lethal/severe **violence**,
//! moral **consequence/acknowledgment**, and **sacred/ritual** vocabulary. All
//! lowercased; matching is case-insensitive. Single tokens match on word
//! boundaries; multi-word entries (e.g. "could not forget") match as substrings.
//!
//! No tradition is weighted more heavily than any other within a language's
//! sacred list — all eleven contribute terms. Patterned on CHAR-1's action-verb
//! lists and DIALOG-1's said-bookism lists (linear scan, not binary search:
//! Unicode-correct sorting of Cyrillic/accented literals is error-prone and the
//! lists are short).

use crate::prose::ProseLanguage;

pub(crate) struct VocabLists {
    pub violence: &'static [&'static str],
    pub consequence: &'static [&'static str],
    pub sacred: &'static [&'static str],
    /// Comic / dismissive context markers — Signal 3 fires when a sacred term
    /// shares a paragraph with one of these (the most cautious detector).
    pub levity: &'static [&'static str],
}

/// The three lists for a language. `Other` falls back to English.
pub(crate) fn lists_for(lang: &ProseLanguage) -> &'static VocabLists {
    match lang {
        ProseLanguage::En => &EN,
        ProseLanguage::Ru => &RU,
        ProseLanguage::De => &DE,
        ProseLanguage::Fr => &FR,
        ProseLanguage::Es => &ES,
        ProseLanguage::Other(_) => &EN,
    }
}

/// Return the first list entry present in `text_lc` (which the caller has
/// already lowercased). A single-token entry matches only on a whole-word
/// boundary; a multi-word entry matches as a substring.
pub(crate) fn scan_list(text_lc: &str, list: &[&'static str]) -> Option<&'static str> {
    for &term in list {
        if term.contains(' ') {
            if text_lc.contains(term) {
                return Some(term);
            }
        } else if contains_word(text_lc, term) {
            return Some(term);
        }
    }
    None
}

/// Whole-word, Unicode-aware containment: `word` appears in `text_lc` delimited
/// by non-alphanumeric boundaries (so "killed" doesn't match "skilled").
fn contains_word(text_lc: &str, word: &str) -> bool {
    text_lc
        .split(|c: char| !c.is_alphanumeric())
        .any(|tok| tok == word)
}

// ── English ───────────────────────────────────────────────────────────────────

static EN: VocabLists = VocabLists {
    violence: &[
        "killed", "murdered", "slain", "executed", "died", "shot", "stabbed", "struck",
        "beaten", "tortured", "destroyed", "massacred", "drowned", "hanged", "burned",
        "starved", "violated", "crushed", "wounded", "severed", "impaled", "strangled",
        "sacrificed", "perished", "slaughtered",
    ],
    consequence: &[
        "grief", "guilt", "mourned", "wept", "anguish", "remorse", "regret", "responsible",
        "reckoned", "consequence", "cost", "burden", "debt", "owed", "atoned", "forgave",
        "forgiven", "acknowledged", "bore", "paid", "answered", "haunted",
        "could not forget", "could not escape",
    ],
    sacred: &[
        "sacred", "holy", "blessed", "divine", "prayer", "ritual", "ceremony", "sacrifice",
        "grace", "sin", "soul", "spirit", "god", "goddess", "temple", "altar", "covenant",
        "revelation", "scripture", "prophet", "imam", "rabbi", "priest", "monk", "baptism",
        "eucharist", "mosque", "synagogue", "enlightenment", "nirvana", "karma", "dharma",
        "moksha", "ahimsa", "gnosis", "pleroma", "demiurge", "theosis", "kenosis",
        "atonement", "redemption", "salvation", "damnation", "purgatory", "paradise",
        "resurrection", "incarnation", "consecrated", "anointed", "eternal", "numinous",
    ],
    levity: &[
        "laughed", "laughing", "joke", "joked", "grinned", "chuckled", "snorted", "smirked",
        "giggled", "mocked", "mockingly", "ridiculous", "absurd", "sarcastic", "sarcastically",
        "ironic", "winked", "teasing", "jest", "silly",
    ],
};

// ── Russian ───────────────────────────────────────────────────────────────────

static RU: VocabLists = VocabLists {
    violence: &[
        "убил", "убила", "убит", "убита", "погиб", "погибла", "умер", "умерла", "расстрелян",
        "зарезал", "задушил", "сжёг", "утопил", "повесил", "разрушил", "уничтожил", "истязал",
        "замучил", "погубил", "растерзал", "пал", "пронзил", "истребил",
    ],
    consequence: &[
        "горе", "вина", "скорбь", "плакал", "плакала", "сожаление", "раскаяние",
        "ответственность", "расплата", "долг", "тяжесть", "бремя", "искупление", "простил",
        "простила", "признал", "признала", "понёс", "понесла", "заплатил",
    ],
    sacred: &[
        "святой", "священный", "молитва", "душа", "бог", "богиня", "храм", "алтарь", "обряд",
        "ритуал", "жертва", "благодать", "грех", "карма", "дхарма", "нирвана", "просветление",
        "сансара", "ахимса", "гнозис", "теозис", "кенозис", "соборность", "покаяние", "мицва",
        "тора", "шаббат", "намаз", "джихад", "спасение", "исповедь", "причастие", "воскресение",
        "пророчество", "откровение", "вечность", "икона", "благословение", "завет", "искупление",
    ],
    levity: &[
        "смеялся", "смеялась", "шутка", "пошутил", "ухмыльнулся", "хихикнул", "усмехнулся",
        "насмешливо", "нелепо", "абсурд", "иронично", "подмигнул", "дразнил", "смешно",
    ],
};

// ── German ────────────────────────────────────────────────────────────────────

static DE: VocabLists = VocabLists {
    violence: &[
        "tötete", "erschoss", "erschlug", "erdrosselte", "ertränkte", "verbrannte",
        "hinrichtete", "starb", "zerstörte", "vernichtete", "marterte", "folterte",
        "schlachtete", "ermordete", "durchbohrte", "opferte", "verhungerte", "ertrank",
        "erhängte", "verstümmelte",
    ],
    consequence: &[
        "trauer", "schuld", "reue", "weinte", "klagte", "verantwortung", "konsequenz", "last",
        "bürde", "sühne", "vergab", "gestand", "trug", "zahlte", "büßte", "anerkannte",
    ],
    sacred: &[
        "heilig", "sakral", "gebet", "seele", "gott", "göttin", "tempel", "altar", "ritus",
        "opfer", "gnade", "sünde", "karma", "dharma", "nirwana", "erleuchtung", "gnosis",
        "theosis", "kenosis", "thora", "namaz", "dschihad", "erlösung", "buße", "beichte",
        "abendmahl", "auferstehung", "prophezeiung", "offenbarung", "ewigkeit", "ikone",
        "weihe", "segen", "fluch", "bund", "sühne",
    ],
    levity: &[
        "lachte", "witz", "scherzte", "grinste", "kicherte", "schmunzelte", "spöttisch",
        "lächerlich", "absurd", "ironisch", "zwinkerte", "neckte", "albern",
    ],
};

// ── French ────────────────────────────────────────────────────────────────────

static FR: VocabLists = VocabLists {
    violence: &[
        "tua", "abattit", "exécuta", "étrangla", "noya", "brûla", "mourut", "détruisit",
        "anéantit", "massacra", "tortura", "poignarda", "pendit", "fusilla", "sacrifia",
        "périt", "affama", "mutila",
    ],
    consequence: &[
        "deuil", "culpabilité", "remords", "pleura", "chagrin", "regret", "responsabilité",
        "conséquence", "poids", "fardeau", "dette", "expiation", "pardonna", "reconnut",
        "porta", "paya", "ne pouvait oublier", "le hantait",
    ],
    sacred: &[
        "sacré", "saint", "prière", "âme", "dieu", "déesse", "temple", "autel", "rite",
        "sacrifice", "grâce", "péché", "karma", "dharma", "nirvana", "éveil", "gnose", "théose",
        "kénose", "tikoun", "torah", "namaz", "djihad", "salut", "pénitence", "confession",
        "eucharistie", "résurrection", "prophétie", "révélation", "éternité", "icône",
        "consécration", "bénédiction", "malédiction", "alliance", "expiation",
    ],
    levity: &[
        "rit", "rire", "blague", "plaisanta", "sourit", "ricana", "gloussa", "moqueur",
        "ridicule", "absurde", "ironique", "cligna", "taquina", "drôle",
    ],
};

// ── Spanish ───────────────────────────────────────────────────────────────────

static ES: VocabLists = VocabLists {
    violence: &[
        "mató", "asesinó", "ejecutó", "estranguló", "ahogó", "quemó", "murió", "destruyó",
        "masacró", "torturó", "apuñaló", "colgó", "fusiló", "sacrificó", "pereció", "mutiló",
        "feneció", "extinguió",
    ],
    consequence: &[
        "duelo", "culpa", "remordimiento", "lloró", "pena", "arrepentimiento", "responsabilidad",
        "consecuencia", "peso", "carga", "deuda", "expiación", "perdonó", "reconoció", "cargó",
        "pagó", "no podía olvidar", "le perseguía",
    ],
    sacred: &[
        "sagrado", "santo", "oración", "alma", "dios", "diosa", "templo", "altar", "rito",
        "sacrificio", "gracia", "pecado", "karma", "dharma", "nirvana", "iluminación", "gnosis",
        "teosis", "kénosis", "tikún", "torá", "namaz", "yihad", "salvación", "penitencia",
        "confesión", "eucaristía", "resurrección", "profecía", "revelación", "eternidad", "icono",
        "consagración", "bendición", "maldición", "alianza", "expiación",
    ],
    levity: &[
        "rió", "risa", "broma", "bromeó", "sonrió", "se burló", "burlonamente", "ridículo",
        "absurdo", "irónico", "guiñó", "bromeando", "gracioso",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_languages_have_three_nonempty_lists() {
        for lang in [
            ProseLanguage::En,
            ProseLanguage::Ru,
            ProseLanguage::De,
            ProseLanguage::Fr,
            ProseLanguage::Es,
        ] {
            let l = lists_for(&lang);
            assert!(l.violence.len() >= 15, "violence too short for {lang:?}");
            assert!(l.consequence.len() >= 14, "consequence too short for {lang:?}");
            assert!(l.sacred.len() >= 30, "sacred too short for {lang:?}");
            assert!(l.levity.len() >= 12, "levity too short for {lang:?}");
            for &w in l.violence.iter().chain(l.consequence).chain(l.sacred).chain(l.levity) {
                assert_eq!(w, w.to_lowercase(), "`{w}` must be lowercase");
            }
        }
    }

    #[test]
    fn other_language_falls_back_to_english() {
        let l = lists_for(&ProseLanguage::Other("pl".into()));
        assert!(scan_list("the soldier killed the man", l.violence).is_some());
    }

    #[test]
    fn whole_word_match_not_substring() {
        let l = lists_for(&ProseLanguage::En);
        // "killed" matches; "skilled" must not.
        assert_eq!(scan_list("he killed the guard", l.violence), Some("killed"));
        assert_eq!(scan_list("a skilled archer", l.violence), None);
    }

    #[test]
    fn multiword_consequence_phrase_matches_as_substring() {
        let l = lists_for(&ProseLanguage::En);
        assert_eq!(scan_list("she could not forget it", l.consequence), Some("could not forget"));
        assert!(scan_list("he felt grief", l.consequence).is_some());
    }

    #[test]
    fn cyrillic_and_accented_match() {
        let ru = lists_for(&ProseLanguage::Ru);
        assert_eq!(scan_list("солдат убил пленного", ru.violence), Some("убил"));
        let es = lists_for(&ProseLanguage::Es);
        assert!(scan_list("el soldado mató al hombre", es.violence).is_some());
        let sacred_fr = lists_for(&ProseLanguage::Fr);
        assert!(scan_list("une prière silencieuse", sacred_fr.sacred).is_some());
    }
}

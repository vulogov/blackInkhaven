//! CHAR-1 — embedded transitive action-verb lists, five languages. High-
//! frequency transitive verbs where subject/object position carries clear
//! agent/patient meaning — the agency score (C-P3) counts a character as
//! *active* when their name precedes one of these. Forms are the dominant
//! narrative tense per language (EN past; RU perfective past, gender pairs; DE
//! Präteritum; FR passé simple; ES pretérito). Matched case-insensitively via a
//! linear scan (~45 entries each). Genre verbs fold in at runtime from
//! `char.extra_action_verbs` (C-P10).

use crate::prose::ProseLanguage;

/// A language's action-verb list.
pub(crate) struct ActionVerbs {
    pub verbs: &'static [&'static str],
}

static EN: ActionVerbs = ActionVerbs {
    verbs: &[
        "attacked", "brought", "built", "carried", "caught", "chose", "convinced", "created",
        "decided", "defeated", "delivered", "destroyed", "directed", "discovered", "drove",
        "escaped", "faced", "fell", "followed", "forced", "found", "gave", "helped", "hit",
        "killed", "led", "left", "lost", "made", "moved", "opened", "placed", "pulled", "pushed",
        "reached", "refused", "released", "rescued", "revealed", "saved", "sent", "showed",
        "stopped", "struck", "took", "told", "turned", "used", "won",
    ],
};

static RU: ActionVerbs = ActionVerbs {
    verbs: &[
        "атаковал", "атаковала", "принёс", "принесла", "построил", "построила", "унёс", "унесла",
        "поймал", "поймала", "выбрал", "выбрала", "убедил", "убедила", "создал", "создала",
        "решил", "решила", "победил", "победила", "разрушил", "разрушила", "обнаружил",
        "обнаружила", "сбежал", "сбежала", "упал", "упала", "вынудил", "вынудила", "нашёл",
        "нашла", "дал", "дала", "помог", "помогла", "ударил", "ударила", "убил", "убила",
        "оставил", "оставила", "сделал", "сделала", "открыл", "открыла", "толкнул", "толкнула",
        "спас", "спасла", "послал", "послала", "показал", "показала", "остановил", "остановила",
        "взял", "взяла", "повернул", "повернула",
    ],
};

static DE: ActionVerbs = ActionVerbs {
    verbs: &[
        "brachte", "baute", "trug", "fing", "wählte", "überzeugte", "erschuf", "entschied",
        "besiegte", "lieferte", "zerstörte", "führte", "entdeckte", "fuhr", "entkam", "begegnete",
        "fiel", "folgte", "zwang", "fand", "gab", "half", "traf", "tötete", "ließ", "verlor",
        "machte", "bewegte", "öffnete", "legte", "zog", "schob", "erreichte", "weigerte",
        "befreite", "rettete", "schickte", "zeigte", "stoppte", "schlug", "nahm", "erzählte",
        "wandte", "benutzte", "gewann",
    ],
};

static FR: ActionVerbs = ActionVerbs {
    verbs: &[
        "attaqua", "apporta", "construisit", "porta", "attrapa", "choisit", "convainquit", "créa",
        "décida", "vainquit", "livra", "détruisit", "dirigea", "découvrit", "conduisit", "affronta",
        "tomba", "suivit", "força", "trouva", "donna", "aida", "frappa", "tua", "mena", "quitta",
        "perdit", "fit", "bougea", "ouvrit", "plaça", "tira", "poussa", "atteignit", "refusa",
        "libéra", "sauva", "envoya", "montra", "arrêta", "prit", "raconta", "tourna", "utilisa",
        "gagna",
    ],
};

static ES: ActionVerbs = ActionVerbs {
    verbs: &[
        "atacó", "trajo", "construyó", "llevó", "atrapó", "eligió", "convenció", "creó", "decidió",
        "venció", "entregó", "destruyó", "dirigió", "descubrió", "condujo", "escapó", "enfrentó",
        "cayó", "siguió", "forzó", "encontró", "dio", "ayudó", "golpeó", "mató", "lideró", "dejó",
        "perdió", "hizo", "movió", "abrió", "colocó", "jaló", "empujó", "alcanzó", "liberó",
        "salvó", "envió", "mostró", "detuvo", "tomó", "contó", "giró", "usó", "ganó",
    ],
};

/// The action-verb list for a language. `Other` falls back to EN (§19).
pub(crate) fn verbs_for(lang: &ProseLanguage) -> &'static ActionVerbs {
    match lang {
        ProseLanguage::En => &EN,
        ProseLanguage::Ru => &RU,
        ProseLanguage::De => &DE,
        ProseLanguage::Fr => &FR,
        ProseLanguage::Es => &ES,
        ProseLanguage::Other(_) => &EN,
    }
}

/// Whether `verb` (any case) is a transitive action verb for this language.
pub(crate) fn is_action_verb(verb: &str, av: &ActionVerbs) -> bool {
    let q = verb.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    av.verbs.iter().any(|&v| v == q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_lowercase_nonempty() {
        for av in [&EN, &RU, &DE, &FR, &ES] {
            assert!(av.verbs.len() >= 40);
            for &v in av.verbs {
                assert_eq!(v, v.to_lowercase(), "`{v}` must be lowercase");
            }
        }
    }

    #[test]
    fn matches_per_language() {
        assert!(is_action_verb("STRUCK", &EN));
        assert!(is_action_verb("killed", &EN));
        assert!(!is_action_verb("was", &EN));
        assert!(is_action_verb("убила", &RU));
        assert!(is_action_verb("tötete", &DE));
        assert!(is_action_verb("frappa", &FR));
        assert!(is_action_verb("mató", &ES));
    }

    #[test]
    fn fallback_language_uses_en() {
        let av = verbs_for(&ProseLanguage::Other("pl".into()));
        assert!(is_action_verb("won", av));
    }
}

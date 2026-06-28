//! NARR-1 — embedded multilingual word lists for the narrative-voice
//! (`prose`) metrics. Pure data: sorted-free `const` arrays grouped per
//! language into a [`Lexicon`]. Lookups are built into hash sets at
//! computation time (see `mod.rs`), so the arrays here can stay readable and
//! unsorted.
//!
//! All entries are stored **lowercase** — tokens are lowercased before lookup,
//! so matching is case-insensitive (German nouns included). Multi-word modal
//! phrases are split into bigram / trigram arrays; the rest are unigrams.
//! Sensory entries are single tokens (a few reflexive verbs are stored by their
//! head word for v1). Lists are representative, curated, and user-extensible via
//! config (`prose.extra_modal_tokens` / `prose.extra_interiority_phrases`).

use super::SensoryChannel;
use SensoryChannel::*;

/// Per-language word lists. Empty slices are valid (e.g. `erlebte_particles`
/// is non-empty only for German).
pub(crate) struct Lexicon {
    pub modal_unigrams: &'static [&'static str],
    pub modal_bigrams: &'static [[&'static str; 2]],
    pub modal_trigrams: &'static [[&'static str; 3]],
    /// Free-indirect-discourse marker phrases (space-joined, lowercase).
    pub interiority: &'static [&'static str],
    /// German *erlebte Rede* modal particles (weaker FID signal). Empty
    /// elsewhere.
    pub erlebte_particles: &'static [&'static str],
    pub sensory: &'static [(&'static str, SensoryChannel)],
    pub passive_exceptions: &'static [&'static str],
}

// ── English ────────────────────────────────────────────────────────────────
const EN: Lexicon = Lexicon {
    modal_unigrams: &[
        "apparently", "appeared", "assumed", "believed", "could", "doubted",
        "expected", "feared", "felt", "hoped", "imagined", "maybe", "might",
        "perhaps", "possibly", "presumably", "probably", "seemed", "should",
        "supposed", "wondered", "would",
    ],
    modal_bigrams: &[],
    modal_trigrams: &[],
    interiority: &[
        "she thought", "he thought", "they thought", "she wondered",
        "he wondered", "they wondered", "she realised", "he realised",
        "they realised", "she knew", "he knew", "they knew", "it seemed to her",
        "it seemed to him", "it seemed to them", "she felt", "he felt",
        "they felt", "she remembered", "he remembered", "they remembered",
        "she decided", "he decided", "they decided", "she noticed",
        "he noticed", "they noticed", "she understood", "he understood",
        "they understood", "she asked herself", "he asked himself",
        "they asked themselves",
    ],
    erlebte_particles: &[],
    sensory: &[
        ("gleam", Visual), ("shadow", Visual), ("pale", Visual), ("dark", Visual),
        ("bright", Visual), ("shape", Visual), ("flicker", Visual),
        ("crimson", Visual), ("dim", Visual), ("glitter", Visual), ("haze", Visual),
        ("shimmer", Visual),
        ("murmur", Auditory), ("crack", Auditory), ("hiss", Auditory),
        ("silence", Auditory), ("rang", Auditory), ("whispered", Auditory),
        ("thunder", Auditory), ("faint", Auditory), ("rustle", Auditory),
        ("clang", Auditory), ("echo", Auditory),
        ("scent", Olfactory), ("smoke", Olfactory), ("damp", Olfactory),
        ("metallic", Olfactory), ("sweetness", Olfactory), ("rot", Olfactory),
        ("reek", Olfactory), ("fragrance", Olfactory), ("stench", Olfactory),
        ("musty", Olfactory),
        ("rough", Tactile), ("cold", Tactile), ("pressure", Tactile),
        ("smooth", Tactile), ("sting", Tactile), ("weight", Tactile),
        ("soft", Tactile), ("grate", Tactile), ("warm", Tactile),
        ("prickle", Tactile), ("numb", Tactile),
        ("surge", Kinesthetic), ("pull", Kinesthetic), ("tremor", Kinesthetic),
        ("spin", Kinesthetic), ("plunge", Kinesthetic), ("sway", Kinesthetic),
        ("rush", Kinesthetic), ("stumble", Kinesthetic), ("heave", Kinesthetic),
        ("lurch", Kinesthetic), ("drift", Kinesthetic),
    ],
    passive_exceptions: &[
        "held", "kept", "built", "caught", "taught", "brought", "sought",
        "bought", "thought", "found", "bound", "wound", "lost", "cost", "sent",
        "meant", "left", "felt", "dealt", "cut", "hit", "put", "set", "led",
        "fed", "bred", "fled", "sped", "shed", "spread", "read", "heard",
    ],
};

// ── Russian ──────────────────────────────────────────────────────────────────
const RU: Lexicon = Lexicon {
    modal_unigrams: &[
        "будто", "вероятно", "видимо", "возможно", "едва", "казалось",
        "мерещилось", "наверное", "наверняка", "ощущалось", "по-видимому",
        "пожалуй", "похоже", "почудилось", "предположительно",
        "представлялось", "словно", "чудилось",
    ],
    modal_bigrams: &[
        ["должно", "быть"], ["едва", "ли"], ["казалось", "бы"], ["как", "будто"],
        ["как", "бы"],
    ],
    modal_trigrams: &[["судя", "по", "всему"]],
    interiority: &[
        "она подумала", "он подумал", "они подумали", "она почувствовала",
        "он почувствовал", "она вспомнила", "он вспомнил", "ей казалось",
        "ему казалось", "она знала", "он знал", "она поняла", "он понял",
        "она решила", "он решил", "она заметила", "он заметил", "ей показалось",
        "ему показалось", "она задалась вопросом", "он задался вопросом",
    ],
    erlebte_particles: &[],
    sensory: &[
        ("мерцал", Visual), ("тень", Visual), ("бледный", Visual),
        ("тёмный", Visual), ("яркий", Visual), ("силуэт", Visual),
        ("мелькнул", Visual), ("багровый", Visual), ("тусклый", Visual),
        ("шёпот", Auditory), ("треск", Auditory), ("шипение", Auditory),
        ("тишина", Auditory), ("звенел", Auditory), ("гром", Auditory),
        ("шорох", Auditory), ("лязг", Auditory), ("эхо", Auditory),
        ("запах", Olfactory), ("дым", Olfactory), ("сырость", Olfactory),
        ("металлический", Olfactory), ("сладость", Olfactory), ("гниль", Olfactory),
        ("зловоние", Olfactory), ("аромат", Olfactory),
        ("шершавый", Tactile), ("холодный", Tactile), ("давление", Tactile),
        ("гладкий", Tactile), ("жжение", Tactile), ("вес", Tactile),
        ("мягкий", Tactile), ("тепло", Tactile),
        ("рывок", Kinesthetic), ("тяга", Kinesthetic), ("дрожь", Kinesthetic),
        ("кружился", Kinesthetic), ("нырнул", Kinesthetic),
        ("покачнулся", Kinesthetic), ("ринулся", Kinesthetic),
        ("споткнулся", Kinesthetic),
    ],
    // Reflexive-only verbs that are not passive (excluded from -ся/-сь passive
    // heuristic). Stored lowercase.
    passive_exceptions: &[
        "казалось", "чудилось", "нравилось", "хотелось", "смеялся",
        "улыбалась", "улыбался", "смеялась", "боялся", "боялась",
        "осталось", "осталась", "остался", "находился", "находилась",
        "оказалось", "оказался", "оказалась", "случилось", "проснулся",
        "проснулась", "обернулся", "обернулась", "вернулся", "вернулась",
    ],
};

// ── German ───────────────────────────────────────────────────────────────────
const DE: Lexicon = Lexicon {
    modal_unigrams: &[
        "angeblich", "anscheinend", "augenscheinlich", "dürfte", "dürften",
        "eigentlich", "etwa", "gewissermaßen", "gleichsam", "hätte", "hätten",
        "könnte", "könnten", "möchte", "möglicherweise", "müsste", "müssten",
        "offenbar", "offensichtlich", "scheinbar", "scheinen", "schien",
        "schienen", "sollte", "sollten", "vermutlich", "vielleicht",
        "wahrscheinlich", "wohl", "würde", "würden",
    ],
    modal_bigrams: &[],
    modal_trigrams: &[],
    interiority: &[
        "sie dachte", "er dachte", "sie fragte sich", "er fragte sich",
        "sie wusste", "er wusste", "sie spürte", "er spürte",
        "sie erinnerte sich", "er erinnerte sich", "sie bemerkte", "er bemerkte",
        "sie verstand", "er verstand", "sie beschloss", "er beschloss",
    ],
    erlebte_particles: &[
        "ja", "doch", "wohl", "eigentlich", "bloß", "nur", "schon", "eben",
        "halt", "denn",
    ],
    sensory: &[
        ("glitzerte", Visual), ("schatten", Visual), ("blass", Visual),
        ("dunkel", Visual), ("hell", Visual), ("umriss", Visual),
        ("flackerte", Visual), ("purpurn", Visual), ("trüb", Visual),
        ("flüstern", Auditory), ("knacken", Auditory), ("zischen", Auditory),
        ("stille", Auditory), ("schellte", Auditory), ("donner", Auditory),
        ("rascheln", Auditory), ("hallen", Auditory),
        ("geruch", Olfactory), ("rauch", Olfactory), ("feuchtigkeit", Olfactory),
        ("metallisch", Olfactory), ("süße", Olfactory), ("fäulnis", Olfactory),
        ("gestank", Olfactory), ("duft", Olfactory),
        ("rau", Tactile), ("kalt", Tactile), ("druck", Tactile),
        ("glatt", Tactile), ("brennen", Tactile), ("gewicht", Tactile),
        ("weich", Tactile), ("warm", Tactile), ("prickeln", Tactile),
        ("taub", Tactile),
        ("ruck", Kinesthetic), ("zug", Kinesthetic), ("zittern", Kinesthetic),
        ("drehte", Kinesthetic), ("tauchte", Kinesthetic),
        ("schwankte", Kinesthetic), ("stürzte", Kinesthetic),
        ("taumelte", Kinesthetic),
    ],
    // sein + adjective/noun collocations that are not Zustandspassiv (stored as
    // head words for the heuristic short-circuit).
    passive_exceptions: &["klar", "bekannt", "bereit", "fertig", "möglich", "nötig"],
};

// ── French ───────────────────────────────────────────────────────────────────
const FR: Lexicon = Lexicon {
    modal_unigrams: &[
        "apparemment", "certes", "dirait-on", "paraissait", "paraît-il",
        "probablement", "semblaient", "semblait", "sembler", "semble-t-il",
        "supposément", "vraisemblablement",
    ],
    modal_bigrams: &[
        ["à", "priori"], ["comme", "si"], ["en", "apparence"], ["il", "paraît"],
        ["il", "semblait"], ["sans", "doute"],
    ],
    modal_trigrams: &[
        ["il", "semblait", "que"], ["on", "aurait", "dit"],
        ["selon", "toute", "apparence"],
    ],
    interiority: &[
        "elle pensait", "il pensait", "elle se demandait", "il se demandait",
        "elle savait", "il savait", "elle se souvint", "il se souvint",
        "elle remarqua", "il remarqua", "elle comprit", "il comprit",
        "elle décida", "il décida", "elle sentait", "il sentait",
        "elle se dit", "il se dit",
    ],
    erlebte_particles: &[],
    sensory: &[
        ("brillait", Visual), ("ombre", Visual), ("pâle", Visual),
        ("sombre", Visual), ("lumineux", Visual), ("silhouette", Visual),
        ("vacillait", Visual), ("cramoisi", Visual),
        ("murmure", Auditory), ("craquement", Auditory), ("sifflement", Auditory),
        ("silence", Auditory), ("retentit", Auditory), ("tonnerre", Auditory),
        ("bruissement", Auditory),
        ("odeur", Olfactory), ("fumée", Olfactory), ("humidité", Olfactory),
        ("métallique", Olfactory), ("douceur", Olfactory), ("pourriture", Olfactory),
        ("pestilence", Olfactory), ("parfum", Olfactory),
        ("rugueux", Tactile), ("froid", Tactile), ("pression", Tactile),
        ("lisse", Tactile), ("brûlure", Tactile), ("poids", Tactile),
        ("doux", Tactile), ("chaud", Tactile), ("picotement", Tactile),
        ("élan", Kinesthetic), ("traction", Kinesthetic), ("tremblement", Kinesthetic),
        ("tournait", Kinesthetic), ("plongea", Kinesthetic), ("vacilla", Kinesthetic),
        ("précipita", Kinesthetic),
    ],
    passive_exceptions: &[],
};

// ── Spanish ──────────────────────────────────────────────────────────────────
const ES: Lexicon = Lexicon {
    modal_unigrams: &[
        "acaso", "aparentemente", "debería", "parecía", "parecían", "podría",
        "podrían", "posiblemente", "probablemente", "quizá", "quizás",
        "seguramente", "supuestamente",
    ],
    modal_bigrams: &[
        ["al", "parecer"], ["como", "si"], ["debía", "de"], ["en", "apariencia"],
        ["parecía", "que"], ["según", "parece"], ["tal", "vez"],
    ],
    modal_trigrams: &[["a", "lo", "mejor"]],
    interiority: &[
        "ella pensaba", "él pensaba", "ella se preguntaba", "él se preguntaba",
        "ella sabía", "él sabía", "ella recordó", "él recordó", "ella notó",
        "él notó", "ella entendió", "él entendió", "ella decidió", "él decidió",
        "ella sintió", "él sintió", "ella se dijo", "él se dijo",
    ],
    erlebte_particles: &[],
    sensory: &[
        ("brillaba", Visual), ("sombra", Visual), ("pálido", Visual),
        ("oscuro", Visual), ("luminoso", Visual), ("silueta", Visual),
        ("parpadeó", Visual), ("carmesí", Visual),
        ("murmullo", Auditory), ("crujido", Auditory), ("siseo", Auditory),
        ("silencio", Auditory), ("resonó", Auditory), ("trueno", Auditory),
        ("susurro", Auditory), ("eco", Auditory),
        ("olor", Olfactory), ("humo", Olfactory), ("humedad", Olfactory),
        ("metálico", Olfactory), ("dulzura", Olfactory), ("podredumbre", Olfactory),
        ("hedor", Olfactory), ("fragancia", Olfactory),
        ("áspero", Tactile), ("frío", Tactile), ("presión", Tactile),
        ("suave", Tactile), ("ardor", Tactile), ("peso", Tactile),
        ("tibio", Tactile), ("hormigueo", Tactile), ("entumecido", Tactile),
        ("impulso", Kinesthetic), ("tirón", Kinesthetic), ("temblor", Kinesthetic),
        ("giró", Kinesthetic), ("hundió", Kinesthetic), ("tambaleó", Kinesthetic),
        ("lanzó", Kinesthetic), ("tropezó", Kinesthetic),
    ],
    passive_exceptions: &[],
};

/// The embedded lexicon for `lang`. `Other` falls back to the English lists for
/// the language-sensitive metrics (rhythm metrics don't consult the lexicon).
pub(crate) fn lexicon(lang: &super::ProseLanguage) -> &'static Lexicon {
    use super::ProseLanguage::*;
    match lang {
        En => &EN,
        Ru => &RU,
        De => &DE,
        Fr => &FR,
        Es => &ES,
        Other(_) => &EN,
    }
}

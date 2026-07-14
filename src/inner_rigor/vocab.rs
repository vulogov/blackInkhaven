//! Reasoning-rigor reader — the multilingual cue-word tables and the whole-word
//! matcher, modelled on `inner_theologian::vocab`. Each rigor category is detected
//! by marker phrases the prose language provides; the advisory descriptions are
//! localized too. Lists are deliberately *conservative* (strong, rarely-innocent
//! cues) because these are advisory signals, not verdicts — a false positive is a
//! wasted glance, but noise erodes trust in the reader.

use crate::prose::ProseLanguage;

/// One language's cue lists. Multi-word / hyphenated entries match as a substring;
/// single tokens match as a whole word (Unicode-aware).
pub(super) struct RigorLists {
    /// Correlative "either … or" pairs — flagged when the first appears before the
    /// second. `("либо","либо")` matches two occurrences.
    pub either_or: &'static [(&'static str, &'static str)],
    /// Forced-binary phrasings ("the only alternative", "one or the other").
    pub dichotomy: &'static [&'static str],
    /// Assertion boosters that assert without arguing ("obviously", "of course").
    pub assertion: &'static [&'static str],
    /// Dismissive characterizations of a view ("so-called", "would have us believe").
    pub straw_man: &'static [&'static str],
    /// Strong absolutes ("always", "never", "without exception") — not the common
    /// "all"/"every", which are too innocent to flag.
    pub overgeneralization: &'static [&'static str],
    /// Conclusion connectives ("therefore", "thus", "hence").
    pub conclusion: &'static [&'static str],
    /// Warrant / reason markers ("because", "since") — their *absence* beside a
    /// conclusion connective is the non-sequitur signal.
    pub warrant: &'static [&'static str],
}

/// One language's advisory sentence per category. `{cue}` is replaced with the
/// matched marker (which is in the prose language).
pub(super) struct RigorText {
    pub false_dichotomy: &'static str,
    pub question_begging: &'static str,
    pub straw_man: &'static str,
    pub overgeneralization: &'static str,
    pub non_sequitur: &'static str,
    pub equivocation: &'static str,
}

pub(super) fn lists_for(lang: &ProseLanguage) -> &'static RigorLists {
    match lang {
        ProseLanguage::Ru => &RU,
        ProseLanguage::De => &DE,
        ProseLanguage::Fr => &FR,
        ProseLanguage::Es => &ES,
        ProseLanguage::En | ProseLanguage::Other(_) => &EN,
    }
}

pub(super) fn text_for(lang: &ProseLanguage) -> &'static RigorText {
    match lang {
        ProseLanguage::Ru => &RU_TEXT,
        ProseLanguage::De => &DE_TEXT,
        ProseLanguage::Fr => &FR_TEXT,
        ProseLanguage::Es => &ES_TEXT,
        ProseLanguage::En | ProseLanguage::Other(_) => &EN_TEXT,
    }
}

/// The first list entry that occurs in `text_lc` (already lowercased). Multi-word
/// or hyphenated entries match as a substring; single tokens as a whole word.
pub(super) fn matches_any(text_lc: &str, list: &[&'static str]) -> Option<&'static str> {
    for &entry in list {
        let hit = if entry.contains(' ') || entry.contains('-') || entry.contains('\'') {
            text_lc.contains(entry)
        } else {
            crate::world::fact_check_lang::contains_word(text_lc, entry)
        };
        if hit {
            return Some(entry);
        }
    }
    None
}

/// The first correlative pair whose parts both occur in order (`a` before a
/// distinct later `b`) — returns a display cue like `either…or`.
pub(super) fn matches_pair(text_lc: &str, pairs: &[(&'static str, &'static str)]) -> Option<String> {
    for &(a, b) in pairs {
        if let Some(i) = word_pos(text_lc, a, 0) {
            if word_pos(text_lc, b, i + a.len()).is_some() {
                return Some(format!("{a}…{b}"));
            }
        }
    }
    None
}

/// The number of whole-word occurrences of `needle` in `text_lc` (both already
/// lowercased). Multi-word needles match as a bounded substring.
pub(super) fn count_word(text_lc: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut n = 0;
    let mut from = 0;
    while let Some(pos) = word_pos(text_lc, needle, from) {
        n += 1;
        from = pos + text_lc[pos..].chars().next().map_or(1, |c| c.len_utf8());
    }
    n
}

/// The byte offset of the first whole-word `needle` in `hay` at/after `from`.
fn word_pos(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(rel) = hay.get(at..)?.find(needle) {
        let start = at + rel;
        let end = start + needle.len();
        let before_ok = hay[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after_ok = hay[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return Some(start);
        }
        at = start + hay[start..].chars().next().map_or(1, |c| c.len_utf8());
    }
    None
}

// ── English ─────────────────────────────────────────────────────────────────
static EN: RigorLists = RigorLists {
    either_or: &[("either", "or")],
    dichotomy: &[
        "the only alternative", "the only choice", "the only option",
        "the only possibility", "there are only two", "one or the other",
        "must be one or the other",
    ],
    assertion: &[
        "obviously", "clearly", "undeniably", "self-evidently", "of course",
        "needless to say", "it goes without saying", "everyone knows",
        "no one can deny", "plainly", "surely",
    ],
    straw_man: &[
        "so-called", "supposedly", "would have us believe", "claims to",
        "pretends to", "purports to", "naive", "simplistic", "allegedly",
        "nothing more than",
    ],
    overgeneralization: &[
        "always", "never", "everyone", "no one", "nobody", "everything",
        "nothing", "without exception", "invariably", "universally", "in every case",
    ],
    conclusion: &[
        "therefore", "thus", "hence", "consequently", "it follows that",
        "which proves", "this shows that", "so it must",
    ],
    warrant: &[
        "because", "since", "given that", "on the grounds", "follows from",
        "for the reason", "inasmuch as", "due to the fact",
    ],
};
static EN_TEXT: RigorText = RigorText {
    false_dichotomy: "Presented as an exhaustive binary (\"{cue}\") — is there a third alternative you have ruled out without argument?",
    question_begging: "Asserted as self-evident (\"{cue}\") — is the claim argued, or is it assumed to spare the argument?",
    straw_man: "Characterizes a view dismissively (\"{cue}\") — are you answering its strongest form, or a weaker one?",
    overgeneralization: "A universal claim (\"{cue}\") — would a single counterexample break it? If so, qualify it.",
    non_sequitur: "A conclusion is drawn (\"{cue}\") with no visible warrant in the paragraph — what supports the inference?",
    equivocation: "The term \"{cue}\" recurs here and carries more than one declared sense — is it used in one sense throughout, or has the argument shifted between them?",
};

// ── Russian ─────────────────────────────────────────────────────────────────
static RU: RigorLists = RigorLists {
    either_or: &[("либо", "либо")],
    dichotomy: &[
        "единственный выбор", "единственная альтернатива", "только два",
        "одно из двух", "или то, или другое", "нет иного выбора",
    ],
    assertion: &[
        "очевидно", "разумеется", "конечно", "несомненно", "само собой",
        "всем известно", "бесспорно", "естественно",
    ],
    straw_man: &[
        "так называемый", "якобы", "будто бы", "наивный", "упрощённый",
        "притворяется", "выдаёт за",
    ],
    overgeneralization: &[
        "всегда", "никогда", "никто", "все без исключения", "каждый раз",
        "ничто", "поголовно", "во всех случаях",
    ],
    conclusion: &[
        "следовательно", "поэтому", "таким образом", "значит",
        "отсюда следует", "тем самым",
    ],
    warrant: &[
        "потому что", "поскольку", "так как", "ибо", "на основании",
        "в силу того",
    ],
};
static RU_TEXT: RigorText = RigorText {
    false_dichotomy: "Подано как исчерпывающая дилемма («{cue}») — есть ли третий вариант, отвергнутый без обоснования?",
    question_begging: "Утверждается как самоочевидное («{cue}») — обосновано ли это или лишь предполагается?",
    straw_man: "Взгляд подан пренебрежительно («{cue}») — возражаете ли вы его сильнейшей форме?",
    overgeneralization: "Универсальное утверждение («{cue}») — не опровергнет ли его один контрпример? Уточните его.",
    non_sequitur: "Вывод сделан («{cue}»), но в абзаце нет видимого основания — что поддерживает этот переход?",
    equivocation: "Термин «{cue}» повторяется здесь и имеет несколько заявленных значений — используется ли он в одном значении, или довод сместился между ними?",
};

// ── German ──────────────────────────────────────────────────────────────────
static DE: RigorLists = RigorLists {
    either_or: &[("entweder", "oder")],
    dichotomy: &[
        "die einzige alternative", "die einzige wahl", "die einzige möglichkeit",
        "eins von beiden", "nur zwei", "das eine oder das andere",
    ],
    assertion: &[
        "offensichtlich", "natürlich", "selbstverständlich", "zweifellos",
        "jeder weiß", "es versteht sich", "gewiss",
    ],
    straw_man: &[
        "sogenannt", "angeblich", "vermeintlich", "naiv", "simpel",
        "will uns glauben machen", "vorgeblich",
    ],
    overgeneralization: &[
        "immer", "nie", "niemand", "ausnahmslos", "jedes mal", "ständig",
        "ohne ausnahme", "in jedem fall",
    ],
    conclusion: &[
        "folglich", "somit", "daher", "deshalb", "was beweist", "demnach",
    ],
    warrant: &["weil", "da", "denn", "aufgrund", "angesichts", "aus dem grund"],
};
static DE_TEXT: RigorText = RigorText {
    false_dichotomy: "Als erschöpfende Alternative dargestellt (\"{cue}\") — gibt es eine dritte Möglichkeit, die Sie ohne Begründung ausgeschlossen haben?",
    question_begging: "Als selbstverständlich behauptet (\"{cue}\") — ist es begründet oder nur vorausgesetzt?",
    straw_man: "Eine Sicht wird abschätzig charakterisiert (\"{cue}\") — antworten Sie ihrer stärksten Form?",
    overgeneralization: "Eine universelle Behauptung (\"{cue}\") — würde ein einziges Gegenbeispiel sie widerlegen? Dann relativieren Sie sie.",
    non_sequitur: "Eine Schlussfolgerung wird gezogen (\"{cue}\"), ohne erkennbaren Grund im Absatz — was stützt den Schluss?",
    equivocation: "Der Begriff \"{cue}\" wiederholt sich hier und trägt mehrere erklärte Bedeutungen — wird er durchgehend in einem Sinn gebraucht, oder ist das Argument zwischen ihnen gewechselt?",
};

// ── French ──────────────────────────────────────────────────────────────────
static FR: RigorLists = RigorLists {
    either_or: &[("soit", "soit")],
    dichotomy: &[
        "la seule alternative", "le seul choix", "la seule option",
        "l'un ou l'autre", "seulement deux", "il n'y a que deux",
    ],
    assertion: &[
        "évidemment", "bien sûr", "manifestement", "indéniablement",
        "il va de soi", "tout le monde sait", "de toute évidence", "assurément",
    ],
    straw_man: &[
        "soi-disant", "prétendument", "prétend", "naïf", "simpliste",
        "voudrait nous faire croire", "prétendu",
    ],
    overgeneralization: &[
        "toujours", "jamais", "personne", "sans exception", "chaque fois",
        "invariablement", "en tout temps", "dans tous les cas",
    ],
    conclusion: &[
        "donc", "par conséquent", "ainsi", "il s'ensuit", "ce qui prouve",
        "dès lors",
    ],
    warrant: &["parce que", "puisque", "car", "étant donné", "en raison de", "du fait que"],
};
static FR_TEXT: RigorText = RigorText {
    false_dichotomy: "Présenté comme une alternative exhaustive (« {cue} ») — y a-t-il une troisième possibilité écartée sans argument ?",
    question_begging: "Affirmé comme évident (« {cue} ») — est-ce argumenté ou simplement supposé ?",
    straw_man: "Une thèse est caractérisée avec dédain (« {cue} ») — répondez-vous à sa forme la plus forte ?",
    overgeneralization: "Une affirmation universelle (« {cue} ») — un seul contre-exemple la briserait-il ? Alors nuancez-la.",
    non_sequitur: "Une conclusion est tirée (« {cue} ») sans justification visible dans le paragraphe — qu'est-ce qui soutient l'inférence ?",
    equivocation: "Le terme « {cue} » revient ici et porte plusieurs sens déclarés — est-il employé dans un seul sens, ou l'argument a-t-il glissé entre eux ?",
};

// ── Spanish ─────────────────────────────────────────────────────────────────
static ES: RigorLists = RigorLists {
    either_or: &[],
    dichotomy: &[
        "la única alternativa", "la única opción", "la única elección",
        "uno u otro", "solo dos", "o lo uno o lo otro",
    ],
    assertion: &[
        "obviamente", "por supuesto", "claramente", "evidentemente",
        "sin duda", "todo el mundo sabe", "es evidente", "naturalmente",
    ],
    straw_man: &[
        "llamado", "supuestamente", "pretende", "ingenuo", "simplista",
        "nos haría creer", "presuntamente",
    ],
    overgeneralization: &[
        "siempre", "nunca", "nadie", "sin excepción", "cada vez",
        "invariablemente", "en todo momento", "en todos los casos",
    ],
    conclusion: &[
        "por lo tanto", "así pues", "por consiguiente", "de ahí que",
        "lo que demuestra", "en consecuencia",
    ],
    warrant: &["porque", "puesto que", "ya que", "dado que", "debido a", "en virtud de"],
};
static ES_TEXT: RigorText = RigorText {
    false_dichotomy: "Presentado como una alternativa exhaustiva (\"{cue}\") — ¿hay una tercera posibilidad descartada sin argumento?",
    question_begging: "Afirmado como evidente (\"{cue}\") — ¿está argumentado o simplemente supuesto?",
    straw_man: "Una postura se caracteriza con desdén (\"{cue}\") — ¿respondes a su forma más fuerte?",
    overgeneralization: "Una afirmación universal (\"{cue}\") — ¿la rompería un solo contraejemplo? Entonces matízala.",
    non_sequitur: "Se saca una conclusión (\"{cue}\") sin justificación visible en el párrafo — ¿qué sostiene la inferencia?",
    equivocation: "El término \"{cue}\" reaparece aquí y tiene varios sentidos declarados — ¿se usa en un solo sentido, o el argumento se ha deslizado entre ellos?",
};

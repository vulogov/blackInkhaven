//! Multilingual support for the Fast track — per-language marker tables for the
//! vocabulary-dependent categories (modal claims, hedging, dialogue attribution)
//! and localized question text for every category. Inherits WORLD-4's `Lang` +
//! `detect_with_confidence`: a paragraph is read in its detected language, and an
//! uncertain detection degrades to English rather than guessing (the same
//! discipline as the fact-checker).
//!
//! The structural / length categories are language-agnostic (they read sentence
//! shape, not words); only their *question text* is localized here.

pub use crate::world::fact_check_lang::Lang;

/// The vocabulary markers for one language's Fast track.
pub struct LangMarkers {
    /// Asserted inevitability (strong).
    pub modal_strong: &'static [&'static str],
    /// Asserted-as-given (moderate).
    pub modal_moderate: &'static [&'static str],
    /// Nearby words that defuse a modal claim (a conditional / hedge).
    pub modal_defuse: &'static [&'static str],
    /// Authorial hedging.
    pub hedge: &'static [&'static str],
    /// Dialogue attribution verbs (a tagged speaker).
    pub attribution: &'static [&'static str],
}

/// The marker table for a language.
pub fn markers(lang: Lang) -> LangMarkers {
    match lang {
        Lang::En => EN,
        Lang::Ru => RU,
        Lang::Es => ES,
        Lang::Fr => FR,
        Lang::De => DE,
    }
}

const EN: LangMarkers = LangMarkers {
    modal_strong: &["must", "had to", "couldn't help but", "inevitably", "no choice", "no other choice"],
    modal_moderate: &["certainly", "obviously", "naturally", "surely", "of course"],
    modal_defuse: &["if", "unless", "perhaps", "maybe", "might", "could have"],
    hedge: &["perhaps", "might have", "seemed to", "as if", "somehow", "apparently", "may have"],
    attribution: &[
        "said", "asked", "replied", "whispered", "shouted", "muttered", "answered", "cried",
        "called", "added", "continued", "murmured", "growled", "snapped", "demanded", "breathed",
    ],
};

const RU: LangMarkers = LangMarkers {
    modal_strong: &["должен", "должна", "должно", "должны", "обязан", "обязана", "неизбежно", "не мог не", "не могла не"],
    modal_moderate: &["конечно", "разумеется", "естественно", "безусловно", "очевидно"],
    modal_defuse: &["если", "разве", "может быть", "возможно", "мог бы", "могла бы"],
    hedge: &["возможно", "кажется", "казалось", "как будто", "словно", "по-видимому", "наверное"],
    attribution: &[
        "сказал", "сказала", "спросил", "спросила", "ответил", "ответила", "прошептал",
        "прошептала", "крикнул", "пробормотал", "пробормотала", "добавил",
    ],
};

const ES: LangMarkers = LangMarkers {
    modal_strong: &["debía", "tenía que", "no podía evitar", "inevitablemente", "no tenía más remedio", "no tuvo elección"],
    modal_moderate: &["ciertamente", "obviamente", "naturalmente", "por supuesto", "claro está"],
    modal_defuse: &["si", "a menos que", "quizá", "tal vez", "podría", "acaso"],
    hedge: &["quizá", "tal vez", "parecía", "como si", "de algún modo", "al parecer", "acaso"],
    attribution: &["dijo", "preguntó", "respondió", "susurró", "gritó", "murmuró", "contestó", "añadió"],
};

const FR: LangMarkers = LangMarkers {
    modal_strong: &["devait", "fallait", "ne pouvait s'empêcher", "inévitablement", "n'avait pas le choix", "forcément"],
    modal_moderate: &["certainement", "évidemment", "naturellement", "bien sûr", "assurément"],
    modal_defuse: &["si", "à moins que", "peut-être", "pourrait", "sauf si"],
    hedge: &["peut-être", "semblait", "comme si", "apparemment", "sans doute", "on aurait dit"],
    attribution: &["dit", "demanda", "répondit", "murmura", "cria", "ajouta", "reprit", "souffla"],
};

const DE: LangMarkers = LangMarkers {
    modal_strong: &["musste", "konnte nicht anders", "unweigerlich", "keine wahl", "zwangsläufig"],
    modal_moderate: &["gewiss", "offensichtlich", "natürlich", "sicherlich", "selbstverständlich"],
    modal_defuse: &["wenn", "falls", "vielleicht", "könnte", "es sei denn"],
    hedge: &["vielleicht", "schien", "als ob", "irgendwie", "anscheinend", "womöglich"],
    attribution: &["sagte", "fragte", "antwortete", "flüsterte", "rief", "murmelte", "erwiderte"],
};

/// A localizable Fast-track question. Rendered into the paragraph's language for
/// the finding's `question`, and into English for the `question_en` fallback.
pub enum Msg<'a> {
    ModalStrong(&'a str),
    ModalModerate(&'a str),
    Hedge(&'a str),
    Anaphora(&'a str),
    Monotone,
    LongSentence(usize),
    UnattributedDialogue(usize),
    /// Detection is English-only (no UD parser); rendered in English in every
    /// language until per-language tense/coreference tables land.
    TenseShift,
    PronounAmbiguity,
}

/// English text for the two parser-adjacent categories (detection is English-only
/// for now, so these render in English in every language).
const TENSE_EN: &str =
    "The passage moves between past and present tense here. Is the shift deliberate?";
const PRONOUN_EN: &str =
    "A pronoun here could point to more than one person named just before. Is the reference clear?";

/// Render a question in the given language.
pub fn render(msg: &Msg, lang: Lang) -> String {
    match lang {
        Lang::En => render_en(msg),
        Lang::Ru => render_ru(msg),
        Lang::Es => render_es(msg),
        Lang::Fr => render_fr(msg),
        Lang::De => render_de(msg),
    }
}

fn render_en(m: &Msg) -> String {
    match m {
        Msg::ModalStrong(w) => format!("This passage treats an outcome as inevitable (\u{201c}{w}\u{201d}). What alternatives did you decide to leave out?"),
        Msg::ModalModerate(w) => format!("The prose asserts this as given (\u{201c}{w}\u{201d}). Is that certainty the narrator\u{2019}s, or a character\u{2019}s?"),
        Msg::Hedge(w) => format!("The prose hedges here (\u{201c}{w}\u{201d}). Is the uncertainty the character\u{2019}s, or the telling\u{2019}s?"),
        Msg::Anaphora(w) => format!("Several sentences here open with \u{201c}{w}\u{201d}. Is the repetition a deliberate cadence?"),
        Msg::Monotone => "A run of sentences here are near-identical in length. Is the even rhythm intended?".into(),
        Msg::LongSentence(n) => format!("One sentence here runs to {n} words. Is its length carrying the reader, or losing them?"),
        Msg::UnattributedDialogue(n) => format!("{n} lines of dialogue pass here without a speaker tag. Can the reader still tell who is speaking?"),
        Msg::TenseShift => TENSE_EN.into(),
        Msg::PronounAmbiguity => PRONOUN_EN.into(),
    }
}

fn render_ru(m: &Msg) -> String {
    match m {
        Msg::ModalStrong(w) => format!("Здесь исход подан как неизбежный (\u{00ab}{w}\u{00bb}). Какие альтернативы вы решили не показывать?"),
        Msg::ModalModerate(w) => format!("Это подано как данность (\u{00ab}{w}\u{00bb}). Чья это уверенность — рассказчика или персонажа?"),
        Msg::Hedge(w) => format!("Здесь повествование осторожничает (\u{00ab}{w}\u{00bb}). Чья это неуверенность — персонажа или самого рассказа?"),
        Msg::Anaphora(w) => format!("Несколько предложений подряд начинаются со слова \u{00ab}{w}\u{00bb}. Это намеренный ритм?"),
        Msg::Monotone => "Несколько предложений здесь почти одинаковой длины. Ровный ритм задуман нарочно?".into(),
        Msg::LongSentence(n) => format!("Одно предложение здесь — {n} слов. Его длина ведёт читателя или теряет его?"),
        Msg::UnattributedDialogue(n) => format!("{n} реплик(и) проходят без указания говорящего. Читатель ещё понимает, кто говорит?"),
        Msg::TenseShift => TENSE_EN.into(),
        Msg::PronounAmbiguity => PRONOUN_EN.into(),
    }
}

fn render_es(m: &Msg) -> String {
    match m {
        Msg::ModalStrong(w) => format!("Aquí el desenlace se presenta como inevitable (\u{00ab}{w}\u{00bb}). ¿Qué alternativas decidiste dejar fuera?"),
        Msg::ModalModerate(w) => format!("Esto se da por sentado (\u{00ab}{w}\u{00bb}). ¿Esa certeza es del narrador o de un personaje?"),
        Msg::Hedge(w) => format!("Aquí la prosa se muestra cautelosa (\u{00ab}{w}\u{00bb}). ¿La incertidumbre es del personaje o del relato?"),
        Msg::Anaphora(w) => format!("Varias frases seguidas empiezan con \u{00ab}{w}\u{00bb}. ¿Es una cadencia deliberada?"),
        Msg::Monotone => "Varias frases aquí tienen una longitud casi idéntica. ¿El ritmo uniforme es intencionado?".into(),
        Msg::LongSentence(n) => format!("Una frase aquí llega a {n} palabras. ¿Su longitud lleva al lector o lo pierde?"),
        Msg::UnattributedDialogue(n) => format!("{n} líneas de diálogo pasan sin indicar quién habla. ¿El lector aún sabe quién habla?"),
        Msg::TenseShift => TENSE_EN.into(),
        Msg::PronounAmbiguity => PRONOUN_EN.into(),
    }
}

fn render_fr(m: &Msg) -> String {
    match m {
        Msg::ModalStrong(w) => format!("Ici l\u{2019}issue est présentée comme inévitable (\u{00ab}{w}\u{00bb}). Quelles possibilités avez-vous choisi d\u{2019}écarter ?"),
        Msg::ModalModerate(w) => format!("Cela est posé comme une évidence (\u{00ab}{w}\u{00bb}). Cette certitude est-elle celle du narrateur ou d\u{2019}un personnage ?"),
        Msg::Hedge(w) => format!("Ici le récit reste prudent (\u{00ab}{w}\u{00bb}). L\u{2019}incertitude est-elle celle du personnage ou du récit ?"),
        Msg::Anaphora(w) => format!("Plusieurs phrases de suite commencent par \u{00ab}{w}\u{00bb}. Est-ce une cadence voulue ?"),
        Msg::Monotone => "Plusieurs phrases ont ici une longueur presque identique. Ce rythme régulier est-il voulu ?".into(),
        Msg::LongSentence(n) => format!("Une phrase ici fait {n} mots. Sa longueur porte-t-elle le lecteur ou le perd-elle ?"),
        Msg::UnattributedDialogue(n) => format!("{n} répliques passent ici sans indiquer qui parle. Le lecteur sait-il encore qui parle ?"),
        Msg::TenseShift => TENSE_EN.into(),
        Msg::PronounAmbiguity => PRONOUN_EN.into(),
    }
}

fn render_de(m: &Msg) -> String {
    match m {
        Msg::ModalStrong(w) => format!("Hier wird ein Ausgang als unausweichlich dargestellt (\u{201e}{w}\u{201c}). Welche Alternativen haben Sie weggelassen?"),
        Msg::ModalModerate(w) => format!("Das wird als gegeben gesetzt (\u{201e}{w}\u{201c}). Ist diese Gewissheit die des Erzählers oder einer Figur?"),
        Msg::Hedge(w) => format!("Hier bleibt die Prosa vorsichtig (\u{201e}{w}\u{201c}). Ist die Unsicherheit die der Figur oder die des Erzählens?"),
        Msg::Anaphora(w) => format!("Mehrere Sätze beginnen hier mit \u{201e}{w}\u{201c}. Ist die Wiederholung ein bewusster Rhythmus?"),
        Msg::Monotone => "Mehrere Sätze sind hier fast gleich lang. Ist der gleichmäßige Rhythmus beabsichtigt?".into(),
        Msg::LongSentence(n) => format!("Ein Satz hier umfasst {n} Wörter. Trägt seine Länge den Leser, oder verliert sie ihn?"),
        Msg::UnattributedDialogue(n) => format!("{n} Dialogzeilen kommen hier ohne Sprecherangabe aus. Weiß der Leser noch, wer spricht?"),
        Msg::TenseShift => TENSE_EN.into(),
        Msg::PronounAmbiguity => PRONOUN_EN.into(),
    }
}

/// The language name (for the Slow track's "respond in …" directive).
pub fn language_name(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "English",
        Lang::Ru => "Russian",
        Lang::Es => "Spanish",
        Lang::Fr => "French",
        Lang::De => "German",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_markers_and_renders() {
        for lang in [Lang::En, Lang::Ru, Lang::Es, Lang::Fr, Lang::De] {
            let m = markers(lang);
            assert!(!m.modal_strong.is_empty());
            assert!(!m.hedge.is_empty());
            assert!(!m.attribution.is_empty());
            let q = render(&Msg::ModalStrong("X"), lang);
            assert!(q.contains('X'));
            assert!(q.ends_with('?'), "{lang:?}: {q}");
        }
    }

    #[test]
    fn english_and_russian_differ() {
        let en = render(&Msg::Monotone, Lang::En);
        let ru = render(&Msg::Monotone, Lang::Ru);
        assert_ne!(en, ru);
        assert!(ru.chars().any(|c| ('а'..='я').contains(&c)));
    }
}

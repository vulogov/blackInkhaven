//! WBLD-1 (WB-P8) — the guided world interview.
//!
//! A fixed six-stage script (World · Sky · Land · People · Rules · Review) that walks
//! the author from an empty project to a first coherent frame. Each step is a
//! plain question whose answer fills a **shaping-command template** — the same
//! `/star`, `/tilt`, `/set …` commands the author could type by hand (WB-P4) — so
//! every recorded delta goes through the one tested `Op` engine and is checked
//! against the schema as it is recorded. The interview holds no logic beyond the
//! script and a cursor; the app parses each answer and accumulates the ops into
//! the pending delta, so the ★ score moves live and the author reviews everything
//! at the end with `/diff` before `/write`. It never generates prose: it only asks
//! and records.
//!
//! WORLD-KEEP-1 (WK-P4) — the interview speaks the **project language**
//! (en / ru / fr / de / es; anything else falls back to English). Every prompt,
//! stage label and conversational line has all five; the shaping commands accept
//! the matching answer words (`orange` / `оранжевая` / `naranja`, `yes` / `да` /
//! `oui`, `ancient` / `древние` / `uralt`), so an author is never asked a
//! question in one language and required to answer in another.

use crate::prose::ProseLanguage;

/// The interview's language — one of the five the project supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lang {
    En,
    Ru,
    Fr,
    De,
    Es,
}

impl Lang {
    pub(super) fn of(language: &ProseLanguage) -> Lang {
        match language {
            ProseLanguage::Ru => Lang::Ru,
            ProseLanguage::Fr => Lang::Fr,
            ProseLanguage::De => Lang::De,
            ProseLanguage::Es => Lang::Es,
            _ => Lang::En,
        }
    }

    fn idx(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::Ru => 1,
            Lang::Fr => 2,
            Lang::De => 3,
            Lang::Es => 4,
        }
    }
}

/// A string in all five languages, in [`Lang`] order (en, ru, fr, de, es).
type L5 = [&'static str; 5];

/// The five question stages (Review is the closing summary, not a step).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// The world's identity — its name is a REQUIRED `world.hjson` field.
    World,
    Sky,
    Land,
    People,
    Rules,
}

impl Stage {
    pub(super) fn label(self, lang: Lang) -> &'static str {
        let l: L5 = match self {
            Stage::World => ["World", "Мир", "Monde", "Welt", "Mundo"],
            Stage::Sky => ["Sky", "Небо", "Ciel", "Himmel", "Cielo"],
            Stage::Land => ["Land", "Земля", "Terre", "Land", "Tierra"],
            Stage::People => ["People", "Народы", "Peuples", "Völker", "Pueblos"],
            Stage::Rules => ["Rules", "Законы", "Règles", "Regeln", "Reglas"],
        };
        l[lang.idx()]
    }
}

/// One interview question. `template` is a shaping-command with a single `{}`
/// placeholder the answer is substituted into (e.g. `"/star {}"`).
pub(super) struct Step {
    pub stage: Stage,
    prompts: L5,
    pub template: &'static str,
}

impl Step {
    pub(super) fn prompt(&self, lang: Lang) -> &'static str {
        self.prompts[lang.idx()]
    }
}

/// The interview script. Ordered by stage; every template is a shaping command,
/// so answers produce `world.hjson` edits the schema check then confirms.
static SCRIPT: &[Step] = &[
    Step {
        stage: Stage::World,
        prompts: [
            "What is the world called?",
            "Как называется мир?",
            "Comment s'appelle le monde ?",
            "Wie heißt die Welt?",
            "¿Cómo se llama el mundo?",
        ],
        template: "/set name {}",
    },
    Step {
        stage: Stage::Sky,
        prompts: [
            "What kind of star? (G Sun-like · K orange · M red dwarf)",
            "Какая звезда? (G — как Солнце · K — оранжевая · M — красный карлик)",
            "Quelle étoile ? (G semblable au Soleil · K orange · M naine rouge)",
            "Was für ein Stern? (G sonnenähnlich · K orange · M roter Zwerg)",
            "¿Qué tipo de estrella? (G similar al Sol · K naranja · M enana roja)",
        ],
        template: "/star {}",
    },
    Step {
        stage: Stage::Sky,
        prompts: [
            "Axial tilt in degrees? (Earth 23.4 — higher means harsher seasons)",
            "Наклон оси в градусах? (у Земли 23,4 — чем больше, тем резче времена года)",
            "Inclinaison de l'axe en degrés ? (Terre 23,4 — plus elle est forte, plus les saisons sont marquées)",
            "Achsneigung in Grad? (Erde 23,4 — je größer, desto härter die Jahreszeiten)",
            "¿Inclinación del eje en grados? (Tierra 23,4 — cuanto mayor, más duras las estaciones)",
        ],
        template: "/tilt {}",
    },
    Step {
        stage: Stage::Sky,
        prompts: [
            "Add a moon? (name, optional orbital period in Earth-days — blank to skip)",
            "Добавить луну? (имя и, по желанию, период обращения в земных сутках — пусто, чтобы пропустить)",
            "Ajouter une lune ? (nom, puis sa période orbitale en jours terrestres si vous voulez — vide pour passer)",
            "Einen Mond hinzufügen? (Name, optional die Umlaufzeit in Erdtagen — leer zum Überspringen)",
            "¿Añadir una luna? (nombre y, si quieres, su periodo orbital en días terrestres — vacío para omitir)",
        ],
        template: "/moon {}",
    },
    Step {
        stage: Stage::Land,
        prompts: [
            "How many continents? (e.g. 3)",
            "Сколько континентов? (например, 3)",
            "Combien de continents ? (par ex. 3)",
            "Wie viele Kontinente? (z. B. 3)",
            "¿Cuántos continentes? (p. ej. 3)",
        ],
        template: "/set geology.generated.continents {}",
    },
    Step {
        stage: Stage::Land,
        prompts: [
            "Sea level, 0..1? (Earth ≈ 0.6 — higher means more ocean)",
            "Уровень моря, от 0 до 1? (у Земли ≈ 0,6 — чем выше, тем больше океана)",
            "Niveau de la mer, de 0 à 1 ? (Terre ≈ 0,6 — plus haut, plus d'océan)",
            "Meeresspiegel, 0 bis 1? (Erde ≈ 0,6 — je höher, desto mehr Ozean)",
            "¿Nivel del mar, de 0 a 1? (Tierra ≈ 0,6 — cuanto más alto, más océano)",
        ],
        template: "/set geology.generated.sea_level {}",
    },
    Step {
        stage: Stage::Land,
        prompts: [
            "Mountains — active, quiet, or ancient?",
            "Горы — активные, спокойные или древние?",
            "Montagnes — actives, calmes ou anciennes ?",
            "Gebirge — aktiv, ruhig oder uralt?",
            "Montañas: ¿activas, tranquilas o antiguas?",
        ],
        template: "/orogeny {}",
    },
    Step {
        stage: Stage::People,
        prompts: [
            "Primary language? (e.g. English)",
            "Основной язык мира? (например, русский)",
            "Langue principale ? (par ex. français)",
            "Hauptsprache? (z. B. Deutsch)",
            "¿Idioma principal? (p. ej. español)",
        ],
        template: "/set primary_language {}",
    },
    Step {
        stage: Stage::People,
        prompts: [
            "Name a nation? (name, optionally its capital cell x y — blank to skip)",
            "Назвать государство? (название и, по желанию, клетка столицы x y — пусто, чтобы пропустить)",
            "Nommer une nation ? (nom, puis la case de sa capitale x y si vous voulez — vide pour passer)",
            "Eine Nation benennen? (Name, optional die Hauptstadtzelle x y — leer zum Überspringen)",
            "¿Nombrar una nación? (nombre y, si quieres, la celda de su capital x y — vacío para omitir)",
        ],
        template: "/nation {}",
    },
    Step {
        stage: Stage::Rules,
        prompts: [
            "Is there magic in this world? (yes/no)",
            "Есть ли в этом мире магия? (да/нет)",
            "Y a-t-il de la magie dans ce monde ? (oui/non)",
            "Gibt es Magie in dieser Welt? (ja/nein)",
            "¿Hay magia en este mundo? (sí/no)",
        ],
        template: "/set magic.enabled {}",
    },
];

/// The interview's conversational lines (everything it says that is not a question).
#[derive(Debug, Clone, Copy)]
pub(super) enum Line {
    /// The opening turn.
    Opening,
    /// A blank answer.
    Skipped,
    /// Prefix of the confirmation after a recorded answer.
    Recorded,
    /// Prefix of the refusal when an answer could not be recorded.
    NotTaken,
    /// The closing turn; `{n}` is the pending-edit count.
    Closing,
}

pub(super) fn line(lang: Lang, which: Line) -> &'static str {
    let l: L5 = match which {
        Line::Opening => [
            "Interview — I'll ask about the world, its sky, land, people, and rules. Answer in your own words (blank to skip a question, Esc to leave). Your answers become pending edits; review them with /diff and commit with /write, then /compile.",
            "Интервью — я спрошу о мире, его небе, земле, народах и законах. Отвечайте своими словами (пустая строка — пропустить вопрос, Esc — выйти). Ответы становятся отложенными правками: просмотрите их через /diff, запишите через /write, затем /compile.",
            "Entretien — je vous interroge sur le monde, son ciel, sa terre, ses peuples et ses règles. Répondez avec vos mots (ligne vide pour passer une question, Échap pour quitter). Vos réponses deviennent des modifications en attente : relisez-les avec /diff, validez avec /write, puis /compile.",
            "Interview — ich frage nach der Welt, ihrem Himmel, ihrem Land, ihren Völkern und ihren Regeln. Antworten Sie in eigenen Worten (leere Zeile überspringt eine Frage, Esc beendet). Ihre Antworten werden zu ausstehenden Änderungen: mit /diff prüfen, mit /write übernehmen, dann /compile.",
            "Entrevista: te preguntaré por el mundo, su cielo, su tierra, sus pueblos y sus reglas. Responde con tus palabras (línea vacía para omitir una pregunta, Esc para salir). Tus respuestas quedan como cambios pendientes: revísalos con /diff, confírmalos con /write y luego /compile.",
        ],
        Line::Skipped => ["(skipped)", "(пропущено)", "(passé)", "(übersprungen)", "(omitido)"],
        Line::Recorded => ["recorded", "записано", "noté", "notiert", "anotado"],
        Line::NotTaken => [
            "didn't take that",
            "не удалось принять ответ",
            "réponse non retenue",
            "das ging nicht",
            "no se pudo anotar",
        ],
        Line::Closing => [
            "That's the frame — {n} pending edit(s). Review with /diff, commit with /write, then /compile to see the world your choices imply.",
            "Каркас готов — отложенных правок: {n}. Просмотрите их через /diff, запишите через /write, затем /compile покажет мир, который следует из ваших решений.",
            "Voilà le cadre — {n} modification(s) en attente. Relisez-les avec /diff, validez avec /write, puis /compile pour voir le monde qu'impliquent vos choix.",
            "Das ist der Rahmen — {n} ausstehende Änderung(en). Mit /diff prüfen, mit /write übernehmen, dann /compile, um die Welt zu sehen, die aus Ihren Entscheidungen folgt.",
            "Ese es el marco: {n} cambio(s) pendiente(s). Revísalos con /diff, confírmalos con /write y luego /compile para ver el mundo que implican tus decisiones.",
        ],
    };
    l[lang.idx()]
}

/// The interview cursor over [`SCRIPT`].
pub(super) struct Interview {
    pos: usize,
    lang: Lang,
}

impl Interview {
    pub(super) fn new(lang: Lang) -> Interview {
        Interview { pos: 0, lang }
    }

    pub(super) fn lang(&self) -> Lang {
        self.lang
    }

    /// The step awaiting an answer, or `None` once the script is exhausted.
    pub(super) fn current(&self) -> Option<&'static Step> {
        SCRIPT.get(self.pos)
    }

    /// Move to the next question.
    pub(super) fn advance(&mut self) {
        self.pos += 1;
    }

    pub(super) fn done(&self) -> bool {
        self.pos >= SCRIPT.len()
    }

    /// `(current 1-based index, total)` for the progress banner.
    pub(super) fn progress(&self) -> (usize, usize) {
        (self.pos + 1, SCRIPT.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Lang; 5] = [Lang::En, Lang::Ru, Lang::Fr, Lang::De, Lang::Es];

    #[test]
    fn every_template_has_exactly_one_placeholder_and_is_a_command() {
        for step in SCRIPT {
            assert_eq!(
                step.template.matches("{}").count(),
                1,
                "step `{}` must have one placeholder",
                step.prompt(Lang::En)
            );
            assert!(step.template.starts_with('/'), "template must be a /command");
        }
    }

    #[test]
    fn cursor_walks_the_whole_script_then_reports_done() {
        let mut iv = Interview::new(Lang::En);
        assert_eq!(iv.progress(), (1, SCRIPT.len()));
        assert_eq!(iv.current().unwrap().stage, Stage::World);
        for _ in 0..SCRIPT.len() {
            assert!(!iv.done());
            iv.advance();
        }
        assert!(iv.done());
        assert!(iv.current().is_none());
    }

    #[test]
    fn stages_appear_in_world_sky_land_people_rules_order() {
        let mut last = 0usize;
        let order = |s: Stage| match s {
            Stage::World => 0,
            Stage::Sky => 1,
            Stage::Land => 2,
            Stage::People => 3,
            Stage::Rules => 4,
        };
        for step in SCRIPT {
            let o = order(step.stage);
            assert!(o >= last, "stages must be non-decreasing");
            last = o;
        }
    }

    #[test]
    fn every_prompt_label_and_line_exists_in_all_five_languages() {
        for step in SCRIPT {
            for lang in ALL {
                assert!(!step.prompt(lang).trim().is_empty());
                assert!(step.prompt(lang).contains('?'), "a question: {}", step.prompt(lang));
            }
            // Four real translations, none a copy of the English.
            for lang in &ALL[1..] {
                assert_ne!(step.prompt(*lang), step.prompt(Lang::En));
            }
        }
        for which in [Line::Opening, Line::Skipped, Line::Recorded, Line::NotTaken, Line::Closing] {
            for lang in ALL {
                assert!(!line(lang, which).is_empty());
            }
        }
        for lang in ALL {
            assert!(line(lang, Line::Closing).contains("{n}"), "the closing line carries the count");
            assert!(line(lang, Line::Opening).contains("/write"));
        }
        assert_eq!(Lang::of(&crate::prose::ProseLanguage::Ru), Lang::Ru);
        assert_eq!(Lang::of(&crate::prose::ProseLanguage::Other("ja".into())), Lang::En);
    }

    /// The answers a prompt invites, in its own language, must be accepted by
    /// the command the step runs — the whole point of translating the questions.
    #[test]
    fn localised_answer_words_are_accepted_by_the_shaping_commands() {
        use crate::worldbuilder::commands::{parse, Command};
        let shape = |line: &str| matches!(parse(line), Command::Shape { .. });
        for star in ["orange", "оранжевая", "красный карлик", "naine rouge", "roter Zwerg", "enana roja", "как Солнце", "К", "naranja"] {
            assert!(shape(&format!("/star {star}")), "/star {star}");
        }
        for o in ["ancient", "древние", "anciennes", "uralt", "antiguas", "спокойные", "ruhig", "actives"] {
            assert!(shape(&format!("/orogeny {o}")), "/orogeny {o}");
        }
        assert!(!shape("/orogeny purple"));
        for tilt in ["23.4", "23,4"] {
            assert!(shape(&format!("/tilt {tilt}")), "/tilt {tilt}");
        }
    }
}

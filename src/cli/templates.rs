//! 1.2.14+ Phase Q.1 — project templates for
//! `inkhaven init --template <name>`.
//!
//! Templates are pure data — embedded constants
//! describing the user-book structure + system-
//! book seed entries the template scaffolds on
//! top of the standard init machinery.  Walked
//! after the standard init returns so a template
//! that fails partway through still leaves a
//! functional empty project behind.
//!
//! Six templates ship:
//!
//! | Name | Use case |
//! |------|----------|
//! | `empty` | default — no extra scaffolding |
//! | `novel` | three-act manuscript + character stubs |
//! | `nonfiction` | intro/parts/conclusion + research methodology |
//! | `rpg-sourcebook` | setting/rules/adventures/appendices + worldbuilding seeds |
//! | `technical` | overview/reference/tutorials/index |
//! | `nanowrimo` | like `novel` with a 50K-word target |
//!
//! `inkhaven template list` enumerates the same
//! set with descriptions for at-the-terminal
//! reference.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store};

/// one project template.
/// Captures the book structure + system-book seed
/// entries the template adds on top of the
/// standard init scaffolding.
#[derive(Debug, Clone, Copy)]
pub struct ProjectTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub manuscript_book: Option<ManuscriptBook>,
    pub seeds: &'static [SystemBookSeed],
    /// Plain-text guidance printed after init
    /// completes — typically the recommended
    /// `project.word_count_goal` and target-date
    /// pacing.  Multi-line; printed verbatim.
    pub post_init_message: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ManuscriptBook {
    /// Display title for the user book the
    /// template creates (e.g. `"Manuscript"`,
    /// `"Sourcebook"`).
    pub title: &'static str,
    /// Chapters created under the book in
    /// canonical order.
    pub chapters: &'static [&'static str],
    /// Optional content-type override for every
    /// paragraph created under this book (e.g.
    /// `"markdown"` for the technical template).
    /// `None` keeps the default Typst.  Reserved
    /// for Q.1.1 — chapter scaffolding currently
    /// inherits the standard content-type.
    #[allow(dead_code)]
    pub paragraph_content_type: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct SystemBookSeed {
    /// System tag of the book that gets the seed
    /// paragraphs (e.g. `"characters"`, `"places"`,
    /// `"threads"`).
    pub system_tag: &'static str,
    /// (paragraph_title, body) tuples.  Empty body
    /// keeps `create_node`'s `= Title\n\n`
    /// skeleton; non-empty body overwrites.
    pub paragraphs: &'static [(&'static str, &'static str)],
}

/// Every template the CLI knows about.  Add new
/// templates here; the registry is consulted by
/// both `apply()` and `list_templates()`.
pub const TEMPLATES: &[ProjectTemplate] = &[
    EMPTY,
    NOVEL,
    NONFICTION,
    RPG_SOURCEBOOK,
    TECHNICAL,
    NANOWRIMO,
    // Russian-literature templates.  Each carries
    // genre-authentic chapter structure +
    // Russian-language book / chapter / seed-
    // paragraph titles.  Post-init message
    // recommends setting `language: "russian"` in
    // inkhaven.hjson so the Snowball stemmer +
    // multilingual prompt resolver flip to
    // Russian.
    RUSSIAN_NOVEL,
    RUSSIAN_LONG_STORY,
    RUSSIAN_SCIFI,
    RUSSIAN_LORE,
    RUSSIAN_UTOPIA,
    // Additional genre templates beyond the
    // mainstream `novel`.
    EPIC_FANTASY,
    MYSTERY,
    FRENCH_NOVEL,
];

pub const EMPTY: ProjectTemplate = ProjectTemplate {
    name: "empty",
    description:
        "no extra scaffolding — system books only.  The current default \
         for hand-authored projects.",
    manuscript_book: None,
    seeds: &[],
    post_init_message: "",
};

pub const NOVEL: ProjectTemplate = ProjectTemplate {
    name: "novel",
    description:
        "three-act manuscript book (Act I / II / III) + Characters \
         seeded with protagonist / antagonist / confidant stubs.  \
         Recommended word-count goal: 80000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Act I — Setup",
            "Act II — Confrontation",
            "Act III — Resolution",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "characters",
        paragraphs: &[
            (
                "protagonist",
                "= protagonist\n\n\
                 The character whose arc the manuscript follows.\n\n\
                 // Edit this paragraph to capture: voice, want,\n\
                 // need, internal conflict, defining scenes.\n",
            ),
            (
                "antagonist",
                "= antagonist\n\n\
                 The force opposing the protagonist's want / need.\n\n\
                 // Doesn't have to be a person — could be a system,\n\
                 // an institution, a part of the protagonist's own\n\
                 // psyche.\n",
            ),
            (
                "confidant",
                "= confidant\n\n\
                 The character the protagonist confides in — and\n\
                 through whom the reader hears the protagonist's\n\
                 internal monologue made external.\n",
            ),
        ],
    }],
    post_init_message:
        "Recommended next steps:\n  \
         · Open the Manuscript book and start Act I\n  \
         · Edit Characters/protagonist (etc.) to capture voice + arc\n  \
         · Set `project.word_count_goal: 80000` in inkhaven.hjson \
            (Ctrl+V Shift+G shows the projection modal)\n",
};

pub const NONFICTION: ProjectTemplate = ProjectTemplate {
    name: "nonfiction",
    description:
        "manuscript with Introduction / Part I / Part II / \
         Conclusion chapters + Research book seeded with a \
         methodology paragraph.  Recommended word-count goal: \
         60000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &["Introduction", "Part I", "Part II", "Conclusion"],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "research",
        paragraphs: &[(
            "methodology",
            "= methodology\n\n\
             How the research feeding this manuscript was conducted:\n\
             sources consulted, interviews held, archival visits,\n\
             criteria for inclusion / exclusion.\n\n\
             // Drives reviewer trust + makes a reproducibility\n\
             // statement easy to assemble when the manuscript ships.\n",
        )],
    }],
    post_init_message:
        "Recommended next steps:\n  \
         · Outline Introduction → state thesis, scope, audience\n  \
         · Edit Research/methodology before adding citation paragraphs\n  \
         · Set `project.word_count_goal: 60000` in inkhaven.hjson\n",
};

pub const RPG_SOURCEBOOK: ProjectTemplate = ProjectTemplate {
    name: "rpg-sourcebook",
    description:
        "Setting / Rules / Adventures / Appendices chapters + \
         Places / Artefacts / Threads seeded with one example \
         each.  Recommended word-count goal: 120000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Sourcebook",
        chapters: &["Setting", "Rules", "Adventures", "Appendices"],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[(
                "example-locale",
                "= example-locale\n\n\
                 A starter Place entry.  Rename or duplicate as your\n\
                 setting grows.\n\n\
                 // Place entries light up in manuscript prose when\n\
                 // mentioned (cyan overlay via the lexicon walker).\n",
            )],
        },
        SystemBookSeed {
            system_tag: "artefacts",
            paragraphs: &[(
                "example-artefact",
                "= example-artefact\n\n\
                 A starter Artefact entry — for named items, magical\n\
                 objects, signature equipment, plot-bearing macguffins.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[(
                "example-arc",
                "{\n  \
                 title:         \"example-arc\"\n  \
                 status:        \"setup\"\n  \
                 weight:        \"major\"\n  \
                 opening:       \"What kicks the arc off — fill in.\"\n  \
                 midpoint:      \"\"\n  \
                 payoff:        \"\"\n  \
                 characters:    []\n  \
                 places:        []\n  \
                 artefacts:     []\n  \
                 related_threads: []\n  \
                 tension:       0\n  \
                 register:      \"\"\n  \
                 notes:         \"Starter Threads entry — see \
                 `inkhaven thread add` for the CLI shortcut.\"\n\
                 }\n",
            )],
        },
    ],
    post_init_message:
        "Recommended next steps:\n  \
         · Setting chapter first — establish geography + cosmology\n  \
         · Rules chapter — system + mechanics; use HJSON paragraphs\n   \
            for character classes / spells / monsters\n  \
         · Threads/example-arc — fill in (Ctrl+V Shift+H lists threads)\n  \
         · Set `project.word_count_goal: 120000`\n",
};

pub const TECHNICAL: ProjectTemplate = ProjectTemplate {
    name: "technical",
    description:
        "Overview / Reference / Tutorials / Index chapters.  No \
         word-count goal default (technical docs are bounded by \
         topic coverage, not length).",
    manuscript_book: Some(ManuscriptBook {
        title: "Documentation",
        chapters: &["Overview", "Reference", "Tutorials", "Index"],
        paragraph_content_type: None,
    }),
    seeds: &[],
    post_init_message:
        "Recommended next steps:\n  \
         · Overview/getting-started — what the system does, who for\n  \
         · Reference chapter — one paragraph per concept / API\n  \
         · Tutorials chapter — narrative, paragraph per task\n",
};

pub const NANOWRIMO: ProjectTemplate = ProjectTemplate {
    name: "nanowrimo",
    description:
        "NaNoWriMo manuscript scaffolding.  Same structure as \
         `novel` but with a 50000-word goal + recommended \
         1667-words/day pacing.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Act I — Setup",
            "Act II — Confrontation",
            "Act III — Resolution",
        ],
        paragraph_content_type: None,
    }),
    seeds: NOVEL.seeds,
    post_init_message:
        "NaNoWriMo target: 50000 words by month-end.\n  \
         · 1667 words / day for 30 days\n  \
         · Set `project.word_count_goal: 50000` in inkhaven.hjson\n  \
         · Set `project.target_date: \"2026-11-30\"` (adjust to your year)\n  \
         · Daily streak heatmap: Ctrl+B Shift+G\n",
};

// ──────────────────────────────────────────────────
// 1.2.14+ Phase D.5 — Russian-literature templates.
//
// Genre conventions researched from canonical
// works:
//
// * Russian novel (роман) — Tolstoy / Dostoyevsky
//   tradition uses `Часть Первая` / `Часть Вторая` /
//   `Часть Третья` + `Эпилог`.  Word-count range
//   80K–500K+; we recommend 100K as a sensible
//   anchor (matches Чехов's smaller novels rather
//   than `Война и мир`'s scale).
//
// * Russian long story (повесть) — Pushkin / Bunin /
//   Gogol tradition.  Single-arc, 5–8 numbered
//   chapters in Roman numerals (I, II, III…), short
//   epilogue, frame narrative common.  Word-count
//   range 20K–50K.
//
// * Russian sci-fi (научная фантастика) — Strugatsky /
//   Belyaev / Lukyanenko tradition.  `Пролог` + 2–3
//   parts + `Эпилог` + `Глоссарий` for invented
//   terms.  Word-count anchor 80K.
//
// * Russian lore (мифология) — былины / collection
//   structure.  Sections per category
//   (`Происхождение мира` / `Боги` / `Герои` /
//   `Чудовища` / `Мифы и сказания`).  Standalone
//   tales, not continuous narrative.
//
// * Russian utopia (утопия) — Чернышевский
//   `Что делать?` / Богданов `Красная звезда`
//   tradition.  Frame narrative (`Прибытие`), then
//   topic-organised chapters per aspect of society.
//
// All five templates pre-seed character / place /
// thread names in Russian (Cyrillic) and recommend
// setting `language: "russian"` in the project
// HJSON so the Snowball stemmer + multilingual
// prompt resolver flip to Russian.
// ──────────────────────────────────────────────────

pub const RUSSIAN_NOVEL: ProjectTemplate = ProjectTemplate {
    name: "russian-novel",
    description:
        "Русский роман.  Three-act `Часть Первая` / `Часть Вторая` / `Часть Третья` + \
         `Эпилог` (Tolstoy / Dostoyevsky tradition).  Seeds Characters with \
         главный герой / антагонист / наперсник stubs.  Recommended goal 100000 \
         words; set `language: \"russian\"` in inkhaven.hjson.",
    manuscript_book: Some(ManuscriptBook {
        title: "Рукопись",
        chapters: &[
            "Часть Первая",
            "Часть Вторая",
            "Часть Третья",
            "Эпилог",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "characters",
        paragraphs: &[
            (
                "главный герой",
                "= главный герой\n\n\
                 Персонаж, чью внутреннюю и внешнюю траекторию проходит весь\n\
                 роман.\n\n\
                 // Заполните: голос, желание (внешняя цель), потребность\n\
                 // (внутренняя цель), внутренний конфликт, поворотные сцены.\n",
            ),
            (
                "антагонист",
                "= антагонист\n\n\
                 Сила, противостоящая желанию или потребности главного героя.\n\n\
                 // Не обязательно человек — это может быть система, институт,\n\
                 // эпоха или часть психики самого героя.\n",
            ),
            (
                "наперсник",
                "= наперсник\n\n\
                 Персонаж, которому главный герой доверяет внутренний\n\
                 монолог — устами наперсника читатель слышит то, что иначе\n\
                 осталось бы за кадром.\n",
            ),
        ],
    }],
    post_init_message:
        "Рекомендуемые следующие шаги:\n  \
         · Откройте `Рукопись/Часть Первая` и начните завязку\n  \
         · Заполните `Characters/главный герой` (голос, желание, потребность)\n  \
         · Установите `language: \"russian\"` в inkhaven.hjson (стеммер + многоязычные промпты)\n  \
         · Установите `project.word_count_goal: 100000` (роман по умолчанию)\n",
};

pub const RUSSIAN_LONG_STORY: ProjectTemplate = ProjectTemplate {
    name: "russian-long-story",
    description:
        "Русская повесть.  Single-arc, 7-chapter scaffolding (I / II / III / IV / V / VI / VII) + \
         `Эпилог` (Pushkin / Bunin / Gogol tradition).  Recommended goal 35000 words.",
    manuscript_book: Some(ManuscriptBook {
        title: "Повесть",
        chapters: &[
            "I",
            "II",
            "III",
            "IV",
            "V",
            "VI",
            "VII",
            "Эпилог",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "characters",
        paragraphs: &[(
            "главный герой",
            "= главный герой\n\n\
             Повесть традиционно сосредоточена на одном персонаже и одной\n\
             линии: внутреннее изменение, не масштабная фабула.\n\n\
             // Заполните: голос, внутренний слом, рамочная подача\n\
             // (рассказчик-свидетель или сам герой).\n",
        )],
    }],
    post_init_message:
        "Рекомендуемые следующие шаги:\n  \
         · Подумайте над рамкой: рассказчик-свидетель или сам герой\n  \
         · Откройте `Повесть/I` и установите тон + место действия\n  \
         · Установите `language: \"russian\"` в inkhaven.hjson\n  \
         · Установите `project.word_count_goal: 35000` (типичный объём повести)\n",
};

pub const RUSSIAN_SCIFI: ProjectTemplate = ProjectTemplate {
    name: "russian-scifi",
    description:
        "Русская научная фантастика.  `Пролог` + three parts + `Эпилог` + `Глоссарий` \
         (Strugatsky / Belyaev tradition).  Pre-seeds Places + Artefacts with \
         genre stubs.  Recommended goal 80000 words.",
    manuscript_book: Some(ManuscriptBook {
        title: "Научная фантастика",
        chapters: &[
            "Пролог",
            "Часть Первая: Земля",
            "Часть Вторая: Полёт",
            "Часть Третья: Звёзды",
            "Эпилог",
            "Глоссарий",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[
                (
                    "звёздная база",
                    "= звёздная база\n\n\
                     Опорный пункт, к которому возвращаются герои.\n\n\
                     // Заполните: расположение (система, год основания), \n\
                     // население, режим (научный / военный / колониальный).\n",
                ),
                (
                    "колония",
                    "= колония\n\n\
                     Поселение на чужой планете — место конфликта между\n\
                     старым (Земля) и новым (среда).\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "artefacts",
            paragraphs: &[(
                "артефакт",
                "= артефакт\n\n\
                 Предмет, чья природа двигает фабулу: реликвия исчезнувшей\n\
                 цивилизации, прототип технологии, символ власти.\n",
            )],
        },
    ],
    post_init_message:
        "Рекомендуемые следующие шаги:\n  \
         · Пролог: введите ключевую концепцию мира одним сценарным стрелком\n  \
         · Глоссарий: добавляйте по мере появления изобретённых терминов\n  \
         · Установите `language: \"russian\"` в inkhaven.hjson\n  \
         · Установите `project.word_count_goal: 80000` (средний роман-НФ)\n",
};

pub const RUSSIAN_LORE: ProjectTemplate = ProjectTemplate {
    name: "russian-lore",
    description:
        "Русский лор / мифология.  Section-per-category structure \
         (`Происхождение мира` / `Боги` / `Герои` / `Чудовища` / `Мифы и сказания`) \
         — collection of legends, not continuous narrative.  Pre-seeds \
         Places + Artefacts + Threads with worldbuilding stubs.",
    manuscript_book: Some(ManuscriptBook {
        title: "Лор",
        chapters: &[
            "Происхождение мира",
            "Боги",
            "Герои",
            "Чудовища",
            "Мифы и сказания",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[(
                "священная гора",
                "= священная гора\n\n\
                 Сакральный центр мира — место, к которому возвращаются\n\
                 главные мифы.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "artefacts",
            paragraphs: &[(
                "реликвия",
                "= реликвия\n\n\
                 Старинная вещь силы — обычно связана с историей сотворения\n\
                 мира или великой войны богов.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[(
                "сотворение мира",
                "{\n  \
                 title:         \"Сотворение мира\"\n  \
                 status:        \"setup\"\n  \
                 weight:        \"major\"\n  \
                 opening:       \"В начале времён не было ни Неба, ни Земли.\"\n  \
                 midpoint:      \"\"\n  \
                 payoff:        \"\"\n  \
                 characters:    []\n  \
                 places:        []\n  \
                 artefacts:     []\n  \
                 related_threads: []\n  \
                 tension:       0\n  \
                 register:      \"sacred\"\n  \
                 notes:         \"Главный космогонический миф; задаёт правила вселенной.\"\n\
                 }\n",
            )],
        },
    ],
    post_init_message:
        "Рекомендуемые следующие шаги:\n  \
         · Происхождение мира: космогония, первичное разделение, имена сил\n  \
         · Боги: пантеон с областями ответственности и взаимными конфликтами\n  \
         · Герои: смертные, чьи деяния стали мифами\n  \
         · Установите `language: \"russian\"` в inkhaven.hjson\n  \
         · Установите `project.word_count_goal: 50000` (сборник легенд)\n",
};

pub const RUSSIAN_UTOPIA: ProjectTemplate = ProjectTemplate {
    name: "russian-utopia",
    description:
        "Русская утопия.  Frame narrative (`Прибытие`) + topic-organised chapters \
         per aspect of society (`Труд` / `Семья` / `Образование` / `Искусство` / `Будущее`) \
         — Чернышевский / Богданов tradition.  Recommended goal 60000 words.",
    manuscript_book: Some(ManuscriptBook {
        title: "Утопия",
        chapters: &[
            "Прибытие",
            "Труд",
            "Семья",
            "Образование",
            "Искусство",
            "Будущее",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[(
                "город будущего",
                "= город будущего\n\n\
                 Главное пространство утопии — место, где принципы\n\
                 нового общества видны на каждом шагу.\n\n\
                 // Заполните: архитектура, ритм жизни, видимые отличия\n\
                 // от старого мира.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "characters",
            paragraphs: &[(
                "проводник",
                "= проводник\n\n\
                 Местный житель утопии, который объясняет герою (и читателю)\n\
                 устройство нового общества.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[(
                "принятие утопии",
                "{\n  \
                 title:         \"Принятие утопии\"\n  \
                 status:        \"setup\"\n  \
                 weight:        \"major\"\n  \
                 opening:       \"Герой прибывает; всё кажется чудом.\"\n  \
                 midpoint:      \"Герой замечает, что цена утопии не нулевая.\"\n  \
                 payoff:        \"Герой делает выбор: остаться или вернуться.\"\n  \
                 characters:    [\"проводник\"]\n  \
                 places:        [\"город будущего\"]\n  \
                 artefacts:     []\n  \
                 related_threads: []\n  \
                 tension:       5\n  \
                 register:      \"\"\n  \
                 notes:         \"Центральная арка традиционной русской утопии.\"\n\
                 }\n",
            )],
        },
    ],
    post_init_message:
        "Рекомендуемые следующие шаги:\n  \
         · Прибытие: первая встреча героя с утопией, что он замечает первым\n  \
         · Каждая следующая глава — один аспект общества, по образцу\n   \
            «Что делать?» Чернышевского и «Красной звезды» Богданова\n  \
         · Установите `language: \"russian\"` в inkhaven.hjson\n  \
         · Установите `project.word_count_goal: 60000` (стандартный размер утопии)\n",
};

// ──────────────────────────────────────────────────
// Additional English-language + French genre
// templates.
//
// * Epic fantasy — Tolkien / Sanderson / Jordan
//   tradition.  Big book series structure with
//   front-matter (map, dramatis personae,
//   glossary) and back-matter (appendices).
//   Single-volume scaffold; author duplicates the
//   book for sequels.  Recommended goal 150K
//   (volume one of a trilogy).
//
// * Mystery — Christie / Doyle / modern
//   procedural tradition.  Clue-tracking matters
//   — pre-seeds Threads with `the-crime`,
//   `the-misdirection`, `the-solution` arcs.
//   Characters include detective + victim +
//   suspects (3).  Places include the crime
//   scene + the detective's HQ.  Recommended
//   goal 70K.
//
// * French novel — `roman` tradition.  Often
//   structured as numbered `Première partie` /
//   `Deuxième partie` / `Troisième partie` with
//   chapter sub-divisions (`Chapitre I`, etc.).
//   Recommended goal 90K.  Post-init message
//   recommends `language: "french"` for the
//   Snowball stemmer.
// ──────────────────────────────────────────────────

pub const EPIC_FANTASY: ProjectTemplate = ProjectTemplate {
    name: "epic-fantasy",
    description:
        "Epic fantasy manuscript.  Volume-one scaffold (Prologue / Three Books / \
         Epilogue) + Appendices.  Pre-seeds Characters (hero / shadow / mentor / \
         herald), Places (the homeland / the wilds / the dark tower), Artefacts \
         (the artefact / the mentor's gift), Threads (the call / the descent / \
         the return).  Recommended goal 150000 words.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Prologue",
            "Book One — The Homeland",
            "Book Two — The Road",
            "Book Three — The Tower",
            "Epilogue",
            "Appendix A — Dramatis Personae",
            "Appendix B — Glossary",
            "Appendix C — Maps",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "characters",
            paragraphs: &[
                (
                    "hero",
                    "= hero\n\n\
                     The protagonist who answers the call.  Ordinary at the\n\
                     start; transformed by the end.\n\n\
                     // Fill in: voice, want, need, internal conflict,\n\
                     // mentor relationship.\n",
                ),
                (
                    "shadow",
                    "= shadow\n\n\
                     The antagonist who embodies the hero's fear or\n\
                     temptation — what the hero could become.\n",
                ),
                (
                    "mentor",
                    "= mentor\n\n\
                     The figure who gives the hero what they need to face\n\
                     the shadow.  Often gone or absent by the midpoint.\n",
                ),
                (
                    "herald",
                    "= herald\n\n\
                     The character or event that delivers the call to\n\
                     adventure.  Doesn't have to be a person.\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[
                (
                    "the homeland",
                    "= the homeland\n\n\
                     Where the hero begins.  Establish what's normal here\n\
                     so the reader feels the loss when the hero leaves.\n",
                ),
                (
                    "the wilds",
                    "= the wilds\n\n\
                     The threshold between the ordinary and the dangerous.\n\
                     Tests of fitness happen here.\n",
                ),
                (
                    "the dark tower",
                    "= the dark tower\n\n\
                     The shadow's seat of power.  Where the climax lands.\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "artefacts",
            paragraphs: &[
                (
                    "the artefact",
                    "= the artefact\n\n\
                     The object the hero must obtain, destroy, or wield.\n\
                     Whose existence sets the plot in motion.\n",
                ),
                (
                    "the mentor's gift",
                    "= the mentor's gift\n\n\
                     What the mentor leaves behind.  Small, easily-carried,\n\
                     load-bearing at the climax.\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[
                (
                    "the call",
                    "{\n  \
                     title:         \"The Call\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"major\"\n  \
                     opening:       \"Hero is summoned (or refuses, then summoned again).\"\n  \
                     midpoint:      \"Hero crosses the threshold.\"\n  \
                     payoff:        \"Hero commits to the road; no turning back.\"\n  \
                     characters:    [\"hero\", \"herald\"]\n  \
                     places:        [\"the homeland\"]\n  \
                     artefacts:     []\n  \
                     related_threads: [\"the descent\"]\n  \
                     tension:       3\n  \
                     register:      \"\"\n  \
                     notes:         \"Act I structural arc.\"\n\
                     }\n",
                ),
                (
                    "the descent",
                    "{\n  \
                     title:         \"The Descent\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"major\"\n  \
                     opening:       \"Hero meets the wilds and is tested.\"\n  \
                     midpoint:      \"Mentor falls; hero is alone.\"\n  \
                     payoff:        \"Hero confronts the shadow at the tower.\"\n  \
                     characters:    [\"hero\", \"shadow\", \"mentor\"]\n  \
                     places:        [\"the wilds\", \"the dark tower\"]\n  \
                     artefacts:     [\"the artefact\"]\n  \
                     related_threads: [\"the return\"]\n  \
                     tension:       8\n  \
                     register:      \"\"\n  \
                     notes:         \"Acts II-III; the bulk of the manuscript.\"\n\
                     }\n",
                ),
                (
                    "the return",
                    "{\n  \
                     title:         \"The Return\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"major\"\n  \
                     opening:       \"\"\n  \
                     midpoint:      \"\"\n  \
                     payoff:        \"Hero brings the boon home; homeland is transformed.\"\n  \
                     characters:    [\"hero\"]\n  \
                     places:        [\"the homeland\"]\n  \
                     artefacts:     [\"the artefact\"]\n  \
                     related_threads: []\n  \
                     tension:       4\n  \
                     register:      \"\"\n  \
                     notes:         \"Epilogue arc; closes the call.\"\n\
                     }\n",
                ),
            ],
        },
    ],
    post_init_message:
        "Recommended next steps:\n  \
         · Prologue: set the mythic register; foreshadow the artefact\n  \
         · Book One: ordinary world + the call (Threads/the call)\n  \
         · Books Two-Three: the descent + climb to the tower\n  \
         · Epilogue + Appendices: pay off the trilogy hooks\n  \
         · Set `project.word_count_goal: 150000` for a volume one\n  \
         · Story view (Ctrl+V Shift+W) is your friend for tracking thread coverage\n",
};

pub const MYSTERY: ProjectTemplate = ProjectTemplate {
    name: "mystery",
    description:
        "Mystery manuscript.  Crime-investigation-revelation structure with \
         clue-tracking.  Pre-seeds Characters (detective, victim, 3 suspects), \
         Places (crime scene, HQ), Threads (the crime, the misdirection, \
         the solution).  Recommended goal 70000 words.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Part I — The Crime",
            "Part II — The Investigation",
            "Part III — The Misdirection",
            "Part IV — The Solution",
            "Epilogue",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "characters",
            paragraphs: &[
                (
                    "the detective",
                    "= the detective\n\n\
                     The investigator.  Idiosyncratic method matters as\n\
                     much as the case — readers want to spend time with\n\
                     this person.\n\n\
                     // Fill in: distinctive method, blind spot, vice or\n\
                     // virtue that complicates the case.\n",
                ),
                (
                    "the victim",
                    "= the victim\n\n\
                     The person whose absence drives the plot.  Their\n\
                     secrets matter as much as the crime itself.\n",
                ),
                (
                    "suspect 1",
                    "= suspect 1\n\n\
                     Has motive AND opportunity.  Probably guilty of\n\
                     SOMETHING, but maybe not THIS.\n",
                ),
                (
                    "suspect 2",
                    "= suspect 2\n\n\
                     Looks innocent — maybe is.  Has secrets the\n\
                     investigation surfaces.\n",
                ),
                (
                    "suspect 3",
                    "= suspect 3\n\n\
                     The misdirection.  Detective and reader both\n\
                     suspect this person around the midpoint.\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[
                (
                    "the crime scene",
                    "= the crime scene\n\n\
                     Where it happened.  Detail matters — physical clues\n\
                     left here are the spine of the investigation.\n",
                ),
                (
                    "the detective's hq",
                    "= the detective's hq\n\n\
                     Where the investigation regroups.  Conversation +\n\
                     evidence pinboard happen here.\n",
                ),
            ],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[
                (
                    "the crime",
                    "{\n  \
                     title:         \"The Crime\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"major\"\n  \
                     opening:       \"The crime is discovered.\"\n  \
                     midpoint:      \"What looked simple is revealed as complex.\"\n  \
                     payoff:        \"The actual sequence of events is revealed.\"\n  \
                     characters:    [\"the detective\", \"the victim\"]\n  \
                     places:        [\"the crime scene\"]\n  \
                     artefacts:     []\n  \
                     related_threads: [\"the misdirection\", \"the solution\"]\n  \
                     tension:       7\n  \
                     register:      \"\"\n  \
                     notes:         \"Central arc.\"\n\
                     }\n",
                ),
                (
                    "the misdirection",
                    "{\n  \
                     title:         \"The Misdirection\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"subplot\"\n  \
                     opening:       \"Evidence points at a plausible wrong suspect.\"\n  \
                     midpoint:      \"Investigator commits to the wrong theory.\"\n  \
                     payoff:        \"Investigator realises the mistake.\"\n  \
                     characters:    [\"the detective\", \"suspect 3\"]\n  \
                     places:        []\n  \
                     artefacts:     []\n  \
                     related_threads: [\"the crime\"]\n  \
                     tension:       6\n  \
                     register:      \"\"\n  \
                     notes:         \"Reader must be misled in the SAME way the detective is.\"\n\
                     }\n",
                ),
                (
                    "the solution",
                    "{\n  \
                     title:         \"The Solution\"\n  \
                     status:        \"setup\"\n  \
                     weight:        \"major\"\n  \
                     opening:       \"\"\n  \
                     midpoint:      \"\"\n  \
                     payoff:        \"Detective explains the crime — clues retroactively snap into place.\"\n  \
                     characters:    [\"the detective\"]\n  \
                     places:        []\n  \
                     artefacts:     []\n  \
                     related_threads: [\"the crime\", \"the misdirection\"]\n  \
                     tension:       9\n  \
                     register:      \"\"\n  \
                     notes:         \"All clues must be visible to the reader BEFORE this point.\"\n\
                     }\n",
                ),
            ],
        },
    ],
    post_init_message:
        "Recommended next steps:\n  \
         · Part I: stage the crime with EVERY clue physically present in the prose\n  \
         · Part III: commit to the wrong theory long enough for the reader to follow\n  \
         · Part IV: the reveal must use ONLY clues the reader has already seen\n  \
         · Set `project.word_count_goal: 70000`\n  \
         · Concordance (Ctrl+B Shift+L) is your friend for tracking clue mentions\n",
};

pub const FRENCH_NOVEL: ProjectTemplate = ProjectTemplate {
    name: "french-novel",
    description:
        "Roman français.  `Première partie` / `Deuxième partie` / `Troisième partie` \
         + `Épilogue` (Hugo / Flaubert / Camus tradition).  Pre-seeds Characters \
         (protagoniste / antagoniste / confident).  Recommended goal 90000 words; \
         set `language: \"french\"` in inkhaven.hjson.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscrit",
        chapters: &[
            "Première partie",
            "Deuxième partie",
            "Troisième partie",
            "Épilogue",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "characters",
        paragraphs: &[
            (
                "protagoniste",
                "= protagoniste\n\n\
                 Le personnage dont le roman suit la trajectoire intérieure\n\
                 et extérieure.\n\n\
                 // À remplir : voix, désir, besoin, conflit intérieur,\n\
                 // scènes-clés.\n",
            ),
            (
                "antagoniste",
                "= antagoniste\n\n\
                 La force qui s'oppose au désir ou au besoin du protagoniste.\n\n\
                 // Pas nécessairement une personne — institution, époque,\n\
                 // ou part de la psyché du héros.\n",
            ),
            (
                "confident",
                "= confident\n\n\
                 Personnage à qui le protagoniste confie son monologue\n\
                 intérieur — c'est par lui que le lecteur entend ce qui\n\
                 resterait autrement non dit.\n",
            ),
        ],
    }],
    post_init_message:
        "Étapes recommandées :\n  \
         · Ouvrez `Manuscrit/Première partie` et établissez le ton\n  \
         · Caractérisez `protagoniste` (voix, désir, besoin) dans Characters\n  \
         · Réglez `language: \"french\"` dans inkhaven.hjson (stemmer + prompts multilingues)\n  \
         · Réglez `project.word_count_goal: 90000`\n",
};

/// apply the named template to
/// a freshly-initialised project.  Called by
/// `cli::init::run` after the standard
/// `Store::open` returns.  Errors are surfaced
/// upward but don't roll back the standard init —
/// a partial template scaffold is recoverable
/// (the author can `inkhaven add` the missing
/// nodes by hand) but a rolled-back init isn't.
pub fn apply(store: &Store, cfg: &Config, name: &str) -> Result<()> {
    let template = TEMPLATES
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            Error::Config(format!(
                "unknown template `{name}` — run `inkhaven template list` \
                 to see available templates"
            ))
        })?;
    if name.eq_ignore_ascii_case("empty") {
        // No-op fast path.  Caller still gets a
        // valid scaffold from the standard init.
        return Ok(());
    }
    if let Some(book) = template.manuscript_book.as_ref() {
        apply_manuscript_book(store, cfg, book)?;
    }
    for seed in template.seeds {
        apply_system_seed(store, cfg, seed)?;
    }
    if !template.post_init_message.is_empty() {
        eprintln!();
        eprintln!("Template `{}`:", template.name);
        for line in template.post_init_message.lines() {
            eprintln!("{line}");
        }
    }
    Ok(())
}

fn apply_manuscript_book(
    store: &Store,
    cfg: &Config,
    book: &ManuscriptBook,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let new_book = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Book,
        book.title,
        None,
        None,
        InsertPosition::End,
    )?;
    eprintln!("  · created book `{}`", book.title);
    // Standard Typst skeleton (index.typ / settings.typ /
    // globals.typ) — same path the tree-pane Add Book chord
    // calls.  Non-fatal: a partial provisioning is better
    // than aborting the whole template.
    if let Err(e) = store.provision_user_book(cfg, &new_book) {
        eprintln!(
            "    (warn: Typst skeleton provisioning failed: {e})"
        );
    }
    for chapter_title in book.chapters {
        let hierarchy = Hierarchy::load(store)?;
        store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Chapter,
            chapter_title,
            Some(&new_book),
            None,
            InsertPosition::End,
        )?;
        eprintln!("      · chapter `{chapter_title}`");
    }
    Ok(())
}

fn apply_system_seed(
    store: &Store,
    cfg: &Config,
    seed: &SystemBookSeed,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let parent = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(seed.system_tag)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(format!(
                "system book `{}` missing — re-open the project to seed it",
                seed.system_tag
            ))
        })?;
    for (title, body) in seed.paragraphs {
        let hierarchy = Hierarchy::load(store)?;
        // Skip duplicates by title so re-running
        // init --template on top of an existing
        // project doesn't double-seed.
        if hierarchy
            .children_of(Some(parent.id))
            .iter()
            .any(|n| n.title.eq_ignore_ascii_case(title))
        {
            continue;
        }
        let mut node = store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Paragraph,
            title,
            Some(&parent),
            None,
            InsertPosition::End,
        )?;
        if !body.is_empty() {
            if let Some(rel) = &node.file {
                let abs = store.project_root().join(rel);
                std::fs::write(&abs, body.as_bytes())
                    .map_err(Error::Io)?;
            }
            store
                .update_paragraph_content(&mut node, body.as_bytes())
                .map_err(|e| {
                    Error::Store(format!("seed {title}: {e}"))
                })?;
        }
        eprintln!(
            "      · seeded {}/{}",
            seed.system_tag, title
        );
    }
    Ok(())
}

/// `inkhaven template list`.
/// Prints a two-column table: name → description.
/// Column widths size to the longest name.
pub fn list_templates() {
    let max_name = TEMPLATES
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(8);
    let name_w = max_name.max(8);
    println!(
        "  {:<width$}  description",
        "name",
        width = name_w,
    );
    println!("  {}", "-".repeat(name_w + 60));
    for t in TEMPLATES {
        let mut first_line = true;
        // Wrap description onto continuation lines
        // indented under the description column.
        let prefix_width = name_w + 4;
        for line in wrap_description(t.description, 70) {
            if first_line {
                println!(
                    "  {:<width$}  {}",
                    t.name,
                    line,
                    width = name_w,
                );
                first_line = false;
            } else {
                println!(
                    "  {:<width$}  {}",
                    "",
                    line,
                    width = name_w,
                );
            }
            let _ = prefix_width; // silence rustc until/if needed
        }
    }
    println!();
    println!("Use with: inkhaven init <path> --template <name>");
}

/// Word-wrap a description string to `width`
/// characters; never breaks inside a word.
fn wrap_description(s: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_every_named_template() {
        let names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
        for required in
            ["empty", "novel", "nonfiction", "rpg-sourcebook", "technical", "nanowrimo"]
        {
            assert!(
                names.contains(&required),
                "missing template `{required}` in TEMPLATES"
            );
        }
    }

    #[test]
    fn empty_template_has_no_scaffolding() {
        let empty = TEMPLATES
            .iter()
            .find(|t| t.name == "empty")
            .unwrap();
        assert!(empty.manuscript_book.is_none());
        assert!(empty.seeds.is_empty());
    }

    #[test]
    fn novel_template_has_three_act_structure() {
        let novel = TEMPLATES
            .iter()
            .find(|t| t.name == "novel")
            .unwrap();
        let book = novel.manuscript_book.as_ref().unwrap();
        assert_eq!(book.chapters.len(), 3);
        assert!(book.chapters[0].contains("Act I"));
        assert!(book.chapters[1].contains("Act II"));
        assert!(book.chapters[2].contains("Act III"));
        // Seeds Characters with three stubs.
        let chars = novel
            .seeds
            .iter()
            .find(|s| s.system_tag == "characters")
            .unwrap();
        assert_eq!(chars.paragraphs.len(), 3);
    }

    #[test]
    fn rpg_template_seeds_places_artefacts_threads() {
        let rpg = TEMPLATES
            .iter()
            .find(|t| t.name == "rpg-sourcebook")
            .unwrap();
        let tags: Vec<&str> =
            rpg.seeds.iter().map(|s| s.system_tag).collect();
        assert!(tags.contains(&"places"));
        assert!(tags.contains(&"artefacts"));
        assert!(tags.contains(&"threads"));
    }

    #[test]
    fn russian_templates_all_registered() {
        let names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
        for required in [
            "russian-novel",
            "russian-long-story",
            "russian-scifi",
            "russian-lore",
            "russian-utopia",
        ] {
            assert!(
                names.contains(&required),
                "missing template `{required}` in TEMPLATES"
            );
        }
    }

    #[test]
    fn russian_novel_has_three_parts_plus_epilogue() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-novel")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        assert_eq!(book.chapters.len(), 4);
        assert!(book.chapters[0].contains("Часть Первая"));
        assert!(book.chapters[3].contains("Эпилог"));
        assert_eq!(book.title, "Рукопись");
        // Seeds Characters with 3 standard roles.
        let chars = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "characters")
            .unwrap();
        assert_eq!(chars.paragraphs.len(), 3);
    }

    #[test]
    fn russian_long_story_uses_roman_numerals() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-long-story")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        // Roman numerals I..VII + Эпилог = 8 chapters.
        assert_eq!(book.chapters.len(), 8);
        for ch in ["I", "II", "III", "IV", "V", "VI", "VII"] {
            assert!(book.chapters.contains(&ch));
        }
        assert!(book.chapters.contains(&"Эпилог"));
    }

    #[test]
    fn russian_scifi_includes_glossary_and_places_seeds() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-scifi")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        assert!(book.chapters.iter().any(|c| c == &"Пролог"));
        assert!(book.chapters.iter().any(|c| c == &"Эпилог"));
        assert!(book.chapters.iter().any(|c| c == &"Глоссарий"));
        let tags: Vec<&str> = t.seeds.iter().map(|s| s.system_tag).collect();
        assert!(tags.contains(&"places"));
        assert!(tags.contains(&"artefacts"));
    }

    #[test]
    fn russian_lore_thread_seed_parses_as_hjson() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-lore")
            .unwrap();
        let threads = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .expect("russian-lore seeds Threads system book");
        let (_, body) = threads.paragraphs[0];
        let _: serde_hjson::Value = serde_hjson::from_str(body)
            .expect("russian-lore threads seed must be valid HJSON");
    }

    #[test]
    fn russian_utopia_thread_seed_parses_as_hjson() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-utopia")
            .unwrap();
        let threads = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .expect("russian-utopia seeds Threads system book");
        let (_, body) = threads.paragraphs[0];
        let _: serde_hjson::Value = serde_hjson::from_str(body)
            .expect("russian-utopia threads seed must be valid HJSON");
    }

    #[test]
    fn russian_utopia_chapters_match_topic_structure() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "russian-utopia")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        // Frame chapter + 4 topic chapters + future = 6.
        for ch in [
            "Прибытие",
            "Труд",
            "Семья",
            "Образование",
            "Искусство",
            "Будущее",
        ] {
            assert!(
                book.chapters.contains(&ch),
                "Russian utopia chapters missing `{ch}`"
            );
        }
    }

    #[test]
    fn epic_fantasy_has_full_scaffolding() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "epic-fantasy")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        // Prologue + 3 books + epilogue + 3 appendices = 8 chapters.
        assert_eq!(book.chapters.len(), 8);
        assert!(book.chapters[0].contains("Prologue"));
        assert!(book.chapters[4].contains("Epilogue"));
        let tags: Vec<&str> =
            t.seeds.iter().map(|s| s.system_tag).collect();
        assert!(tags.contains(&"characters"));
        assert!(tags.contains(&"places"));
        assert!(tags.contains(&"artefacts"));
        assert!(tags.contains(&"threads"));
        // Three thread seeds for the three-act
        // call-descent-return shape.
        let threads = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .unwrap();
        assert_eq!(threads.paragraphs.len(), 3);
    }

    #[test]
    fn mystery_threads_parse_as_valid_hjson() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "mystery")
            .unwrap();
        let threads = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .unwrap();
        for (name, body) in threads.paragraphs {
            let _: serde_hjson::Value = serde_hjson::from_str(body)
                .unwrap_or_else(|e| {
                    panic!("mystery thread seed `{name}` invalid HJSON: {e}")
                });
        }
    }

    #[test]
    fn mystery_seeds_three_suspects() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "mystery")
            .unwrap();
        let chars = t
            .seeds
            .iter()
            .find(|s| s.system_tag == "characters")
            .unwrap();
        let suspect_count = chars
            .paragraphs
            .iter()
            .filter(|(name, _)| name.starts_with("suspect"))
            .count();
        assert_eq!(suspect_count, 3);
    }

    #[test]
    fn french_novel_uses_french_partition_names() {
        let t = TEMPLATES
            .iter()
            .find(|t| t.name == "french-novel")
            .unwrap();
        let book = t.manuscript_book.as_ref().unwrap();
        assert_eq!(book.title, "Manuscrit");
        for ch in [
            "Première partie",
            "Deuxième partie",
            "Troisième partie",
            "Épilogue",
        ] {
            assert!(
                book.chapters.contains(&ch),
                "missing chapter `{ch}`"
            );
        }
    }

    #[test]
    fn nanowrimo_template_inherits_novel_seeds() {
        let nano = TEMPLATES
            .iter()
            .find(|t| t.name == "nanowrimo")
            .unwrap();
        let novel = TEMPLATES
            .iter()
            .find(|t| t.name == "novel")
            .unwrap();
        assert_eq!(nano.seeds.len(), novel.seeds.len());
    }

    #[test]
    fn wrap_description_handles_short_strings() {
        let lines = wrap_description("short", 70);
        assert_eq!(lines, vec!["short".to_string()]);
    }

    #[test]
    fn wrap_description_wraps_long_strings() {
        let s = "a ".repeat(50);
        let lines = wrap_description(s.trim(), 20);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.chars().count() <= 20);
        }
    }

    #[test]
    fn rpg_thread_seed_parses_as_hjson() {
        // The Threads seed body is HJSON; pin that
        // it parses so a future schema tweak can't
        // ship a stub the user can't open.
        let rpg = TEMPLATES
            .iter()
            .find(|t| t.name == "rpg-sourcebook")
            .unwrap();
        let threads = rpg
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .unwrap();
        let (_, body) = threads.paragraphs[0];
        let _: serde_hjson::Value = serde_hjson::from_str(body)
            .expect("rpg-sourcebook threads seed must be valid HJSON");
    }
}

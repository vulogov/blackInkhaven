//! WORLD-KEEP-2 (WK2-P3) — the world's proposals in the project's language.
//!
//! The compile layers speak English internally: a settlement is a `city` at a
//! `river_mouth` in a `temperate_grassland`, a people is `woodland-reverent,
//! proud` and holds to `ancestor veneration`. Those values are *data* — they
//! key signatures, dedup, the fact-checker — and stay exactly as they are.
//!
//! What the author READS (a proposal's rationale) and what is COMMITTED to
//! their books on accept (the Place paragraph, the ruler stub, the language
//! brief, the Mythology entry, a critique Note) is prose, and prose follows the
//! project language: en / ru / fr / de / es, falling back to English.
//!
//! Translating only the sentence frame would commit "Город of ~23k at a river
//! mouth". So this module carries the **vocabularies** too — settlement class,
//! siting basis, the twelve biomes, the culture layer's ethos and belief
//! phrases, the language-profile terms, the myth glosses and their word lists —
//! in forms chosen so the frames need no grammatical agreement (locative
//! phrases, nominative noun phrases, adjectives that agree with "people").
//! A value the tables do not know — an author-declared ethos or belief — is
//! passed through untouched: it is already in the author's own words.
//!
//! English output is byte-identical to what the generators always produced.

use crate::prose::ProseLanguage;
use crate::world::proposals::PlaceProposal;

/// The five languages the world's prose can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WLang {
    En,
    Ru,
    Fr,
    De,
    Es,
}

impl WLang {
    pub fn of(language: &ProseLanguage) -> WLang {
        match language {
            ProseLanguage::Ru => WLang::Ru,
            ProseLanguage::Fr => WLang::Fr,
            ProseLanguage::De => WLang::De,
            ProseLanguage::Es => WLang::Es,
            _ => WLang::En,
        }
    }

    /// From a project's `language` setting (`"russian"`, `"ru"`, …).
    pub fn of_config(language: &str) -> WLang {
        WLang::of(&ProseLanguage::from_label(language))
    }

    /// The project's world-prose language, read from its config; English when
    /// the directory is not an initialised project (a bare `world.hjson`).
    pub fn of_project(project: &std::path::Path) -> WLang {
        let layout = crate::project::ProjectLayout::new(project);
        crate::config::Config::load_layered(&layout.config_path())
            .map(|cfg| WLang::of_config(&cfg.language))
            .unwrap_or(WLang::En)
    }

    fn idx(self) -> usize {
        match self {
            WLang::En => 0,
            WLang::Ru => 1,
            WLang::Fr => 2,
            WLang::De => 3,
            WLang::Es => 4,
        }
    }
}

/// One term in all five languages (en, ru, fr, de, es).
type T5 = [&'static str; 5];

fn look(table: &[(&'static str, T5)], key: &str, lang: WLang) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, t)| t[lang.idx()])
}

const CLASS: &[(&str, T5)] = &[
    ("city", ["city", "город", "ville", "Stadt", "ciudad"]),
    ("town", ["town", "посёлок", "bourg", "Kleinstadt", "villa"]),
    ("village", ["village", "деревня", "village", "Dorf", "aldea"]),
];

/// Where a settlement sits — a complete locative phrase, so no frame has to
/// decline it.
const BASIS: &[(&str, T5)] = &[
    (
        "river_mouth",
        ["at a river mouth", "в устье реки", "à l'embouchure d'un fleuve", "an einer Flussmündung", "en la desembocadura de un río"],
    ),
    (
        "confluence",
        ["at a confluence", "у слияния рек", "au confluent de deux rivières", "an einem Zusammenfluss", "en una confluencia de ríos"],
    ),
    (
        "fertile_valley",
        ["in a fertile valley", "в плодородной долине", "dans une vallée fertile", "in einem fruchtbaren Tal", "en un valle fértil"],
    ),
];

/// The twelve biomes — nominative noun phrases.
const BIOME: &[(&str, T5)] = &[
    ("ice_cap", ["ice cap", "ледяной щит", "calotte glaciaire", "Eiskappe", "casquete glaciar"]),
    ("tundra", ["tundra", "тундра", "toundra", "Tundra", "tundra"]),
    ("taiga", ["taiga", "тайга", "taïga", "Taiga", "taiga"]),
    ("temperate_forest", ["temperate forest", "умеренные леса", "forêt tempérée", "gemäßigter Wald", "bosque templado"]),
    (
        "temperate_grassland",
        ["temperate grassland", "умеренные степи", "prairie tempérée", "gemäßigtes Grasland", "pradera templada"],
    ),
    ("mediterranean", ["mediterranean", "средиземноморский пояс", "zone méditerranéenne", "Mittelmeerzone", "zona mediterránea"]),
    ("cold_desert", ["cold desert", "холодная пустыня", "désert froid", "Kältewüste", "desierto frío"]),
    ("hot_desert", ["hot desert", "жаркая пустыня", "désert chaud", "Heißwüste", "desierto cálido"]),
    ("savanna", ["savanna", "саванна", "savane", "Savanne", "sabana"]),
    (
        "tropical_seasonal",
        ["tropical seasonal", "сезонные тропики", "tropiques à saisons", "wechselfeuchte Tropen", "trópico estacional"],
    ),
    (
        "tropical_rainforest",
        ["tropical rainforest", "влажный тропический лес", "forêt tropicale humide", "tropischer Regenwald", "selva tropical"],
    ),
    ("ocean", ["ocean", "океан", "océan", "Ozean", "océano"]),
];

/// The culture layer's biome ethos and tempers — adjectives agreeing with
/// "people" (народ / un peuple / ein Volk [predicative] / un pueblo).
const ETHOS: &[(&str, T5)] = &[
    ("woodland-reverent", ["woodland-reverent", "чтущий леса", "révérant la forêt", "waldverehrend", "reverente del bosque"]),
    (
        "austere and hospitable",
        ["austere and hospitable", "суровый и гостеприимный", "austère et hospitalier", "karg und gastfreundlich", "austero y hospitalario"],
    ),
    (
        "roaming and herd-proud",
        [
            "roaming and herd-proud",
            "кочевой и гордый своими стадами",
            "nomade et fier de ses troupeaux",
            "umherziehend und stolz auf seine Herden",
            "errante y orgulloso de sus rebaños",
        ],
    ),
    (
        "vivid and ceremonious",
        ["vivid and ceremonious", "яркий и церемонный", "haut en couleur et cérémonieux", "farbenfroh und zeremoniell", "vistoso y ceremonioso"],
    ),
    ("hardy and close-knit", ["hardy and close-knit", "выносливый и сплочённый", "endurant et soudé", "zäh und eng verbunden", "recio y unido"]),
    (
        "mercantile and civic",
        ["mercantile and civic", "торговый и гражданственный", "marchand et civique", "handelstüchtig und bürgerlich", "mercantil y cívico"],
    ),
    (
        "settled and pragmatic",
        ["settled and pragmatic", "оседлый и практичный", "sédentaire et pragmatique", "sesshaft und pragmatisch", "sedentario y pragmático"],
    ),
    ("settled", ["settled", "оседлый", "sédentaire", "sesshaft", "sedentario"]),
    ("proud", ["proud", "гордый", "fier", "stolz", "orgulloso"]),
    ("cautious", ["cautious", "осторожный", "prudent", "vorsichtig", "cauto"]),
    ("curious", ["curious", "любознательный", "curieux", "neugierig", "curioso"]),
    ("devout", ["devout", "набожный", "dévot", "fromm", "devoto"]),
    ("stubborn", ["stubborn", "упрямый", "obstiné", "stur", "obstinado"]),
    ("generous", ["generous", "щедрый", "généreux", "großzügig", "generoso"]),
    ("secretive", ["secretive", "скрытный", "secret", "verschlossen", "reservado"]),
    ("boisterous", ["boisterous", "шумный", "exubérant", "ausgelassen", "bullicioso"]),
];

/// The culture layer's beliefs — nominative noun phrases.
const BELIEF: &[(&str, T5)] = &[
    (
        "ancestor veneration",
        ["ancestor veneration", "почитание предков", "le culte des ancêtres", "Ahnenverehrung", "la veneración de los antepasados"],
    ),
    ("a sky-pantheon", ["a sky-pantheon", "небесный пантеон", "un panthéon céleste", "ein Himmelspantheon", "un panteón celeste"]),
    (
        "one hidden god",
        ["one hidden god", "единый сокрытый бог", "un dieu unique et caché", "ein einziger verborgener Gott", "un único dios oculto"],
    ),
    (
        "nature spirits of river and stone",
        [
            "nature spirits of river and stone",
            "духи природы — рек и камня",
            "les esprits de la nature, des rivières et des pierres",
            "Naturgeister von Fluss und Stein",
            "los espíritus de la naturaleza, del río y la piedra",
        ],
    ),
    (
        "a cult of the seasons",
        ["a cult of the seasons", "культ времён года", "un culte des saisons", "ein Kult der Jahreszeiten", "un culto de las estaciones"],
    ),
    (
        "reverence for the founding dead",
        [
            "reverence for the founding dead",
            "почитание умерших основателей",
            "la révérence envers les fondateurs défunts",
            "die Verehrung der toten Gründer",
            "la reverencia por los fundadores difuntos",
        ],
    ),
    ("their own ways", ["their own ways", "свои обычаи", "ses propres coutumes", "eigene Bräuche", "sus propias costumbres"]),
];

/// Language-profile terms: morphology (feminine where the label is), then sound.
const PROFILE: &[(&str, T5)] = &[
    ("isolating", ["isolating", "изолирующая", "isolante", "isolierend", "aislante"]),
    ("agglutinative", ["agglutinative", "агглютинативная", "agglutinante", "agglutinierend", "aglutinante"]),
    ("fusional", ["fusional", "флективная", "flexionnelle", "flektierend", "flexiva"]),
    ("tonal", ["tonal", "тональный", "tonale", "tonal", "tonal"]),
    ("guttural", ["guttural", "гортанный", "gutturale", "guttural", "gutural"]),
    (
        "liquid and vowel-rich",
        ["liquid and vowel-rich", "плавный, богатый гласными", "liquide et riche en voyelles", "fließend und vokalreich", "líquida y rica en vocales"],
    ),
    (
        "clipped and consonantal",
        [
            "clipped and consonantal",
            "отрывистый, с обилием согласных",
            "hachée et consonantique",
            "abgehackt und konsonantenreich",
            "cortada y consonántica",
        ],
    ),
];

/// The myth seeds' glosses and motif names, keyed by the English text the
/// generator produces.
const MYTH_TEXT: &[(&str, T5)] = &[
    (
        "the forebears watch, guide, and judge the living",
        [
            "the forebears watch, guide, and judge the living",
            "предки наблюдают за живыми, направляют и судят их",
            "les ancêtres veillent sur les vivants, les guident et les jugent",
            "die Ahnen wachen über die Lebenden, leiten und richten sie",
            "los antepasados velan por los vivos, los guían y los juzgan",
        ],
    ),
    (
        "the high gods who rule from the sky",
        [
            "the high gods who rule from the sky",
            "верховные боги, правящие с небес",
            "les grands dieux qui règnent depuis le ciel",
            "die hohen Götter, die vom Himmel herrschen",
            "los altos dioses que gobiernan desde el cielo",
        ],
    ),
    (
        "a single god who withholds its face",
        [
            "a single god who withholds its face",
            "единый бог, скрывающий свой лик",
            "un dieu unique qui dérobe son visage",
            "ein einziger Gott, der sein Antlitz verbirgt",
            "un único dios que oculta su rostro",
        ],
    ),
    (
        "the small gods bound to features of the land",
        [
            "the small gods bound to features of the land",
            "малые боги, привязанные к приметам земли",
            "les petits dieux attachés aux lieux du pays",
            "die kleinen Götter, die an Orte des Landes gebunden sind",
            "los dioses menores ligados a los lugares de la tierra",
        ],
    ),
    (
        "rites and reversals bound to the turning of the seasons",
        [
            "rites and reversals bound to the turning of the seasons",
            "обряды и перемены, связанные с круговоротом времён года",
            "des rites et des renversements liés au cycle des saisons",
            "Riten und Umkehrungen, gebunden an den Lauf der Jahreszeiten",
            "ritos e inversiones ligados al ciclo de las estaciones",
        ],
    ),
    (
        "the first ancestors whose choices still bind the living",
        [
            "the first ancestors whose choices still bind the living",
            "первые предки, чей выбор до сих пор связывает живых",
            "les premiers ancêtres dont les choix lient encore les vivants",
            "die ersten Ahnen, deren Entscheidungen die Lebenden noch binden",
            "los primeros antepasados cuyas decisiones aún atan a los vivos",
        ],
    ),
    ("the turning year", ["the turning year", "круговорот года", "l'année qui tourne", "das sich wendende Jahr", "el año que gira"]),
    (
        "the founder's shadow",
        ["the founder's shadow", "тень основателя", "l'ombre du fondateur", "der Schatten des Gründers", "la sombra del fundador"],
    ),
];

/// The words a symbol is recognised by in the prose — they must be words of
/// the PROSE language, or `myth scan` finds nothing. Keyed by belief.
///
/// `myth scan` matches whole words exactly (no stemming), so the inflected
/// languages carry the commonest case forms too. It is a starting vocabulary:
/// the author extends it in the Mythology book like any declared symbol.
fn myth_vocabulary(belief: &str, lang: WLang) -> Option<&'static [&'static str]> {
    let v: &'static [&'static str] = match (belief, lang) {
        ("ancestor veneration", WLang::Ru) => &["предки", "предков", "предкам", "предками", "предок", "праотцы", "усопшие"],
        ("ancestor veneration", WLang::Fr) => &["ancêtre", "ancêtres", "aïeux", "les morts"],
        ("ancestor veneration", WLang::De) => &["Ahnen", "Ahn", "Vorfahren", "die Toten", "den Toten"],
        ("ancestor veneration", WLang::Es) => &["antepasado", "antepasados", "ancestros", "los muertos"],
        ("a sky-pantheon", WLang::Ru) => &["небо", "неба", "небу", "небеса", "небес", "солнце", "солнца", "буря", "гром"],
        ("a sky-pantheon", WLang::Fr) => &["ciel", "soleil", "orage", "tonnerre", "les cieux"],
        ("a sky-pantheon", WLang::De) => &["Himmel", "Himmels", "Sonne", "Sturm", "Donner"],
        ("a sky-pantheon", WLang::Es) => &["cielo", "sol", "tormenta", "trueno", "los cielos"],
        ("one hidden god", WLang::Ru) => &["сокрытый", "сокрытого", "незримый", "незримого", "единый", "единого"],
        ("one hidden god", WLang::Fr) => &["le caché", "l'invisible", "l'unique"],
        ("one hidden god", WLang::De) => &["der Verborgene", "der Ungesehene", "der Eine"],
        ("one hidden god", WLang::Es) => &["el oculto", "el invisible", "el único"],
        ("nature spirits of river and stone", WLang::Ru) => &["река", "реки", "реку", "камень", "камня", "родник", "роща", "рощи", "древние места"],
        ("nature spirits of river and stone", WLang::Fr) => &["rivière", "pierre", "source", "bosquet", "les lieux anciens"],
        ("nature spirits of river and stone", WLang::De) => &["Fluss", "Flusses", "Flüsse", "Stein", "Steine", "Quelle", "Hain", "die alten Orte"],
        ("nature spirits of river and stone", WLang::Es) => &["río", "piedra", "manantial", "arboleda", "los lugares antiguos"],
        _ => return None,
    };
    Some(v)
}

// ── single-term translation (unknown values pass through) ───────────────────

/// A settlement class (`city`). Unknown → as given.
pub fn class(lang: WLang, key: &str) -> String {
    look(CLASS, key, lang).map(str::to_string).unwrap_or_else(|| key.replace('_', " "))
}

/// A siting basis as a locative phrase (`river_mouth` → "at a river mouth").
pub fn basis(lang: WLang, key: &str) -> String {
    look(BASIS, key, lang).map(str::to_string).unwrap_or_else(|| key.replace('_', " "))
}

/// A biome (`temperate_forest`). Unknown → as given.
pub fn biome(lang: WLang, key: &str) -> String {
    look(BIOME, key, lang).map(str::to_string).unwrap_or_else(|| key.replace('_', " "))
}

/// A culture ethos (`"woodland-reverent, proud"`): each comma-separated part is
/// translated if the culture layer generated it, and kept if the author wrote it.
pub fn ethos(lang: WLang, value: &str) -> String {
    value
        .split(',')
        .map(|part| {
            let part = part.trim();
            look(ETHOS, part, lang).unwrap_or(part).to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A belief. A generated one is translated; a declared one is the author's.
pub fn belief(lang: WLang, value: &str) -> String {
    look(BELIEF, value.trim(), lang).unwrap_or(value.trim()).to_string()
}

/// A `"SOV · agglutinative · tonal"` profile: the word order is notation and
/// stays; morphology and sound are translated.
pub fn profile(lang: WLang, value: &str) -> String {
    value
        .split('·')
        .map(|part| {
            let part = part.trim();
            look(PROFILE, part, lang).unwrap_or(part).to_string()
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn profile_term(lang: WLang, value: &str) -> String {
    let v = value.trim();
    if v.is_empty() { "—".to_string() } else { look(PROFILE, v, lang).unwrap_or(v).to_string() }
}

/// A polity's capital descriptor (`"the temperate_forest city"`) → "city
/// (temperate forest)" in the target language; anything else passes through.
fn capital(lang: WLang, value: &str) -> String {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix("the ") {
        if let Some((b, c)) = rest.rsplit_once(' ') {
            if look(CLASS, c, lang).is_some() && look(BIOME, b, lang).is_some() {
                return format!("{} ({})", class(lang, c), biome(lang, b));
            }
        }
    }
    v.replace('_', " ")
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn fmt_pop(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

// ── proposals: what the author reads ────────────────────────────────────────

/// Rewrite a batch of freshly generated proposals into `lang`: the rationale,
/// and — for Mythology entries — the vocabulary, gloss and name that will be
/// committed. Signatures and the canonical payload keys are untouched, so
/// dedup and the fact-checker see the same data in every language. A no-op for
/// English.
pub fn localize_all(proposals: &mut [PlaceProposal], lang: WLang) {
    if lang == WLang::En {
        return;
    }
    for p in proposals {
        localize(p, lang);
    }
}

fn localize(p: &mut PlaceProposal, lang: WLang) {
    let s = |p: &PlaceProposal, k: &str| p.payload.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let u = |p: &PlaceProposal, k: &str| p.payload.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    match p.kind.as_str() {
        "place" => {
            let (c, pop) = (upper_first(&class(lang, &s(p, "class"))), fmt_pop(u(p, "population")));
            let (b, z) = (basis(lang, &s(p, "basis")), biome(lang, &s(p, "biome")));
            p.rationale = match lang {
                WLang::Ru => format!("{c}, ~{pop} жителей, {b}; природная зона: {z}."),
                WLang::Fr => format!("{c} d'environ {pop} habitants, {b} ; milieu : {z}."),
                WLang::De => format!("{c} mit etwa {pop} Einwohnern, {b}; Naturraum: {z}."),
                WLang::Es => format!("{c} de unos {pop} habitantes, {b}; entorno: {z}."),
                WLang::En => return,
            };
        }
        "character" => {
            let realm = s(p, "realm");
            let (e, b) = (ethos_or_default(lang, &s(p, "ethos"), "settled"), belief_or_default(lang, &s(p, "belief")));
            p.rationale = match lang {
                WLang::Ru => format!("Правитель державы {realm}. Народ: {e}. Вера: {b}."),
                WLang::Fr => format!("Souverain de {realm}. Un peuple {e}. Croyance : {b}."),
                WLang::De => format!("Herrscher von {realm}. Ein Volk: {e}. Glaube: {b}."),
                WLang::Es => format!("Gobernante de {realm}. Un pueblo {e}. Creencia: {b}."),
                WLang::En => return,
            };
        }
        "language" => {
            let (realm, prof, sample) = (s(p, "realm"), profile(lang, &s(p, "profile")), s(p, "naming_sample"));
            p.rationale = match lang {
                WLang::Ru => format!("Язык державы {realm}: {prof}; пример: «{sample}»."),
                WLang::Fr => format!("La langue de {realm} : {prof} ; exemple : « {sample} »."),
                WLang::De => format!("Die Sprache von {realm}: {prof}; Beispiel: „{sample}“."),
                WLang::Es => format!("La lengua de {realm}: {prof}; ejemplo: «{sample}»."),
                WLang::En => return,
            };
        }
        k if k.starts_with("myth-") => localize_myth(p, lang),
        _ => {}
    }
}

fn ethos_or_default(lang: WLang, value: &str, default_key: &str) -> String {
    ethos(lang, if value.trim().is_empty() { default_key } else { value })
}

fn belief_or_default(lang: WLang, value: &str) -> String {
    belief(lang, if value.trim().is_empty() { "their own ways" } else { value })
}

fn localize_myth(p: &mut PlaceProposal, lang: WLang) {
    let belief_en = p.payload.get("belief").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let is_motif = p.payload.get("myth_kind").and_then(|v| v.as_str()) == Some("motif");
    let peoples: Vec<String> = p
        .payload
        .get("traditions")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    // The committed entry: gloss and motif name from the tables…
    for key in ["gloss", "name"] {
        let cur = p.payload.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if let Some(t) = look(MYTH_TEXT, &cur, lang) {
            p.payload[key] = serde_json::json!(t);
        } else if key == "gloss" {
            // The generic fallback gloss for a declared belief.
            if let Some(rest) = cur.strip_prefix("what these peoples hold sacred: ") {
                let frame = match lang {
                    WLang::Ru => "то, что эти народы считают священным: ",
                    WLang::Fr => "ce que ces peuples tiennent pour sacré : ",
                    WLang::De => "was diesen Völkern heilig ist: ",
                    WLang::Es => "lo que estos pueblos tienen por sagrado: ",
                    WLang::En => "what these peoples hold sacred: ",
                };
                p.payload["gloss"] = serde_json::json!(format!("{frame}{rest}"));
            }
        }
    }
    // …and the words the symbol is recognised by, in the prose language.
    if !is_motif {
        if let Some(words) = myth_vocabulary(&belief_en, lang) {
            p.payload["vocabulary"] = serde_json::json!(words);
        } else if look(BELIEF, &belief_en, WLang::En).is_none() {
            // A declared belief: its own words, split Unicode-aware (the English
            // generator split on ASCII and lost every non-Latin word).
            let words = belief_words(&belief_en, lang);
            if !words.is_empty() {
                p.payload["vocabulary"] = serde_json::json!(words);
            }
        }
    }
    // The displayed name follows the committed entry.
    let display = if is_motif {
        p.payload.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string()
    } else {
        p.payload
            .get("vocabulary")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    if !display.is_empty() {
        p.name = display;
    }
    let b = upper_first(&belief(lang, &belief_en));
    let who = peoples.join(", ");
    p.rationale = match (lang, is_motif) {
        (WLang::Ru, false) => format!("{b} — у народов: {who}. Символ для книги «Мифология»."),
        (WLang::Ru, true) => format!("{b} — у народов: {who}. Мотив для книги «Мифология»."),
        (WLang::Fr, false) => format!("{b} — chez : {who}. Un symbole pour le livre Mythologie."),
        (WLang::Fr, true) => format!("{b} — chez : {who}. Un motif pour le livre Mythologie."),
        (WLang::De, false) => format!("{b} — bei: {who}. Ein Symbol für das Buch Mythologie."),
        (WLang::De, true) => format!("{b} — bei: {who}. Ein Motiv für das Buch Mythologie."),
        (WLang::Es, false) => format!("{b} — entre: {who}. Un símbolo para el libro Mitología."),
        (WLang::Es, true) => format!("{b} — entre: {who}. Un motivo para el libro Mitología."),
        (WLang::En, _) => return,
    };
}

/// Content words of a declared belief, for a symbol's vocabulary: Unicode-aware,
/// lowercased, longer than three letters, not a stop-word of the language,
/// in order, de-duplicated, at most four.
fn belief_words(belief: &str, lang: WLang) -> Vec<String> {
    let name = match lang {
        WLang::En => "english",
        WLang::Ru => "russian",
        WLang::Fr => "french",
        WLang::De => "german",
        WLang::Es => "spanish",
    };
    let stop = crate::config::built_in_stop_words(name);
    let mut out: Vec<String> = Vec::new();
    for raw in belief.split(|c: char| !c.is_alphanumeric()) {
        let w = raw.to_lowercase();
        if w.chars().count() > 3 && !stop.contains(&w.as_str()) && !out.contains(&w) {
            out.push(w);
        }
        if out.len() == 4 {
            break;
        }
    }
    out
}

// ── what is committed to the author's books on accept ───────────────────────

/// The Place paragraph. `class` / `basis` / `biome` are the payload's canonical
/// keys. English is the sentence the committer always wrote.
pub fn place_prose(lang: WLang, name: &str, class_key: &str, pop: u64, basis_key: &str, biome_key: &str) -> String {
    let (c, b, z) = (class(lang, class_key), basis(lang, basis_key), biome(lang, biome_key));
    match lang {
        WLang::En => format!(
            "{name} is a {class_key} of roughly {pop} people, set at a {} in a {} zone.",
            basis_key.replace('_', " "),
            biome_key.replace('_', " ")
        ),
        WLang::Ru => format!("{name} — {c} с населением около {pop} человек. Расположение: {b}. Природная зона: {z}."),
        WLang::Fr => format!("{name} est une localité ({c}) d'environ {pop} habitants, située {b}. Milieu : {z}."),
        WLang::De => format!("{name} ist ein Ort ({c}) mit etwa {pop} Einwohnern, gelegen {b}. Naturraum: {z}."),
        WLang::Es => format!("{name} es una localidad ({c}) de unos {pop} habitantes, situada {b}. Entorno: {z}."),
    }
}

/// The ruler Character stub (everything above the provenance comment). `None`
/// for English — the generator's own sentence stands.
pub fn ruler_body(lang: WLang, name: &str, realm: &str, capital_en: &str, pop: u64, ethos_en: &str, belief_en: &str) -> Option<String> {
    if lang == WLang::En {
        return None;
    }
    let cap = capital(lang, capital_en);
    let has_cap = !capital_en.trim().is_empty();
    let mut body = match lang {
        WLang::Ru => {
            let c = if has_cap { format!("; столица: {cap}") } else { String::new() };
            format!("{name} правит державой {realm}{c}. Население — около {pop} человек.\n\n")
        }
        WLang::Fr => {
            let c = if has_cap { format!(" ; capitale : {cap}") } else { String::new() };
            format!("{name} règne sur {realm}{c}. Le royaume compte environ {pop} habitants.\n\n")
        }
        WLang::De => {
            let c = if has_cap { format!("; Hauptstadt: {cap}") } else { String::new() };
            format!("{name} herrscht über {realm}{c}. Das Reich zählt etwa {pop} Einwohner.\n\n")
        }
        WLang::Es => {
            let c = if has_cap { format!("; capital: {cap}") } else { String::new() };
            format!("{name} gobierna {realm}{c}. El reino cuenta con unos {pop} habitantes.\n\n")
        }
        WLang::En => unreachable!("handled above"),
    };
    if !ethos_en.is_empty() || !belief_en.is_empty() {
        let (e, b) = (ethos_or_default(lang, ethos_en, "settled and pragmatic"), belief_or_default(lang, belief_en));
        body.push_str(&match lang {
            WLang::Ru => format!("Народ: {e}. Вера: {b}.\n\n"),
            WLang::Fr => format!("Un peuple {e}. Croyance : {b}.\n\n"),
            WLang::De => format!("Ein Volk: {e}. Glaube: {b}.\n\n"),
            WLang::Es => format!("Un pueblo {e}. Creencia: {b}.\n\n"),
            WLang::En => unreachable!("handled above"),
        });
    }
    Some(body)
}

/// The language design brief (everything above the provenance comment). `None`
/// for English.
pub fn language_brief(lang: WLang, realm: &str, order: &str, morph: &str, sound: &str, sample: &str) -> Option<String> {
    let dash = |v: &str| if v.trim().is_empty() { "—".to_string() } else { v.trim().to_string() };
    let (o, m, s, n) = (dash(order), profile_term(lang, morph), profile_term(lang, sound), dash(sample));
    Some(match lang {
        WLang::En => return None,
        WLang::Ru => format!(
            "Мир предлагает этот язык для державы {realm}. Это *набросок*, а не готовый язык — воплотите его в разделе ConLang (Phonology, Grammar, Dictionary).\n\nПорядок слов: {o}\nМорфология: {m}\nЗвук: {s}\nПример имени: {n}\n\n"
        ),
        WLang::Fr => format!(
            "Le monde propose cette langue pour {realm}. C'est un *profil*, non une langue achevée — réalisez-la dans la suite ConLang (Phonology, Grammar, Dictionary).\n\nOrdre des mots : {o}\nMorphologie : {m}\nSonorité : {s}\nExemple de nom : {n}\n\n"
        ),
        WLang::De => format!(
            "Die Welt schlägt diese Sprache für {realm} vor. Es ist ein *Profil*, keine fertige Sprache — arbeiten Sie sie in der ConLang-Suite aus (Phonology, Grammar, Dictionary).\n\nWortstellung: {o}\nMorphologie: {m}\nKlang: {s}\nNamensbeispiel: {n}\n\n"
        ),
        WLang::Es => format!(
            "El mundo propone esta lengua para {realm}. Es un *perfil*, no una lengua acabada: desarróllela en la suite ConLang (Phonology, Grammar, Dictionary).\n\nOrden de palabras: {o}\nMorfología: {m}\nSonoridad: {s}\nEjemplo de nombre: {n}\n\n"
        ),
    })
}

/// " — held by A, B" appended to a motif's description.
pub fn held_by(lang: WLang, peoples: &[String]) -> String {
    let who = peoples.join(", ");
    match lang {
        WLang::En => format!(" — held by {who}"),
        WLang::Ru => format!(" — у народов: {who}"),
        WLang::Fr => format!(" — chez : {who}"),
        WLang::De => format!(" — bei: {who}"),
        WLang::Es => format!(" — entre: {who}"),
    }
}

/// A critique Note's `(title, body)`. `severity` is `high` / `medium` / `low`.
pub fn critique_note(lang: WLang, world: &str, aspect: &str, severity: &str, issue: &str, recommendation: &str) -> (String, String) {
    let sev: T5 = match severity {
        "high" => ["high", "высокая", "élevée", "hoch", "alta"],
        "low" => ["low", "низкая", "faible", "niedrig", "baja"],
        _ => ["medium", "средняя", "moyenne", "mittel", "media"],
    };
    let sv = sev[lang.idx()];
    match lang {
        WLang::En => (
            format!("World critique — {aspect} ({sv})"),
            format!(
                "Recommendation for the world `{world}` ({aspect}, {sv} severity).\n\nIssue: {issue}\n\nRecommendation: {recommendation}\n\n"
            ),
        ),
        WLang::Ru => (
            format!("Критика мира — {aspect} ({sv})"),
            format!("Рекомендация для мира `{world}` ({aspect}; важность: {sv}).\n\nПроблема: {issue}\n\nРекомендация: {recommendation}\n\n"),
        ),
        WLang::Fr => (
            format!("Critique du monde — {aspect} ({sv})"),
            format!("Recommandation pour le monde `{world}` ({aspect} ; gravité : {sv}).\n\nProblème : {issue}\n\nRecommandation : {recommendation}\n\n"),
        ),
        WLang::De => (
            format!("Weltkritik — {aspect} ({sv})"),
            format!("Empfehlung für die Welt `{world}` ({aspect}; Schwere: {sv}).\n\nProblem: {issue}\n\nEmpfehlung: {recommendation}\n\n"),
        ),
        WLang::Es => (
            format!("Crítica del mundo — {aspect} ({sv})"),
            format!("Recomendación para el mundo `{world}` ({aspect}; gravedad: {sv}).\n\nProblema: {issue}\n\nRecomendación: {recommendation}\n\n"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [WLang; 5] = [WLang::En, WLang::Ru, WLang::Fr, WLang::De, WLang::Es];

    #[test]
    fn every_table_entry_has_five_non_empty_terms_and_english_is_the_key() {
        for table in [CLASS, BIOME, ETHOS, BELIEF, PROFILE, MYTH_TEXT] {
            for (key, t) in table {
                for term in t {
                    assert!(!term.trim().is_empty(), "{key}");
                }
                // English is what the compile layer emits (biomes carry `_`).
                assert_eq!(t[0].replace(' ', "_"), key.replace(' ', "_"), "{key}");
            }
        }
        for (key, t) in BASIS {
            assert!(t.iter().all(|x| !x.is_empty()), "{key}");
        }
    }

    #[test]
    fn the_tables_cover_everything_the_compile_layers_generate() {
        use crate::world::types::climate::Biome;
        for b in [
            Biome::IceCap, Biome::Tundra, Biome::Taiga, Biome::TemperateForest, Biome::TemperateGrassland,
            Biome::Mediterranean, Biome::ColdDesert, Biome::HotDesert, Biome::Savanna, Biome::TropicalSeasonal,
            Biome::TropicalRainforest, Biome::Ocean,
        ] {
            assert!(look(BIOME, b.as_str(), WLang::Ru).is_some(), "biome {}", b.as_str());
        }
        for c in ["city", "town", "village"] {
            assert!(look(CLASS, c, WLang::De).is_some());
        }
        for b in ["river_mouth", "confluence", "fertile_valley"] {
            assert!(look(BASIS, b, WLang::Fr).is_some());
        }
        // A real compiled world: every ethos part, belief and profile term is known.
        let def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("T")).unwrap();
        let layers = crate::world::plausibility::compile_layers(&def);
        let seed = def.seed_u64();
        let pol = crate::world::compile::compile_polities(&layers.demographics, &[], seed);
        let biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                layers.demographics.settlements.iter().find(|s| (s.x, s.y) == q.capital_pos).map(|s| s.biome.clone()).unwrap_or_default()
            })
            .collect();
        let cul = crate::world::compile::compile_culture(&pol, &biomes, &[], seed);
        assert!(!cul.cultures.is_empty());
        for c in &cul.cultures {
            for part in c.ethos.split(',') {
                assert!(look(ETHOS, part.trim(), WLang::Ru).is_some(), "ethos part {part:?}");
            }
            assert!(look(BELIEF, &c.belief, WLang::Ru).is_some(), "belief {:?}", c.belief);
            for part in c.language_profile.split('·').skip(1) {
                assert!(look(PROFILE, part.trim(), WLang::Ru).is_some(), "profile term {part:?}");
            }
        }
    }

    #[test]
    fn unknown_values_pass_through_untouched() {
        assert_eq!(ethos(WLang::Ru, "люди моря, гордый"), "люди моря, гордый");
        assert_eq!(ethos(WLang::Ru, "sea-bound, proud"), "sea-bound, гордый");
        assert_eq!(belief(WLang::De, "the Drowned King"), "the Drowned King");
        assert_eq!(biome(WLang::Fr, "crystal_waste"), "crystal waste");
        assert_eq!(profile(WLang::Ru, "SOV · agglutinative · tonal"), "SOV · агглютинативная · тональный");
        assert_eq!(capital(WLang::Ru, "the temperate_forest city"), "город (умеренные леса)");
        assert_eq!(capital(WLang::Ru, "Stormhold"), "Stormhold");
    }

    fn proposal(kind: &str, payload: serde_json::Value) -> PlaceProposal {
        PlaceProposal {
            id: uuid::Uuid::new_v4(),
            signature: "sig".into(),
            kind: kind.into(),
            name: "Name".into(),
            payload,
            rationale: "ENGLISH".into(),
            status: "pending".into(),
            created_at: 1,
        }
    }

    #[test]
    fn english_is_left_exactly_as_generated() {
        let mut ps = vec![proposal("place", serde_json::json!({ "class": "city", "population": 23000, "basis": "river_mouth", "biome": "temperate_grassland" }))];
        let before = ps[0].clone();
        localize_all(&mut ps, WLang::En);
        assert_eq!(ps[0].rationale, before.rationale);
        assert_eq!(ps[0].payload, before.payload);
        assert_eq!(
            place_prose(WLang::En, "Korason", "city", 23000, "river_mouth", "temperate_grassland"),
            "Korason is a city of roughly 23000 people, set at a river mouth in a temperate grassland zone."
        );
        assert!(ruler_body(WLang::En, "X", "Y", "", 1, "", "").is_none());
        assert!(language_brief(WLang::En, "Y", "SOV", "isolating", "tonal", "Ka").is_none());
        assert_eq!(critique_note(WLang::En, "W", "climate", "high", "i", "r").0, "World critique — climate (high)");
    }

    #[test]
    fn proposals_read_in_the_project_language_with_no_english_left_in_them() {
        let english_leftovers = ["city", "river", "grassland", "proud", "veneration", "The ", " of ", "tonal ·"];
        for lang in &ALL[1..] {
            let mut ps = vec![
                proposal("place", serde_json::json!({ "class": "city", "population": 23000, "basis": "river_mouth", "biome": "temperate_grassland" })),
                proposal("character", serde_json::json!({ "realm": "Karon", "ethos": "woodland-reverent, proud", "belief": "ancestor veneration" })),
                proposal("language", serde_json::json!({ "realm": "Karon", "profile": "SOV · agglutinative · guttural", "naming_sample": "Kaeth" })),
                proposal("myth-symbol", serde_json::json!({ "myth_kind": "symbol", "belief": "ancestor veneration", "traditions": ["Karon"], "gloss": "the forebears watch, guide, and judge the living", "name": "", "vocabulary": ["ancestor"] })),
                proposal("myth-motif", serde_json::json!({ "myth_kind": "motif", "belief": "a cult of the seasons", "traditions": ["Serai"], "gloss": "rites and reversals bound to the turning of the seasons", "name": "the turning year", "vocabulary": [] })),
            ];
            localize_all(&mut ps, *lang);
            for p in &ps {
                assert_ne!(p.rationale, "ENGLISH", "{lang:?} {}", p.kind);
                for bad in english_leftovers {
                    assert!(!p.rationale.contains(bad), "{lang:?} {}: {:?} contains {bad:?}", p.kind, p.rationale);
                }
            }
            // The Mythology entry that will be committed is in the prose language.
            assert_ne!(ps[3].payload["gloss"], "the forebears watch, guide, and judge the living");
            assert_ne!(ps[3].payload["vocabulary"][0], "ancestor");
            assert_eq!(ps[3].name, ps[3].payload["vocabulary"][0].as_str().unwrap());
            assert_ne!(ps[4].payload["name"], "the turning year");
            assert_eq!(ps[4].name, ps[4].payload["name"].as_str().unwrap());
            // Canonical keys the fact-checker and dedup read are untouched.
            assert_eq!(ps[0].payload["class"], "city");
            assert_eq!(ps[0].payload["biome"], "temperate_grassland");
            assert_eq!(ps[1].payload["belief"], "ancestor veneration");
        }
    }

    #[test]
    fn a_declared_belief_in_russian_gets_a_real_vocabulary() {
        let mut ps = vec![proposal(
            "myth-symbol",
            serde_json::json!({ "myth_kind": "symbol", "belief": "Утонувший король под волнами", "traditions": ["Карон"],
                "gloss": "what these peoples hold sacred: Утонувший король под волнами", "name": "", "vocabulary": ["утонувший король под волнами"] }),
        )];
        localize_all(&mut ps, WLang::Ru);
        let vocab: Vec<&str> = ps[0].payload["vocabulary"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(vocab, ["утонувший", "король", "волнами"]);
        assert!(ps[0].payload["gloss"].as_str().unwrap().starts_with("то, что эти народы считают священным: "));
    }

    #[test]
    fn committed_prose_is_whole_sentences_in_each_language() {
        let ru = place_prose(WLang::Ru, "Корасон", "city", 23000, "river_mouth", "temperate_grassland");
        assert_eq!(ru, "Корасон — город с населением около 23000 человек. Расположение: в устье реки. Природная зона: умеренные степи.");
        let de = ruler_body(WLang::De, "Maela", "Karon", "the temperate_forest city", 90000, "woodland-reverent, proud", "a sky-pantheon").unwrap();
        assert_eq!(
            de,
            "Maela herrscht über Karon; Hauptstadt: Stadt (gemäßigter Wald). Das Reich zählt etwa 90000 Einwohner.\n\nEin Volk: waldverehrend, stolz. Glaube: ein Himmelspantheon.\n\n"
        );
        let fr = language_brief(WLang::Fr, "Karon", "SOV", "agglutinative", "liquid and vowel-rich", "").unwrap();
        assert!(fr.contains("Morphologie : agglutinante") && fr.contains("Sonorité : liquide et riche en voyelles") && fr.contains("Exemple de nom : —"));
        let (title, body) = critique_note(WLang::Es, "Tharn", "clima", "high", "p", "r");
        assert_eq!(title, "Crítica del mundo — clima (alta)");
        assert!(body.contains("gravedad: alta") && body.contains("Problema: p") && body.contains("Recomendación: r"));
        for lang in ALL {
            assert!(held_by(lang, &["A".into(), "B".into()]).ends_with("A, B"));
        }
    }
}

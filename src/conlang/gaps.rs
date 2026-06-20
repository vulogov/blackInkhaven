//! LANG-1 P6 — semantic-gap finder.
//!
//! Diff a language's lexicon coverage against a reference *concept scope* —
//! either the bundled **Swadesh-100** core vocabulary or an author-supplied
//! HJSON topic list — and report which concepts are still missing. Closes the
//! vocabulary-building loop: the missing list is exactly what `generate-lexicon`
//! should coin next.
//!
//! Glosses are written in the project working language, so the reference scope
//! must be too. The bundled Swadesh list is therefore translated across every
//! supported working language (en/ru/fr/de/es), and matching is Unicode-aware
//! (lowercasing + word-token containment), per the multilingual requirement.

use serde::Deserialize;

/// One concept the lexicon is checked against. `label` is the
/// working-language word shown in the report; `aliases` are alternative glosses
/// that also count as covering it.
#[derive(Debug, Clone)]
pub struct ScopeConcept {
    pub label: String,
    pub aliases: Vec<String>,
}

impl ScopeConcept {
    fn plain(label: &str) -> Self {
        ScopeConcept {
            label: label.to_string(),
            aliases: Vec::new(),
        }
    }
}

/// HJSON shape for a user `--scope <file>`: a named list of concepts, each a
/// bare string or `{ label, aliases }`.
#[derive(Debug, Deserialize)]
pub struct ScopeFile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub concepts: Vec<ConceptSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ConceptSpec {
    Bare(String),
    Rich {
        label: String,
        #[serde(default)]
        aliases: Vec<String>,
    },
}

impl ScopeFile {
    /// Parse a scope from HJSON (whole body, or a fenced ```hjson block).
    pub fn from_hjson(body: &str) -> Result<Self, String> {
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        serde_hjson::from_str::<Self>(block)
            .map_err(|e| format!("scope HJSON parse failed: {e}"))
    }

    pub fn into_concepts(self) -> Vec<ScopeConcept> {
        self.concepts
            .into_iter()
            .map(|c| match c {
                ConceptSpec::Bare(label) => ScopeConcept::plain(&label),
                ConceptSpec::Rich { label, aliases } => ScopeConcept { label, aliases },
            })
            .filter(|c| !c.label.trim().is_empty())
            .collect()
    }
}

/// The result of diffing a scope against the lexicon.
#[derive(Debug, Clone)]
pub struct GapReport {
    pub scope_name: String,
    pub total: usize,
    /// Concept labels present in the lexicon.
    pub covered: Vec<String>,
    /// Concept labels with no matching gloss — the work list.
    pub missing: Vec<String>,
}

impl GapReport {
    pub fn coverage_pct(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.covered.len() as f32 / self.total as f32 * 100.0
    }
}

/// Normalize a gloss/concept for matching: lowercase (Unicode-aware) and split
/// into alphanumeric word tokens, dropping a leading English/Romance infinitive
/// marker so "to drink" and "drink" align.
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Tokens worth ignoring when deciding coverage — articles / infinitive
/// markers across the supported working languages. A concept still matches if
/// any *content* token lines up, so "the sun" covers "sun" and "boire" covers
/// "to drink" only via the explicit alias, not via these.
fn is_stopword(t: &str) -> bool {
    matches!(
        t,
        "to" | "the" | "a" | "an" // english
            | "le" | "la" | "les" | "un" | "une" | "des" | "du" | "de" // french
            | "der" | "die" | "das" | "ein" | "eine" // german
            | "el" | "los" | "las" | "una" | "unos" | "unas" // spanish
    )
}

/// Diff `scope` against the `glosses` already in the lexicon. A concept counts
/// as covered when its label (or any alias), reduced to content tokens, is a
/// subset of some gloss's content tokens — so multi-word glosses and articles
/// don't hide a match. Order of `missing` follows the scope order (Swadesh's is
/// frequency-ranked), so the report reads most-core-first.
pub fn find_gaps(scope_name: &str, scope: &[ScopeConcept], glosses: &[String]) -> GapReport {
    // Pre-tokenize every gloss into a content-token set.
    let gloss_sets: Vec<std::collections::BTreeSet<String>> = glosses
        .iter()
        .map(|g| {
            tokens(g)
                .into_iter()
                .filter(|t| !is_stopword(t))
                .collect()
        })
        .collect();

    let matches = |phrase: &str| -> bool {
        let want: Vec<String> = tokens(phrase)
            .into_iter()
            .filter(|t| !is_stopword(t))
            .collect();
        if want.is_empty() {
            return false;
        }
        gloss_sets
            .iter()
            .any(|gs| want.iter().all(|w| gs.contains(w)))
    };

    let mut covered = Vec::new();
    let mut missing = Vec::new();
    for c in scope {
        let hit = matches(&c.label) || c.aliases.iter().any(|a| matches(a));
        if hit {
            covered.push(c.label.clone());
        } else {
            missing.push(c.label.clone());
        }
    }
    GapReport {
        scope_name: scope_name.to_string(),
        total: scope.len(),
        covered,
        missing,
    }
}

/// Is `name` a recognised built-in scope?
pub fn is_builtin(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "swadesh" | "swadesh_100" | "swadesh100"
    )
}

/// The bundled Swadesh-100 core vocabulary, projected into the given working
/// language (`english`/`russian`/`french`/`german`/`spanish`; unknown →
/// English). The list is frequency-ranked, so a partial lexicon's gaps surface
/// the most fundamental missing words first.
pub fn swadesh_100(working_language: &str) -> Vec<ScopeConcept> {
    // Column order: en, ru, fr, de, es.
    const ROWS: &[[&str; 5]] = &[
        ["I", "я", "je", "ich", "yo"],
        ["you", "ты", "tu", "du", "tú"],
        ["we", "мы", "nous", "wir", "nosotros"],
        ["this", "этот", "ce", "dies", "este"],
        ["that", "тот", "cela", "das", "eso"],
        ["who", "кто", "qui", "wer", "quién"],
        ["what", "что", "quoi", "was", "qué"],
        ["not", "не", "ne", "nicht", "no"],
        ["all", "все", "tout", "alle", "todo"],
        ["many", "много", "beaucoup", "viele", "muchos"],
        ["one", "один", "un", "eins", "uno"],
        ["two", "два", "deux", "zwei", "dos"],
        ["big", "большой", "grand", "groß", "grande"],
        ["long", "длинный", "long", "lang", "largo"],
        ["small", "маленький", "petit", "klein", "pequeño"],
        ["woman", "женщина", "femme", "Frau", "mujer"],
        ["man", "мужчина", "homme", "Mann", "hombre"],
        ["person", "человек", "personne", "Mensch", "persona"],
        ["fish", "рыба", "poisson", "Fisch", "pez"],
        ["bird", "птица", "oiseau", "Vogel", "pájaro"],
        ["dog", "собака", "chien", "Hund", "perro"],
        ["louse", "вошь", "pou", "Laus", "piojo"],
        ["tree", "дерево", "arbre", "Baum", "árbol"],
        ["seed", "семя", "graine", "Samen", "semilla"],
        ["leaf", "лист", "feuille", "Blatt", "hoja"],
        ["root", "корень", "racine", "Wurzel", "raíz"],
        ["bark", "кора", "écorce", "Rinde", "corteza"],
        ["skin", "кожа", "peau", "Haut", "piel"],
        ["flesh", "мясо", "chair", "Fleisch", "carne"],
        ["blood", "кровь", "sang", "Blut", "sangre"],
        ["bone", "кость", "os", "Knochen", "hueso"],
        ["grease", "жир", "graisse", "Fett", "grasa"],
        ["egg", "яйцо", "œuf", "Ei", "huevo"],
        ["horn", "рог", "corne", "Horn", "cuerno"],
        ["tail", "хвост", "queue", "Schwanz", "cola"],
        ["feather", "перо", "plume", "Feder", "pluma"],
        ["hair", "волосы", "cheveux", "Haar", "pelo"],
        ["head", "голова", "tête", "Kopf", "cabeza"],
        ["ear", "ухо", "oreille", "Ohr", "oreja"],
        ["eye", "глаз", "œil", "Auge", "ojo"],
        ["nose", "нос", "nez", "Nase", "nariz"],
        ["mouth", "рот", "bouche", "Mund", "boca"],
        ["tooth", "зуб", "dent", "Zahn", "diente"],
        ["tongue", "язык", "langue", "Zunge", "lengua"],
        ["claw", "коготь", "griffe", "Kralle", "garra"],
        ["foot", "нога", "pied", "Fuß", "pie"],
        ["knee", "колено", "genou", "Knie", "rodilla"],
        ["hand", "рука", "main", "Hand", "mano"],
        ["belly", "живот", "ventre", "Bauch", "vientre"],
        ["neck", "шея", "cou", "Hals", "cuello"],
        ["breast", "грудь", "sein", "Brust", "pecho"],
        ["heart", "сердце", "cœur", "Herz", "corazón"],
        ["liver", "печень", "foie", "Leber", "hígado"],
        ["drink", "пить", "boire", "trinken", "beber"],
        ["eat", "есть", "manger", "essen", "comer"],
        ["bite", "кусать", "mordre", "beißen", "morder"],
        ["see", "видеть", "voir", "sehen", "ver"],
        ["hear", "слышать", "entendre", "hören", "oír"],
        ["know", "знать", "savoir", "wissen", "saber"],
        ["sleep", "спать", "dormir", "schlafen", "dormir"],
        ["die", "умирать", "mourir", "sterben", "morir"],
        ["kill", "убивать", "tuer", "töten", "matar"],
        ["swim", "плавать", "nager", "schwimmen", "nadar"],
        ["fly", "летать", "voler", "fliegen", "volar"],
        ["walk", "ходить", "marcher", "gehen", "caminar"],
        ["come", "приходить", "venir", "kommen", "venir"],
        ["lie", "лежать", "coucher", "liegen", "yacer"],
        ["sit", "сидеть", "asseoir", "sitzen", "sentarse"],
        ["stand", "стоять", "debout", "stehen", "estar de pie"],
        ["give", "давать", "donner", "geben", "dar"],
        ["say", "говорить", "dire", "sagen", "decir"],
        ["sun", "солнце", "soleil", "Sonne", "sol"],
        ["moon", "луна", "lune", "Mond", "luna"],
        ["star", "звезда", "étoile", "Stern", "estrella"],
        ["water", "вода", "eau", "Wasser", "agua"],
        ["rain", "дождь", "pluie", "Regen", "lluvia"],
        ["stone", "камень", "pierre", "Stein", "piedra"],
        ["sand", "песок", "sable", "Sand", "arena"],
        ["earth", "земля", "terre", "Erde", "tierra"],
        ["cloud", "облако", "nuage", "Wolke", "nube"],
        ["smoke", "дым", "fumée", "Rauch", "humo"],
        ["fire", "огонь", "feu", "Feuer", "fuego"],
        ["ash", "пепел", "cendre", "Asche", "ceniza"],
        ["burn", "гореть", "brûler", "brennen", "quemar"],
        ["path", "дорога", "chemin", "Weg", "camino"],
        ["mountain", "гора", "montagne", "Berg", "montaña"],
        ["red", "красный", "rouge", "rot", "rojo"],
        ["green", "зелёный", "vert", "grün", "verde"],
        ["yellow", "жёлтый", "jaune", "gelb", "amarillo"],
        ["white", "белый", "blanc", "weiß", "blanco"],
        ["black", "чёрный", "noir", "schwarz", "negro"],
        ["night", "ночь", "nuit", "Nacht", "noche"],
        ["hot", "горячий", "chaud", "heiß", "caliente"],
        ["cold", "холодный", "froid", "kalt", "frío"],
        ["full", "полный", "plein", "voll", "lleno"],
        ["new", "новый", "neuf", "neu", "nuevo"],
        ["good", "хороший", "bon", "gut", "bueno"],
        ["round", "круглый", "rond", "rund", "redondo"],
        ["dry", "сухой", "sec", "trocken", "seco"],
        ["name", "имя", "nom", "Name", "nombre"],
    ];
    let col = match working_language.trim().to_ascii_lowercase().as_str() {
        "russian" | "ru" => 1,
        "french" | "fr" => 2,
        "german" | "de" => 3,
        "spanish" | "es" => 4,
        _ => 0, // english + fallback
    };
    ROWS.iter().map(|r| ScopeConcept::plain(r[col])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swadesh_is_100_per_language() {
        for lang in ["english", "russian", "french", "german", "spanish"] {
            assert_eq!(swadesh_100(lang).len(), 100, "{lang}");
        }
        // unknown language falls back to english
        assert_eq!(swadesh_100("klingon")[0].label, "I");
        assert_eq!(swadesh_100("russian")[0].label, "я");
    }

    #[test]
    fn gaps_match_through_articles_and_multiword() {
        let scope = swadesh_100("english");
        // gloss the sun, two stones, a bird → those three covered
        let glosses = vec![
            "the sun".to_string(),
            "two".to_string(),
            "a bird".to_string(),
            "to give".to_string(),
        ];
        let r = find_gaps("swadesh_100", &scope, &glosses);
        assert!(r.covered.contains(&"sun".to_string()));
        assert!(r.covered.contains(&"two".to_string()));
        assert!(r.covered.contains(&"bird".to_string()));
        assert!(r.covered.contains(&"give".to_string()));
        assert!(r.missing.contains(&"water".to_string()));
        // ranking: missing list keeps Swadesh order (most-core first)
        assert_eq!(r.missing[0], "I");
    }

    #[test]
    fn coverage_pct_and_russian_matching() {
        let scope = swadesh_100("russian");
        let glosses = vec!["солнце".to_string(), "вода".to_string()];
        let r = find_gaps("swadesh", &scope, &glosses);
        assert!(r.covered.contains(&"солнце".to_string()));
        assert!(r.covered.contains(&"вода".to_string()));
        assert_eq!(r.covered.len(), 2);
        assert!((r.coverage_pct() - 2.0).abs() < 0.01);
    }

    #[test]
    fn scope_file_parses_bare_and_rich() {
        let body = r#"{ name: "Seafaring", concepts: ["hull", { label: "tide", aliases: ["ebb"] }] }"#;
        let scope = ScopeFile::from_hjson(body).expect("parse");
        assert_eq!(scope.name.as_deref(), Some("Seafaring"));
        let concepts = scope.into_concepts();
        assert_eq!(concepts.len(), 2);
        assert_eq!(concepts[1].label, "tide");
        assert_eq!(concepts[1].aliases, vec!["ebb".to_string()]);
        // a gloss "ebb" covers "tide" via its alias
        let r = find_gaps("Seafaring", &concepts, &["ebb".to_string()]);
        assert!(r.covered.contains(&"tide".to_string()));
        assert!(r.missing.contains(&"hull".to_string()));
    }
}

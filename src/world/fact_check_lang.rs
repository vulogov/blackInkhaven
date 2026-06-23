//! WORLD-4 — per-language pattern tables for the fast fact-checker (RFC §3.7).
//! The five baseline languages (EN, RU, ES, FR, DE) each get their own units,
//! number words, weather vocabulary, and so on, so the checks fire on prose
//! written in the author's language. The paragraph's language is detected with
//! `whatlang` (reusing the project's existing `iso_from_alpha3` mapping); an
//! unsupported language falls back to English vocabulary (and the slow LLM track
//! covers what the patterns miss).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    Es,
    Fr,
    De,
}

/// Detect a paragraph's language, mapping to one of the five baselines (default
/// English). Reuses the same whatlang → ISO mapping the prompt system uses.
pub fn detect(text: &str) -> Lang {
    // A script + function-word heuristic, deliberately not `whatlang`: for the
    // *fixed set* of five baselines this is far more robust than a general
    // detector, which confuses sister languages (Russian↔Bulgarian,
    // Spanish↔Portuguese) on short prose. Cyrillic ⇒ Russian (the only Cyrillic
    // baseline); otherwise score the Latin baselines by their distinctive
    // function words and take the best (English on a tie or no signal).
    let lower = text.to_lowercase();
    let cyrillic = lower.chars().filter(|c| ('а'..='я').contains(c) || *c == 'ё').count();
    if cyrillic >= 3 {
        return Lang::Ru;
    }
    let score = |words: &[&str]| words.iter().filter(|w| contains_word(&lower, w)).count();
    let de = score(&["der", "das", "und", "ein", "eine", "durch", "über", "ohne", "nicht", "mit", "den", "dem", "ist", "auch"]);
    let fr = score(&["le", "les", "et", "une", "sans", "dans", "pour", "avec", "est", "du", "ne", "qui", "trois", "jours"]);
    let es = score(&["el", "los", "las", "sin", "por", "con", "para", "una", "del", "muy", "tres", "días", "pero", "como"]);
    let en = score(&["the", "and", "of", "to", "in", "for", "with", "that", "was", "three", "days", "she", "he"]);
    // English last so it wins ties (max_by_key keeps the last maximum).
    let best = [(Lang::De, de), (Lang::Fr, fr), (Lang::Es, es), (Lang::En, en)]
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0);
    match best {
        Some((lang, _)) => lang,
        None => Lang::En,
    }
}

/// Whole-word containment that is Unicode-aware (so Cyrillic / accented words
/// match correctly, and `Or` doesn't fire inside `Orenarm`).
pub fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok =
            haystack[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after_ok = haystack[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        // Advance past this position by one character in the haystack.
        from = start + haystack[start..].chars().next().map_or(1, |c| c.len_utf8());
    }
    false
}

impl Lang {
    /// Spelled-out numbers 1–12 → value, for durations + moon counts.
    pub fn numbers(&self) -> &'static [(&'static str, f32)] {
        match self {
            Lang::En => &[
                ("one", 1.0), ("two", 2.0), ("three", 3.0), ("four", 4.0), ("five", 5.0),
                ("six", 6.0), ("seven", 7.0), ("eight", 8.0), ("nine", 9.0), ("ten", 10.0),
                ("eleven", 11.0), ("twelve", 12.0),
            ],
            Lang::Ru => &[
                ("один", 1.0), ("одна", 1.0), ("одно", 1.0), ("два", 2.0), ("две", 2.0),
                ("три", 3.0), ("четыре", 4.0), ("пять", 5.0), ("шесть", 6.0), ("семь", 7.0),
                ("восемь", 8.0), ("девять", 9.0), ("десять", 10.0), ("одиннадцать", 11.0),
                ("двенадцать", 12.0),
            ],
            Lang::Es => &[
                ("uno", 1.0), ("una", 1.0), ("dos", 2.0), ("tres", 3.0), ("cuatro", 4.0),
                ("cinco", 5.0), ("seis", 6.0), ("siete", 7.0), ("ocho", 8.0), ("nueve", 9.0),
                ("diez", 10.0), ("once", 11.0), ("doce", 12.0),
            ],
            Lang::Fr => &[
                ("un", 1.0), ("une", 1.0), ("deux", 2.0), ("trois", 3.0), ("quatre", 4.0),
                ("cinq", 5.0), ("six", 6.0), ("sept", 7.0), ("huit", 8.0), ("neuf", 9.0),
                ("dix", 10.0), ("onze", 11.0), ("douze", 12.0),
            ],
            Lang::De => &[
                ("ein", 1.0), ("eine", 1.0), ("eins", 1.0), ("zwei", 2.0), ("drei", 3.0),
                ("vier", 4.0), ("fünf", 5.0), ("sechs", 6.0), ("sieben", 7.0), ("acht", 8.0),
                ("neun", 9.0), ("zehn", 10.0), ("elf", 11.0), ("zwölf", 12.0),
            ],
        }
    }

    pub fn day_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["day", "days"],
            Lang::Ru => &["день", "дня", "дней"],
            Lang::Es => &["día", "días", "dia", "dias"],
            Lang::Fr => &["jour", "jours"],
            Lang::De => &["Tag", "Tage", "Tagen"],
        }
    }

    pub fn week_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["week", "weeks"],
            Lang::Ru => &["неделя", "недели", "недель"],
            Lang::Es => &["semana", "semanas"],
            Lang::Fr => &["semaine", "semaines"],
            Lang::De => &["Woche", "Wochen"],
        }
    }

    /// Distance unit groups → km conversion factor.
    pub fn distance_units(&self) -> &'static [(&'static [&'static str], f32)] {
        match self {
            Lang::En => &[
                (&["km", "kilometre", "kilometres", "kilometer", "kilometers"], 1.0),
                (&["mile", "miles", "mi"], 1.609),
                (&["league", "leagues"], 4.828),
            ],
            Lang::Ru => &[
                (&["км", "километр", "километра", "километров"], 1.0),
                (&["миля", "мили", "миль"], 1.609),
                (&["лига", "лиги", "лиг"], 4.828),
            ],
            Lang::Es => &[
                (&["km", "kilómetro", "kilómetros", "kilometro", "kilometros"], 1.0),
                (&["milla", "millas"], 1.609),
                (&["legua", "leguas"], 4.828),
            ],
            Lang::Fr => &[
                (&["km", "kilomètre", "kilomètres", "kilometre", "kilometres"], 1.0),
                (&["lieue", "lieues"], 4.0),
            ],
            Lang::De => &[
                (&["km", "Kilometer"], 1.0),
                (&["Meile", "Meilen"], 1.609),
            ],
        }
    }

    pub fn cold_weather(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["snow", "snowed", "snowing", "frost", "freezing", "blizzard", "frozen"],
            Lang::Ru => &["снег", "снегопад", "мороз", "метель", "замёрз", "замерз", "вьюга"],
            Lang::Es => &["nieve", "nevó", "nevaba", "helada", "ventisca", "congelado", "escarcha"],
            Lang::Fr => &["neige", "neigé", "neigeait", "gel", "blizzard", "gelé", "givre"],
            Lang::De => &["Schnee", "Schneesturm", "Frost", "gefroren", "Eissturm"],
        }
    }

    pub fn hot_weather(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["sweltering", "scorching", "tropical heat", "jungle heat", "blistering"],
            Lang::Ru => &["зной", "знойный", "тропическая жара", "палящее солнце", "жарища"],
            Lang::Es => &["sofocante", "abrasador", "calor tropical", "bochorno"],
            Lang::Fr => &["étouffante", "chaleur tropicale", "canicule", "torride"],
            Lang::De => &["schwül", "sengende", "tropische Hitze", "glühende Hitze"],
        }
    }

    pub fn thousand_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["thousand"],
            Lang::Ru => &["тысяча", "тысячи", "тысяч"],
            Lang::Es => &["mil"],
            Lang::Fr => &["mille"],
            Lang::De => &["Tausend"],
        }
    }

    pub fn million_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["million"],
            Lang::Ru => &["миллион", "миллиона", "миллионов"],
            Lang::Es => &["millón", "millones", "millon"],
            Lang::Fr => &["million", "millions"],
            Lang::De => &["Million", "Millionen"],
        }
    }

    pub fn moon_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["moon", "moons"],
            Lang::Ru => &["луна", "луны", "лун"],
            Lang::Es => &["luna", "lunas"],
            Lang::Fr => &["lune", "lunes"],
            Lang::De => &["Mond", "Monde"],
        }
    }

    pub fn extraction_words(&self) -> &'static [&'static str] {
        match self {
            Lang::En => &["mine", "mines", "mining", "mined", "ore", "vein", "veins", "quarry", "smelt", "smelting", "deposits"],
            Lang::Ru => &["рудник", "рудники", "шахта", "руда", "руду", "жила", "карьер", "добыча", "плавка", "месторождение"],
            Lang::Es => &["mina", "minas", "minería", "mineral", "veta", "cantera", "fundición", "yacimiento"],
            Lang::Fr => &["mine", "mines", "minerai", "filon", "carrière", "fonderie", "gisement"],
            Lang::De => &["Mine", "Bergwerk", "Erz", "Ader", "Steinbruch", "Schmelze", "Lagerstätte"],
        }
    }

    /// Canonical mineral (the English label the world stores) → its names in this
    /// language.
    pub fn metals(&self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Lang::En => &[
                ("gold", &["gold"]), ("silver", &["silver"]), ("iron", &["iron"]),
                ("copper", &["copper"]), ("coal", &["coal"]), ("tin", &["tin"]), ("lead", &["lead"]),
            ],
            Lang::Ru => &[
                ("gold", &["золото", "золота"]), ("silver", &["серебро", "серебра"]),
                ("iron", &["железо", "железа"]), ("copper", &["медь", "меди"]),
                ("coal", &["уголь", "угля"]), ("tin", &["олово"]), ("lead", &["свинец"]),
            ],
            Lang::Es => &[
                ("gold", &["oro"]), ("silver", &["plata"]), ("iron", &["hierro"]),
                ("copper", &["cobre"]), ("coal", &["carbón", "carbon"]), ("tin", &["estaño"]),
                ("lead", &["plomo"]),
            ],
            Lang::Fr => &[
                ("gold", &["or"]), ("silver", &["argent"]), ("iron", &["fer"]),
                ("copper", &["cuivre"]), ("coal", &["charbon"]), ("tin", &["étain"]),
                ("lead", &["plomb"]),
            ],
            Lang::De => &[
                ("gold", &["Gold"]), ("silver", &["Silber"]), ("iron", &["Eisen"]),
                ("copper", &["Kupfer"]), ("coal", &["Kohle"]), ("tin", &["Zinn"]),
                ("lead", &["Blei"]),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_baseline_languages() {
        // Longer, unambiguous samples (whatlang is unreliable on short text).
        assert_eq!(
            detect("Гонец скакал три долгих дня без отдыха, пересекая высокие горы и широкие реки."),
            Lang::Ru
        );
        assert_eq!(
            detect("Der Bote ritt drei lange Tage ohne Rast durch das weite Land und über die hohen Berge."),
            Lang::De
        );
        // Short or ambiguous text falls back to English — that's intended.
        assert_eq!(detect("rode 600 km"), Lang::En);
    }

    #[test]
    fn unicode_word_matching() {
        assert!(contains_word("за три дня", "три"));
        assert!(!contains_word("материк", "три")); // not a substring match
        assert!(contains_word("в шахте добывали", "шахте"));
    }
}

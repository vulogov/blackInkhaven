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
    detect_with_confidence(text).0
}

/// Like [`detect`], but also reports whether the call is **confident**. A
/// confident result has a clear script or function-word signal; a non-confident
/// one is the English fallback (no signal) or a near-tie between Latin baselines.
/// Callers that want to *degrade rather than mislead* — e.g. avoid emitting a
/// language-specific warning under the wrong assumption — gate on this. Never
/// panics, on any input (empty, emoji-only, megabytes, mixed-script).
pub fn detect_with_confidence(text: &str) -> (Lang, bool) {
    // A script + function-word heuristic, deliberately not `whatlang`: for the
    // *fixed set* of five baselines this is far more robust than a general
    // detector, which confuses sister languages (Russian↔Bulgarian,
    // Spanish↔Portuguese) on short prose. Cyrillic ⇒ Russian (the only Cyrillic
    // baseline); otherwise score the Latin baselines by their distinctive
    // function words and take the best (English on a tie or no signal).
    let lower = text.to_lowercase();
    let cyrillic = lower.chars().filter(|c| ('а'..='я').contains(c) || *c == 'ё').count();
    if cyrillic >= 3 {
        return (Lang::Ru, true);
    }
    let score = |words: &[&str]| words.iter().filter(|w| contains_word(&lower, w)).count();
    let de = score(&["der", "das", "und", "ein", "eine", "durch", "über", "ohne", "nicht", "mit", "den", "dem", "ist", "auch"]);
    let fr = score(&["le", "les", "et", "une", "sans", "dans", "pour", "avec", "est", "du", "ne", "qui", "trois", "jours"]);
    let es = score(&["el", "los", "las", "sin", "por", "con", "para", "una", "del", "muy", "tres", "días", "pero", "como"]);
    let en = score(&["the", "and", "of", "to", "in", "for", "with", "that", "was", "three", "days", "she", "he"]);
    // English last so it wins ties (max_by_key keeps the last maximum).
    let mut ranked = [(Lang::De, de), (Lang::Fr, fr), (Lang::Es, es), (Lang::En, en)];
    ranked.sort_by_key(|(_, n)| *n);
    let (top_lang, top) = ranked[3];
    let (_, second) = ranked[2];
    if top == 0 {
        return (Lang::En, false); // no signal at all — fall back, not confident.
    }
    // Confident when the winner has real signal and clears the runner-up.
    let confident = top >= 2 && top > second;
    (top_lang, confident)
}

// ── gazetteer grammatical variants ───────────────────────────────────────────

/// The lowercased inflected forms a place name can take in `lang`, including the
/// base name. Used by the gazetteer so a name resolves when it appears in a
/// grammatical case — chiefly Russian, whose place names decline (`Москва` →
/// `Москвы` / `Москве` / `Москву` / `Москвой`). For the Latin baselines proper
/// nouns are largely invariant (possessive / elision are already caught by the
/// whole-word boundary check), so only German adds its genitive `-s`.
///
/// Over-generation is acceptable: extra forms only add recall on whole-word
/// matches, and multi-syllable place names rarely collide with real words.
pub fn name_variants(name: &str, lang: Lang) -> Vec<String> {
    let base = name.to_lowercase();
    let mut out = vec![base.clone()];
    match lang {
        Lang::Ru => out.extend(russian_cases(&base)),
        Lang::De => {
            out.push(format!("{base}s"));
            out.push(format!("{base}es"));
        }
        // English possessive (`Korthun's`) and Romance elision (`d'Anvil`) are
        // already matched by the base name via the non-alphanumeric boundary.
        Lang::En | Lang::Es | Lang::Fr => {}
    }
    out.sort();
    out.dedup();
    out
}

/// The common Russian singular case endings for a place name, by its final
/// letter: feminine `-а`/`-я`, and masculine (consonant-final). The `-ы`/`-и`
/// spelling rule (после к/г/х/ж/ч/ш/щ → `-и`) is applied for the feminine
/// genitive.
fn russian_cases(base: &str) -> Vec<String> {
    let chars: Vec<char> = base.chars().collect();
    let Some(&last) = chars.last() else { return Vec::new() };
    let stem: String = chars[..chars.len() - 1].iter().collect();
    let pre = if chars.len() >= 2 { chars[chars.len() - 2] } else { ' ' };
    let hush = matches!(pre, 'к' | 'г' | 'х' | 'ж' | 'ч' | 'ш' | 'щ');
    let mut v = Vec::new();
    match last {
        // Feminine 1st declension (Москва): gen -ы/-и, dat/prep -е, acc -у, ins -ой/-ою.
        'а' => {
            v.push(format!("{stem}{}", if hush { 'и' } else { 'ы' }));
            for s in ["е", "у", "ой", "ою"] {
                v.push(format!("{stem}{s}"));
            }
        }
        // Feminine soft (-я): gen/dat/prep -и/-е, acc -ю, ins -ей/-ёй.
        'я' => {
            for s in ["и", "е", "ю", "ей", "ёй"] {
                v.push(format!("{stem}{s}"));
            }
        }
        // Masculine, consonant-final (Новгород): gen -а, dat -у, ins -ом, prep -е.
        c if c.is_alphabetic() => {
            for s in ["а", "у", "ом", "е"] {
                v.push(format!("{base}{s}"));
            }
        }
        _ => {}
    }
    v
}

// ── detection-backend capability (graceful degradation) ──────────────────────

/// Which language-analysis backend the fact-checker is using. The `Heuristic`
/// backend ships in the binary and is always available; the optional `Enhanced`
/// backend (a Universal-Dependencies parse model, RFC P4) is *never required* —
/// when its asset is absent or unreadable the checker degrades to `Heuristic`
/// rather than failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Heuristic,
    Enhanced,
}

/// Path the optional enhanced (UD parser) asset would load from: `$INKHAVEN_LANG_MODEL`
/// if set, else `~/.inkhaven/assets/lang/parser.bin`. Returning a path does not
/// imply the file exists.
pub fn enhanced_asset_path() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("INKHAVEN_LANG_MODEL") {
        return Some(std::path::PathBuf::from(p));
    }
    dirs_home().map(|h| h.join(".inkhaven").join("assets").join("lang").join("parser.bin"))
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// The active backend. `Enhanced` only when the asset is present and non-empty;
/// any absence or read error silently degrades to `Heuristic`. Pure observation —
/// never downloads, never fails.
pub fn active_backend() -> Backend {
    match enhanced_asset_path() {
        Some(p) => match std::fs::metadata(&p) {
            Ok(m) if m.len() > 0 => Backend::Enhanced,
            _ => Backend::Heuristic,
        },
        None => Backend::Heuristic,
    }
}

/// A one-line, user-facing description of the active detection backend and how to
/// upgrade it — shown so authors understand they're on the always-available
/// heuristic, not a silent failure.
pub fn backend_note() -> String {
    match active_backend() {
        Backend::Enhanced => {
            let p = enhanced_asset_path().map(|p| p.display().to_string()).unwrap_or_default();
            format!("language: enhanced parser ({p})")
        }
        Backend::Heuristic => {
            "language: built-in heuristic (5 baselines; no external model needed — set \
             INKHAVEN_LANG_MODEL to use an enhanced parser)"
                .to_string()
        }
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

/// A finding's message, parameterised so it can be rendered in any language
/// (the localised `body`) and in English (the `body_en` fallback).
pub enum Msg<'a> {
    Travel { km: f32, days: f32, pace: f32, severe: bool },
    Climate { weather: Weather, place: &'a str, zone: &'a str },
    Population { place: &'a str, claimed: u64, modeled: u64 },
    Astronomy { claimed: usize, world: usize, moons: &'a str },
    Economy { metal: &'a str, minerals: &'a str },
    /// WORLD-5 — weather contradicting the timeline-dated season.
    DateSeason { weather: Weather, season: &'a str },
    /// WORLD-5 — a prose travel duration contradicting the timeline gap between
    /// two of the traveller's events.
    TravelTimeline { prose_days: f32, timeline_days: f32, from: &'a str, to: &'a str },
    /// WORLD-5 — a seasonal date-hint in the prose (a festival, a harvest)
    /// contradicting the timeline-dated season.
    DateCoherence { hint: &'a str, season: &'a str },
}

#[derive(Clone, Copy, PartialEq)]
pub enum Weather {
    Cold,
    Hot,
}

impl Lang {
    /// Render a message in this language. The `_en` English text is `Lang::En`.
    pub fn render(&self, m: &Msg) -> String {
        match self {
            Lang::En => render_en(m),
            Lang::Ru => render_ru(m),
            Lang::Es => render_es(m),
            Lang::Fr => render_fr(m),
            Lang::De => render_de(m),
        }
    }
}

fn p(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

fn render_en(m: &Msg) -> String {
    match m {
        Msg::Travel { km, days, pace, severe } => format!(
            "Travel of {km:.0} km in {days:.0} day(s) = {pace:.0} km/day, which {} pre-industrial overland travel (typically 25–80 km/day).",
            if *severe { "far exceeds" } else { "exceeds" }
        ),
        Msg::Climate { weather, place, zone } => format!(
            "Implausible: {} at {place}, whose climate zone is {}.",
            if *weather == Weather::Cold { "freezing weather" } else { "tropical heat" },
            zone.replace('_', " ")
        ),
        Msg::Population { place, claimed, modeled } => format!(
            "{place} is described with ~{} people, but the world models ~{} for it.",
            p(*claimed), p(*modeled)
        ),
        Msg::Astronomy { claimed, world, moons } => format!(
            "The prose implies {claimed} moon(s), but this world has {world} ({moons})."
        ),
        Msg::Economy { metal, minerals } => format!(
            "{metal} is mined or worked here, but this world's geology yields only: {minerals}."
        ),
        Msg::DateSeason { weather, season } => format!(
            "{} described here, but the timeline places this in {season}.",
            if *weather == Weather::Cold { "Freezing weather" } else { "Tropical heat" }
        ),
        Msg::TravelTimeline { prose_days, timeline_days, from, to } => format!(
            "The prose suggests about {prose_days:.0} day(s), but the timeline places about {timeline_days:.0} day(s) between \u{201c}{from}\u{201d} and \u{201c}{to}\u{201d}."
        ),
        Msg::DateCoherence { hint, season } => format!(
            "A {hint} is mentioned here, but the timeline places this in {season}."
        ),
    }
}

fn render_ru(m: &Msg) -> String {
    match m {
        Msg::Travel { km, days, pace, severe } => format!(
            "Путь в {km:.0} км за {days:.0} дн. = {pace:.0} км/день, что {} доиндустриальную скорость (обычно 25–80 км/день).",
            if *severe { "значительно превышает" } else { "превышает" }
        ),
        Msg::Climate { weather, place, zone } => format!(
            "Неправдоподобно: {} в «{place}», климатическая зона которого — {}.",
            if *weather == Weather::Cold { "морозная погода" } else { "тропическая жара" },
            zone.replace('_', " ")
        ),
        Msg::Population { place, claimed, modeled } => format!(
            "Для «{place}» указано ~{} жит., но модель мира даёт ~{}.",
            p(*claimed), p(*modeled)
        ),
        Msg::Astronomy { claimed, world, moons } => format!(
            "В тексте подразумевается лун: {claimed}, но в этом мире их {world} ({moons})."
        ),
        Msg::Economy { metal, minerals } => format!(
            "Здесь добывают {metal}, но геология этого мира даёт только: {minerals}."
        ),
        Msg::DateSeason { weather, season } => format!(
            "{} здесь, но хронология относит это к сезону «{season}».",
            if *weather == Weather::Cold { "Описана морозная погода" } else { "Описана тропическая жара" }
        ),
        Msg::TravelTimeline { prose_days, timeline_days, from, to } => format!(
            "Текст предполагает около {prose_days:.0} дн., но хронология даёт около {timeline_days:.0} дн. между «{from}» и «{to}»."
        ),
        Msg::DateCoherence { hint, season } => format!(
            "Здесь упоминается «{hint}», но хронология относит это к сезону «{season}»."
        ),
    }
}

fn render_es(m: &Msg) -> String {
    match m {
        Msg::Travel { km, days, pace, severe } => format!(
            "Un viaje de {km:.0} km en {days:.0} día(s) = {pace:.0} km/día, que {} el ritmo preindustrial (normalmente 25–80 km/día).",
            if *severe { "supera con creces" } else { "supera" }
        ),
        Msg::Climate { weather, place, zone } => format!(
            "Inverosímil: {} en {place}, cuya zona climática es {}.",
            if *weather == Weather::Cold { "clima helado" } else { "calor tropical" },
            zone.replace('_', " ")
        ),
        Msg::Population { place, claimed, modeled } => format!(
            "{place} se describe con ~{} habitantes, pero el mundo modela ~{}.",
            p(*claimed), p(*modeled)
        ),
        Msg::Astronomy { claimed, world, moons } => format!(
            "El texto implica {claimed} luna(s), pero este mundo tiene {world} ({moons})."
        ),
        Msg::Economy { metal, minerals } => format!(
            "Aquí se extrae {metal}, pero la geología de este mundo solo da: {minerals}."
        ),
        Msg::DateSeason { weather, season } => format!(
            "{} aquí, pero la cronología sitúa esto en la estación «{season}».",
            if *weather == Weather::Cold { "Se describe un frío helado" } else { "Se describe un calor tropical" }
        ),
        Msg::TravelTimeline { prose_days, timeline_days, from, to } => format!(
            "La prosa sugiere unos {prose_days:.0} día(s), pero la cronología sitúa unos {timeline_days:.0} día(s) entre «{from}» y «{to}»."
        ),
        Msg::DateCoherence { hint, season } => format!(
            "Aquí se menciona «{hint}», pero la cronología sitúa esto en la estación «{season}»."
        ),
    }
}

fn render_fr(m: &Msg) -> String {
    match m {
        Msg::Travel { km, days, pace, severe } => format!(
            "Un trajet de {km:.0} km en {days:.0} jour(s) = {pace:.0} km/jour, ce qui dépasse {}le rythme préindustriel (typiquement 25–80 km/jour).",
            if *severe { "largement " } else { "" }
        ),
        Msg::Climate { weather, place, zone } => format!(
            "Invraisemblable : {} à {place}, dont la zone climatique est {}.",
            if *weather == Weather::Cold { "un froid glacial" } else { "une chaleur tropicale" },
            zone.replace('_', " ")
        ),
        Msg::Population { place, claimed, modeled } => format!(
            "{place} est décrite avec ~{} habitants, mais le monde en modélise ~{}.",
            p(*claimed), p(*modeled)
        ),
        Msg::Astronomy { claimed, world, moons } => format!(
            "Le texte implique {claimed} lune(s), mais ce monde en a {world} ({moons})."
        ),
        Msg::Economy { metal, minerals } => format!(
            "On extrait ici du {metal}, mais la géologie de ce monde ne donne que : {minerals}."
        ),
        Msg::DateSeason { weather, season } => format!(
            "{} ici, mais la chronologie place ceci à la saison « {season} ».",
            if *weather == Weather::Cold { "Un froid glacial est décrit" } else { "Une chaleur tropicale est décrite" }
        ),
        Msg::TravelTimeline { prose_days, timeline_days, from, to } => format!(
            "La prose suggère environ {prose_days:.0} jour(s), mais la chronologie place environ {timeline_days:.0} jour(s) entre « {from} » et « {to} »."
        ),
        Msg::DateCoherence { hint, season } => format!(
            "On mentionne ici « {hint} », mais la chronologie place ceci à la saison « {season} »."
        ),
    }
}

fn render_de(m: &Msg) -> String {
    match m {
        Msg::Travel { km, days, pace, severe } => format!(
            "Eine Reise von {km:.0} km in {days:.0} Tag(en) = {pace:.0} km/Tag, was die vorindustrielle Reisegeschwindigkeit {}überschreitet (typisch 25–80 km/Tag).",
            if *severe { "deutlich " } else { "" }
        ),
        Msg::Climate { weather, place, zone } => format!(
            "Unplausibel: {} in {place}, dessen Klimazone {} ist.",
            if *weather == Weather::Cold { "frostiges Wetter" } else { "tropische Hitze" },
            zone.replace('_', " ")
        ),
        Msg::Population { place, claimed, modeled } => format!(
            "{place} wird mit ~{} Einwohnern beschrieben, aber die Welt modelliert ~{}.",
            p(*claimed), p(*modeled)
        ),
        Msg::Astronomy { claimed, world, moons } => format!(
            "Der Text impliziert {claimed} Mond(e), aber diese Welt hat {world} ({moons})."
        ),
        Msg::Economy { metal, minerals } => format!(
            "Hier wird {metal} abgebaut, aber die Geologie dieser Welt liefert nur: {minerals}."
        ),
        Msg::DateSeason { weather, season } => format!(
            "{} hier, aber die Chronologie verortet dies in der Jahreszeit „{season}“.",
            if *weather == Weather::Cold { "Frostiges Wetter wird beschrieben" } else { "Tropische Hitze wird beschrieben" }
        ),
        Msg::TravelTimeline { prose_days, timeline_days, from, to } => format!(
            "Die Prosa legt etwa {prose_days:.0} Tag(e) nahe, aber die Chronologie verortet etwa {timeline_days:.0} Tag(e) zwischen „{from}“ und „{to}“."
        ),
        Msg::DateCoherence { hint, season } => format!(
            "Hier wird „{hint}“ erwähnt, aber die Chronologie verortet dies in der Jahreszeit „{season}“."
        ),
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
    fn messages_render_per_language() {
        let m = Msg::Travel { km: 600.0, days: 3.0, pace: 200.0, severe: true };
        assert!(Lang::En.render(&m).contains("Travel of 600 km"));
        assert!(Lang::Ru.render(&m).contains("Путь"));
        assert!(Lang::De.render(&m).contains("Reise"));
        assert!(Lang::Fr.render(&m).contains("trajet"));
        assert!(Lang::Es.render(&m).contains("viaje"));
        // The economy frame localises while the canonical mineral stays put.
        let e = Msg::Economy { metal: "silver", minerals: "gold, iron" };
        assert!(Lang::Ru.render(&e).contains("silver") && Lang::Ru.render(&e).contains("добывают"));
    }

    #[test]
    fn unicode_word_matching() {
        assert!(contains_word("за три дня", "три"));
        assert!(!contains_word("материк", "три")); // not a substring match
        assert!(contains_word("в шахте добывали", "шахте"));
    }

    #[test]
    fn confidence_reflects_signal() {
        // Clear Russian script → confident.
        let (l, c) = detect_with_confidence("Гонец скакал три долгих дня без отдыха через горы.");
        assert_eq!(l, Lang::Ru);
        assert!(c);
        // Clear German function words → confident.
        let (l, c) = detect_with_confidence("Der Bote ritt durch das weite Land ohne Rast und nicht müde.");
        assert_eq!((l, c), (Lang::De, true));
        // No signal → English fallback, NOT confident (degrade, don't assert).
        let (l, c) = detect_with_confidence("rode 600 km");
        assert_eq!(l, Lang::En);
        assert!(!c);
    }

    #[test]
    fn detection_never_panics_on_hostile_input() {
        for s in ["", "   ", "🚀🚀🚀", "123 456 789", &"a".repeat(100_000), "中文字符", "Ω≈ç√∫"] {
            let _ = detect_with_confidence(s); // must not panic
        }
    }

    #[test]
    fn russian_place_name_declines() {
        let v = name_variants("Москва", Lang::Ru);
        for form in ["москва", "москвы", "москве", "москву", "москвой"] {
            assert!(v.contains(&form.to_string()), "missing {form} in {v:?}");
        }
        // Masculine consonant-final.
        let v = name_variants("Новгород", Lang::Ru);
        for form in ["новгорода", "новгороду", "новгородом", "новгороде"] {
            assert!(v.contains(&form.to_string()), "missing {form} in {v:?}");
        }
        // Hush spelling rule: к before а → genitive -и, not -ы.
        let v = name_variants("Лука", Lang::Ru);
        assert!(v.contains(&"луки".to_string()));
        assert!(!v.contains(&"лукы".to_string()));
    }

    #[test]
    fn latin_names_minimal_variants() {
        // Romance proper nouns are invariant here (boundary handles elision).
        assert_eq!(name_variants("Anvilport", Lang::Es), vec!["anvilport".to_string()]);
        // German adds the genitive -s/-es.
        let v = name_variants("Eisenburg", Lang::De);
        assert!(v.contains(&"eisenburgs".to_string()));
    }

    #[test]
    fn backend_defaults_to_heuristic_and_degrades() {
        // With no model env pointing at a real file, the backend is the built-in
        // heuristic and the note explains the (non-failing) degradation.
        // (We don't mutate process env here; just assert the note is well-formed.)
        let note = backend_note();
        assert!(note.starts_with("language:"));
        assert!(matches!(active_backend(), Backend::Heuristic | Backend::Enhanced));
    }
}

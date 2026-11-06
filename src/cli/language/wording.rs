//! The sentences the language family repeats (LANG-IMPROVE-1 LI-8).
//!
//! Every subcommand that cannot find a language, has no language to list,
//! points at `font-build`, or counts dictionary entries builds its sentence
//! here, so the words are the same from every entry point and a snapshot
//! test pins them.  Rules the module encodes:
//!
//! * a missing language always says `run \`inkhaven language init <name>\` first`;
//! * hints quote the full command (`inkhaven language font-build …`), never
//!   the bare subcommand;
//! * counts are spelled out (`1 entry`, `12 entries`), not `entr(y/ies)`;
//! * confirmations ("wrote …", "… → path") go to stderr; stdout carries data.

use crate::error::Error;

/// `language \`X\` not found — run \`inkhaven language init X\` first`.
pub(crate) fn language_not_found(language: &str) -> Error {
    Error::Config(format!(
        "language `{language}` not found — run `inkhaven language init {language}` first"
    ))
}

/// The proto-language variant of [`language_not_found`].
pub(crate) fn proto_language_not_found(proto: &str) -> Error {
    Error::Config(format!(
        "proto-language `{proto}` not found — run `inkhaven language init {proto}` first"
    ))
}

/// What a listing says when the project has no languages at all.
pub(crate) const NO_LANGUAGES: &str = "no languages yet — run `inkhaven language init <name>` first";

/// A character that is linked but commands no language.
pub(crate) fn character_has_no_language(character: &str) -> Error {
    Error::Config(format!(
        "character `{character}` has no linked language — run `inkhaven language link-character \
         {character} <language> <proficiency>` first"
    ))
}

/// The command that builds a language's font, quoted in full.
pub(crate) fn font_build_command(language: &str) -> String {
    format!("inkhaven language font-build --language {language} --format ttf")
}

/// The parenthetical a Typst export prints when the book names a font family.
pub(crate) fn typst_font_hint(language: &str, output: &std::path::Path) -> String {
    format!(
        "(build the font with `{}` and compile with `typst compile --font-path <dir> {}`)",
        font_build_command(language),
        output.display()
    )
}

/// `1 entry`, `12 entries`.
pub(crate) fn entries(n: usize) -> String {
    plural(n, "entry", "entries")
}

/// `{n} {one}` when `n == 1`, otherwise `{n} {many}`.
pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The `compose --kind` vocabulary: canonical name first, then the aliases
/// the command accepts.  [`compose_kind`] and the error it raises read from
/// this one table, so the list in the message can never drift from the match.
pub(crate) const COMPOSE_KINDS: &[(&str, &[&str])] = &[
    ("names", &["name"]),
    ("prose", &["sample", "sample-text"]),
    ("poem", &["poetry", "verse"]),
    ("blessing", &[]),
    ("curse", &[]),
    ("incantation", &[]),
    ("ceremony", &[]),
];

/// Resolve a `--kind` argument to its canonical name.
pub(crate) fn compose_kind(kind: &str) -> Result<&'static str, Error> {
    let k = kind.trim().to_ascii_lowercase();
    COMPOSE_KINDS
        .iter()
        .find(|(name, aliases)| *name == k || aliases.contains(&k.as_str()))
        .map(|(name, _)| *name)
        .ok_or_else(|| Error::Config(format!("unknown --kind `{kind}` (expected {})", compose_kind_list())))
}

/// `names (or name) | prose (or sample, sample-text) | … | ceremony`.
pub(crate) fn compose_kind_list() -> String {
    COMPOSE_KINDS
        .iter()
        .map(|(name, aliases)| {
            if aliases.is_empty() {
                name.to_string()
            } else {
                format!("{name} (or {})", aliases.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::stress::{StressPlacement, StressRule};

    #[test]
    fn a_missing_language_always_says_how_to_create_it() {
        assert_eq!(
            language_not_found("Avesha").to_string(),
            "config error: language `Avesha` not found — run `inkhaven language init Avesha` first"
        );
        assert_eq!(
            proto_language_not_found("Proto-Tira").to_string(),
            "config error: proto-language `Proto-Tira` not found — run `inkhaven language init Proto-Tira` first"
        );
        assert_eq!(NO_LANGUAGES, "no languages yet — run `inkhaven language init <name>` first");
    }

    #[test]
    fn a_character_without_a_language_is_told_how_to_link_one() {
        assert_eq!(
            character_has_no_language("Mira").to_string(),
            "config error: character `Mira` has no linked language — run `inkhaven language link-character Mira <language> <proficiency>` first"
        );
    }

    #[test]
    fn font_hints_quote_the_full_command() {
        assert_eq!(
            font_build_command("Avesha"),
            "inkhaven language font-build --language Avesha --format ttf"
        );
        assert_eq!(
            typst_font_hint("Avesha", std::path::Path::new("out/grammar.typ")),
            "(build the font with `inkhaven language font-build --language Avesha --format ttf` and compile with `typst compile --font-path <dir> out/grammar.typ`)"
        );
    }

    #[test]
    fn counts_are_spelled_out() {
        assert_eq!(entries(0), "0 entries");
        assert_eq!(entries(1), "1 entry");
        assert_eq!(entries(12), "12 entries");
        assert_eq!(plural(1, "rule", "rules"), "1 rule");
        assert_eq!(plural(3, "rule", "rules"), "3 rules");
    }

    #[test]
    fn the_kind_list_in_the_error_is_the_table_the_match_reads() {
        assert_eq!(compose_kind("Poetry").unwrap(), "poem");
        assert_eq!(compose_kind("ceremony").unwrap(), "ceremony");
        assert_eq!(compose_kind("sample-text").unwrap(), "prose");
        let err = compose_kind("sonnet").unwrap_err().to_string();
        assert_eq!(
            err,
            "config error: unknown --kind `sonnet` (expected names (or name) | prose (or sample, sample-text) | poem (or poetry, verse) | blessing | curse | incantation | ceremony)"
        );
        for (name, aliases) in COMPOSE_KINDS {
            assert!(err.contains(name), "{name} missing from the error");
            for a in *aliases {
                assert!(err.contains(a), "alias {a} missing from the error");
            }
        }
    }

    #[test]
    fn stress_rules_display_as_words_not_debug() {
        let st = StressRule { primary: StressPlacement::Penultimate };
        assert_eq!(st.to_string(), "penultimate");
        assert_eq!(StressPlacement::LatinRule.to_string(), "latin (heavy penult, otherwise antepenult)");
        assert_eq!(StressPlacement::Initial.to_string(), "initial");
        assert_eq!(StressPlacement::Final.to_string(), "final");
        assert_eq!(StressPlacement::Antepenultimate.to_string(), "antepenultimate");
    }
}

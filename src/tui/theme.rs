//! Decoded form of `ThemeConfig`. Parses the user-supplied hex strings once
//! at startup and exposes ratatui `Color`s for every place in the renderer
//! that used to hard-code colours. Missing/invalid values fall back to the
//! shipped Catppuccin Mocha defaults — the TUI never panics on a malformed
//! theme block.

use ratatui::style::{Color, Modifier};

use crate::config::{color_or, ThemeConfig};

#[derive(Debug, Clone)]
pub struct Theme {
    pub pane_bg: Color,
    pub pane_fg: Color,
    pub line_number_fg: Color,
    pub current_line_bg: Color,

    pub border_focused: Color,
    pub border_unfocused: Color,
    pub border_dirty: Color,
    pub border_saved: Color,
    pub border_readonly: Color,

    pub modal_bg: Color,
    pub modal_border: Color,
    pub modal_fg: Color,

    pub places_fg: Color,
    pub characters_fg: Color,
    pub artefacts_fg: Color,
    pub notes_underline_fg: Color,
    /// 1.2.9+ — colour for inline filter-word warnings
    /// (`just`, `really`, `очень`, …).  Underlined so the
    /// warning is visible even on terminals that flatten
    /// fg colour overlays.
    pub style_warning_filter_word_fg: Color,
    /// 1.2.9+ — colour for repeated-phrase warnings.
    /// Different from filter-word fg so the two
    /// overlays are visually distinct when they
    /// overlap.
    pub style_warning_repeated_phrase_fg: Color,
    /// 1.2.9+ — colour for show-don't-tell warnings.
    /// Distinct from filter-word + repeated-phrase fg
    /// so the three overlays stay distinguishable
    /// when adjacent.
    pub style_warning_show_dont_tell_fg: Color,
    /// 1.2.13+ — colour for invented-language
    /// dictionary-entry overlays.  Painted on
    /// invented words in the manuscript when a
    /// language book's Dictionary chapter contains
    /// a matching headword.  Default is a soft
    /// teal-mauve picked to stay distinct from the
    /// existing places (cyan) / characters
    /// (amber) / artefacts (peach) chips and from
    /// the show-don't-tell teal.  Phase D adds
    /// per-language overrides — one colour per
    /// Language sub-book — so multi-language
    /// projects can colour-code in the manuscript.
    pub language_word_fg: Color,
    /// 1.2.12+ — per-detector style modifier for
    /// the three style-warning overlays.  Defaults to
    /// `Modifier::UNDERLINED` for all three
    /// (preserves the 1.2.9-1.2.11 behaviour) but
    /// users can override via the corresponding
    /// `theme.style_warning_*_modifier` HJSON fields
    /// — `"underline"`, `"bold"`, `"dim"`,
    /// `"reversed"`, `"none"`, or `"+"`-combined
    /// like `"underline+bold"`.  The teal underline
    /// reads differently on different terminal
    /// palettes; this gives users the escape hatch
    /// without forcing them to learn ratatui's
    /// modifier API.
    pub style_warning_filter_word_modifier: Modifier,
    pub style_warning_repeated_phrase_modifier: Modifier,
    pub style_warning_show_dont_tell_modifier: Modifier,
    /// 1.2.9+ — POV / character chip on the status
    /// bar (Ctrl+B Shift+P).  Explicit RGB defaults
    /// so the chip stays readable across terminal
    /// palettes (the original `Color::Magenta`
    /// surfaced as a pale pink on Catppuccin and
    /// killed contrast against the white fg).
    pub pov_chip_bg: Color,
    pub pov_chip_fg: Color,

    pub search_match_bg: Color,
    pub search_current_bg: Color,

    pub tree_open_marker: Color,
    pub tree_book_fg: Color,
    pub tree_chapter_fg: Color,
    pub tree_subchapter_fg: Color,
    pub tree_paragraph_fg: Color,
    pub tree_image_fg: Color,
    pub tree_script_fg: Color,
    pub editor_position_fg: Color,
    pub ai_scope_fg: Color,
    pub ai_infer_fg: Color,
    pub grammar_change_fg: Color,

    pub syntax_heading: Color,
    pub syntax_bold: Color,
    pub syntax_italic: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_comment: Color,
    pub syntax_keyword: Color,
    pub syntax_function: Color,
    pub syntax_operator: Color,
    pub syntax_list_marker: Color,
    pub syntax_raw: Color,
    pub syntax_tag: Color,
    pub syntax_quote: Color,
}

impl Theme {
    pub fn from_config(cfg: &ThemeConfig) -> Self {
        // Per-field default mirrors `ThemeConfig::default()` — kept in sync
        // by hand because the round-trip would otherwise involve parsing
        // the defaults from strings (works, but adds a layer of indirection
        // for no gain).
        Self {
            pane_bg: color_or(&cfg.pane_bg, Color::Rgb(0x1e, 0x1e, 0x2e)),
            pane_fg: color_or(&cfg.pane_fg, Color::Rgb(0xcd, 0xd6, 0xf4)),
            line_number_fg: color_or(&cfg.line_number_fg, Color::Rgb(0x6c, 0x70, 0x86)),
            current_line_bg: color_or(&cfg.current_line_bg, Color::Rgb(0x31, 0x32, 0x44)),

            border_focused: color_or(&cfg.border_focused, Color::Rgb(0xcb, 0xa6, 0xf7)),
            border_unfocused: color_or(&cfg.border_unfocused, Color::Rgb(0x45, 0x47, 0x5a)),
            border_dirty: color_or(&cfg.border_dirty, Color::Rgb(0xf9, 0xe2, 0xaf)),
            border_saved: color_or(&cfg.border_saved, Color::Rgb(0xa6, 0xe3, 0xa1)),
            border_readonly: color_or(&cfg.border_readonly, Color::Rgb(0x94, 0xe2, 0xd5)),

            modal_bg: color_or(&cfg.modal_bg, Color::Rgb(0x18, 0x18, 0x25)),
            modal_border: color_or(&cfg.modal_border, Color::Rgb(0xcb, 0xa6, 0xf7)),
            modal_fg: color_or(&cfg.modal_fg, Color::Rgb(0xcd, 0xd6, 0xf4)),

            places_fg: color_or(&cfg.places_fg, Color::Rgb(0x89, 0xdc, 0xeb)),
            characters_fg: color_or(&cfg.characters_fg, Color::Rgb(0xf9, 0xe2, 0xaf)),
            // Catppuccin Mocha "peach" — a clearly distinct yellow-
            // orange so Artefacts don't clash with Characters' amber.
            artefacts_fg: color_or(&cfg.artefacts_fg, Color::Rgb(0xfa, 0xb3, 0x87)),
            // Underline uses the regular pane text colour by default;
            // a separate knob lets the user tint the underline if the
            // pane_fg is too subtle.
            notes_underline_fg: color_or(&cfg.notes_underline_fg, Color::Rgb(0xcd, 0xd6, 0xf4)),
            style_warning_filter_word_fg: color_or(
                &cfg.style_warning_filter_word_fg,
                // Soft amber — visible against most themes
                // without being alarming.  Filter words are
                // a "consider rewriting" prompt, not an
                // error.
                Color::Rgb(0xf9, 0xc4, 0x4e),
            ),
            style_warning_repeated_phrase_fg: color_or(
                &cfg.style_warning_repeated_phrase_fg,
                // Soft magenta — distinct from filter-word
                // amber + the existing places/characters
                // overlays.
                Color::Rgb(0xeb, 0x6f, 0x92),
            ),
            style_warning_show_dont_tell_fg: color_or(
                &cfg.style_warning_show_dont_tell_fg,
                // Soft teal — distinct from filter-word
                // amber + repeated-phrase magenta so the
                // three overlays stay visually separate
                // when adjacent.
                Color::Rgb(0x94, 0xe2, 0xd5),
            ),
            // 1.2.13+ — invented-language overlay.
            // Soft mauve-teal mix; distinct from the
            // four existing entity-overlay colours.
            language_word_fg: color_or(
                &cfg.language_word_fg,
                Color::Rgb(0xb4, 0xa8, 0xe1),
            ),
            // 1.2.12+ — per-detector modifier overrides;
            // default is UNDERLINED for all three (1.2.9
            // baseline).  See `parse_style_modifier`.
            style_warning_filter_word_modifier: parse_style_modifier(
                &cfg.style_warning_filter_word_modifier,
            ),
            style_warning_repeated_phrase_modifier: parse_style_modifier(
                &cfg.style_warning_repeated_phrase_modifier,
            ),
            style_warning_show_dont_tell_modifier: parse_style_modifier(
                &cfg.style_warning_show_dont_tell_modifier,
            ),
            pov_chip_bg: color_or(
                &cfg.pov_chip_bg,
                // Deep magenta — guarantees contrast
                // against the white-bold fg below
                // regardless of the terminal's named-
                // magenta mapping.  Catppuccin's named
                // magenta is a pastel that washed out
                // against white.
                Color::Rgb(0x8b, 0x1d, 0x88),
            ),
            pov_chip_fg: color_or(
                &cfg.pov_chip_fg,
                Color::Rgb(0xff, 0xff, 0xff),
            ),

            search_match_bg: color_or(&cfg.search_match_bg, Color::Rgb(0xf3, 0x8b, 0xa8)),
            search_current_bg: color_or(&cfg.search_current_bg, Color::Rgb(0xf5, 0xc2, 0xe7)),

            tree_open_marker: color_or(&cfg.tree_open_marker, Color::Rgb(0xa6, 0xe3, 0xa1)),
            tree_book_fg: color_or(&cfg.tree_book_fg, Color::Rgb(0xf5, 0xc2, 0xe7)),
            tree_chapter_fg: color_or(&cfg.tree_chapter_fg, Color::Rgb(0x89, 0xb4, 0xfa)),
            tree_subchapter_fg: color_or(&cfg.tree_subchapter_fg, Color::Rgb(0x94, 0xe2, 0xd5)),
            tree_paragraph_fg: color_or(&cfg.tree_paragraph_fg, Color::Rgb(0xcd, 0xd6, 0xf4)),
            // Same peach as the Artefacts editor overlay so the "this
            // is media, not text" cue is consistent.
            tree_image_fg: color_or(&cfg.tree_image_fg, Color::Rgb(0xfa, 0xb3, 0x87)),
            // Catppuccin-mocha "mauve" — distinct from prose / image
            // / data colours, signals "this is code, not prose".
            tree_script_fg: color_or(&cfg.tree_script_fg, Color::Rgb(0xcb, 0xa6, 0xf7)),
            editor_position_fg: color_or(&cfg.editor_position_fg, Color::Rgb(0x89, 0xdc, 0xeb)),
            ai_scope_fg: color_or(&cfg.ai_scope_fg, Color::Rgb(0xfa, 0xb3, 0x87)),
            ai_infer_fg: color_or(&cfg.ai_infer_fg, Color::Rgb(0x94, 0xe2, 0xd5)),
            grammar_change_fg: color_or(
                &cfg.grammar_change_fg,
                // Catppuccin Mocha red; user's spec defaults to "red" so
                // this honours that intent while keeping palette
                // consistency.
                Color::Rgb(0xf3, 0x8b, 0xa8),
            ),

            syntax_heading: color_or(&cfg.syntax_heading, Color::Rgb(0xcb, 0xa6, 0xf7)),
            syntax_bold: color_or(&cfg.syntax_bold, Color::Rgb(0xf9, 0xe2, 0xaf)),
            syntax_italic: color_or(&cfg.syntax_italic, Color::Rgb(0x94, 0xe2, 0xd5)),
            syntax_string: color_or(&cfg.syntax_string, Color::Rgb(0xa6, 0xe3, 0xa1)),
            syntax_number: color_or(&cfg.syntax_number, Color::Rgb(0xfa, 0xb3, 0x87)),
            syntax_comment: color_or(&cfg.syntax_comment, Color::Rgb(0x6c, 0x70, 0x86)),
            syntax_keyword: color_or(&cfg.syntax_keyword, Color::Rgb(0xcb, 0xa6, 0xf7)),
            syntax_function: color_or(&cfg.syntax_function, Color::Rgb(0x89, 0xdc, 0xeb)),
            syntax_operator: color_or(&cfg.syntax_operator, Color::Rgb(0x94, 0xe2, 0xd5)),
            syntax_list_marker: color_or(&cfg.syntax_list_marker, Color::Rgb(0xcb, 0xa6, 0xf7)),
            syntax_raw: color_or(&cfg.syntax_raw, Color::Rgb(0xfa, 0xb3, 0x87)),
            syntax_tag: color_or(&cfg.syntax_tag, Color::Rgb(0x89, 0xb4, 0xfa)),
            syntax_quote: color_or(&cfg.syntax_quote, Color::Rgb(0x93, 0x99, 0xb2)),
        }
    }

    /// Set a theme colour by field name at runtime. Used by the
    /// `ink.theme.set` Bund stdlib word so scripts can recolour
    /// the interface without restarting the TUI. `hex` is parsed
    /// via the same `color_or` helper as HJSON config so the
    /// accepted forms match.
    ///
    /// Returns `Err` with the offending field name when no field
    /// matches — keeps the script's error message useful.
    pub fn set_by_name(&mut self, field: &str, hex: &str) -> Result<(), String> {
        let parsed = crate::config::parse_color(hex).ok_or_else(|| {
            format!("unrecognised colour `{hex}` — use #rrggbb or a named colour")
        })?;
        match field {
            "pane_bg" => self.pane_bg = parsed,
            "pane_fg" => self.pane_fg = parsed,
            "line_number_fg" => self.line_number_fg = parsed,
            "current_line_bg" => self.current_line_bg = parsed,
            "border_focused" => self.border_focused = parsed,
            "border_unfocused" => self.border_unfocused = parsed,
            "border_dirty" => self.border_dirty = parsed,
            "border_saved" => self.border_saved = parsed,
            "border_readonly" => self.border_readonly = parsed,
            "modal_bg" => self.modal_bg = parsed,
            "modal_border" => self.modal_border = parsed,
            "modal_fg" => self.modal_fg = parsed,
            "places_fg" => self.places_fg = parsed,
            "characters_fg" => self.characters_fg = parsed,
            "artefacts_fg" => self.artefacts_fg = parsed,
            "notes_underline_fg" => self.notes_underline_fg = parsed,
            "style_warning_filter_word_fg" => self.style_warning_filter_word_fg = parsed,
            "style_warning_repeated_phrase_fg" => self.style_warning_repeated_phrase_fg = parsed,
            "style_warning_show_dont_tell_fg" => self.style_warning_show_dont_tell_fg = parsed,
            "language_word_fg" => self.language_word_fg = parsed,
            "pov_chip_bg" => self.pov_chip_bg = parsed,
            "pov_chip_fg" => self.pov_chip_fg = parsed,
            "search_match_bg" => self.search_match_bg = parsed,
            "search_current_bg" => self.search_current_bg = parsed,
            "tree_open_marker" => self.tree_open_marker = parsed,
            "tree_book_fg" => self.tree_book_fg = parsed,
            "tree_chapter_fg" => self.tree_chapter_fg = parsed,
            "tree_subchapter_fg" => self.tree_subchapter_fg = parsed,
            "tree_paragraph_fg" => self.tree_paragraph_fg = parsed,
            "tree_image_fg" => self.tree_image_fg = parsed,
            "tree_script_fg" => self.tree_script_fg = parsed,
            "editor_position_fg" => self.editor_position_fg = parsed,
            "ai_scope_fg" => self.ai_scope_fg = parsed,
            "ai_infer_fg" => self.ai_infer_fg = parsed,
            "grammar_change_fg" => self.grammar_change_fg = parsed,
            "syntax_heading" => self.syntax_heading = parsed,
            "syntax_bold" => self.syntax_bold = parsed,
            "syntax_italic" => self.syntax_italic = parsed,
            "syntax_string" => self.syntax_string = parsed,
            "syntax_number" => self.syntax_number = parsed,
            "syntax_comment" => self.syntax_comment = parsed,
            "syntax_keyword" => self.syntax_keyword = parsed,
            "syntax_function" => self.syntax_function = parsed,
            "syntax_operator" => self.syntax_operator = parsed,
            "syntax_list_marker" => self.syntax_list_marker = parsed,
            "syntax_raw" => self.syntax_raw = parsed,
            "syntax_tag" => self.syntax_tag = parsed,
            "syntax_quote" => self.syntax_quote = parsed,
            other => return Err(format!("unknown theme field `{other}`")),
        }
        Ok(())
    }
}

/// 1.2.12+ — parse the HJSON string form of a
/// `Modifier` chord into a ratatui `Modifier`.
/// Recognised tokens (case-insensitive):
///
///   * `none`      → `Modifier::empty()`
///   * `underline` → `Modifier::UNDERLINED`
///   * `bold`      → `Modifier::BOLD`
///   * `dim`       → `Modifier::DIM`
///   * `reversed`  → `Modifier::REVERSED`
///   * `italic`    → `Modifier::ITALIC`
///   * `<empty>` or unknown → default
///     `Modifier::UNDERLINED` (preserves
///     1.2.9-1.2.11 behaviour).
///
/// Multiple modifiers can be combined with `+`:
/// `underline+bold` lights up both bits.
pub fn parse_style_modifier(raw: &str) -> Modifier {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Modifier::UNDERLINED;
    }
    let mut acc = Modifier::empty();
    let mut saw_known = false;
    for token in trimmed.split('+') {
        let lc = token.trim().to_lowercase();
        match lc.as_str() {
            "none" => {
                saw_known = true;
            }
            "underline" | "underlined" => {
                acc |= Modifier::UNDERLINED;
                saw_known = true;
            }
            "bold" => {
                acc |= Modifier::BOLD;
                saw_known = true;
            }
            "dim" => {
                acc |= Modifier::DIM;
                saw_known = true;
            }
            "reversed" | "reverse" => {
                acc |= Modifier::REVERSED;
                saw_known = true;
            }
            "italic" => {
                acc |= Modifier::ITALIC;
                saw_known = true;
            }
            _ => {}
        }
    }
    if saw_known { acc } else { Modifier::UNDERLINED }
}

#[cfg(test)]
mod tests_style_modifier {
    use super::*;

    #[test]
    fn empty_input_defaults_to_underlined() {
        assert_eq!(parse_style_modifier(""), Modifier::UNDERLINED);
        assert_eq!(parse_style_modifier("   "), Modifier::UNDERLINED);
    }

    #[test]
    fn unknown_token_defaults_to_underlined() {
        assert_eq!(parse_style_modifier("rainbow"), Modifier::UNDERLINED);
    }

    #[test]
    fn each_single_token_works_case_insensitively() {
        assert_eq!(parse_style_modifier("bold"), Modifier::BOLD);
        assert_eq!(parse_style_modifier("BOLD"), Modifier::BOLD);
        assert_eq!(parse_style_modifier("Dim"), Modifier::DIM);
        assert_eq!(parse_style_modifier("reversed"), Modifier::REVERSED);
        assert_eq!(parse_style_modifier("italic"), Modifier::ITALIC);
        assert_eq!(parse_style_modifier("underline"), Modifier::UNDERLINED);
    }

    #[test]
    fn none_clears_modifiers() {
        assert_eq!(parse_style_modifier("none"), Modifier::empty());
    }

    #[test]
    fn plus_combinator_unions_modifiers() {
        let m = parse_style_modifier("underline+bold");
        assert!(m.contains(Modifier::UNDERLINED));
        assert!(m.contains(Modifier::BOLD));
        let m = parse_style_modifier("dim+italic+reversed");
        assert!(m.contains(Modifier::DIM));
        assert!(m.contains(Modifier::ITALIC));
        assert!(m.contains(Modifier::REVERSED));
    }
}

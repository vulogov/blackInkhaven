//! Decoded form of `ThemeConfig`. Parses the user-supplied hex strings once
//! at startup and exposes ratatui `Color`s for every place in the renderer
//! that used to hard-code colours. Missing/invalid values fall back to the
//! shipped Catppuccin Mocha defaults — the TUI never panics on a malformed
//! theme block.

use ratatui::style::Color;

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

    pub search_match_bg: Color,
    pub search_current_bg: Color,

    pub tree_open_marker: Color,
    pub tree_book_fg: Color,
    pub tree_chapter_fg: Color,
    pub tree_subchapter_fg: Color,
    pub tree_paragraph_fg: Color,
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

            search_match_bg: color_or(&cfg.search_match_bg, Color::Rgb(0xf3, 0x8b, 0xa8)),
            search_current_bg: color_or(&cfg.search_current_bg, Color::Rgb(0xf5, 0xc2, 0xe7)),

            tree_open_marker: color_or(&cfg.tree_open_marker, Color::Rgb(0xa6, 0xe3, 0xa1)),
            tree_book_fg: color_or(&cfg.tree_book_fg, Color::Rgb(0xf5, 0xc2, 0xe7)),
            tree_chapter_fg: color_or(&cfg.tree_chapter_fg, Color::Rgb(0x89, 0xb4, 0xfa)),
            tree_subchapter_fg: color_or(&cfg.tree_subchapter_fg, Color::Rgb(0x94, 0xe2, 0xd5)),
            tree_paragraph_fg: color_or(&cfg.tree_paragraph_fg, Color::Rgb(0xcd, 0xd6, 0xf4)),
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
}

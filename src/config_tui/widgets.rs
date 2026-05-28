//! 1.2.10+ — typed widgets for the config TUI.
//!
//! Phase 2 covers the most common scalar types:
//! `bool`, integer, float, and string.  Other types
//! (color, path, enum, list) surface a "use Ctrl+B 0
//! for now" fallback; the rich pickers land in a
//! subsequent commit.
//!
//! All widgets share the same shape:
//!
//!   * `start(value)` — build the widget with the
//!     current value pre-loaded.
//!   * `handle_key(key)` — advance the widget on a
//!     keystroke; returns `EditOutcome` so the parent
//!     event loop knows what to do.
//!   * `render(frame, area)` — paint.
//!
//! Widgets *don't* persist anywhere; the parent
//! captures the committed value via `EditOutcome::Commit`
//! and stages it into the `edited` JSON tree.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use serde_json::Value;

pub enum EditOutcome {
    /// Still editing — no commit.
    Continue,
    /// User pressed Enter — caller stages this value
    /// and closes the widget.
    Commit(Value),
    /// User pressed Esc — abandon.
    Cancel,
}

pub enum Widget {
    Bool(BoolWidget),
    Int(IntWidget),
    Float(FloatWidget),
    Text(TextWidget),
    Color(ColorWidget),
    Path(PathWidget),
    Enum(EnumWidget),
    List(ListWidget),
    Unsupported(UnsupportedWidget),
}

impl Widget {
    /// 1.2.10+ — choose the widget kind based on
    /// the field's *refined* type (from
    /// `schema::refined_type`).  Caller passes the
    /// `ConfigType` and the field's dotted `path` so
    /// path-aware widgets (Color → theme preview)
    /// can tailor their UI.
    pub fn start_for_typed(
        value: &Value,
        ty: &crate::config_tui::schema::ConfigType,
        type_label: &str,
        path: &str,
    ) -> Self {
        use crate::config_tui::schema::ConfigType as CT;
        match ty {
            CT::Bool => Self::Bool(BoolWidget {
                value: value.as_bool().unwrap_or(false),
            }),
            CT::Int => Self::Int(IntWidget::from_i64(
                value.as_i64().unwrap_or(0),
            )),
            CT::Float => Self::Float(FloatWidget::from_f64(
                value.as_f64().unwrap_or(0.0),
            )),
            CT::Color => Self::Color(ColorWidget::from_str_with_path(
                value.as_str().unwrap_or(""),
                path,
            )),
            CT::Path => Self::Path(PathWidget::from_str(
                value.as_str().unwrap_or(""),
            )),
            CT::Enum(variants) => Self::Enum(EnumWidget::from_str(
                variants.clone(),
                value.as_str().unwrap_or(""),
            )),
            CT::StringList => Self::List(ListWidget::from_value(value)),
            CT::String => Self::Text(TextWidget::from_str(
                value.as_str().unwrap_or(""),
            )),
            _ => Self::Unsupported(UnsupportedWidget {
                type_label: type_label.to_string(),
            }),
        }
    }

    /// Legacy entry-point (Phase 2) — kept for any
    /// caller that doesn't have the refined type
    /// handy.  Loses Color / Path / Enum / List
    /// refinement.
    #[allow(dead_code)]
    pub fn start_for(value: &Value, type_label: &str) -> Self {
        match value {
            Value::Bool(b) => Self::Bool(BoolWidget { value: *b }),
            Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    Self::Int(IntWidget::from_i64(n.as_i64().unwrap_or(0)))
                } else {
                    Self::Float(FloatWidget::from_f64(
                        n.as_f64().unwrap_or(0.0),
                    ))
                }
            }
            Value::String(s) => Self::Text(TextWidget::from_str(s)),
            _ => Self::Unsupported(UnsupportedWidget {
                type_label: type_label.to_string(),
            }),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match self {
            Self::Bool(w) => w.handle_key(key),
            Self::Int(w) => w.handle_key(key),
            Self::Float(w) => w.handle_key(key),
            Self::Text(w) => w.handle_key(key),
            Self::Color(w) => w.handle_key(key),
            Self::Path(w) => w.handle_key(key),
            Self::Enum(w) => w.handle_key(key),
            Self::List(w) => w.handle_key(key),
            Self::Unsupported(_) => match key.code {
                KeyCode::Esc => EditOutcome::Cancel,
                _ => EditOutcome::Continue,
            },
        }
    }

    pub fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        match self {
            Self::Bool(w) => w.render(f, area, title),
            Self::Int(w) => w.render(f, area, title),
            Self::Float(w) => w.render(f, area, title),
            Self::Text(w) => w.render(f, area, title),
            Self::Color(w) => w.render(f, area, title),
            Self::Path(w) => w.render(f, area, title),
            Self::Enum(w) => w.render(f, area, title),
            Self::List(w) => w.render(f, area, title),
            Self::Unsupported(w) => w.render(f, area, title),
        }
    }
}

// ── bool ──────────────────────────────────────────────

pub struct BoolWidget {
    pub value: bool,
}

impl BoolWidget {
    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Char(' ') | KeyCode::Char('t') | KeyCode::Char('f') => {
                self.value = !self.value;
                EditOutcome::Continue
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.value = true;
                EditOutcome::Continue
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.value = false;
                EditOutcome::Continue
            }
            KeyCode::Enter => EditOutcome::Commit(Value::Bool(self.value)),
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (bool) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let on = if self.value { "[x]" } else { "[ ]" };
        let off = if self.value { "[ ]" } else { "[x]" };
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{on} true"),
                    if self.value {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::DIM)
                    },
                ),
                Span::raw("      "),
                Span::styled(
                    format!("{off} false"),
                    if !self.value {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::DIM)
                    },
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Space / t / f toggles · y true · n false · Enter commits · Esc cancels",
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── int ───────────────────────────────────────────────

pub struct IntWidget {
    buffer: String,
    error: Option<String>,
}

impl IntWidget {
    fn from_i64(v: i64) -> Self {
        Self { buffer: v.to_string(), error: None }
    }

    fn parse(&mut self) -> Option<i64> {
        self.error = None;
        match self.buffer.parse::<i64>() {
            Ok(v) => Some(v),
            Err(e) => {
                self.error = Some(format!("not an integer: {e}"));
                None
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                EditOutcome::Continue
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '-' || c == '+' => {
                self.buffer.push(c);
                EditOutcome::Continue
            }
            KeyCode::Up => {
                if let Some(v) = self.parse() {
                    self.buffer = (v + 1).to_string();
                }
                EditOutcome::Continue
            }
            KeyCode::Down => {
                if let Some(v) = self.parse() {
                    self.buffer = (v - 1).to_string();
                }
                EditOutcome::Continue
            }
            KeyCode::Enter => {
                if let Some(v) = self.parse() {
                    EditOutcome::Commit(Value::from(v))
                } else {
                    EditOutcome::Continue
                }
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (int) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let mut lines = vec![
            Line::from(""),
            Line::from(format!("    {}│", self.buffer)),
            Line::from(""),
        ];
        if let Some(err) = &self.error {
            lines.push(Line::from(Span::styled(
                format!("  ⚠ {err}"),
                Style::default().fg(Color::Red),
            )));
        }
        lines.push(Line::from(Span::styled(
            "  ↑ +1 · ↓ −1 · digits + sign + Backspace · Enter commits · Esc cancels",
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── float ─────────────────────────────────────────────

pub struct FloatWidget {
    buffer: String,
    error: Option<String>,
}

impl FloatWidget {
    fn from_f64(v: f64) -> Self {
        Self { buffer: format!("{v}"), error: None }
    }

    fn parse(&mut self) -> Option<f64> {
        self.error = None;
        match self.buffer.parse::<f64>() {
            Ok(v) => Some(v),
            Err(e) => {
                self.error = Some(format!("not a number: {e}"));
                None
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                EditOutcome::Continue
            }
            KeyCode::Char(c)
                if c.is_ascii_digit()
                    || c == '-'
                    || c == '+'
                    || c == '.'
                    || c == 'e'
                    || c == 'E' =>
            {
                self.buffer.push(c);
                EditOutcome::Continue
            }
            KeyCode::Enter => match self.parse() {
                Some(v) => match serde_json::Number::from_f64(v) {
                    Some(n) => EditOutcome::Commit(Value::Number(n)),
                    None => {
                        self.error = Some("infinite / NaN not allowed".into());
                        EditOutcome::Continue
                    }
                },
                None => EditOutcome::Continue,
            },
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (float) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let mut lines = vec![
            Line::from(""),
            Line::from(format!("    {}│", self.buffer)),
            Line::from(""),
        ];
        if let Some(err) = &self.error {
            lines.push(Line::from(Span::styled(
                format!("  ⚠ {err}"),
                Style::default().fg(Color::Red),
            )));
        }
        lines.push(Line::from(Span::styled(
            "  digits / `.` / `e` / sign / Backspace · Enter commits · Esc cancels",
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── text ──────────────────────────────────────────────

pub struct TextWidget {
    buffer: String,
}

impl TextWidget {
    fn from_str(s: &str) -> Self {
        Self { buffer: s.to_string() }
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                EditOutcome::Continue
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.buffer.push(c);
                EditOutcome::Continue
            }
            KeyCode::Enter => EditOutcome::Commit(Value::String(self.buffer.clone())),
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (string) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let lines = vec![
            Line::from(""),
            Line::from(format!("    {}│", self.buffer)),
            Line::from(""),
            Line::from(Span::styled(
                "  type to edit · Backspace deletes · Enter commits · Esc cancels",
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── unsupported ───────────────────────────────────────

pub struct UnsupportedWidget {
    pub type_label: String,
}

impl UnsupportedWidget {
    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} ({}) ", self.type_label))
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {} editor coming in a later release.", self.type_label),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  For now, edit this field via the main app's Ctrl+B 0",
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(Span::styled(
                "  in-app HJSON editor.",
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Esc to dismiss.",
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── color ─────────────────────────────────────────────

/// 1.2.10+ — drives the per-color mock the
/// preview pane paints.  Picked from the field's
/// path suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorPreviewKind {
    Fg,
    Bg,
    Border,
}

pub struct ColorWidget {
    buffer: String,
    /// Field path — drives the theme-preview pane's
    /// mock layout.  Suffix decides whether the
    /// candidate value paints as fg / bg / border.
    path: String,
}

impl ColorWidget {
    /// Path-less constructor — kept for legacy
    /// callers (none after Phase 6C, but reserved
    /// for future tooling).
    #[allow(dead_code)]
    fn from_str(s: &str) -> Self {
        Self { buffer: s.to_string(), path: String::new() }
    }

    fn from_str_with_path(s: &str, path: &str) -> Self {
        Self {
            buffer: s.to_string(),
            path: path.to_string(),
        }
    }

    fn preview_kind(&self) -> ColorPreviewKind {
        if self.path.ends_with("_bg") {
            ColorPreviewKind::Bg
        } else if self.path.ends_with("_border") {
            ColorPreviewKind::Border
        } else {
            ColorPreviewKind::Fg
        }
    }

    fn parsed(&self) -> Option<(u8, u8, u8)> {
        let raw = self.buffer.trim();
        let hex = raw.strip_prefix('#').unwrap_or(raw);
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                EditOutcome::Continue
            }
            KeyCode::Char(c)
                if c.is_ascii_hexdigit() || c == '#' =>
            {
                self.buffer.push(c.to_ascii_lowercase());
                EditOutcome::Continue
            }
            KeyCode::Enter => {
                if self.parsed().is_some() {
                    // Normalise to lowercase `#rrggbb`
                    // form on commit so the saved
                    // HJSON is stable.
                    let raw = self.buffer.trim();
                    let hex = raw.strip_prefix('#').unwrap_or(raw);
                    let normalised = format!("#{}", hex.to_lowercase());
                    EditOutcome::Commit(Value::String(normalised))
                } else {
                    EditOutcome::Continue
                }
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (color) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines = vec![
            Line::from(""),
            Line::from(format!("    {}│", self.buffer)),
            Line::from(""),
        ];
        match self.parsed() {
            Some((r, g, b)) => {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        "████████████████",
                        Style::default().fg(Color::Rgb(r, g, b)),
                    ),
                    Span::styled(
                        format!("  rgb({r}, {g}, {b})"),
                        dim,
                    ),
                ]));
                // ── Theme preview block ──────────
                let candidate = Color::Rgb(r, g, b);
                let kind = self.preview_kind();
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "    preview:",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                match kind {
                    ColorPreviewKind::Fg => {
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "the candidate colour on the editor background",
                                Style::default().fg(candidate),
                            ),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "BOLD sample · ",
                                Style::default()
                                    .fg(candidate)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                "DIM sample",
                                Style::default()
                                    .fg(candidate)
                                    .add_modifier(Modifier::DIM),
                            ),
                        ]));
                    }
                    ColorPreviewKind::Bg => {
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "  body text on the candidate background  ",
                                Style::default()
                                    .bg(candidate)
                                    .fg(Color::Rgb(0xcd, 0xd6, 0xf4)),
                            ),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "  DIM text                              ",
                                Style::default()
                                    .bg(candidate)
                                    .fg(Color::Rgb(0xcd, 0xd6, 0xf4))
                                    .add_modifier(Modifier::DIM),
                            ),
                        ]));
                    }
                    ColorPreviewKind::Border => {
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "┌──────────────────┐",
                                Style::default().fg(candidate),
                            ),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled("│", Style::default().fg(candidate)),
                            Span::raw("  body content    "),
                            Span::styled("│", Style::default().fg(candidate)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                "└──────────────────┘",
                                Style::default().fg(candidate),
                            ),
                        ]));
                    }
                }
            }
            None => {
                lines.push(Line::from(Span::styled(
                    "    ⚠ not a valid #RRGGBB hex",
                    Style::default().fg(Color::Red),
                )));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "    hex digits + `#` · Backspace · Enter commits (when valid) · Esc",
            dim,
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── path ──────────────────────────────────────────────

pub struct PathWidget {
    buffer: String,
}

impl PathWidget {
    fn from_str(s: &str) -> Self {
        Self { buffer: s.to_string() }
    }

    fn exists(&self) -> bool {
        std::path::Path::new(self.buffer.trim()).exists()
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                EditOutcome::Continue
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.buffer.push(c);
                EditOutcome::Continue
            }
            KeyCode::Enter => {
                // Don't block save on non-existent
                // paths — many config paths (backup
                // out_dir, etc.) get created on
                // first use.
                EditOutcome::Commit(Value::String(self.buffer.clone()))
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (path) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let exists = self.exists();
        let status = if self.buffer.trim().is_empty() {
            ("(empty)", Color::DarkGray)
        } else if exists {
            ("✓ exists", Color::Green)
        } else {
            ("○ does not exist (will be created on first use)", Color::Yellow)
        };
        let lines = vec![
            Line::from(""),
            Line::from(format!("    {}│", self.buffer)),
            Line::from(""),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(status.0, Style::default().fg(status.1)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "    type to edit · Backspace · Enter commits · Esc cancels",
                dim,
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── enum ──────────────────────────────────────────────

pub struct EnumWidget {
    variants: Vec<&'static str>,
    cursor: usize,
}

impl EnumWidget {
    fn from_str(variants: Vec<&'static str>, current: &str) -> Self {
        let cursor = variants
            .iter()
            .position(|v| *v == current)
            .unwrap_or(0);
        Self { variants, cursor }
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                EditOutcome::Continue
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.variants.len() {
                    self.cursor += 1;
                }
                EditOutcome::Continue
            }
            KeyCode::Home => {
                self.cursor = 0;
                EditOutcome::Continue
            }
            KeyCode::End => {
                self.cursor = self.variants.len().saturating_sub(1);
                EditOutcome::Continue
            }
            KeyCode::Enter => {
                let value = self
                    .variants
                    .get(self.cursor)
                    .copied()
                    .unwrap_or("");
                EditOutcome::Commit(Value::String(value.to_string()))
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (enum) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line<'_>> = vec![Line::from("")];
        for (i, v) in self.variants.iter().enumerate() {
            let marker = if i == self.cursor { "▶" } else { " " };
            let style = if i == self.cursor { bold } else { dim };
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(marker, bold),
                Span::raw("  "),
                Span::styled((*v).to_string(), style),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "    ↑↓ select · Home/End · Enter commits · Esc cancels",
            dim,
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── list of strings ───────────────────────────────────

pub struct ListWidget {
    items: Vec<String>,
    cursor: usize,
    mode: ListMode,
    inline_buffer: String,
}

enum ListMode {
    Browse,
    Add,
    Edit,
}

impl ListWidget {
    fn from_value(value: &Value) -> Self {
        let items = match value {
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        };
        Self {
            items,
            cursor: 0,
            mode: ListMode::Browse,
            inline_buffer: String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        match self.mode {
            ListMode::Browse => self.handle_browse(key),
            ListMode::Add => self.handle_inline_edit(key, false),
            ListMode::Edit => self.handle_inline_edit(key, true),
        }
    }

    fn handle_browse(&mut self, key: KeyEvent) -> EditOutcome {
        match key.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                EditOutcome::Continue
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.items.len() {
                    self.cursor += 1;
                }
                EditOutcome::Continue
            }
            KeyCode::Home => {
                self.cursor = 0;
                EditOutcome::Continue
            }
            KeyCode::End => {
                self.cursor = self.items.len().saturating_sub(1);
                EditOutcome::Continue
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.mode = ListMode::Add;
                self.inline_buffer.clear();
                EditOutcome::Continue
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if !self.items.is_empty() {
                    self.items.remove(self.cursor);
                    if self.cursor >= self.items.len() && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                EditOutcome::Continue
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if !self.items.is_empty() {
                    self.mode = ListMode::Edit;
                    self.inline_buffer = self.items[self.cursor].clone();
                }
                EditOutcome::Continue
            }
            KeyCode::Enter
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let value: Value = Value::Array(
                    self.items
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                );
                EditOutcome::Commit(value)
            }
            KeyCode::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let value: Value = Value::Array(
                    self.items
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                );
                EditOutcome::Commit(value)
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn handle_inline_edit(&mut self, key: KeyEvent, replace: bool) -> EditOutcome {
        match key.code {
            KeyCode::Esc => {
                self.mode = ListMode::Browse;
                self.inline_buffer.clear();
            }
            KeyCode::Enter => {
                let trimmed = self.inline_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    if replace && !self.items.is_empty() {
                        self.items[self.cursor] = trimmed;
                    } else {
                        self.items.push(trimmed);
                        self.cursor = self.items.len() - 1;
                    }
                }
                self.mode = ListMode::Browse;
                self.inline_buffer.clear();
            }
            KeyCode::Backspace => {
                self.inline_buffer.pop();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.inline_buffer.push(c);
            }
            _ => {}
        }
        EditOutcome::Continue
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Edit · {title} (list of {}) ",
                self.items.len()
            ))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line<'_>> = vec![Line::from("")];
        if self.items.is_empty() {
            lines.push(Line::from(Span::styled(
                "    (empty list — press `a` to add an entry)",
                dim,
            )));
        }
        let visible_items_max = inner.height.saturating_sub(6) as usize;
        let first =
            self.cursor.saturating_sub(visible_items_max.saturating_sub(1));
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(first)
            .take(visible_items_max)
        {
            let marker = if i == self.cursor { "▶" } else { " " };
            let style = if i == self.cursor { bold } else { Style::default() };
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(marker, bold),
                Span::raw("  "),
                Span::styled(item.clone(), style),
            ]));
        }
        match self.mode {
            ListMode::Browse => {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "    ↑↓ select · a add · d delete · e edit · Ctrl+S commits · Esc",
                    dim,
                )));
            }
            ListMode::Add => {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "    new entry:",
                    bold,
                )));
                lines.push(Line::from(format!(
                    "      {}│",
                    self.inline_buffer
                )));
                lines.push(Line::from(Span::styled(
                    "    Enter adds · Esc cancels",
                    dim,
                )));
            }
            ListMode::Edit => {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "    edit:",
                    bold,
                )));
                lines.push(Line::from(format!(
                    "      {}│",
                    self.inline_buffer
                )));
                lines.push(Line::from(Span::styled(
                    "    Enter saves · Esc cancels",
                    dim,
                )));
            }
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
}

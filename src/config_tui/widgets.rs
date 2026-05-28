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
    /// can tailor their UI.  1.2.11+ adds
    /// `project_root` for the Path widget's F3 file-
    /// picker fallback root.
    pub fn start_for_typed(
        value: &Value,
        ty: &crate::config_tui::schema::ConfigType,
        type_label: &str,
        path: &str,
        project_root: &std::path::Path,
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
            CT::Path => Self::Path(PathWidget::from_str_with_root(
                value.as_str().unwrap_or(""),
                project_root,
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
    /// 1.2.11+ — current input mode.  Hex is the
    /// default; `h` toggles into HSL where three
    /// sliders (Hue 0-360 / Saturation 0-100 /
    /// Lightness 0-100) drive the colour.  Switching
    /// modes converts the current value through the
    /// HSL ↔ RGB formulae below so the on-screen
    /// preview never jumps.
    mode: ColorMode,
    /// HSL state — only consulted when `mode` is
    /// `Hsl`.  Kept on the widget even in Hex mode so
    /// a round-trip toggle doesn't reset to (0, 0, 0).
    hsl: HslState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Hex,
    Hsl,
}

#[derive(Debug, Clone, Copy)]
struct HslState {
    /// 0..=360 — wraps at the boundary.
    h: u16,
    /// 0..=100.
    s: u8,
    /// 0..=100.
    l: u8,
    /// 0=Hue, 1=Saturation, 2=Lightness.
    active: u8,
}

impl HslState {
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        Self {
            h: h.round() as u16,
            s: (s * 100.0).round() as u8,
            l: (l * 100.0).round() as u8,
            active: 0,
        }
    }

    fn to_rgb(&self) -> (u8, u8, u8) {
        hsl_to_rgb(
            self.h as f64,
            self.s as f64 / 100.0,
            self.l as f64 / 100.0,
        )
    }
}

impl ColorWidget {
    /// Path-less constructor — kept for legacy
    /// callers (none after Phase 6C, but reserved
    /// for future tooling).
    #[allow(dead_code)]
    fn from_str(s: &str) -> Self {
        Self {
            buffer: s.to_string(),
            path: String::new(),
            mode: ColorMode::Hex,
            hsl: HslState { h: 0, s: 0, l: 0, active: 0 },
        }
    }

    fn from_str_with_path(s: &str, path: &str) -> Self {
        let hsl = parse_hex(s)
            .map(|(r, g, b)| HslState::from_rgb(r, g, b))
            .unwrap_or(HslState { h: 0, s: 0, l: 0, active: 0 });
        Self {
            buffer: s.to_string(),
            path: path.to_string(),
            mode: ColorMode::Hex,
            hsl,
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
        match self.mode {
            ColorMode::Hex => parse_hex(&self.buffer),
            ColorMode::Hsl => Some(self.hsl.to_rgb()),
        }
    }

    /// 1.2.11+ — sync `buffer` to the current HSL
    /// state's hex equivalent.  Called whenever an
    /// HSL slider moves so the on-disk commit value
    /// stays in lockstep with what the preview shows.
    fn sync_buffer_from_hsl(&mut self) {
        let (r, g, b) = self.hsl.to_rgb();
        self.buffer = format!("#{r:02x}{g:02x}{b:02x}");
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        // ── HSL mode ───────────────────────────────
        if matches!(self.mode, ColorMode::Hsl) {
            return self.handle_key_hsl(key);
        }
        // ── Hex mode (default) ─────────────────────
        match key.code {
            KeyCode::Char('h') | KeyCode::Char('H') => {
                // Flip into HSL mode, seeding from the
                // current hex value if it parses;
                // otherwise keep the cached HSL state.
                if let Some((r, g, b)) = parse_hex(&self.buffer) {
                    self.hsl = HslState::from_rgb(r, g, b);
                }
                self.mode = ColorMode::Hsl;
                EditOutcome::Continue
            }
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

    fn handle_key_hsl(&mut self, key: KeyEvent) -> EditOutcome {
        let shifted = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('h') | KeyCode::Char('H') => {
                // Toggle back to hex mode.  Buffer is
                // already in sync (every slider move
                // calls `sync_buffer_from_hsl`).
                self.mode = ColorMode::Hex;
                EditOutcome::Continue
            }
            KeyCode::Tab => {
                self.hsl.active = (self.hsl.active + 1) % 3;
                EditOutcome::Continue
            }
            KeyCode::BackTab => {
                self.hsl.active = (self.hsl.active + 2) % 3;
                EditOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right => {
                let step: i32 = if shifted { 10 } else { 1 };
                let delta = if matches!(key.code, KeyCode::Right) {
                    step
                } else {
                    -step
                };
                match self.hsl.active {
                    0 => {
                        // Hue wraps at 360.
                        let next = (self.hsl.h as i32 + delta).rem_euclid(360);
                        self.hsl.h = next as u16;
                    }
                    1 => {
                        let next = (self.hsl.s as i32 + delta).clamp(0, 100);
                        self.hsl.s = next as u8;
                    }
                    _ => {
                        let next = (self.hsl.l as i32 + delta).clamp(0, 100);
                        self.hsl.l = next as u8;
                    }
                }
                self.sync_buffer_from_hsl();
                EditOutcome::Continue
            }
            KeyCode::Home => {
                match self.hsl.active {
                    0 => self.hsl.h = 0,
                    1 => self.hsl.s = 0,
                    _ => self.hsl.l = 0,
                }
                self.sync_buffer_from_hsl();
                EditOutcome::Continue
            }
            KeyCode::End => {
                match self.hsl.active {
                    0 => self.hsl.h = 360,
                    1 => self.hsl.s = 100,
                    _ => self.hsl.l = 100,
                }
                self.sync_buffer_from_hsl();
                EditOutcome::Continue
            }
            KeyCode::Up => {
                self.hsl.active = (self.hsl.active + 2) % 3;
                EditOutcome::Continue
            }
            KeyCode::Down => {
                self.hsl.active = (self.hsl.active + 1) % 3;
                EditOutcome::Continue
            }
            KeyCode::Enter => {
                let raw = self.buffer.trim();
                let hex = raw.strip_prefix('#').unwrap_or(raw);
                let normalised = format!("#{}", hex.to_lowercase());
                EditOutcome::Commit(Value::String(normalised))
            }
            KeyCode::Esc => EditOutcome::Cancel,
            _ => EditOutcome::Continue,
        }
    }

    fn render(&self, f: &mut ratatui::Frame, area: Rect, title: &str) {
        let mode_tag = match self.mode {
            ColorMode::Hex => "hex",
            ColorMode::Hsl => "hsl",
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit · {title} (color · {mode_tag}) "))
            .border_style(
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines = Vec::new();
        match self.mode {
            ColorMode::Hex => {
                lines.push(Line::from(""));
                lines.push(Line::from(format!("    {}│", self.buffer)));
                lines.push(Line::from(""));
            }
            ColorMode::Hsl => {
                // Three labelled sliders.  The active
                // one carries a ▶ marker in the gutter.
                lines.push(Line::from(""));
                lines.push(hsl_slider_line(
                    "Hue       ",
                    self.hsl.h as u32,
                    360,
                    self.hsl.active == 0,
                ));
                lines.push(hsl_slider_line(
                    "Saturation",
                    self.hsl.s as u32,
                    100,
                    self.hsl.active == 1,
                ));
                lines.push(hsl_slider_line(
                    "Lightness ",
                    self.hsl.l as u32,
                    100,
                    self.hsl.active == 2,
                ));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "    hex equivalent: {}",
                    self.buffer
                )));
                lines.push(Line::from(""));
            }
        }
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
        let hint = match self.mode {
            ColorMode::Hex => {
                "    hex digits + `#` · Backspace · h hsl mode · Enter commits (when valid) · Esc"
            }
            ColorMode::Hsl => {
                "    Tab cycle slider · ←→ ±1 · Shift+←→ ±10 · h hex mode · Enter commits · Esc"
            }
        };
        lines.push(Line::from(Span::styled(hint, dim)));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

/// 1.2.11+ — render one HSL slider row as a styled
/// `Line`.  Layout:
///
/// ```text
///   ▶ Hue        [████████░░░░░░░░░░░░] 145
///     Saturation [██████████░░░░░░░░░░] 100
/// ```
///
/// The active slider gets a `▶` marker in the gutter
/// and a bold label; inactive sliders dim down so the
/// focus is visible at a glance.  Bar length is fixed
/// at 20 chars (each char = 5% of saturation/lightness
/// or 18° of hue).
fn hsl_slider_line(label: &str, value: u32, max: u32, active: bool) -> Line<'static> {
    const BAR_WIDTH: u32 = 20;
    let filled = ((value as u64 * BAR_WIDTH as u64) / max.max(1) as u64) as u32;
    let mut bar = String::new();
    bar.push('[');
    for i in 0..BAR_WIDTH {
        bar.push(if i < filled { '█' } else { '░' });
    }
    bar.push(']');
    let marker = if active { "  ▶ " } else { "    " };
    let label_style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    let bar_style = if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    Line::from(vec![
        Span::raw(marker.to_string()),
        Span::styled(label.to_string(), label_style),
        Span::raw(" "),
        Span::styled(bar, bar_style),
        Span::raw(format!(" {value}")),
    ])
}

/// 1.2.11+ — `#rrggbb` → `(r, g, b)` with the `#` and
/// case-insensitivity already handled.  Extracted so
/// `ColorWidget::parsed` and the HSL seeding share the
/// same parse path.
fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let raw = s.trim();
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// 1.2.11+ — standard HSL → sRGB conversion.  `h` in
/// degrees [0, 360]; `s` and `l` in fractions [0, 1].
/// Result clamps to u8 channels.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = if h.is_finite() { h.rem_euclid(360.0) } else { 0.0 };
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_u8 = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

/// 1.2.11+ — standard sRGB → HSL.  Returns `(h, s, l)`
/// with `h` in degrees [0, 360], `s` and `l` in
/// fractions [0, 1].
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);
    let delta = cmax - cmin;
    let l = (cmax + cmin) / 2.0;
    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };
    let h_raw = if delta == 0.0 {
        0.0
    } else if (cmax - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (cmax - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h_raw.is_finite() {
        h_raw.rem_euclid(360.0)
    } else {
        0.0
    };
    (h, s, l)
}

// ── path ──────────────────────────────────────────────

/// 1.2.11+ — path widget gains an embedded F3
/// file picker (the main TUI's tree-style filesystem
/// browser).  Text-input mode is the default; F3
/// toggles into picker mode rooted at the buffer's
/// parent directory (or the project root, if the
/// buffer is empty / points to nowhere).  Inside the
/// picker, arrow keys + PgUp/PgDn navigate, Right
/// expands a directory, Left collapses, Enter selects
/// the highlighted entry (overwrites the buffer; user
/// presses Enter AGAIN in text mode to commit), Esc
/// drops back to text mode without changing the
/// buffer.  Mirrors the main TUI's F3 chord so muscle
/// memory transfers.
pub struct PathWidget {
    buffer: String,
    picker: Option<crate::tui::file_picker::FilePicker>,
    /// Project root captured at widget-start so the
    /// picker has a sensible fallback when the buffer
    /// is empty / invalid / relative.  Set by the
    /// config-TUI when constructing the widget.
    project_root: std::path::PathBuf,
}

impl PathWidget {
    pub(super) fn from_str_with_root(s: &str, project_root: &std::path::Path) -> Self {
        Self {
            buffer: s.to_string(),
            picker: None,
            project_root: project_root.to_path_buf(),
        }
    }

    fn exists(&self) -> bool {
        std::path::Path::new(self.buffer.trim()).exists()
    }

    /// 1.2.11+ — root the picker at the directory
    /// most likely to be useful to the user: the
    /// buffer's parent if the buffer is set + has a
    /// parent that exists; otherwise the project
    /// root.
    fn picker_root(&self) -> std::path::PathBuf {
        let candidate = std::path::Path::new(self.buffer.trim());
        if !candidate.as_os_str().is_empty() {
            if candidate.is_dir() {
                return candidate.to_path_buf();
            }
            if let Some(parent) = candidate.parent() {
                if !parent.as_os_str().is_empty() && parent.exists() {
                    return parent.to_path_buf();
                }
            }
        }
        self.project_root.clone()
    }

    fn handle_key(&mut self, key: KeyEvent) -> EditOutcome {
        // ── picker mode ──────────────────────────────
        if self.picker.is_some() {
            match key.code {
                KeyCode::Up => {
                    if let Some(p) = self.picker.as_mut() {
                        p.move_up();
                    }
                }
                KeyCode::Down => {
                    if let Some(p) = self.picker.as_mut() {
                        p.move_down();
                    }
                }
                KeyCode::PageUp => {
                    if let Some(p) = self.picker.as_mut() {
                        p.page_up(10);
                    }
                }
                KeyCode::PageDown => {
                    if let Some(p) = self.picker.as_mut() {
                        p.page_down(10);
                    }
                }
                KeyCode::Home => {
                    if let Some(p) = self.picker.as_mut() {
                        p.jump_first();
                    }
                }
                KeyCode::End => {
                    if let Some(p) = self.picker.as_mut() {
                        p.jump_last();
                    }
                }
                KeyCode::Right => {
                    if let Some(p) = self.picker.as_mut() {
                        p.expand();
                    }
                }
                KeyCode::Left => {
                    if let Some(p) = self.picker.as_mut() {
                        p.collapse_or_step_out();
                    }
                }
                KeyCode::Enter => {
                    // Selection — overwrite buffer.
                    // Stay in widget; user presses
                    // Enter again in text mode to
                    // commit the value to config.
                    if let Some(p) = self.picker.as_ref() {
                        if let Some(entry) = p.current() {
                            self.buffer = entry.path.to_string_lossy().into_owned();
                        }
                    }
                    self.picker = None;
                }
                KeyCode::Esc | KeyCode::F(3) => {
                    // Drop back to text mode without
                    // touching the buffer.
                    self.picker = None;
                }
                _ => {}
            }
            return EditOutcome::Continue;
        }
        // ── text mode (default) ──────────────────────
        match key.code {
            KeyCode::F(3) => {
                // Open the picker.  Rooted at the
                // best-guess parent of the current
                // buffer.
                let root = self.picker_root();
                self.picker = Some(crate::tui::file_picker::FilePicker::new(
                    root,
                    crate::tui::file_picker::PickerContext::EditorLoad,
                ));
                EditOutcome::Continue
            }
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
        if let Some(picker) = self.picker.as_ref() {
            self.render_picker(f, inner, picker);
        } else {
            self.render_text_mode(f, inner);
        }
    }

    fn render_text_mode(&self, f: &mut ratatui::Frame, inner: Rect) {
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
                "    type to edit · Backspace · F3 picker · Enter commits · Esc cancels",
                dim,
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }

    fn render_picker(
        &self,
        f: &mut ratatui::Frame,
        inner: Rect,
        picker: &crate::tui::file_picker::FilePicker,
    ) {
        // Top: root header + buffer hint.  Body: tree
        // list.  Bottom: chord hint.
        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines: Vec<Line<'_>> = Vec::new();
        lines.push(Line::from(vec![
            Span::raw("  root: "),
            Span::styled(
                picker.root.display().to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
        // Reserve one trailing row for the hint line.
        let body_rows = (inner.height as usize).saturating_sub(4).max(1);
        // Window the entries around the cursor so the
        // selection stays on-screen.
        let n = picker.entries.len();
        let start = picker.cursor.saturating_sub(body_rows / 2).min(n.saturating_sub(body_rows).max(0));
        for (i, entry) in picker.entries.iter().enumerate().skip(start).take(body_rows) {
            let selected = i == picker.cursor;
            let indent: String = std::iter::repeat("  ").take(entry.depth + 1).collect();
            let glyph = if entry.is_dir {
                if entry.expanded {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let name = entry
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| entry.path.display().to_string());
            let row = format!("{indent}{glyph}{name}");
            let style = if selected {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else if entry.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ navigate · → expand · ← collapse · Enter select · Esc / F3 cancel",
            dim,
        )));
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

#[cfg(test)]
mod tests_hsl {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn rgb_to_hsl_known_anchors() {
        // Pure red: H=0°, S=1, L=0.5.
        let (h, s, l) = rgb_to_hsl(255, 0, 0);
        assert!(close(h, 0.0, 0.5));
        assert!(close(s, 1.0, 0.001));
        assert!(close(l, 0.5, 0.001));
        // Pure green: H=120°.
        let (h, _, _) = rgb_to_hsl(0, 255, 0);
        assert!(close(h, 120.0, 0.5));
        // Pure blue: H=240°.
        let (h, _, _) = rgb_to_hsl(0, 0, 255);
        assert!(close(h, 240.0, 0.5));
        // Pure gray: S=0, L=0.5.
        let (_, s, l) = rgb_to_hsl(128, 128, 128);
        assert!(close(s, 0.0, 0.001));
        assert!(close(l, 0.5, 0.005));
    }

    #[test]
    fn hsl_to_rgb_known_anchors() {
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), (255, 0, 0));
        assert_eq!(hsl_to_rgb(120.0, 1.0, 0.5), (0, 255, 0));
        assert_eq!(hsl_to_rgb(240.0, 1.0, 0.5), (0, 0, 255));
        // White and black.
        assert_eq!(hsl_to_rgb(0.0, 0.0, 1.0), (255, 255, 255));
        assert_eq!(hsl_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
    }

    #[test]
    fn round_trip_preserves_colour_within_rounding() {
        // Pick a handful of mid-range colours and verify
        // that RGB → HSL → RGB lands back within 2 LSB
        // per channel.  2 LSB is the conventional
        // tolerance: HSL stores fractional values, and
        // back to u8 is a lossy step.
        let cases = [
            (0xcd, 0xd6, 0xf4), // catppuccin text
            (0x89, 0xb4, 0xfa), // catppuccin blue
            (0xf3, 0x8b, 0xa8), // catppuccin pink
            (0x1e, 0x1e, 0x2e), // catppuccin base
        ];
        for (r, g, b) in cases {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!(
                (r2 as i32 - r as i32).abs() <= 2
                    && (g2 as i32 - g as i32).abs() <= 2
                    && (b2 as i32 - b as i32).abs() <= 2,
                "round-trip drift on #{r:02x}{g:02x}{b:02x}: got #{r2:02x}{g2:02x}{b2:02x}",
            );
        }
    }

    #[test]
    fn hue_wraps_at_boundary() {
        // 360° is the same colour as 0°; rendering /
        // delta math must respect the wrap.
        assert_eq!(hsl_to_rgb(360.0, 1.0, 0.5), hsl_to_rgb(0.0, 1.0, 0.5));
    }

    #[test]
    fn parse_hex_handles_with_and_without_octothorpe() {
        assert_eq!(parse_hex("#cdd6f4"), Some((0xcd, 0xd6, 0xf4)));
        assert_eq!(parse_hex("cdd6f4"), Some((0xcd, 0xd6, 0xf4)));
        assert_eq!(parse_hex("CDD6F4"), Some((0xcd, 0xd6, 0xf4)));
        assert_eq!(parse_hex("not hex"), None);
        assert_eq!(parse_hex("#cdd6"), None);
    }
}

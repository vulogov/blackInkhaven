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
    Unsupported(UnsupportedWidget),
}

impl Widget {
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

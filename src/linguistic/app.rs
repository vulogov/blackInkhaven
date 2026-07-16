//! L-P0 — the Linguistic companion's application state + rendering.
//!
//! A two-pane read-model for now: the Languages tree on the left, a preview of
//! the selected node on the right, driven by the shared [`crate::tui_host`]
//! event loop. Chat + the analysis verbs layer on in later waves.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use uuid::Uuid;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{Store, SYSTEM_TAG_LANGUAGES};
use crate::system_tree::SystemBookTree;
use crate::tui_host::TuiHost;

use super::LinguisticInvocation;

/// How many characters of a node body to preview before eliding.
const PREVIEW_LIMIT: usize = 8_000;

pub struct LinguisticApp {
    #[allow(dead_code)] // held for the non-interactive + write paths that land in later waves
    layout: ProjectLayout,
    #[allow(dead_code)]
    cfg: Config,
    store: Store,
    hierarchy: Hierarchy,
    /// Left-pane tree over the `Language` system book.
    tree: SystemBookTree,
    /// Cached preview text for the selected node (recomputed on move).
    preview: String,
    /// Transient status line (hints, errors).
    status: String,
    should_quit: bool,
}

impl LinguisticApp {
    pub(crate) fn new(
        layout: ProjectLayout,
        cfg: Config,
        store: Store,
        hierarchy: Hierarchy,
        inv: LinguisticInvocation,
    ) -> Result<LinguisticApp> {
        let mut tree = SystemBookTree::new(&hierarchy, SYSTEM_TAG_LANGUAGES);

        // `--language <name>`: reveal that language's book so it opens focused.
        if let Some(name) = inv.language.as_deref() {
            if let Some(id) = find_language_node(&hierarchy, name) {
                tree.reveal(&hierarchy, id);
            }
        }

        let status = if tree.root.is_none() {
            "No Language system book yet — open the editor and create a language first."
                .to_string()
        } else if tree.is_empty() {
            "No languages yet. Use `inkhaven language init <name>` to scaffold one.".to_string()
        } else {
            "↑/↓ or j/k move · →/l expand · ←/h collapse · q quits".to_string()
        };

        let mut app = LinguisticApp {
            layout,
            cfg,
            store,
            hierarchy,
            tree,
            preview: String::new(),
            status,
            should_quit: false,
        };
        app.refresh_preview();
        Ok(app)
    }

    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        crate::tui_host::run_loop(self, terminal)
    }

    /// Recompute the preview from the currently-selected node.
    fn refresh_preview(&mut self) {
        self.preview = match self.tree.selected() {
            Some(id) => self.node_preview(id),
            None => String::new(),
        };
    }

    /// Load a readable preview of `id`: its title + body (or a child summary for
    /// a chapter/book). Never fails loudly — a store hiccup shows a short note.
    fn node_preview(&self, id: Uuid) -> String {
        let Some(node) = self.hierarchy.get(id) else {
            return String::new();
        };
        match self.store.get_content(id) {
            Ok(Some(bytes)) if !bytes.is_empty() => {
                let body = String::from_utf8_lossy(&bytes);
                let mut s = body.chars().take(PREVIEW_LIMIT).collect::<String>();
                if body.chars().count() > PREVIEW_LIMIT {
                    s.push_str("\n\n… (truncated)");
                }
                s
            }
            _ => {
                // Container node (book/chapter): list its immediate children.
                let kids = self.hierarchy.children_of(Some(id));
                if kids.is_empty() {
                    format!("{}\n\n(empty)", node.title)
                } else {
                    let mut s = format!("{}\n\n", node.title);
                    for k in kids {
                        s.push_str(&format!("  • {}\n", k.title));
                    }
                    s
                }
            }
        }
    }
}

/// Find a language sub-book by (case-insensitive) title under the Languages book.
fn find_language_node(hierarchy: &Hierarchy, name: &str) -> Option<Uuid> {
    let root = hierarchy.iter().find(|n| {
        n.kind == crate::store::NodeKind::Book
            && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
    })?;
    hierarchy
        .children_of(Some(root.id))
        .into_iter()
        .find(|n| n.title.eq_ignore_ascii_case(name))
        .map(|n| n.id)
}

impl TuiHost for LinguisticApp {
    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        self.render_title(frame, chunks[0]);
        self.render_body(frame, chunks[1]);
        self.render_status(frame, chunks[2]);
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Universal exits.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.should_quit = true;
            return;
        }
        let mut moved = true;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.tree.move_down(),
            KeyCode::Up | KeyCode::Char('k') => self.tree.move_up(),
            KeyCode::Right | KeyCode::Char('l') => self.tree.step_in(&self.hierarchy),
            KeyCode::Left | KeyCode::Char('h') => self.tree.step_out(&self.hierarchy),
            KeyCode::Enter => self.tree.toggle(&self.hierarchy),
            KeyCode::Home | KeyCode::Char('g') => self.tree.to_top(),
            KeyCode::End | KeyCode::Char('G') => self.tree.to_bottom(),
            _ => moved = false,
        }
        if moved {
            self.refresh_preview();
        }
    }
}

impl LinguisticApp {
    fn render_title(&self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled(" ⟐ Inner Linguist ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("· "),
            Span::styled("Languages", Style::default().add_modifier(Modifier::DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);
        self.render_tree(frame, cols[0]);
        self.render_preview(frame, cols[1]);
    }

    fn render_tree(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Tree ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();
        for (i, row) in self.tree.rows().iter().enumerate() {
            let fold = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let title = self
                .hierarchy
                .get(row.id)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| "?".to_string());
            let indent = "  ".repeat(row.depth);
            let text = format!("{indent}{fold}{title}");
            let style = if i == self.tree.cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no languages)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        frame.render_widget(Paragraph::new(lines).scroll((self.tree.scroll as u16, 0)), inner);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .tree
            .selected()
            .and_then(|id| self.hierarchy.get(id))
            .map(|n| format!(" {} ", n.title))
            .unwrap_or_else(|| " Preview ".to_string());
        let block = Block::default().borders(Borders::ALL).title(title);
        let para = Paragraph::new(self.preview.as_str())
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {}", self.status),
                Style::default().add_modifier(Modifier::DIM),
            )),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::Node;

    fn node(id: Uuid, kind: &str, title: &str, parent: Option<Uuid>, lang_root: bool) -> Node {
        let mut raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": title,
            "slug": title.to_lowercase(),
            "path": [],
            "parent_id": parent,
            "order": 1,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        });
        if lang_root {
            raw["system_tag"] = serde_json::json!(SYSTEM_TAG_LANGUAGES);
        }
        serde_json::from_value(raw).expect("test node deserialises")
    }

    #[test]
    fn find_language_node_matches_case_insensitively() {
        let root = Uuid::now_v7();
        let quenya = Uuid::now_v7();
        let sindarin = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![
            node(root, "book", "Languages", None, true),
            node(quenya, "book", "Quenya", Some(root), false),
            node(sindarin, "book", "Sindarin", Some(root), false),
        ]);
        assert_eq!(find_language_node(&h, "quenya"), Some(quenya));
        assert_eq!(find_language_node(&h, "SINDARIN"), Some(sindarin));
        assert_eq!(find_language_node(&h, "Adûnaic"), None);
    }

    #[test]
    fn find_language_node_is_none_without_a_languages_book() {
        let ch = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![node(ch, "book", "Draft", None, false)]);
        assert_eq!(find_language_node(&h, "Quenya"), None);
    }
}

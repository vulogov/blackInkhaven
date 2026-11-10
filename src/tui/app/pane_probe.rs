//! Tree-pane and right-pane probes (3.19) — the pane counterpart of
//! `modal_probe`. Each test drives one pane of the headless `App` with the
//! full key battery at several terminal sizes and then runs scripted flows
//! that assert the pane's contracts:
//!
//! * tree — rows always mirror the hierarchy, the cursor stays in range,
//!   the `/` filter is a real text box, leaving the pane keeps the needle,
//!   a mouse click lands on the row drawn under it (rows wrap), and the
//!   add / move / status / type / clipboard / delete chords change the
//!   store the way the legend says.
//! * AI pane / Thoughts / Output / AI prompt — no panics at any size, a
//!   chorded letter never fires a pane action, `Home`/`G`/`PgUp` clamp to
//!   the rendered extent so the opposite key moves at once, and the Output
//!   `/` query is a real text box.
//!
//! Both open a real `Store`, so they are `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture pane_probe
//! ```
use super::modal_probe::{battery, ctrl, k, km, open_para, panic_text, probe_app, render, send, write_body, SIZES};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use super::*;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Silence panics (the batteries catch hundreds of them on purpose) but keep
/// the last message, so an assertion that fails mid-probe is still reported.
fn install_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic>".into()
        };
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        *LAST_PANIC.lock().unwrap() = Some(format!("{msg}{at}"));
    }));
}

/// Run a probe body, write + print its report whatever happens, and re-raise
/// a body panic with its message.
pub(super) fn run_probe(name: &str, body: impl FnOnce(&mut Vec<String>)) {
    install_hook();
    let mut out: Vec<String> = Vec::new();
    let r = catch_unwind(AssertUnwindSafe(|| body(&mut out)));
    let report = out.join("\n");
    let report_path = std::env::temp_dir().join(format!("inkhaven-{name}-probe.txt"));
    std::fs::write(&report_path, &report).unwrap();
    println!("{report}");
    println!("report: {}", report_path.display());
    if r.is_err() {
        let msg = LAST_PANIC.lock().unwrap().clone().unwrap_or_default();
        let _ = std::panic::take_hook();
        panic!("{name} probe failed: {msg}");
    }
}

/// Press every battery key with `focus`, closing whatever modal a key opened
/// and ending any filter edit, rendering at three sizes after each. Returns
/// the problem lines (panics / errors / invariant breaks).
fn battery_on_pane(app: &mut App, focus: Focus, label: &str, check: &dyn Fn(&App) -> Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (name, key) in battery() {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            continue;
        }
        app.modal = Modal::None;
        app.focus = focus;
        match send(app, key) {
            Ok(true) => out.push(format!("[{label}] key {name} requested QUIT")),
            Ok(false) => {}
            Err(e) => out.push(format!("[{label}] key {name}: {e} · status: {}", app.status)),
        }
        for (w, h) in [(80, 24), (30, 10), (20, 6)] {
            if let Err(e) = render(app, w, h) {
                out.push(format!("[{label}] render {w}x{h} after {name}: {e}"));
            }
        }
        if let Some(problem) = check(app) {
            out.push(format!("[{label}] after {name}: {problem}"));
        }
        // Reset whatever the key switched on so the next key starts clean.
        let mut escs = 0;
        while !matches!(app.modal, Modal::None) && escs < 4 {
            escs += 1;
            let _ = send(app, k(KeyCode::Esc));
        }
        app.modal = Modal::None;
        if app.tree_filter_editing {
            app.focus = Focus::Tree;
            let _ = send(app, k(KeyCode::Esc));
        }
        app.output_query_focused = false;
        app.link_pick_for = None;
    }
    out
}

/// Tree invariants: the cursor is in range, every row is a live node, and
/// without a filter the rows are exactly the hierarchy minus folded subtrees.
fn tree_invariants(app: &App) -> Option<String> {
    if !app.rows.is_empty() && app.tree_cursor >= app.rows.len() {
        return Some(format!("cursor {} past {} rows", app.tree_cursor, app.rows.len()));
    }
    if let Some((id, _)) = app.rows.iter().find(|(id, _)| app.hierarchy.get(*id).is_none()) {
        return Some(format!("row {id} is not in the hierarchy"));
    }
    if app.tree_filter.is_empty() {
        let expect: Vec<Uuid> = app
            .hierarchy
            .flatten_with_collapsed(&app.collapsed_nodes)
            .into_iter()
            .map(|(n, _)| n.id)
            .collect();
        let got: Vec<Uuid> = app.rows.iter().map(|(id, _)| *id).collect();
        if expect != got {
            return Some(format!("rows ({}) differ from the folded hierarchy ({})", got.len(), expect.len()));
        }
    }
    None
}

fn title_of(app: &App, id: Uuid) -> String {
    app.hierarchy.get(id).map(|n| n.title.clone()).unwrap_or_default()
}

fn row_of_title(app: &App, title: &str) -> Option<usize> {
    app.rows.iter().position(|(id, _)| title_of(app, *id) == title)
}

fn click(app: &mut App, col: u16, row: u16) -> Result<(), String> {
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: KeyModifiers::NONE };
    match catch_unwind(AssertUnwindSafe(|| app.handle_mouse(ev))) {
        Ok(()) => Ok(()),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_key(k(KeyCode::Char(c)));
    }
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn tree_pane_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("tree", |out| {
    let (tmp, mut app, pid) = probe_app();
    std::env::set_current_dir(tmp.path()).unwrap();

    // A title long enough to wrap inside the 30-column tree pane.
    let long_title = "A very long paragraph title that certainly wraps across several lines of a narrow tree pane";
    {
        let chapter = app.hierarchy.get(pid).and_then(|n| n.parent_id).and_then(|c| app.hierarchy.get(c).cloned()).expect("chapter");
        let mut n = app
            .store
            .create_node(&app.cfg, &app.hierarchy, NodeKind::Paragraph, long_title, Some(&chapter), None, InsertPosition::End)
            .expect("long paragraph");
        app.store.update_paragraph_content(&mut n, b"= Long\n\nwrapped body\n").unwrap();
        app.reload_hierarchy();
    }
    open_para(&mut app, pid);
    app.focus = Focus::Tree;

    // Baseline renders with the tree focused and the cursor at both ends.
    for (w, h) in SIZES {
        for cursor in [0usize, app.rows.len().saturating_sub(1)] {
            app.tree_cursor = cursor;
            if let Err(e) = render(&mut app, w, h) {
                out.push(format!("[TREE baseline] render {w}x{h} cursor {cursor}: {e}"));
            }
        }
    }
    app.tree_cursor = 0;

    // ── key battery ──────────────────────────────────────────────────────
    let before: Vec<Uuid> = app.hierarchy.iter().map(|n| n.id).collect();
    out.extend(battery_on_pane(&mut app, Focus::Tree, "TREE battery", &tree_invariants));
    let after: Vec<Uuid> = app.hierarchy.iter().map(|n| n.id).collect();
    out.push(format!("[TREE battery] nodes before {} · after {} (no key may delete without confirmation)", before.len(), after.len()));
    assert!(before.iter().all(|id| after.contains(id)), "a battery key deleted a node without confirmation");
    app.tree_filter.clear();
    app.tree_filter_editing = false;
    app.collapsed_nodes.clear();
    app.tree_marked.clear();
    app.rebuild_rows_preserving_cursor();

    // ── `/` filter is a text box ─────────────────────────────────────────
    app.focus = Focus::Tree;
    app.modal = Modal::None;
    let _ = app.handle_key(k(KeyCode::Char('/')));
    type_str(&mut app, "pxara");
    for _ in 0..3 {
        let _ = app.handle_key(k(KeyCode::Left));
    }
    let _ = app.handle_key(k(KeyCode::Backspace));
    let _ = app.handle_key(ctrl('q')); // Ctrl+letter must not land as text
    let needle = app.tree_filter.as_str().to_string();
    let titles: Vec<String> = app.rows.iter().map(|(id, _)| title_of(&app, *id)).collect();
    out.push(format!("[TREE filter] after `/ pxara ←←← ⌫ Ctrl+Q`: needle {needle:?} · rows {titles:?}"));
    assert_eq!(needle, "para", "tree filter chords");
    assert!(app.tree_filter_editing, "still editing the filter");
    assert!(titles.iter().any(|t| t == "Para Two"), "filtered rows keep Para Two");
    assert!(titles.iter().all(|t| t.to_lowercase().contains("para") || t == "Probe Book" || t == "Chapter One"), "a non-matching row survived the filter: {titles:?}");
    // Leaving the pane keeps the needle as if Enter had been pressed.
    let _ = app.handle_key(k(KeyCode::Tab));
    out.push(format!("[TREE filter] after Tab: focus {:?} · editing {} · needle {:?} · rows {}", app.focus, app.tree_filter_editing, app.tree_filter.as_str(), app.rows.len()));
    assert!(!app.tree_filter_editing, "Tab should end filter editing");
    assert_eq!(app.tree_filter.as_str(), "para", "Tab should keep the needle");
    assert_ne!(app.focus, Focus::Tree);
    app.change_focus(Focus::Tree);
    app.focus = Focus::Tree;
    let _ = app.handle_key(k(KeyCode::Esc));
    out.push(format!("[TREE filter] after Esc: needle {:?} · rows {}", app.tree_filter.as_str(), app.rows.len()));
    assert!(app.tree_filter.is_empty(), "Esc clears a kept filter");
    assert!(tree_invariants(&app).is_none());
    // An empty needle abandoned by a focus change is dropped.
    let _ = app.handle_key(k(KeyCode::Char('/')));
    let _ = app.handle_key(k(KeyCode::Tab));
    assert!(!app.tree_filter_editing && app.tree_filter.is_empty());
    app.change_focus(Focus::Tree);
    app.focus = Focus::Tree;

    // ── mouse click lands on the row drawn under it ──────────────────────
    // Scroll the tree so the top row is not row 0, then click every body
    // line and compare with the line → row table the frame built.
    let (w, h) = (80u16, 30u16);
    app.tree_cursor = app.rows.len() - 1;
    render(&mut app, w, h).unwrap();
    let body_y = app.layout_tree.y + 1;
    out.push(format!("[TREE mouse] {}x{} · tree rect {:?} · rows {} · scroll {} · line map {:?}", w, h, app.layout_tree, app.rows.len(), app.tree_scroll, app.tree_line_rows));
    assert!(app.tree_scroll > 0, "the tree should be scrolled with the cursor on the last row");
    let distinct = { let mut v = app.tree_line_rows.clone(); v.dedup(); v.len() };
    assert!(distinct >= 2, "the body should show several rows at 80x30: {:?}", app.tree_line_rows);
    // The map covers the body (borders + footer excluded) and no more.
    let body_h = app.tree_line_rows.len();
    assert!(body_h >= 4 && body_h + 2 < app.layout_tree.height as usize, "line map {body_h} lines vs pane height {}", app.layout_tree.height);
    // Independent expectation: walk the rows from `tree_scroll` by their
    // wrapped heights.
    let width = app.layout_tree.width.saturating_sub(2) as usize;
    let mut expect: Vec<usize> = Vec::new();
    let mut r = app.tree_scroll;
    while expect.len() < body_h && r < app.rows.len() {
        for _ in 0..app.tree_row_visual_height(r, width) {
            if expect.len() < body_h {
                expect.push(r);
            }
        }
        r += 1;
    }
    assert_eq!(app.tree_line_rows, expect, "line map must follow the wrapped row heights");
    let long_row = row_of_title(&app, long_title).expect("long row");
    assert!(app.tree_row_visual_height(long_row, width) > 1, "the long title must wrap at this width");
    for line in 0..body_h {
        let expected = app.tree_line_rows[line];
        let x = app.layout_tree.x + 2;
        if let Err(e) = click(&mut app, x, body_y + line as u16) {
            out.push(format!("[TREE mouse] click line {line}: {e}"));
            continue;
        }
        if app.tree_cursor != expected {
            out.push(format!("[TREE mouse] click line {line}: cursor {} · expected row {expected} ({})", app.tree_cursor, title_of(&app, app.rows[expected].0)));
        }
        // Re-render so the next click sees the frame a user would.
        render(&mut app, w, h).unwrap();
    }
    assert!(!out.iter().any(|l| l.starts_with("[TREE mouse] click")), "a click landed on the wrong row:\n{}", out.join("\n"));
    // A click below the last drawn line is ignored (no cursor jump).
    let before = app.tree_cursor;
    let x = app.layout_tree.x + 2;
    let _ = click(&mut app, x, body_y + body_h as u16 + 5);
    assert_eq!(app.tree_cursor, before, "a click past the drawn rows must not move the cursor");

    // ── structural chords ────────────────────────────────────────────────
    app.focus = Focus::Tree;
    app.modal = Modal::None;
    app.tree_cursor = row_of_title(&app, "Para Two").expect("Para Two row");
    let _ = app.handle_key(k(KeyCode::Char('+')));
    assert!(matches!(app.modal, Modal::Adding { .. }), "+ opens the add modal");
    type_str(&mut app, "Added Para");
    let _ = app.handle_key(k(KeyCode::Enter));
    app.modal = Modal::None;
    let added = app.hierarchy.iter().find(|n| n.title == "Added Para").map(|n| n.id).expect("+ then Enter creates the paragraph");
    out.push(format!("[TREE add] created `Added Para` · rows {}", app.rows.len()));
    // Move up / down swap siblings.
    app.focus = Focus::Tree;
    app.tree_cursor = row_of_title(&app, "Added Para").unwrap();
    let siblings = |a: &App| -> Vec<String> {
        let parent = a.hierarchy.get(added).and_then(|n| n.parent_id);
        a.hierarchy.children_of(parent).iter().map(|n| n.title.clone()).collect()
    };
    let before = siblings(&app);
    let _ = app.handle_key(k(KeyCode::Char('u')));
    let after_up = siblings(&app);
    let _ = app.handle_key(k(KeyCode::Char('j')));
    let after_down = siblings(&app);
    out.push(format!("[TREE move] {before:?} → u → {after_up:?} → j → {after_down:?}"));
    assert_ne!(before, after_up, "u must swap with the previous sibling");
    assert_eq!(before, after_down, "j must swap back");
    assert_eq!(app.rows.get(app.tree_cursor).map(|(id, _)| *id), Some(added), "the cursor follows the moved node");
    // Status + type cycles.
    let _ = app.handle_key(k(KeyCode::Char('o')));
    let status = app.hierarchy.get(added).and_then(|n| n.status.clone());
    out.push(format!("[TREE status] after o: {status:?}"));
    assert!(status.is_some(), "o cycles the status off None");
    let _ = app.handle_key(k(KeyCode::Char('t')));
    let ct = app.hierarchy.get(added).and_then(|n| n.content_type.clone());
    out.push(format!("[TREE type] after t: {ct:?}"));
    assert_eq!(ct.as_deref(), Some("hjson"), "t cycles typst → hjson");
    // Clipboard: copy then affix into the chapter.
    app.tree_cursor = row_of_title(&app, "Added Para").unwrap();
    let _ = app.handle_key(k(KeyCode::Char('y')));
    app.tree_cursor = row_of_title(&app, "Chapter One").unwrap();
    let count_before = app.hierarchy.iter().filter(|n| n.title == "Added Para").count();
    let _ = app.handle_key(k(KeyCode::Char('f')));
    let count_after = app.hierarchy.iter().filter(|n| n.title == "Added Para").count();
    out.push(format!("[TREE clipboard] y + f on the chapter: copies {count_before} → {count_after}"));
    assert_eq!(count_after, count_before + 1, "f affixes a copy");
    // Delete with confirmation.
    app.tree_cursor = row_of_title(&app, "Added Para").unwrap();
    let kills = app.kill_ring.len();
    let _ = app.handle_key(k(KeyCode::Char('-')));
    assert!(matches!(app.modal, Modal::Deleting { .. }), "- opens the delete modal on a paragraph");
    let _ = app.handle_key(km(KeyCode::Enter, KeyModifiers::CONTROL));
    assert!(matches!(app.modal, Modal::Deleting { .. }), "a chorded Enter must not confirm a delete");
    let _ = app.handle_key(k(KeyCode::Char('y')));
    let remaining = app.hierarchy.iter().filter(|n| n.title == "Added Para").count();
    out.push(format!("[TREE delete] - then y: copies {count_after} → {remaining} · kill ring {} → {}", kills, app.kill_ring.len()));
    assert_eq!(remaining, count_after - 1, "y confirms the delete");
    assert_eq!(app.kill_ring.len(), kills + 1, "the deleted paragraph lands in the kill ring");
    assert!(tree_invariants(&app).is_none());
    // Folding.
    app.tree_cursor = row_of_title(&app, "Para Two").unwrap();
    let full = app.rows.len();
    let _ = app.handle_key(k(KeyCode::Char('x')));
    let folded = app.rows.len();
    app.tree_cursor = row_of_title(&app, "Probe Book").unwrap();
    let _ = app.handle_key(k(KeyCode::Right));
    let book_open = app.rows.len();
    out.push(format!("[TREE fold] rows {full} → x → {folded} → → on the book → {book_open}"));
    assert!(folded < full, "x folds every branch");
    assert!(book_open > folded, "→ expands the book");
    assert!(tree_invariants(&app).is_none());

    drop(tmp);
    let panics: Vec<&String> = out.iter().filter(|l| l.contains("PANIC") || l.contains("Err ") || l.contains("QUIT")).collect();
    assert!(panics.is_empty(), "{} panicking / erroring probe steps:\n{}", panics.len(), panics.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    let breaks: Vec<&String> = out.iter().filter(|l| l.contains("] after ") && (l.contains("cursor") || l.contains("differ") || l.contains("not in the hierarchy"))).collect();
    assert!(breaks.is_empty(), "tree invariants broke:\n{}", breaks.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    });
}

/// Render one frame and return it as text rows.
fn screen_text(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| app.draw(f)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect()
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn overlay_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("overlay", |out| {
        let (tmp, mut app, pid) = probe_app();
        std::env::set_current_dir(tmp.path()).unwrap();
        // Thirty book prompts so the picker has to scroll.
        {
            let prompts = app.hierarchy.iter().find(|n| n.kind == NodeKind::Book && n.title == "Prompts").cloned().expect("Prompts book");
            for i in 0..30 {
                let h = Hierarchy::load(&app.store).unwrap();
                let mut n = app
                    .store
                    .create_node(&app.cfg, &h, NodeKind::Paragraph, &format!("Prompt {i:02}"), Some(&prompts), None, InsertPosition::End)
                    .unwrap();
                write_body(&app.store, tmp.path(), &mut n, format!("= Prompt {i:02}\n\nRewrite {{{{selection}}}} as variant {i}.\n").as_bytes());
            }
            app.reload_hierarchy();
        }
        open_para(&mut app, pid);

        // ── search bar + results overlay ─────────────────────────────────
        app.focus = Focus::SearchBar;
        app.modal = Modal::None;
        type_str(&mut app, "harbor");
        let _ = app.handle_key(k(KeyCode::Enter));
        out.push(format!("[SEARCH] `harbor` → overlay={} · {} result(s) · {}", app.show_results_overlay, app.results.len(), app.status));
        assert!(app.show_results_overlay, "Enter runs the search and opens the overlay");
        assert!(!app.results.is_empty(), "the fixture prose mentions the harbor");
        for (w, h) in SIZES {
            app.show_results_overlay = true;
            app.focus = Focus::SearchBar;
            if let Err(e) = render(&mut app, w, h) {
                out.push(format!("[SEARCH overlay] render {w}x{h}: {e}"));
            }
        }
        let n = app.results.len();
        let _ = app.handle_key(k(KeyCode::End));
        assert_eq!(app.results_cursor, n - 1, "End selects the last result");
        let _ = app.handle_key(k(KeyCode::Down));
        assert_eq!(app.results_cursor, n - 1, "Down stops at the last result");
        let _ = app.handle_key(k(KeyCode::Home));
        assert_eq!(app.results_cursor, 0);
        let _ = app.handle_key(k(KeyCode::Char('x')));
        assert!(!app.show_results_overlay, "typing closes the overlay");
        let _ = app.handle_key(k(KeyCode::Backspace));
        let _ = app.handle_key(k(KeyCode::Enter));
        assert!(app.show_results_overlay);
        let _ = app.handle_key(k(KeyCode::Esc));
        assert!(!app.show_results_overlay && app.focus == Focus::SearchBar, "Esc closes the overlay and keeps the bar focused");
        let _ = app.handle_key(k(KeyCode::Enter));
        let _ = app.handle_key(k(KeyCode::Enter));
        out.push(format!("[SEARCH] Enter on the first result → focus {:?} · opened {:?} · overlay {}", app.focus, app.opened.as_ref().map(|d| d.title.clone()), app.show_results_overlay));
        assert!(!app.show_results_overlay, "opening a result closes the overlay");
        app.show_results_overlay = true;
        app.focus = Focus::SearchBar;
        out.extend(battery_on_pane(&mut app, Focus::SearchBar, "SEARCH battery", &|_| None));
        app.show_results_overlay = false;
        app.search_input.clear();

        // ── prompt picker ────────────────────────────────────────────────
        app.focus = Focus::AiPrompt;
        app.ai_input.clear();
        app.show_prompt_picker = false;
        type_str(&mut app, "/");
        let total = app.prompt_picker_matches().len();
        out.push(format!("[PROMPTS] `/` → picker={} · {total} candidates", app.show_prompt_picker));
        assert!(app.show_prompt_picker && total >= 30, "the picker lists the book prompts");
        for (w, h) in SIZES {
            if let Err(e) = render(&mut app, w, h) {
                out.push(format!("[PROMPTS] render {w}x{h}: {e}"));
            }
        }
        for _ in 0..(total - 1) {
            let _ = app.handle_key(k(KeyCode::Down));
        }
        assert_eq!(app.prompt_picker_cursor, total - 1, "Down walks to the last candidate");
        let last = app.prompt_picker_matches().last().map(|p| p.description.clone()).unwrap_or_default();
        let screen = screen_text(&mut app, 80, 24);
        let visible = screen.iter().any(|row| row.contains(&last));
        out.push(format!("[PROMPTS] last candidate {last:?} visible at 80x24: {visible}"));
        assert!(visible, "the selected prompt must be scrolled into view");
        let _ = app.handle_key(k(KeyCode::Up));
        type_str(&mut app, "prompt 1");
        let narrowed = app.prompt_picker_matches().len();
        out.push(format!("[PROMPTS] `/prompt 1` → {narrowed} candidates · cursor {}", app.prompt_picker_cursor));
        assert!(narrowed < total && app.prompt_picker_cursor < narrowed, "the filter narrows and clamps the cursor");
        let _ = app.handle_key(k(KeyCode::Tab));
        out.push(format!("[PROMPTS] Tab → picker={} · prompt {:?}", app.show_prompt_picker, app.ai_input.as_str().lines().next()));
        assert!(!app.show_prompt_picker && !app.ai_input.as_str().starts_with('/'), "Tab expands the template");
        app.ai_input.clear();
        type_str(&mut app, "/");
        out.extend(battery_on_pane(&mut app, Focus::AiPrompt, "PROMPTS battery", &|_| None));
        app.ai_input.clear();
        app.show_prompt_picker = false;

        // ── file picker ──────────────────────────────────────────────────
        app.focus = Focus::Editor;
        app.modal = Modal::None;
        app.open_file_picker(super::super::file_picker::PickerContext::EditorLoad);
        assert!(matches!(app.modal, Modal::FilePicker(_)));
        for key in [k(KeyCode::Right), k(KeyCode::Down), k(KeyCode::End), k(KeyCode::Left), k(KeyCode::Home), k(KeyCode::PageDown), k(KeyCode::PageUp)] {
            let _ = app.handle_key(key);
            for (w, h) in [(80, 24), (20, 6), (8, 3)] {
                if let Err(e) = render(&mut app, w, h) {
                    out.push(format!("[FILES] render {w}x{h}: {e}"));
                }
            }
        }
        let _ = app.handle_key(k(KeyCode::Enter)); // a directory under EditorLoad is rejected
        out.push(format!("[FILES] Enter on the first entry → modal open={} · {}", !matches!(app.modal, Modal::None), app.status));
        let _ = app.handle_key(k(KeyCode::Esc));
        assert!(matches!(app.modal, Modal::None), "Esc closes the file picker");

        drop(tmp);
        let panics: Vec<&String> = out.iter().filter(|l| l.contains("PANIC") || l.contains("Err ") || l.contains("QUIT")).collect();
        assert!(panics.is_empty(), "{} panicking / erroring probe steps:\n{}", panics.len(), panics.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    });
}

fn fake_inference(lines: usize, status: super::super::inference::InferenceStatus) -> super::super::inference::Inference {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let response: String = (0..lines)
        .map(|i| format!("probe reply line {i} — the harbor froze and the boats waited through the long cold of January\n"))
        .collect();
    super::super::inference::Inference {
        provider: "probe".into(),
        model: "probe-model".into(),
        response,
        status,
        rx,
        started_at: std::time::Instant::now(),
    }
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn right_pane_probe() {
    use super::super::inference::InferenceStatus;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("right-pane", |out| {
    let (tmp, mut app, pid) = probe_app();
    std::env::set_current_dir(tmp.path()).unwrap();
    open_para(&mut app, pid);

    // ── batteries: AI pane (no / streaming / done reply), Thoughts, Output,
    //    Canon, AI prompt ──────────────────────────────────────────────────
    let no_change = |_: &App| None;
    for (label, pane, inference) in [
        ("AI none", RightPane::Ai, None),
        ("AI streaming", RightPane::Ai, Some(InferenceStatus::Streaming)),
        ("AI done", RightPane::Ai, Some(InferenceStatus::Done)),
        ("AI error", RightPane::Ai, Some(InferenceStatus::Error("probe error".into()))),
        ("Output", RightPane::Output, None),
        ("Thoughts", RightPane::Thoughts, None),
        ("Canon", RightPane::Canon, None),
    ] {
        app.right_pane = pane;
        app.thoughts = (0..40).map(|i| format!("reflection {i}: the ice holds the boats like a hand.")).collect();
        app.inference = inference.map(|s| fake_inference(60, s));
        // The done-state letters edit the open paragraph; run them on a
        // throwaway buffer state and restore it after.
        let saved_lines = app.opened.as_ref().map(|d| d.textarea.lines().to_vec());
        out.extend(battery_on_pane(&mut app, Focus::Ai, &format!("RIGHT {label}"), &no_change));
        if let (Some(lines), Some(d)) = (saved_lines, app.opened.as_mut()) {
            d.textarea = tui_textarea::TextArea::new(lines);
            d.dirty = false;
        }
        app.ai_fullscreen = false;
        app.right_fullscreen = false;
    }
    app.right_pane = RightPane::Ai;
    app.inference = None;
    out.extend(battery_on_pane(&mut app, Focus::AiPrompt, "RIGHT AI prompt", &no_change));
    app.ai_input.clear();
    app.show_prompt_picker = false;
    app.inference = None;
    app.ai_fullscreen = true;
    app.chat_history.clear();
    for i in 0..30 {
        app.chat_history.push(crate::ai::stream::ChatTurn::User(format!("question {i}")));
        app.chat_history.push(crate::ai::stream::ChatTurn::Assistant(format!("answer {i}: the harbor froze.\nSecond line.\n")));
    }
    out.extend(battery_on_pane(&mut app, Focus::AiPrompt, "RIGHT AI fullscreen prompt", &no_change));
    out.extend(battery_on_pane(&mut app, Focus::Ai, "RIGHT AI fullscreen pane", &no_change));
    app.ai_fullscreen = false;
    app.ai_input.clear();
    app.show_prompt_picker = false;
    app.chat_selection = None;
    app.chat_search = None;

    // ── single-response scroll clamps ─────────────────────────────────────
    app.right_pane = RightPane::Ai;
    app.focus = Focus::Ai;
    app.modal = Modal::None;
    app.inference = Some(fake_inference(80, InferenceStatus::Done));
    app.ai_mode = AiMode::Paragraph;
    app.ai_response_scroll = 0;
    render(&mut app, 80, 24).unwrap();
    let max = app.ai_response_scroll_max.get();
    out.push(format!("[AI scroll] rendered extent {max}"));
    assert!(max > 0, "an 80-line reply must scroll at 80x24");
    let _ = app.handle_key(k(KeyCode::Home));
    let at_home = app.ai_response_scroll;
    let _ = app.handle_key(k(KeyCode::Down));
    let after_down = app.ai_response_scroll;
    out.push(format!("[AI scroll] Home → {at_home} · ↓ → {after_down} (max {max})"));
    assert_eq!(at_home, max, "Home jumps to the rendered top");
    assert_eq!(after_down, max - 1, "↓ right after Home must move");
    for _ in 0..50 {
        let _ = app.handle_key(k(KeyCode::PageUp));
    }
    assert_eq!(app.ai_response_scroll, max, "PgUp clamps to the extent");
    let _ = app.handle_key(k(KeyCode::PageDown));
    assert_eq!(app.ai_response_scroll, max.saturating_sub(10), "PgDn moves right after the clamp");
    let _ = app.handle_key(k(KeyCode::End));
    assert_eq!(app.ai_response_scroll, 0, "End re-arms follow");

    // ── chorded letters never apply the reply ─────────────────────────────
    let lines_before = app.opened.as_ref().map(|d| d.textarea.lines().to_vec()).unwrap();
    for key in [ctrl('r'), km(KeyCode::Char('i'), KeyModifiers::ALT), ctrl('b'), ctrl('t'), ctrl('g'), ctrl('c'), ctrl('z'), ctrl('e')] {
        app.focus = Focus::Ai;
        app.modal = Modal::None;
        let _ = app.handle_key(key);
        assert!(matches!(app.modal, Modal::None), "a chorded letter opened the diff review ({key:?})");
    }
    let lines_after = app.opened.as_ref().map(|d| d.textarea.lines().to_vec()).unwrap();
    out.push(format!("[AI guard] chorded r/i/b/t/g/c/z/e: buffer unchanged={} · inference still shown={}", lines_before == lines_after, app.inference.is_some()));
    assert_eq!(lines_before, lines_after, "a chorded letter applied the AI reply");
    assert!(app.inference.is_some(), "a chorded letter cleared the reply");
    app.focus = Focus::Ai;
    let _ = app.handle_key(k(KeyCode::Char('r')));
    let lines_plain = app.opened.as_ref().map(|d| d.textarea.lines().to_vec()).unwrap();
    // With `ai.diff_review_on_apply` (the default) `r` opens the diff review
    // instead of writing straight into the buffer; either is "applied".
    let reviewed = matches!(app.modal, Modal::AiDiffReview { .. });
    out.push(format!("[AI guard] plain r: diff review opened={reviewed} · buffer changed={}", lines_plain != lines_before));
    assert!(reviewed || lines_plain != lines_before, "plain r must still apply the reply");
    app.modal = Modal::None;
    if let Some(d) = app.opened.as_mut() {
        d.textarea = tui_textarea::TextArea::new(lines_before);
        d.dirty = false;
    }

    // ── Thoughts clamps + guard ───────────────────────────────────────────
    app.right_pane = RightPane::Thoughts;
    app.focus = Focus::Ai;
    app.inference = None;
    app.thoughts = (0..80).map(|i| format!("reflection {i}: a long line that should wrap at the pane width to make the extent larger")).collect();
    app.thoughts_scroll = 0;
    render(&mut app, 80, 24).unwrap();
    let tmax = app.thoughts_scroll_max.get();
    out.push(format!("[THOUGHTS scroll] rendered extent {tmax}"));
    assert!(tmax > 0);
    let _ = app.handle_key(k(KeyCode::Char('G')));
    let at_end = app.thoughts_scroll;
    let _ = app.handle_key(k(KeyCode::Up));
    let after_up = app.thoughts_scroll;
    out.push(format!("[THOUGHTS scroll] G → {at_end} · ↑ → {after_up}"));
    assert_eq!(at_end, tmax, "G jumps to the rendered bottom");
    assert_eq!(after_up, tmax - 1, "↑ right after G must move");
    let n = app.thoughts.len();
    let _ = app.handle_key(ctrl('c'));
    assert_eq!(app.thoughts.len(), n, "Ctrl+C must not clear the Thoughts pane");
    let _ = app.handle_key(k(KeyCode::Char('c')));
    assert!(app.thoughts.is_empty(), "plain c clears the Thoughts pane");

    // ── fullscreen chat history clamps ────────────────────────────────────
    app.right_pane = RightPane::Ai;
    app.ai_fullscreen = true;
    app.focus = Focus::AiPrompt;
    app.chat_history_scroll = 0;
    render(&mut app, 80, 24).unwrap();
    let cmax = app.chat_history_scroll_max.get();
    out.push(format!("[CHAT scroll] rendered extent {cmax}"));
    assert!(cmax > 0, "60 turns must scroll at 80x24");
    for _ in 0..200 {
        let _ = app.handle_key(k(KeyCode::PageUp));
    }
    let top = app.chat_history_scroll;
    let _ = app.handle_key(k(KeyCode::Down));
    let after = app.chat_history_scroll;
    out.push(format!("[CHAT scroll] PgUp×200 → {top} · ↓ → {after}"));
    assert_eq!(top, cmax, "PgUp clamps to the history top");
    assert_eq!(after, cmax - 1, "↓ right after the clamp must move");
    app.ai_fullscreen = false;
    app.chat_history_scroll = 0;

    // ── Output `/` query is a text box ───────────────────────────────────
    app.right_pane = RightPane::Output;
    app.focus = Focus::Ai;
    app.modal = Modal::None;
    app.output_filter.clear();
    let _ = app.handle_key(k(KeyCode::Char('/')));
    assert!(app.output_query_focused, "/ opens the query line");
    type_str(&mut app, "ha");
    let _ = app.handle_key(k(KeyCode::Left));
    type_str(&mut app, "x");
    let _ = app.handle_key(ctrl('q'));
    let q = app.output_filter.text_query.clone();
    out.push(format!("[OUTPUT query] after `/ h a ← x Ctrl+Q`: {q:?}"));
    assert_eq!(q.as_deref(), Some("hxa"), "output query chords");
    render(&mut app, 80, 24).unwrap();
    let _ = app.handle_key(k(KeyCode::Enter));
    assert!(!app.output_query_focused && app.output_filter.text_query.as_deref() == Some("hxa"), "Enter keeps the query");
    let _ = app.handle_key(k(KeyCode::Char('/')));
    let _ = app.handle_key(k(KeyCode::Esc));
    assert!(app.output_filter.text_query.is_none(), "Esc clears the query");

    drop(tmp);
    let panics: Vec<&String> = out.iter().filter(|l| l.contains("PANIC") || l.contains("Err ") || l.contains("QUIT")).collect();
    assert!(panics.is_empty(), "{} panicking / erroring probe steps:\n{}", panics.len(), panics.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    });
}

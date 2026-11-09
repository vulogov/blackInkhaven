//! Modal probe (3.19) — opens every modal the headless harness can reach
//! through its real opener, renders it at nine terminal sizes from 8x3 to
//! 200x60, feeds a battery of ~115 keys plus mouse scrolls, then Enter and
//! Esc, and asserts that nothing panics and that the staged-Esc contracts
//! hold (HJSON / comment / footnote editors keep typed text on one Esc;
//! the shell-help and HJSON-restart overlays close before their panes).
//!
//! It opens a real `Store` (needs the cached embedding model), so it is
//! `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture modal_probe
//! ```
//!
//! The full per-modal log is printed and written to
//! `<temp dir>/inkhaven-modal-probe.txt`.
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn km(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
fn ctrl(c: char) -> KeyEvent {
    km(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".into()
    }
}

fn render(app: &mut App, w: u16, h: u16) -> Result<(), String> {
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| app.draw(f)).map(|_| ())
    }));
    match r {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(format!("io: {e}")),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn battery() -> Vec<(String, KeyEvent)> {
    let mut v: Vec<(String, KeyEvent)> = Vec::new();
    let mut push = |n: &str, e: KeyEvent| v.push((n.to_string(), e));
    for _ in 0..3 { push("Up", k(KeyCode::Up)); }
    for _ in 0..8 { push("Down", k(KeyCode::Down)); }
    for _ in 0..3 { push("PageDown", k(KeyCode::PageDown)); }
    push("End", k(KeyCode::End));
    push("Down", k(KeyCode::Down));
    for _ in 0..3 { push("PageUp", k(KeyCode::PageUp)); }
    push("Home", k(KeyCode::Home));
    push("Up", k(KeyCode::Up));
    for _ in 0..3 { push("Left", k(KeyCode::Left)); }
    for _ in 0..6 { push("Right", k(KeyCode::Right)); }
    for _ in 0..3 { push("Tab", k(KeyCode::Tab)); }
    push("BackTab", k(KeyCode::BackTab));
    for c in ['a', 'b', 'Z', '0', '/', '?', ' ', '.', ':', '-', '+', 'x', 'y', 'n', 'd', 'v', 's', 'e', 'r', 'g', 'G', 'j', 'k', 'q', 'p', 'c', 't', 'm', 'w', 'f', 'o', 'u', 'i', 'h', 'l'] {
        push(&format!("Char({c:?})"), k(KeyCode::Char(c)));
    }
    for _ in 0..3 { push("Backspace", k(KeyCode::Backspace)); }
    for _ in 0..2 { push("Delete", k(KeyCode::Delete)); }
    push("Insert", k(KeyCode::Insert));
    for n in 1..=5u8 { push(&format!("F{n}"), k(KeyCode::F(n))); }
    for c in "abcdefghijklmnoprstuvwxyz".chars() {
        push(&format!("Ctrl+{c}"), ctrl(c));
    }
    push("Alt+b", km(KeyCode::Char('b'), KeyModifiers::ALT));
    push("Alt+f", km(KeyCode::Char('f'), KeyModifiers::ALT));
    push("Shift+Up", km(KeyCode::Up, KeyModifiers::SHIFT));
    push("Shift+Down", km(KeyCode::Down, KeyModifiers::SHIFT));
    push("Shift+Enter", km(KeyCode::Enter, KeyModifiers::SHIFT));
    push("Ctrl+Enter", km(KeyCode::Enter, KeyModifiers::CONTROL));
    push("Ctrl+Right", km(KeyCode::Right, KeyModifiers::CONTROL));
    push("Ctrl+Backspace", km(KeyCode::Backspace, KeyModifiers::CONTROL));
    v
}

fn send(app: &mut App, key: KeyEvent) -> Result<bool, String> {
    match catch_unwind(AssertUnwindSafe(|| app.handle_key(key))) {
        Ok(Ok(b)) => Ok(b),
        Ok(Err(e)) => Err(format!("Err {e:#}")),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn scroll(app: &mut App, up: bool) -> Result<(), String> {
    let ev = MouseEvent {
        kind: if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown },
        column: 40,
        row: 12,
        modifiers: KeyModifiers::NONE,
    };
    match catch_unwind(AssertUnwindSafe(|| app.handle_mouse_in_modal(ev))) {
        Ok(()) => Ok(()),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

const SIZES: [(u16, u16); 9] = [(8, 3), (20, 6), (40, 12), (60, 20), (79, 24), (80, 24), (100, 30), (120, 40), (200, 60)];

fn exercise(
    app: &mut App,
    label: &str,
    focus: Focus,
    open: &mut dyn FnMut(&mut App),
    out: &mut Vec<String>,
) -> bool {
    let mut reopen = |app: &mut App, out: &mut Vec<String>| -> bool {
        app.modal = Modal::None;
        app.focus = focus;
        app.status.clear();
        if let Err(p) = catch_unwind(AssertUnwindSafe(|| open(app))) {
            out.push(format!("[{label}] OPEN PANIC: {}", panic_text(p)));
            return false;
        }
        !matches!(app.modal, Modal::None)
    };
    if !reopen(app, out) {
        out.push(format!("[{label}] did not open — status: {}", app.status));
        return false;
    }
    let disc = std::mem::discriminant(&app.modal);
    for (w, h) in SIZES {
        if let Err(e) = render(app, w, h) {
            out.push(format!("[{label}] render {w}x{h}: {e}"));
        }
    }
    let mut closers: Vec<String> = Vec::new();
    let mut changers: Vec<String> = Vec::new();
    for (name, key) in battery() {
        if matches!(label, "Deleting" | "TagDeleteConfirm" | "ConfirmQuit" | "ConfirmQuitUncommitted")
            && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Enter)
        {
            continue;
        }
        if matches!(app.modal, Modal::None) || std::mem::discriminant(&app.modal) != disc {
            if !reopen(app, out) {
                out.push(format!("[{label}] could not reopen before {name}"));
                return true;
            }
        }
        match send(app, key) {
            Ok(true) => out.push(format!("[{label}] key {name} requested QUIT")),
            Ok(false) => {}
            Err(e) => out.push(format!("[{label}] key {name}: {e} · status: {}", app.status)),
        }
        if matches!(app.modal, Modal::None) {
            closers.push(name.clone());
        } else if std::mem::discriminant(&app.modal) != disc {
            changers.push(name.clone());
        }
        if let Err(e) = render(app, 80, 24) {
            out.push(format!("[{label}] render after {name}: {e}"));
        }
        if let Err(e) = render(app, 20, 6) {
            out.push(format!("[{label}] render 20x6 after {name}: {e}"));
        }
    }
    if matches!(app.modal, Modal::None) || std::mem::discriminant(&app.modal) != disc {
        reopen(app, out);
    }
    for up in [true, true, true, false, false, false, false, false] {
        if let Err(e) = scroll(app, up) {
            out.push(format!("[{label}] mouse scroll: {e}"));
        }
    }
    if let Err(e) = render(app, 80, 24) {
        out.push(format!("[{label}] render after scroll: {e}"));
    }
    // Enter phase
    if matches!(app.modal, Modal::None) || std::mem::discriminant(&app.modal) != disc {
        reopen(app, out);
    }
    let skip_enter = matches!(label, "Deleting" | "ConfirmQuit" | "ConfirmQuitUncommitted" | "TagDeleteConfirm");
    let enter_res = if skip_enter { Ok(false) } else { send(app, k(KeyCode::Enter)) };
    let enter_state = if skip_enter {
        "skipped"
    } else if matches!(app.modal, Modal::None) {
        "closed"
    } else if std::mem::discriminant(&app.modal) != disc {
        "changed variant"
    } else {
        "open"
    };
    if let Err(e) = &enter_res {
        out.push(format!("[{label}] Enter: {e} · status: {}", app.status));
    }
    if let Err(e) = render(app, 80, 24) {
        out.push(format!("[{label}] render after Enter: {e}"));
    }
    // Esc phase
    if matches!(app.modal, Modal::None) || std::mem::discriminant(&app.modal) != disc {
        reopen(app, out);
    }
    let mut escs = 0;
    while !matches!(app.modal, Modal::None) && escs < 4 {
        escs += 1;
        if let Err(e) = send(app, k(KeyCode::Esc)) {
            out.push(format!("[{label}] Esc #{escs}: {e}"));
            break;
        }
    }
    let esc_note = if matches!(app.modal, Modal::None) {
        format!("Esc×{escs} closes")
    } else {
        "ESC×4 DID NOT CLOSE".to_string()
    };
    out.push(format!(
        "[{label}] ok · {esc_note} · Enter→{enter_state} ({}) · closers: {:?} · variant-changers: {:?}",
        app.status.chars().take(60).collect::<String>(),
        closers,
        changers
    ));
    app.modal = Modal::None;
    true
}

/// Select five characters at the start of row 2 so `start_add_comment` has an
/// anchor span whatever earlier modals did to the buffer.
fn select_span(a: &mut App) {
    if let Some(d) = a.opened.as_mut() {
        d.textarea.cancel_selection();
        d.textarea.move_cursor(tui_textarea::CursorMove::Jump(2, 0));
        d.textarea.start_selection();
        for _ in 0..5 {
            d.textarea.move_cursor(tui_textarea::CursorMove::Forward);
        }
    }
}

fn probe_app() -> (tempfile::TempDir, App, Uuid) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = Config::default();
    cfg.images.preview_enabled = false;
    cfg.sync_interval_seconds = 0;
    cfg.timeline.enabled = true;
    let layout = ProjectLayout::new(tmp.path());
    layout.create_layout().unwrap();
    cfg.save(&layout.config_path()).unwrap();
    let store = Store::open(layout.clone(), &cfg).expect("open store");
    let h = Hierarchy::load(&store).unwrap();
    let book = store
        .create_node(&cfg, &h, NodeKind::Book, "Probe Book", None, None, InsertPosition::End)
        .unwrap();
    let h = Hierarchy::load(&store).unwrap();
    let chapter = store
        .create_node(&cfg, &h, NodeKind::Chapter, "Chapter One", Some(&book), None, InsertPosition::End)
        .unwrap();
    let h = Hierarchy::load(&store).unwrap();
    let mut para = store
        .create_node(&cfg, &h, NodeKind::Paragraph, "Para One", Some(&chapter), None, InsertPosition::End)
        .unwrap();
    store
        .update_paragraph_content(
            &mut para,
            "= Para One\n\nThe harbor froze in January. \"Either it was a blessing or a curse,\" said Mara. The town waited. Snow kept falling on the quay, and the long boats sat locked in the ice like sleeping animals.\n\nA second paragraph with more words to wrap around narrow widths and give the previews something to chew on — ünïcödé too.\n\n#figure(rect(), caption: [Harbor]) <fig:harbor>\n".as_bytes(),
        )
        .unwrap();
    let h = Hierarchy::load(&store).unwrap();
    let mut para2 = store
        .create_node(&cfg, &h, NodeKind::Paragraph, "Para Two", Some(&chapter), None, InsertPosition::End)
        .unwrap();
    store
        .update_paragraph_content(&mut para2, b"= Para Two\n\nThe harbor thawed in March. Mara left.\n")
        .unwrap();
    let pid = para.id;
    // System books (Threads / Snippets / Character …) get one child each so
    // the pickers that read them have something to show.
    for (book_title, child, kind) in [("Threads", "Thread A", NodeKind::Paragraph), ("Snippets", "Snippet A", NodeKind::Paragraph), ("Character", "Mara", NodeKind::Paragraph), ("Characters", "Mara", NodeKind::Paragraph), ("Scripts", "script-a", NodeKind::Script), ("Sources", "Source A", NodeKind::Paragraph)] {
        let h = Hierarchy::load(&store).unwrap();
        let Some(sys) = h.iter().find(|n| n.kind == NodeKind::Book && n.title == book_title).cloned() else { continue };
        let h = Hierarchy::load(&store).unwrap();
        if let Ok(mut n) = store.create_node(&cfg, &h, kind, child, Some(&sys), None, InsertPosition::End) {
            let _ = store.update_paragraph_content(&mut n, format!("= {child}\n\nA body for {child}.\n").as_bytes());
        }
    }
    let app = App::new(layout, cfg, store).expect("App::new");
    (tmp, app, pid)
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn modal_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    std::panic::set_hook(Box::new(|_| {}));
    let (tmp, mut app, pid) = probe_app();
    // Any modal that writes a file on Enter (SaveMarkdown with a battery-edited
    // path, say) must land in the temp dir, never in the working tree.
    std::env::set_current_dir(tmp.path()).unwrap();
    let row = app.rows.iter().position(|(id, _)| *id == pid).expect("para row");
    app.tree_cursor = row;
    app.open_selected().unwrap();
    assert!(app.opened.is_some(), "paragraph did not open");
    app.focus = Focus::Editor;
    let mut out: Vec<String> = Vec::new();
    for (w, h) in SIZES {
        app.modal = Modal::None;
        if let Err(e) = render(&mut app, w, h) {
            out.push(format!("[BASELINE no modal] render {w}x{h}: {e}"));
        }
    }
    let md_dest = tmp.path().join("out.md");

    type Opener = Box<dyn FnMut(&mut App)>;
    let mut openers: Vec<(&str, Focus, Opener)> = vec![
        ("Adding", Focus::Tree, Box::new(|a: &mut App| a.open_add_modal(NodeKind::Paragraph))),
        ("Renaming", Focus::Tree, Box::new(|a: &mut App| a.open_rename_modal())),
        ("StructuralTypePicker", Focus::Tree, Box::new(|a: &mut App| a.open_structural_type_picker())),
        ("BundEval", Focus::Editor, Box::new(|a: &mut App| a.bund_open_eval_modal())),
        ("BundInput", Focus::Editor, Box::new(|a: &mut App| a.open_bund_input("prompt?", ""))),
        ("BundPane", Focus::Editor, Box::new(|a: &mut App| a.open_bund_pane("probe"))),
        ("ScriptPicker", Focus::Editor, Box::new(|a: &mut App| a.bund_open_script_picker())),
        ("Progress", Focus::Editor, Box::new(|a: &mut App| a.open_progress_modal())),
        ("ParagraphTarget", Focus::Editor, Box::new(|a: &mut App| a.open_paragraph_target_modal())),
        ("GoalsEditor", Focus::Editor, Box::new(|a: &mut App| a.open_goals_editor())),
        ("SaveMarkdown", Focus::Editor, Box::new(move |a: &mut App| a.open_save_markdown_modal("body".into(), md_dest.clone(), "probe".into()))),
        ("LinkPicker", Focus::Editor, Box::new(|a: &mut App| a.open_link_picker_modal())),
        ("BacklinkPicker", Focus::Editor, Box::new(|a: &mut App| a.open_backlink_picker_modal())),
        ("BookmarkPicker", Focus::Editor, Box::new(|a: &mut App| { a.toggle_bookmark(); a.open_bookmark_picker_modal(); })),
        ("Conversations", Focus::Editor, Box::new(|a: &mut App| a.open_conversations_modal())),
        ("FuzzyParagraphPicker", Focus::Editor, Box::new(|a: &mut App| a.open_fuzzy_paragraph_picker())),
        ("RecentParagraphPicker", Focus::Editor, Box::new(|a: &mut App| a.open_recent_paragraph_picker())),
        ("CitePicker", Focus::Editor, Box::new(|a: &mut App| a.open_cite_picker())),
        ("UniversePicker", Focus::Editor, Box::new(|a: &mut App| a.open_universe_picker())),
        ("XrefPicker", Focus::Editor, Box::new(|a: &mut App| a.open_xref_picker())),
        ("SnippetIncludePicker", Focus::Editor, Box::new(|a: &mut App| a.open_snippet_insert_picker())),
        ("SnippetsOverview", Focus::Editor, Box::new(|a: &mut App| a.open_snippets_overview())),
        ("CostDashboard", Focus::Editor, Box::new(|a: &mut App| a.open_cost_dashboard())),
        ("CommandPalette", Focus::Editor, Box::new(|a: &mut App| a.open_command_palette())),
        ("SimilarPicker", Focus::Editor, Box::new(|a: &mut App| a.toggle_similar_paragraph_mode())),
        ("Thesaurus", Focus::Editor, Box::new(|a: &mut App| { if let Some(d) = a.opened.as_mut() { d.textarea.move_cursor(tui_textarea::CursorMove::Jump(2, 6)); } a.open_thesaurus(); })),
        ("FactsSearch", Focus::Editor, Box::new(|a: &mut App| a.open_facts_search())),
        ("FilePicker", Focus::Editor, Box::new(|a: &mut App| a.open_file_picker(super::super::file_picker::PickerContext::EditorLoad))),
        ("QuickRef", Focus::Editor, Box::new(|a: &mut App| a.open_quickref())),
        ("Credits", Focus::Editor, Box::new(|a: &mut App| a.open_credits())),
        ("BookInfo", Focus::Editor, Box::new(|a: &mut App| a.open_book_info())),
        ("LlmPicker", Focus::Editor, Box::new(|a: &mut App| a.open_llm_picker())),
        ("ImagePicker", Focus::Editor, Box::new(|a: &mut App| { a.try_open_image_picker(); })),
        ("ImagePreview", Focus::Tree, Box::new(|a: &mut App| {
            let picker = ratatui_image::picker::Picker::from_fontsize((10, 20));
            let img = image::DynamicImage::new_rgb8(4, 4);
            a.modal = Modal::ImagePreview { title: "img".into(), fs_rel: "x.png".into(), size_bytes: 1, proto: picker.new_resize_protocol(img) };
        })),
        ("RenderedPreview", Focus::Editor, Box::new(|a: &mut App| a.open_rendered_paragraph_preview())),
        ("SaveRenderedPng", Focus::Editor, Box::new(|a: &mut App| { a.open_rendered_paragraph_preview(); a.open_save_rendered_png_picker(false); })),
        ("TagPicker", Focus::Tree, Box::new(move |a: &mut App| a.open_tag_picker_modal(super::super::modal::TagPickerTarget::TreeSelection(vec![pid])))),
        ("TagAddPrompt", Focus::Tree, Box::new(move |a: &mut App| { a.open_tag_picker_modal(super::super::modal::TagPickerTarget::TreeSelection(vec![pid])); a.open_tag_add_prompt(); })),
        ("TagDeleteConfirm", Focus::Tree, Box::new(move |a: &mut App| { a.open_tag_picker_modal(super::super::modal::TagPickerTarget::TreeSelection(vec![pid])); a.open_tag_delete_confirm(); })),
        ("TagRenamePrompt", Focus::Tree, Box::new(move |a: &mut App| { a.open_tag_picker_modal(super::super::modal::TagPickerTarget::TreeSelection(vec![pid])); a.open_tag_rename_prompt(); })),
        ("GotoLine", Focus::Editor, Box::new(|a: &mut App| a.open_goto_line())),
        ("TagSearchResults", Focus::Tree, Box::new(move |a: &mut App| { a.open_tag_picker_modal(super::super::modal::TagPickerTarget::TreeSelection(vec![pid])); a.open_tag_add_prompt(); for c in "probe".chars() { let _ = a.handle_key(k(KeyCode::Char(c))); } let _ = a.handle_key(k(KeyCode::Enter)); let _ = a.handle_key(k(KeyCode::Char('t'))); a.modal = Modal::None; a.open_tag_search_results_for_cursor(); })),
        ("DiagnosticsList", Focus::Editor, Box::new(|a: &mut App| a.open_diagnostics_list())),
        ("EventPicker", Focus::Editor, Box::new(|a: &mut App| a.open_event_picker())),
        ("TimelineView", Focus::Editor, Box::new(|a: &mut App| a.open_timeline_view())),
        ("TimelineNewEventPrompt", Focus::Editor, Box::new(|a: &mut App| { a.open_timeline_view(); a.timeline_open_new_event_prompt(); })),
        ("TimelineEditEventPrompt", Focus::Editor, Box::new(|a: &mut App| a.open_edit_event_metadata_prompt())),
        ("AiDiffReview", Focus::Editor, Box::new(|a: &mut App| a.open_ai_diff_review(super::super::inference::InferenceAction::Replace, "= Para One\n\nReplaced body.\n"))),
        ("SnapshotAnnotation", Focus::Editor, Box::new(|a: &mut App| a.create_snapshot_of_current())),
        ("StoryView", Focus::Editor, Box::new(|a: &mut App| { a.image_picker = Some(ratatui_image::picker::Picker::from_fontsize((10, 20))); a.open_story_view(); })),
        ("SaveStoryPng", Focus::Editor, Box::new(|a: &mut App| { a.image_picker = Some(ratatui_image::picker::Picker::from_fontsize((10, 20))); a.open_story_view(); a.open_save_story_png_picker(); })),
        ("FunctionPicker", Focus::Editor, Box::new(|a: &mut App| a.open_function_picker())),
        ("ChatSearchPrompt", Focus::Editor, Box::new(|a: &mut App| a.open_chat_search_prompt())),
        ("StatusFilter", Focus::Tree, Box::new(|a: &mut App| a.open_status_filter("Ready"))),
        ("HelpQuery", Focus::Editor, Box::new(|a: &mut App| a.open_help_query_modal())),
        ("FindReplace", Focus::Editor, Box::new(|a: &mut App| a.open_find_modal(true))),
        ("FindOnly", Focus::Editor, Box::new(|a: &mut App| a.open_find_modal(false))),
        ("ReplaceReview", Focus::Editor, Box::new(|a: &mut App| a.open_replace_review("the".into(), "a".into(), crate::replace::ReplaceOpts { regex: false, word_boundary: false, ignore_case: true }))),
        ("ImpositionPreview", Focus::Editor, Box::new(|a: &mut App| a.open_imposition_preview())),
        ("SubmissionsTracker", Focus::Editor, Box::new(|a: &mut App| a.open_submissions_tracker())),
        ("SubmissionGenPicker", Focus::Editor, Box::new(|a: &mut App| a.open_submission_gen())),
        ("PlanOutline", Focus::Editor, Box::new(|a: &mut App| a.open_plan_outline())),
        ("EditorialPass", Focus::Editor, Box::new(|a: &mut App| a.open_editorial_pass())),
        ("RevisionDecision", Focus::Editor, Box::new(move |a: &mut App| a.open_revision_decision(crate::editorial::EditorialFinding {
            category: "probe".into(), severity: crate::editorial::Severity::Info,
            location: crate::editorial::Location { chapter: None, paragraph: Some(pid), char_range: None, path: None },
            message: "probe finding".into(), hint: None, source: "doctor", autofixable: false }, pid))),
        ("StoryBible", Focus::Editor, Box::new(|a: &mut App| a.open_story_bible())),
        ("ConlangHub", Focus::Editor, Box::new(|a: &mut App| a.open_conlang_hub())),
        ("WorldOverview", Focus::Editor, Box::new(|a: &mut App| a.open_world_overview())),
        ("WorldProposals", Focus::Editor, Box::new(|a: &mut App| a.open_world_proposals())),
        ("InnerSocratesOverview", Focus::Editor, Box::new(|a: &mut App| a.open_inner_socrates_overview())),
        ("InnerPoetOverview", Focus::Editor, Box::new(|a: &mut App| a.open_inner_poet_overview())),
        ("InnerStylistOverview", Focus::Editor, Box::new(|a: &mut App| a.open_inner_stylist_overview())),
        ("StyleReport", Focus::Editor, Box::new(|a: &mut App| a.open_style_report())),
        ("ContinuityLedger", Focus::Editor, Box::new(|a: &mut App| a.open_continuity_ledger())),
        ("ReadThrough", Focus::Editor, Box::new(|a: &mut App| a.open_read_through())),
        ("Chronicle", Focus::Editor, Box::new(|a: &mut App| a.open_chronicle())),
        ("Canon", Focus::Editor, Box::new(|a: &mut App| a.open_canon())),
        ("Knowledge", Focus::Editor, Box::new(|a: &mut App| a.open_knowledge())),
        ("Bonds", Focus::Editor, Box::new(|a: &mut App| a.open_bonds())),
        ("ReaderHub", Focus::Editor, Box::new(|a: &mut App| a.open_reader_hub())),
        ("Cast", Focus::Editor, Box::new(|a: &mut App| a.open_cast())),
        ("PoemFormPicker", Focus::Editor, Box::new(|a: &mut App| a.open_poem_form_picker())),
        ("VerseTranslationView", Focus::Editor, Box::new(|a: &mut App| a.open_verse_translation_view())),
        ("InnerEditorOverview", Focus::Editor, Box::new(|a: &mut App| a.open_inner_editor_overview())),
        ("LangInsert", Focus::Editor, Box::new(|a: &mut App| a.maybe_trigger_lang_insert())),
        ("SnapshotPicker", Focus::Editor, Box::new(|a: &mut App| { a.create_snapshot_of_current(); let _ = a.handle_key(k(KeyCode::Enter)); a.open_snapshot_picker(); })),
        ("SnapshotDiff", Focus::Editor, Box::new(|a: &mut App| { a.create_snapshot_of_current(); let _ = a.handle_key(k(KeyCode::Enter)); a.open_snapshot_picker(); let _ = a.handle_key(k(KeyCode::Char('v'))); })),
        ("SnapshotBrowser", Focus::Editor, Box::new(|a: &mut App| a.open_snapshot_browser())),
        ("KillRingPicker", Focus::Editor, Box::new(|a: &mut App| { a.modal = Modal::None; a.focus = Focus::Editor; let _ = a.handle_key(ctrl('k')); let _ = a.handle_key(ctrl('k')); a.open_kill_ring_picker(); })),
        ("ShellPane", Focus::Editor, Box::new(|a: &mut App| a.open_shell_pane(true))),
        ("HjsonEditor", Focus::Editor, Box::new(|a: &mut App| a.open_hjson_editor())),
        ("ConfirmQuit", Focus::Editor, Box::new(|a: &mut App| a.modal = Modal::ConfirmQuit)),
        ("ConfirmQuitUncommitted", Focus::Editor, Box::new(|a: &mut App| a.modal = Modal::ConfirmQuitUncommitted { count: 3 })),
        ("TtsUnavailable", Focus::Editor, Box::new(|a: &mut App| a.modal = Modal::TtsUnavailable { title: "t".into(), reason: "r".into() })),
        ("WritingStreakHeatmap", Focus::Editor, Box::new(|a: &mut App| { a.modal = Modal::None; a.focus = Focus::Editor; let _ = a.handle_key(k(KeyCode::Char('x'))); let _ = a.save_focused(); a.open_writing_streak_heatmap(); })),
        ("TtsSaveAsAudio", Focus::Editor, Box::new(|a: &mut App| a.tts_open_save_as_audio_picker())),
        ("TtsPlayback", Focus::Editor, Box::new(|a: &mut App| a.modal = Modal::TtsPlayback { started_at: std::time::Instant::now(), preview: "p".into(), voice_label: "v".into() })),
        ("SentenceRhythm", Focus::Editor, Box::new(|a: &mut App| a.open_sentence_rhythm())),
        ("Concordance", Focus::Editor, Box::new(|a: &mut App| a.open_concordance())),
        ("TranslationLanguagePicker", Focus::Editor, Box::new(|a: &mut App| a.start_translate_in_direction(super::super::modal::TranslationDirection::ToInvented))),
        ("ThreadsPicker", Focus::Editor, Box::new(|a: &mut App| a.open_threads_picker())),
        ("ThreadWeaveView", Focus::Editor, Box::new(|a: &mut App| { a.open_threads_picker(); a.open_thread_weave_view(); })),
        ("CommentsPanel", Focus::Editor, Box::new(|a: &mut App| { select_span(a); a.start_add_comment(); for c in "probe comment".chars() { let _ = a.handle_key(k(KeyCode::Char(c))); } let _ = a.handle_key(ctrl('s')); a.open_comments_panel(); })),
        ("FootnoteEditor", Focus::Editor, Box::new(|a: &mut App| a.start_insert_footnote())),
        ("ProjectGoalModal", Focus::Editor, Box::new(|a: &mut App| a.open_project_goal_modal())),
        ("StyleTransferPicker", Focus::Editor, Box::new(|a: &mut App| a.start_style_transfer_picker())),
        ("CommentEditor", Focus::Editor, Box::new(|a: &mut App| { select_span(a); a.start_add_comment(); })),
        ("ThreadDoctor", Focus::Editor, Box::new(|a: &mut App| a.open_thread_doctor())),
        ("DoctorPanel", Focus::Editor, Box::new(|a: &mut App| a.open_doctor_panel())),
        ("Journal", Focus::Editor, Box::new(|a: &mut App| a.open_journal())),
        ("TtsVoicePicker", Focus::Editor, Box::new(|a: &mut App| a.open_tts_voice_picker())),
        ("ReaderPace", Focus::Editor, Box::new(|a: &mut App| { let _ = a.save_focused(); a.open_reader_pace(); })),
        ("SnippetPicker", Focus::Editor, Box::new(|a: &mut App| a.open_snippet_picker(super::super::snippets::SnippetPickerKind::Char, String::new()))),
        ("Outline", Focus::Editor, Box::new(|a: &mut App| a.open_outline())),
        ("DialogueFingerprint", Focus::Editor, Box::new(|a: &mut App| a.open_dialogue_view())),
        ("CharacterArc", Focus::Editor, Box::new(|a: &mut App| a.open_character_arc_view())),
        ("GraphNeighbourhood", Focus::Editor, Box::new(|a: &mut App| a.open_graph_neighbourhood_view())),
        ("GraphHub", Focus::Editor, Box::new(|a: &mut App| a.open_graph_hub())),
        ("GraphEdgeInbox", Focus::Editor, Box::new(|a: &mut App| a.open_graph_inbox_view())),
        // Direct constructions for modals whose real openers need data the
        // fixture cannot provide (deleted paragraphs, sidecar comments, a
        // second voice sample, an invented language). They exercise the
        // renderers' scroll windows and the `TextInput` filters.
        ("KillRingPicker*direct", Focus::Editor, Box::new(|a: &mut App| {
            a.kill_ring.clear();
            for i in 0..12 {
                a.kill_ring.push_back(super::super::state::DeletedParagraphStash {
                    parent_id: None,
                    anchor_id: None,
                    title: format!("Stash {i}"),
                    slug: format!("stash-{i}"),
                    word_count: 3,
                    content: format!("= Stash {i}\n\nbody {i}\n").into_bytes(),
                    tags: vec![],
                    linked_paragraphs: vec![],
                    status: None,
                    target_words: None,
                    content_type: None,
                    event: None,
                });
            }
            a.modal = Modal::KillRingPicker { cursor: 0 };
        })),
        ("CommentsPanel*direct", Focus::Editor, Box::new(move |a: &mut App| {
            let entries: Vec<super::super::modal::CommentsPanelEntry> = (0..30)
                .map(|i| super::super::modal::CommentsPanelEntry {
                    paragraph_id: pid,
                    paragraph_breadcrumb: format!("probe-book/chapter-one/para-{i}"),
                    typ_abs_path: std::path::PathBuf::from("/nonexistent/probe/para.typ"),
                    comment_index: i,
                    author: "probe".into(),
                    created_at: chrono::Utc::now(),
                    resolved: i % 3 == 0,
                    text: format!("comment number {i} about the harbor"),
                    char_start: 0,
                    char_end: 5,
                    paragraph_position: i + 1,
                    paragraph_total_comments: 30,
                })
                .collect();
            let visible = (0..entries.len()).collect();
            a.modal = Modal::CommentsPanel {
                entries,
                cursor: 0,
                filter: super::super::input::TextInput::new(),
                filter_active: false,
                hide_resolved: false,
                visible,
            };
        })),
        ("StyleTransferPicker*direct", Focus::Editor, Box::new(move |a: &mut App| {
            let entries: Vec<(Uuid, String)> =
                (0..40).map(|i| (Uuid::new_v4(), format!("Voice sample {i}"))).collect();
            let visible = (0..entries.len()).collect();
            a.modal = Modal::StyleTransferPicker {
                entries,
                cursor: 0,
                filter: super::super::input::TextInput::new(),
                filter_active: false,
                visible,
                target_paragraph_id: pid,
            };
        })),
        ("LangInsert*direct", Focus::Editor, Box::new(|a: &mut App| {
            let entries: Vec<(String, String)> =
                (0..40).map(|i| (format!("word{i}"), format!("gloss {i}"))).collect();
            a.modal = Modal::LangInsert {
                language: "Probe".into(),
                entries,
                query: super::super::input::TextInput::new(),
                cursor: 0,
                trigger: (0, 0, 0),
            };
        })),
        ("Deleting", Focus::Tree, Box::new(|a: &mut App| a.open_delete_modal())),
    ];

    let mut unopened: Vec<String> = Vec::new();
    for (label, focus, open) in openers.iter_mut() {
        // Keep a paragraph open for every modal (some openers close it).
        if app.opened.is_none() {
            app.tree_cursor = row;
            let _ = app.open_selected();
        }
        if !exercise(&mut app, label, *focus, open.as_mut(), &mut out) {
            unopened.push(label.to_string());
        }
    }
    out.push(format!("UNOPENED ({}): {:?}", unopened.len(), unopened));
    // Dirty-buffer Esc semantics for the multi-line boxes and overlays.
    let dirty_check = |app: &mut App, label: &str, open: &mut dyn FnMut(&mut App), out: &mut Vec<String>| {
        app.modal = Modal::None;
        app.focus = Focus::Editor;
        open(app);
        if matches!(app.modal, Modal::None) { out.push(format!("[ESC-DIRTY {label}] did not open: {}", app.status)); return; }
        for c in "xyz".chars() { let _ = app.handle_key(k(KeyCode::Char(c))); }
        let _ = app.handle_key(k(KeyCode::Esc));
        out.push(format!("[ESC-DIRTY {label}] after typing + one Esc: modal {} · status: {}", if matches!(app.modal, Modal::None) { "CLOSED" } else { "still open" }, app.status));
        app.modal = Modal::None;
    };
    dirty_check(&mut app, "HjsonEditor", &mut |a: &mut App| a.open_hjson_editor(), &mut out);
    dirty_check(&mut app, "CommentEditor", &mut |a: &mut App| { select_span(a); a.start_add_comment(); }, &mut out);
    dirty_check(&mut app, "FootnoteEditor", &mut |a: &mut App| a.start_insert_footnote(), &mut out);
    // Filters on the shared chord set: Ctrl+letter must never land as text and
    // Esc must leave the filter before it closes the modal.
    app.modal = Modal::None;
    app.focus = Focus::Editor;
    app.open_tts_voice_picker();
    if !matches!(app.modal, Modal::None) {
        for key in [k(KeyCode::Char('/')), k(KeyCode::Char('d')), k(KeyCode::Char('e')), ctrl('k'), ctrl('a'), k(KeyCode::Char('x'))] {
            let _ = app.handle_key(key);
        }
        let filter = match &app.modal {
            Modal::TtsVoicePicker { state } => Some((state.filter.as_str().to_string(), state.filter_focused)),
            _ => None,
        };
        out.push(format!("[FILTER TtsVoicePicker] after `/ d e Ctrl+K Ctrl+A x`: {filter:?} (want (\"xde\", true))"));
        assert_eq!(filter, Some(("xde".to_string(), true)), "voice picker filter chords");
        let _ = app.handle_key(k(KeyCode::Esc));
        let still_open = !matches!(app.modal, Modal::None);
        let unfocused = matches!(&app.modal, Modal::TtsVoicePicker { state } if !state.filter_focused);
        out.push(format!("[FILTER TtsVoicePicker] after Esc: open={still_open} unfocused={unfocused}"));
        assert!(still_open && unfocused, "first Esc should only leave the voice filter");
        let _ = app.handle_key(k(KeyCode::Esc));
        assert!(matches!(app.modal, Modal::None), "second Esc should close the voice picker");
    } else {
        out.push(format!("[FILTER TtsVoicePicker] did not open: {}", app.status));
    }
    // Shell pane: `?`/help overlay then Esc — does Esc close the overlay or the pane?
    app.modal = Modal::None; app.open_shell_pane(true);
    if let Modal::ShellPane { show_help, .. } = &mut app.modal { *show_help = true; }
    let _ = app.handle_key(k(KeyCode::Esc));
    out.push(format!("[ESC-OVERLAY ShellPane help] after Esc: modal {}", if matches!(app.modal, Modal::None) { "CLOSED" } else { "still open" }));
    app.modal = Modal::None; app.open_hjson_editor();
    if let Modal::HjsonEditor { restart_required, .. } = &mut app.modal { *restart_required = true; }
    let _ = app.handle_key(k(KeyCode::Esc));
    out.push(format!("[ESC-OVERLAY Hjson restart] after Esc: modal {}", if matches!(app.modal, Modal::None) { "CLOSED" } else { "still open" }));
    app.modal = Modal::None;
    let report = out.join("\n");
    let report_path = std::env::temp_dir().join("inkhaven-modal-probe.txt");
    std::fs::write(&report_path, &report).unwrap();
    println!("{report}");
    println!("report: {}", report_path.display());
    drop(tmp);

    // Contracts.
    let panics: Vec<&String> = out.iter().filter(|l| l.contains("PANIC") || l.contains("Err ")).collect();
    assert!(panics.is_empty(), "{} panicking / erroring probe steps:\n{}", panics.len(), panics.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    for l in out.iter().filter(|l| l.starts_with("[ESC-DIRTY")) {
        assert!(l.contains("still open"), "one Esc discarded typed text: {l}");
    }
    for l in out.iter().filter(|l| l.starts_with("[ESC-OVERLAY")) {
        assert!(l.contains("still open"), "Esc closed the pane instead of its overlay: {l}");
    }
    assert!(!out.iter().any(|l| l.contains("DID NOT CLOSE")), "a modal did not close on Esc");
}

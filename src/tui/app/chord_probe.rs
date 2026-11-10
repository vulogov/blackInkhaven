//! Chord-layer probe (3.19) — walks the default binding table
//! (`KeyBindings::defaults()`: top-level keys and the `Ctrl+B` / `Ctrl+Z` /
//! `Ctrl+V` sub-chords) and, for every entry and every pane its scope
//! covers:
//!
//! * **shadowing** (static) — the entry must be what the resolver returns
//!   for its own chord in that pane; an earlier broader entry that wins
//!   makes the later one unreachable;
//! * **liveness** (dynamic) — the prefix and the chord are fed through
//!   `handle_key` on the headless `App`; nothing may panic, and the chord
//!   must change something observable (a modal, the status line, focus,
//!   a pane, the buffer …). A chord that changes nothing is reported as
//!   SILENT — either dead or lacking a status hint;
//! * **documentation** (static) — the chord's display form must appear in
//!   `Documentation/KEYBINDING.md`, and every `Ctrl+B …` / `Ctrl+V …` /
//!   `Ctrl+Z …` row of that file must resolve to a binding.
//!
//! Opens a real `Store`, so it is `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture chord_probe
//! ```
use super::modal_probe::{k, open_para, probe_app, render, send};
use super::pane_probe::run_probe;
use super::super::keybind::{Action, KeyBindings, Layer};
use super::super::keymap::KeyChord;
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn ev(c: &KeyChord) -> KeyEvent {
    KeyEvent::new(c.code, c.modifiers)
}

fn action_name(a: &Action) -> String {
    serde_json::to_string(a).unwrap_or_default().trim_matches('"').to_string()
}

/// Everything a chord could visibly change, as one comparable string. The
/// prefix-pending flags are left out on purpose (the chord always clears
/// them), as is the status line's prefix hint (snapshotted after the prefix).
fn fingerprint(a: &App) -> String {
    format!(
        "{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{:?}|{}|{}|{}|{}|{}|{:?}",
        a.status,
        a.focus,
        std::mem::discriminant(&a.modal),
        a.right_pane,
        a.ai_fullscreen,
        a.right_fullscreen,
        a.opened.as_ref().map(|d| (d.id, d.dirty, d.textarea.lines().len(), d.textarea.cursor())),
        a.rows.len(),
        a.hierarchy.iter().count(),
        a.inference.is_some(),
        a.bg_job.is_some(),
        a.tree_cursor,
        a.secondary.is_some(),
        a.split_view,
        a.similar_mode,
        std::mem::discriminant(&a.ai_mode),
        a.link_pick_for.is_some(),
        a.ai_input.as_str(),
        a.chat_history.len(),
        a.thoughts.len(),
        a.show_prompt_picker,
        a.show_results_overlay,
        a.tree_marked.len(),
        a.collapsed_nodes.len(),
    )
}

/// Chords the probe never fires: they reach outside the process.
fn denied(name: &str) -> bool {
    matches!(
        name,
        "editor.tts_read_paragraph" | "global.toggle_mouse_capture"
    )
}

fn reset(app: &mut App, pid: Uuid, fixture: &[String], focus: Focus) {
    app.modal = Modal::None;
    app.meta_pending = false;
    app.view_pending = false;
    app.bund_pending = false;
    app.link_pick_for = None;
    app.ai_fullscreen = false;
    app.right_fullscreen = false;
    app.right_pane = RightPane::Ai;
    app.inference = None;
    if app.bg_job.is_some() {
        let _ = app.cancel_bg_job();
    }
    app.secondary = None;
    app.split_view = false;
    app.similar_mode = false;
    app.secondary_focused = false;
    app.show_prompt_picker = false;
    app.show_results_overlay = false;
    app.ai_input.clear();
    app.search_input.clear();
    app.tree_marked.clear();
    app.tree_filter.clear();
    app.tree_filter_editing = false;
    app.status.clear();
    if app.opened.as_ref().map(|d| d.id) != Some(pid) {
        open_para(app, pid);
    }
    if app.opened.as_ref().is_some_and(|d| d.textarea.lines() != fixture) {
        if let Some(d) = app.opened.as_mut() {
            d.textarea = tui_textarea::TextArea::new(fixture.to_vec());
            d.dirty = true;
        }
        let _ = app.save_focused();
    }
    if let Some(r) = app.rows.iter().position(|(id, _)| *id == pid) {
        app.tree_cursor = r;
    }
    app.focus = focus;
}

fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::TopLevel => "",
        Layer::MetaSub => "Ctrl+B",
        Layer::BundSub => "Ctrl+Z",
        Layer::ViewSub => "Ctrl+V",
    }
}

/// The ways KEYBINDING.md spells the key after the prefix.
fn key_forms(chord: &KeyChord) -> Vec<String> {
    match chord.code {
        KeyCode::Char(' ') => vec!["Space".into()],
        KeyCode::Char(c) if chord.modifiers == KeyModifiers::NONE && c.is_alphabetic() => {
            vec![c.to_string(), c.to_uppercase().to_string()]
        }
        KeyCode::Char(c) if chord.modifiers == KeyModifiers::NONE => vec![c.to_string()],
        KeyCode::Char(c) if chord.modifiers == KeyModifiers::SHIFT => {
            vec![format!("Shift+{}", c.to_uppercase()), format!("Shift+{c}")]
        }
        KeyCode::Up => vec!["↑".into(), "Up".into()],
        KeyCode::Down => vec!["↓".into(), "Down".into()],
        KeyCode::Left => vec!["←".into(), "Left".into()],
        KeyCode::Right => vec!["→".into(), "Right".into()],
        _ => vec![chord.to_display_string()],
    }
}

/// The doc section whose key table lists this layer's sub-keys bare
/// (`| \`N\` / \`n\` |`): §1.1 for Ctrl+B, §1.2 for Ctrl+V.
fn layer_section<'a>(doc: &'a str, layer: Layer) -> Option<&'a str> {
    let (start, end) = match layer {
        Layer::MetaSub => ("### 1.1 ", "### 1.2 "),
        Layer::ViewSub => ("### 1.2 ", "## 2. "),
        _ => return None,
    };
    let s = doc.find(start)?;
    let e = doc[s..].find(end).map(|i| s + i).unwrap_or(doc.len());
    Some(&doc[s..e])
}

/// A chord counts as documented when its combined form (`Ctrl+B Shift+V`,
/// `Ctrl+Z v`, `F12`) appears anywhere, or its bare key heads a row of the
/// layer's own table.
fn documented(doc: &str, layer: Layer, chord: &KeyChord) -> bool {
    let prefix = layer_label(layer);
    let forms = key_forms(chord);
    let combined = forms.iter().any(|key| {
        let form = if prefix.is_empty() { key.clone() } else { format!("{prefix} {key}") };
        doc.contains(&format!("`{form}`"))
            || (!prefix.is_empty() && doc.contains(&format!("`{prefix}` then `{key}`")))
    });
    if combined {
        return true;
    }
    let Some(section) = layer_section(doc, layer) else { return false };
    // The status filters share one `1` … `7` row.
    if matches!(chord.code, KeyCode::Char('1'..='7')) && section.contains("| `1` … `7`") {
        return true;
    }
    section
        .lines()
        .any(|line| forms.iter().any(|key| line.starts_with(&format!("| `{key}`"))))
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn chord_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("chord", |out| {
        let (tmp, mut app, pid) = probe_app();
        std::env::set_current_dir(tmp.path()).unwrap();
        open_para(&mut app, pid);
        let fixture: Vec<String> = app.opened.as_ref().unwrap().textarea.lines().to_vec();
        let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Documentation/KEYBINDING.md")).unwrap();
        let kb = KeyBindings::defaults();
        let layers: Vec<(Layer, &Vec<_>, Option<KeyChord>)> = vec![
            (Layer::TopLevel, &kb.top_level, None),
            (Layer::MetaSub, &kb.meta_sub, Some(kb.meta_prefix)),
            (Layer::BundSub, &kb.bund_sub, kb.bund_prefix),
            (Layer::ViewSub, &kb.view_sub, kb.view_prefix),
        ];
        let focuses = [Focus::Tree, Focus::Editor, Focus::Ai, Focus::AiPrompt, Focus::SearchBar];
        let mut fired = 0usize;
        let mut silent: Vec<String> = Vec::new();
        let mut shadowed: Vec<String> = Vec::new();
        let mut undocumented: Vec<String> = Vec::new();
        for (layer, table, prefix) in &layers {
            for e in table.iter() {
                let name = action_name(&e.action);
                if !documented(&doc, *layer, &e.chord) {
                    undocumented.push(format!("{} {} → {name} ({:?})", layer_label(*layer), e.chord.to_display_string(), e.scope));
                }
                assert!(!e.action.label().is_empty() && !e.action.description().is_empty(), "{name} has no label / description");
                for focus in focuses {
                    if !e.scope.matches(focus) {
                        continue;
                    }
                    let key = ev(&e.chord);
                    let resolved = match layer {
                        Layer::TopLevel => kb.resolve_top_level(&key, focus),
                        Layer::MetaSub => kb.resolve_meta_sub(&key, focus),
                        Layer::BundSub => kb.resolve_bund_sub(&key, focus),
                        Layer::ViewSub => kb.resolve_view_sub(&key, focus),
                    };
                    if resolved.as_ref() != Some(&e.action) {
                        shadowed.push(format!(
                            "{} {} in {focus:?}: {name} is shadowed by {}",
                            layer_label(*layer),
                            e.chord.to_display_string(),
                            resolved.as_ref().map(action_name).unwrap_or_else(|| "nothing".into())
                        ));
                        continue;
                    }
                    if denied(&name) {
                        continue;
                    }
                    reset(&mut app, pid, &fixture, focus);
                    if let Some(p) = prefix {
                        match send(&mut app, ev(p)) {
                            Ok(_) => {}
                            Err(err) => out.push(format!("[CHORD {name}] prefix in {focus:?}: {err}")),
                        }
                    }
                    let before = fingerprint(&app);
                    fired += 1;
                    match send(&mut app, key) {
                        Ok(true) => out.push(format!("[CHORD {name}] in {focus:?} requested QUIT")),
                        Ok(false) => {}
                        Err(err) => out.push(format!("[CHORD {name}] in {focus:?}: {err}")),
                    }
                    for (w, h) in [(80, 24), (20, 6)] {
                        if let Err(err) = render(&mut app, w, h) {
                            out.push(format!("[CHORD {name}] render {w}x{h} after chord in {focus:?}: {err}"));
                        }
                    }
                    let after = fingerprint(&app);
                    if before == after {
                        silent.push(format!("{} {} in {focus:?} → {name}", layer_label(*layer), e.chord.to_display_string()));
                    }
                    // A chord that opened a modal must close on Esc.
                    let mut escs = 0;
                    while !matches!(app.modal, Modal::None) && escs < 4 {
                        escs += 1;
                        let _ = send(&mut app, k(KeyCode::Esc));
                    }
                    if !matches!(app.modal, Modal::None) {
                        out.push(format!("[CHORD {name}] modal did not close on Esc×4 in {focus:?}"));
                        app.modal = Modal::None;
                    }
                }
            }
        }
        // Documented chords that resolve to nothing.
        let mut doc_only: Vec<String> = Vec::new();
        for line in doc.lines() {
            for (layer, label) in [(Layer::MetaSub, "Ctrl+B"), (Layer::BundSub, "Ctrl+Z"), (Layer::ViewSub, "Ctrl+V")] {
                let head = format!("| `{label} ");
                let Some(rest) = line.strip_prefix(&head) else { continue };
                let Some(sub) = rest.split('`').next() else { continue };
                let Ok(chord) = KeyChord::parse(sub.trim()) else { continue };
                let key = ev(&chord);
                let any = focuses.iter().any(|f| {
                    let r = match layer {
                        Layer::MetaSub => kb.resolve_meta_sub(&key, *f),
                        Layer::BundSub => kb.resolve_bund_sub(&key, *f),
                        _ => kb.resolve_view_sub(&key, *f),
                    };
                    matches!(r, Some(a) if a != Action::None)
                });
                if !any {
                    doc_only.push(format!("{label} {}", sub.trim()));
                }
            }
        }
        let entries: usize = layers.iter().map(|(_, t, _)| t.len()).sum();
        out.push(format!("[CHORDS] {entries} bindings · {fired} chord firings · {} shadowed · {} silent · {} undocumented · {} documented-but-unbound", shadowed.len(), silent.len(), undocumented.len(), doc_only.len()));
        for s in &shadowed {
            out.push(format!("[SHADOWED] {s}"));
        }
        for s in &silent {
            out.push(format!("[SILENT] {s}"));
        }
        for s in &undocumented {
            out.push(format!("[UNDOCUMENTED] {s}"));
        }
        for s in &doc_only {
            out.push(format!("[DOC-ONLY] {s}"));
        }
        drop(tmp);
        let panics: Vec<&String> = out.iter().filter(|l| l.contains("PANIC") || l.contains("Err ") || l.contains("QUIT") || l.contains("did not close")).collect();
        assert!(panics.is_empty(), "{} failing chord steps:\n{}", panics.len(), panics.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
        assert!(shadowed.is_empty(), "unreachable bindings:\n{}", shadowed.join("\n"));
    });
}

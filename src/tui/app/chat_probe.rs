//! AI chat probe (3.21) — drives the chat pipeline headlessly against a stub
//! provider: the real `start_inference` runs (scope context, prompt
//! resolution, history, system prompt, spawn), then the probe takes over the
//! stream channel and feeds tokens, a completion or an error, and checks what
//! the AI pane, the apply keys, cancel / edit / retry, the scopes, the `/name`
//! prompts, the history ring, persistence, the conversations modal and the
//! Bund words promise. Nothing reaches a network: the stub provider's own
//! task fails and its receiver is replaced before anything is read.
//!
//! Needs the cached embedding model (Book scope retrieves), so it is
//! `#[ignore]`d:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture chat_probe
//! ```
use super::modal_probe::{ctrl, k, open_para, probe_app, render, send, SIZES};
use super::pane_probe::run_probe;
use super::*;
use crate::config::LlmProvider;
use crossterm::event::KeyCode;
use tokio::sync::mpsc::UnboundedSender;

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        send(app, k(KeyCode::Char(c))).unwrap();
    }
}

/// Type `text` into the AI prompt and press Enter; hand back the stream the
/// probe now controls. `None` when no inference started (a refused send).
fn send_prompt(app: &mut App, text: &str) -> Option<UnboundedSender<StreamMsg>> {
    app.modal = Modal::None;
    app.focus = Focus::AiPrompt;
    app.ai_input.clear();
    type_str(app, text);
    send(app, k(KeyCode::Enter)).unwrap();
    // Only a stream this Enter started counts; a finished reply left over
    // from the previous send does not.
    let inf = app.inference.as_mut().filter(|i| matches!(i.status, InferenceStatus::Streaming))?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    inf.rx = rx;
    Some(tx)
}

fn finish(app: &mut App, tx: &UnboundedSender<StreamMsg>, reply: &str) {
    for piece in reply.split_inclusive(' ') {
        tx.send(StreamMsg::Token(piece.to_string())).unwrap();
    }
    tx.send(StreamMsg::Done(None)).unwrap();
    app.pump_inference();
}

fn fail(app: &mut App, tx: &UnboundedSender<StreamMsg>, msg: &str) {
    tx.send(StreamMsg::Error(msg.to_string())).unwrap();
    app.pump_inference();
}

fn buffer(app: &App) -> Vec<String> {
    app.opened.as_ref().map(|d| d.textarea.lines().to_vec()).unwrap_or_default()
}

fn ai_key(app: &mut App, code: KeyCode) {
    app.modal = Modal::None;
    app.focus = Focus::Ai;
    send(app, k(code)).unwrap();
}

fn render_all(app: &mut App, label: &str, problems: &mut Vec<String>) {
    for (w, h) in SIZES {
        if let Err(e) = render(app, w, h) {
            problems.push(format!("[RENDER {label}] {w}x{h}: {e}"));
        }
    }
}

fn frame_text(app: &mut App, w: u16, h: u16) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| app.draw(f)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut rows = Vec::new();
    for y in 0..h {
        let mut s = String::new();
        for x in 0..w {
            s.push_str(buf[(x, y)].symbol());
        }
        rows.push(s);
    }
    rows.join("\n")
}

#[test]
#[ignore = "probe: opens a real Store with the embedding model; run with --ignored --nocapture"]
fn chat_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("chat", |out| {
        let (tmp, mut app, pid) = probe_app();
        std::env::set_current_dir(tmp.path()).unwrap();
        let mut problems: Vec<String> = Vec::new();
        // A problem is reported where it happens (the report survives a later
        // panic) and counted at the end.
        macro_rules! note {
            ($e:expr) => {{
                let m: String = $e;
                out.push(format!("  !! {m}"));
                problems.push(m);
            }};
        }
        // The fixture project ships no prompts.hjson; seed one library prompt
        // for the `/name` checks.
        app.prompts.prompts.push(crate::ai::prompts::Prompt {
            name: "probe-tighten".into(),
            description: "Tighten the prose".into(),
            template: "Tighten this without losing meaning:\n\n{{selection}}".into(),
            language: None,
        });

        // A keyless stub provider so `resolve_provider` succeeds; its task
        // fails fast (no such adapter) and is never read.
        app.cfg.llm.providers.insert(
            "probe".into(),
            LlmProvider { model: "probe-model".into(), api_key_env: None, base_url: None },
        );
        app.cfg.llm.default = "probe".into();
        app.ai = crate::ai::AiClient::from_config(&app.cfg.llm).unwrap();
        open_para(&mut app, pid);
        app.right_pane = RightPane::Ai;
        let original = buffer(&app);
        out.push(format!("[FIXTURE] provider=probe · paragraph `{}` {} lines", original[0], original.len()));
        render_all(&mut app, "idle", &mut problems);

        // ── SEND: state after Enter, while streaming, after Done ──────────
        let tx = send_prompt(&mut app, "hello there").expect("inference started");
        if app.focus != Focus::AiPrompt || !app.ai_input.is_empty() {
            note!(format!("[SEND] focus {:?}, input `{}`", app.focus, app.ai_input.as_str()));
        }
        if app.ai_prompt_history.last().map(String::as_str) != Some("hello there") {
            note!("[SEND] the prompt did not enter the history ring".into());
        }
        if app.pending_chat_user_msg.as_deref() != Some("hello there") {
            note!(format!("[SEND] pending user message {:?}", app.pending_chat_user_msg));
        }
        render_all(&mut app, "waiting", &mut problems);
        let shown = frame_text(&mut app, 80, 24);
        if !shown.contains("waiting") {
            note!("[SEND] no `waiting` cue before the first token".into());
        }
        tx.send(StreamMsg::Token("Hi ".into())).unwrap();
        app.pump_inference();
        if app.inference.as_ref().map(|i| i.response.as_str()) != Some("Hi ") {
            note!("[SEND] the first token did not reach the response".into());
        }
        render_all(&mut app, "streaming", &mut problems);
        // The title truncates in a narrow pane; read the cues on a wide frame.
        let shown = frame_text(&mut app, 200, 60);
        if !shown.contains("streaming") || !shown.contains("Esc cancel") {
            note!("[SEND] the title lacks the streaming / Esc cancel cues".into());
        }
        if shown.contains(" r ") && shown.contains("replace") {
            note!("[SEND] apply hints shown while streaming".into());
        }
        finish(&mut app, &tx, "there, friend.");
        if !app.inference_done_with_text() {
            note!("[SEND] inference not Done with text after the stream closed".into());
        }
        if app.chat_history.len() != 2 {
            note!(format!("[SEND] chat history has {} turns, expected 2", app.chat_history.len()));
        } else {
            let ok = matches!(&app.chat_history[0], ChatTurn::User(u) if u == "hello there")
                && matches!(&app.chat_history[1], ChatTurn::Assistant(a) if a == "Hi there, friend.");
            if !ok {
                note!(format!("[SEND] history pairing wrong: {:?}", app.chat_history));
            }
        }
        if !app.status.contains("responded in") {
            note!(format!("[SEND] status after Done: {}", app.status));
        }
        render_all(&mut app, "done", &mut problems);
        // At 80 columns the pane is 18 wide: the state must still be the first
        // thing after `AI` in the title (3.21 — the llm chip used to take it).
        let narrow = frame_text(&mut app, 80, 24);
        if !narrow.contains("AI · done") {
            note!("[SEND] the 18-column title does not lead with the state".into());
        }
        // (The `· N turn(s)` chip closes the title and is cut at every size in
        // SIZES — the pane is ~50 columns on a 200-column frame — so it is
        // not required here; the history length is asserted above.)
        let shown = frame_text(&mut app, 200, 60).replace('\n', " ");
        for cue in ["done", "replace", "insert", "Hi there, friend."] {
            if !shown.contains(cue) {
                note!(format!("[SEND] done frame lacks `{cue}`"));
            }
        }
        out.push("[SEND] waiting → streaming → done · history paired · hints shown when done".into());

        // ── APPLY: i / t / b / r / g against the open paragraph ──────────
        {
            ai_key(&mut app, KeyCode::Char('b'));
            let b = buffer(&app);
            if b.iter().rev().find(|l| !l.is_empty()).map(String::as_str) != Some("Hi there, friend.") || app.focus != Focus::Editor {
                note!(format!("[APPLY b] last line {:?}, focus {:?}", b.last(), app.focus));
            }
            if b[0] != original[0] {
                note!("[APPLY b] the heading moved".into());
            }
            ai_key(&mut app, KeyCode::Char('t'));
            let b = buffer(&app);
            if b[0] != original[0] {
                note!(format!("[APPLY t] the reply landed above the `= Title` heading: first line {:?}", b[0]));
            }
            if !b.iter().any(|l| l == "Hi there, friend.") {
                note!("[APPLY t] the reply is not in the buffer".into());
            }
            // Insert at the cursor: put the cursor on the last line first.
            if let Some(d) = app.opened.as_mut() {
                d.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
                d.textarea.move_cursor(tui_textarea::CursorMove::End);
            }
            ai_key(&mut app, KeyCode::Char('i'));
            let b = buffer(&app);
            if !b.iter().rev().find(|l| !l.is_empty()).is_some_and(|l| l.ends_with("Hi there, friend.")) {
                note!(format!("[APPLY i] last line {:?}", b.last()));
            }
            if !app.opened.as_ref().is_some_and(|d| d.dirty) {
                note!("[APPLY] the buffer is not dirty after an apply".into());
            }
            // Replace goes through the diff review (default on); reject keeps
            // the buffer, accept replaces it.
            let before = buffer(&app);
            ai_key(&mut app, KeyCode::Char('r'));
            if !matches!(app.modal, Modal::AiDiffReview { .. }) {
                note!(format!("[APPLY r] no diff review ({})", app.status));
            }
            render_all(&mut app, "diff review", &mut problems);
            send(&mut app, k(KeyCode::Char('r'))).unwrap();
            if !matches!(app.modal, Modal::None) || buffer(&app) != before {
                note!("[APPLY r] reject changed the buffer or left the modal".into());
            }
            ai_key(&mut app, KeyCode::Char('r'));
            send(&mut app, k(KeyCode::Char('a'))).unwrap();
            let b = buffer(&app);
            if b != vec!["Hi there, friend.".to_string()] {
                note!(format!("[APPLY r] accept left {:?} (expected exactly the reply, no trailing blank lines)", b));
            }
            out.push("[APPLY] b appends · t keeps the heading first · i inserts at the cursor · r reviews, reject/accept".into());
        }
        // Grammar apply lifts only the corrected block.
        {
            if let Some(d) = app.opened.as_mut() {
                d.textarea = tui_textarea::TextArea::new(original.clone());
            }
            let tx = send_prompt(&mut app, "check grammar").unwrap();
            finish(&mut app, &tx, "Here you go.\n<<<CORRECTED>>>\n= Para One\n\nThe corrected body.\n<<<END>>>\nAnything else?");
            ai_key(&mut app, KeyCode::Char('g'));
            if matches!(app.modal, Modal::AiDiffReview { .. }) {
                send(&mut app, k(KeyCode::Char('a'))).unwrap();
            }
            let b = buffer(&app);
            if b != vec!["= Para One".to_string(), String::new(), "The corrected body.".to_string()] {
                note!(format!("[APPLY g] buffer {:?}", b));
            }
            if app.opened.as_ref().and_then(|d| d.correction_baseline.as_ref()).is_none() {
                note!("[APPLY g] no correction baseline for the highlight".into());
            }
            out.push("[APPLY g] corrected block lifted, baseline kept".into());
        }

        // ── ERROR: a failed stream leaves the history alone ───────────────
        {
            let turns = app.chat_history.len();
            let tx = send_prompt(&mut app, "will fail").unwrap();
            fail(&mut app, &tx, "boom: provider down");
            if !app.status.contains("boom") {
                note!(format!("[ERROR] status `{}`", app.status));
            }
            if app.chat_history.len() != turns {
                note!("[ERROR] a failed turn entered the history".into());
            }
            render_all(&mut app, "error", &mut problems);
            let shown = frame_text(&mut app, 120, 40);
            if !shown.contains("error") || !shown.contains("boom") {
                note!("[ERROR] the pane does not show the error".into());
            }
            let before = buffer(&app);
            ai_key(&mut app, KeyCode::Char('r'));
            if buffer(&app) != before || !matches!(app.modal, Modal::None) {
                note!("[ERROR] an apply key acted on an error".into());
            }
            // Esc on an error clears it and goes back to the prompt.
            ai_key(&mut app, KeyCode::Esc);
            if app.inference.is_some() || app.focus != Focus::AiPrompt {
                note!(format!("[ERROR] Esc: inference {} focus {:?}", app.inference.is_some(), app.focus));
            }
            out.push("[ERROR] shown in the pane · history untouched · apply keys inert · Esc clears".into());
        }

        // ── ESC: streaming cancels, a finished reply bounces to the prompt ─
        {
            let tx = send_prompt(&mut app, "cancel me").unwrap();
            tx.send(StreamMsg::Token("partial".into())).unwrap();
            app.pump_inference();
            let turns = app.chat_history.len();
            app.pending_rewrite_diff = Some("Sentence rhythm rewrite".into());
            app.fact_check_pending = Some((pid, "Para One".into()));
            app.pending_continuation_draft = true;
            app.pending_style_transfer = true;
            ai_key(&mut app, KeyCode::Esc);
            if app.inference.is_some() || app.chat_history.len() != turns || !app.status.contains("cancelled") {
                note!(format!("[CANCEL] inference {} turns {} status `{}`", app.inference.is_some(), app.chat_history.len(), app.status));
            }
            if app.pending_rewrite_diff.is_some() || app.fact_check_pending.is_some() || app.pending_continuation_draft || app.pending_style_transfer {
                note!("[CANCEL] a pending flow flag survived the cancel".into());
                app.pending_rewrite_diff = None;
                app.fact_check_pending = None;
                app.pending_continuation_draft = false;
                app.pending_style_transfer = false;
            }
            // The next ordinary reply must not open a diff review.
            let tx = send_prompt(&mut app, "after cancel").unwrap();
            finish(&mut app, &tx, "plain answer");
            if !matches!(app.modal, Modal::None) {
                note!("[CANCEL] the reply after a cancelled rewrite opened a modal".into());
                app.modal = Modal::None;
            }
            // A finished reply: Esc bounces to the prompt and keeps the reply.
            ai_key(&mut app, KeyCode::Esc);
            if app.focus != Focus::AiPrompt || !app.inference_done_with_text() {
                note!(format!("[ESC done] focus {:?}, reply kept {}", app.focus, app.inference_done_with_text()));
            }
            out.push("[CANCEL/ESC] streaming Esc cancels and disarms pending flows · done Esc bounces".into());
        }

        // ── SCOPES: what the model is sent ────────────────────────────────
        {
            app.ai_mode = AiMode::Paragraph;
            let tx = send_prompt(&mut app, "about this paragraph").unwrap();
            let sent = app.pending_chat_user_msg.clone().unwrap_or_default();
            if !sent.starts_with("── Paragraph: Para One ──") || !sent.ends_with("\n\nabout this paragraph") {
                note!(format!("[SCOPE Paragraph] sent `{}`", sent.chars().take(60).collect::<String>()));
            }
            if app.ai_mode != AiMode::None {
                note!("[SCOPE Paragraph] did not reset to None after the send".into());
            }
            finish(&mut app, &tx, "ok");
            app.ai_mode = AiMode::Selection;
            if send_prompt(&mut app, "no selection").is_some() || !app.status.contains("needs a non-empty selection") {
                note!(format!("[SCOPE Selection] without a selection: `{}`", app.status));
            }
            app.ai_mode = AiMode::Chapter;
            let tx = send_prompt(&mut app, "about the chapter").unwrap();
            let sent = app.pending_chat_user_msg.clone().unwrap_or_default();
            if !sent.contains("── Chapter context: Chapter One (") {
                note!(format!("[SCOPE Chapter] sent `{}`", sent.chars().take(60).collect::<String>()));
            }
            finish(&mut app, &tx, "ok");
            // Book scope: retrieval, a clean question in the transcript, the
            // conversation view, and the scope sticking for the follow-up.
            app.clear_chat_history();
            app.ai_mode = AiMode::Book;
            let tx = send_prompt(&mut app, "where did the harbor freeze").unwrap();
            if app.pending_chat_user_msg.as_deref() != Some("where did the harbor freeze") {
                note!(format!("[SCOPE Book] transcript question {:?}", app.pending_chat_user_msg));
            }
            if app.book_rag_last_retrieval.as_ref().map(|p| p.len()).unwrap_or(0) == 0 {
                note!(format!("[SCOPE Book] no retrieval: `{}`", app.status));
            }
            if app.ai_mode != AiMode::Book {
                note!(format!("[SCOPE Book] the scope reset to {:?} on send; the conversation view and the cached retrieval need it to stay", app.ai_mode));
                app.ai_mode = AiMode::Book;
            }
            finish(&mut app, &tx, "It froze in March [probe-book/chapter-one/para-two].");
            render_all(&mut app, "book", &mut problems);
            let shown = frame_text(&mut app, 100, 30);
            if !shown.contains("❯ User") || !shown.contains("Retrieved passages") {
                note!("[SCOPE Book] the pane does not show the conversation + retrieval section".into());
            }
            ai_key(&mut app, KeyCode::Char('p'));
            // The view pins to the newest turn, so the expanded section's
            // header may sit above the fold; its passage crumbs prove it.
            let shown = frame_text(&mut app, 100, 30);
            if !shown.contains("▼ Retrieved passages") && !shown.contains("[probe-book/") {
                note!("[SCOPE Book] `p` did not expand the retrieved passages".into());
            }
            ai_key(&mut app, KeyCode::Char(']'));
            ai_key(&mut app, KeyCode::Char('['));
            app.ai_mode = AiMode::None;
            app.clear_chat_history();
            out.push("[SCOPES] Paragraph / Selection / Chapter / Book contexts as documented".into());
        }

        // ── /name: library prompts, extra words, unknown names ────────────
        {
            let name = app.prompts.prompts.first().map(|p| p.name.clone()).expect("a system prompt");
            let template = app.prompts.prompts[0].template.clone();
            // Typing `/` opens the picker; Esc closes it and keeps the text, so
            // Enter submits the direct form.
            app.modal = Modal::None;
            app.focus = Focus::AiPrompt;
            app.ai_input.clear();
            type_str(&mut app, &format!("/{name}"));
            let picker_before = app.show_prompt_picker;
            send(&mut app, k(KeyCode::Esc)).unwrap();
            out.push(format!("[SLASH] picker open after typing: {picker_before} · after Esc: {} · focus {:?} · input `{}` · status `{}`", app.show_prompt_picker, app.focus, app.ai_input.as_str(), app.status));
            send(&mut app, k(KeyCode::Enter)).unwrap();
            let tx = match app.inference.as_mut().filter(|i| matches!(i.status, InferenceStatus::Streaming)) {
                Some(inf) => {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    inf.rx = rx;
                    tx
                }
                None => {
                    note!(format!("[SLASH] `/{name}` did not send: `{}`", app.status));
                    send_prompt(&mut app, "fallback").unwrap()
                }
            };
            let sent = app.pending_chat_user_msg.clone().unwrap_or_default();
            if sent.is_empty() || sent.starts_with('/') {
                note!(format!("[SLASH] `/{name}` sent `{}`", sent.chars().take(40).collect::<String>()));
            }
            finish(&mut app, &tx, "ok");
            // With extra words the picker matches nothing; Enter must send
            // the line directly (3.21) without an Esc first.
            match send_prompt(&mut app, &format!("/{name} and keep it short")) {
                Some(tx) => {
                    let sent = app.pending_chat_user_msg.clone().unwrap_or_default();
                    if !sent.ends_with("and keep it short") || sent.starts_with('/') {
                        note!(format!("[SLASH] extra words: sent ends with `{}`", sent.chars().rev().take(30).collect::<String>().chars().rev().collect::<String>()));
                    }
                    finish(&mut app, &tx, "ok");
                }
                None => note!(format!("[SLASH] `/{name} and keep it short` was refused: `{}`", app.status)),
            }
            if send_prompt(&mut app, "/no-such-prompt-zz").is_some() || !app.status.contains("no prompt") {
                note!(format!("[SLASH] unknown name: `{}`", app.status));
            }
            app.focus = Focus::AiPrompt;
            app.ai_input.clear();
            type_str(&mut app, "/");
            if !app.show_prompt_picker {
                note!("[SLASH] typing `/` did not open the picker".into());
            }
            send(&mut app, k(KeyCode::Esc)).unwrap();
            app.ai_input.clear();
            out.push(format!("[SLASH] `/{name}` renders the template · extra words appended · unknown refused · `/` opens the picker (template {} chars)", template.len()));
        }

        // ── HISTORY RING, EDIT LAST, RETRY, CLEAR ─────────────────────────
        {
            app.clear_chat_history();
            let tx = send_prompt(&mut app, "alpha question").unwrap();
            finish(&mut app, &tx, "alpha answer");
            app.focus = Focus::AiPrompt;
            app.ai_input.clear();
            type_str(&mut app, "bet");
            send(&mut app, k(KeyCode::Up)).unwrap();
            let up = app.ai_input.as_str().to_string();
            send(&mut app, k(KeyCode::Down)).unwrap();
            let down = app.ai_input.as_str().to_string();
            if up != "alpha question" || down != "bet" {
                note!(format!("[RING] Up `{up}` Down `{down}`"));
            }
            app.ai_input.clear();
            ai_key(&mut app, KeyCode::Char('e'));
            if app.chat_history.len() != 0 || app.ai_input.as_str() != "alpha question" || app.focus != Focus::AiPrompt {
                note!(format!("[EDIT] turns {} input `{}` focus {:?}", app.chat_history.len(), app.ai_input.as_str(), app.focus));
            }
            send(&mut app, k(KeyCode::Enter)).unwrap();
            let tx = {
                let inf = app.inference.as_mut().expect("resend started");
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                inf.rx = rx;
                tx
            };
            finish(&mut app, &tx, "alpha answer 2");
            ai_key(&mut app, KeyCode::Char('z'));
            if app.chat_history.len() != 0 || app.inference.as_ref().map(|i| matches!(i.status, InferenceStatus::Streaming)) != Some(true) {
                note!("[RETRY] z did not pop the pair and resend".into());
            }
            if let Some(inf) = app.inference.as_mut() {
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                inf.rx = rx;
                finish(&mut app, &tx, "alpha answer 3");
            }
            app.focus = Focus::Ai;
            send(&mut app, ctrl('b')).unwrap();
            send(&mut app, k(KeyCode::Char('c'))).unwrap();
            if !app.chat_history.is_empty() || app.inference.is_some() {
                note!("[CLEAR] Ctrl+B c left history or a reply".into());
            }
            out.push("[RING/EDIT/RETRY/CLEAR] ok".into());
        }

        // ── PERSISTENCE, CONVERSATIONS, FULLSCREEN ────────────────────────
        {
            let tx = send_prompt(&mut app, "persist me").unwrap();
            finish(&mut app, &tx, "persisted reply");
            app.save_chat_history_to_disk().unwrap();
            app.chat_history.clear();
            let n = app.load_chat_history_from_disk().unwrap();
            if n != 2 || app.chat_history.len() != 2 {
                note!(format!("[PERSIST] reloaded {n} turns"));
            }
            app.open_conversations_modal();
            send(&mut app, k(KeyCode::Char('s'))).unwrap();
            let saved = match &app.modal {
                Modal::Conversations { entries, .. } => entries.len(),
                _ => 0,
            };
            if saved != 1 {
                note!(format!("[CONV] {saved} saved conversations after `s` ({})", app.status));
            }
            send(&mut app, k(KeyCode::Char('n'))).unwrap();
            if !app.chat_history.is_empty() || !matches!(app.modal, Modal::None) {
                note!("[CONV] `n` did not start a fresh chat".into());
            }
            app.open_conversations_modal();
            render_all(&mut app, "conversations", &mut problems);
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if app.chat_history.len() != 2 || app.focus != Focus::Ai {
                note!(format!("[CONV] reopen: {} turns, focus {:?}", app.chat_history.len(), app.focus));
            }
            app.toggle_ai_fullscreen();
            render_all(&mut app, "fullscreen", &mut problems);
            let shown = frame_text(&mut app, 80, 24);
            if !shown.contains("Chat history") || !shown.contains("persisted reply") {
                out.push(format!("[FRAME fullscreen 80x24]\n{shown}"));
                note!("[FULLSCREEN] the history pane does not show the turns".into());
            }
            // A cleared chat shows no evidence from the previous conversation.
            if shown.contains("Retrieved passages") {
                note!("[FULLSCREEN] the retrieved passages of an earlier Book conversation are still shown".into());
            }
            app.focus = Focus::Ai;
            send(&mut app, k(KeyCode::PageUp)).unwrap();
            render(&mut app, 80, 24).unwrap();
            if app.chat_history_scroll > app.chat_history_scroll_max.get() {
                note!("[FULLSCREEN] PgUp scrolled past the rendered extent".into());
            }
            app.toggle_ai_fullscreen();
            if !app.chat_history_path().exists() {
                note!("[FULLSCREEN] leaving fullscreen did not persist the chat".into());
            }
            out.push("[PERSIST/CONV/FULLSCREEN] save + reload · s / n / Enter · fullscreen renders and clamps".into());
        }

        // ── SELECTION + SEARCH over the history ───────────────────────────
        {
            app.toggle_chat_selection_mode();
            let sel = app.chat_selection.map(|s| s.turn);
            if sel != Some(app.chat_history.len() - 1) {
                note!(format!("[SELECT] started at {sel:?}"));
            }
            app.chat_selection_step(-1);
            app.chat_selection_step(-5);
            if app.chat_selection.map(|s| s.turn) != Some(0) {
                note!("[SELECT] stepping below zero did not clamp".into());
            }
            app.chat_selection_jump(99);
            if app.chat_selection.map(|s| s.turn) != Some(app.chat_history.len() - 1) {
                note!("[SELECT] jump past the end did not clamp".into());
            }
            let before = buffer(&app).join("\n");
            app.chat_selection_into_editor();
            if !buffer(&app).join("\n").contains("persisted reply") || buffer(&app).join("\n") == before {
                note!("[SELECT] t did not insert the selected turn".into());
            }
            app.chat_selection_copy();
            app.toggle_chat_selection_mode();
            if app.chat_selection.is_some() {
                note!("[SELECT] toggling off left the selection".into());
            }
            let hits = app.chat_search_matches("persisted");
            if hits.is_empty() {
                note!("[SEARCH] no match for a word in the history".into());
            }
            if !app.chat_search_matches("").is_empty() {
                note!("[SEARCH] an empty query matched".into());
            }
            out.push(format!("[SELECT/SEARCH] clamps · insert · {} match(es)", hits.len()));
        }

        // ── BUND: ink.ai.send / poll / history ────────────────────────────
        {
            app.clear_chat_history();
            app.ink_ai_send("from a script").unwrap();
            let (st, _, _) = app.ink_ai_poll();
            if st != "streaming" {
                note!(format!("[BUND] poll after send: {st}"));
            }
            let inf = app.inference.as_mut().unwrap();
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            inf.rx = rx;
            finish(&mut app, &tx, "script reply");
            let (st, resp, _) = app.ink_ai_poll();
            if st != "done" || resp != "script reply" {
                note!(format!("[BUND] poll after Done: {st} `{resp}`"));
            }
            if app.ink_ai_history().len() != 2 {
                note!("[BUND] history does not hold the scripted exchange".into());
            }
            if app.ink_ai_send("   ").is_ok() {
                note!("[BUND] a blank prompt was accepted".into());
            }
            let (st, _, _) = { app.inference = None; app.ink_ai_poll() };
            if st != "none" {
                note!(format!("[BUND] poll with no inference: {st}"));
            }
            out.push("[BUND] send → streaming · Done → done + reply · history · blank refused".into());
        }

        out.push(format!("PROBLEMS ({}):", problems.len()));
        out.extend(problems.iter().cloned());
        assert!(problems.is_empty(), "{} problem(s) — see the report", problems.len());
    });
}

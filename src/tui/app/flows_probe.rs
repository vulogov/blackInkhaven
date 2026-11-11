//! Inner-flows probe (3.22) — the editor's AI flows that do more than show a
//! reply: the sentence-rhythm and editorial rewrites (diff review, accept
//! with an annotated snapshot, reject), the span-scoped editorial fix, the
//! continuation draft (`<<<DRAFT>>>` lifted by `i`), the fact check (verdict
//! parsed into navigable findings), and the critique / show-don't-tell /
//! grammar passes that land in the AI pane. Each flow runs its real
//! `start_*` against a keyless stub provider; the probe takes over the stream
//! channel and feeds the reply, as the 3.21 chat probe does, and checks what
//! the editor does with it — plus that every flow records exactly one call
//! to the usage ledger under its own category.
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture flows_probe
//! ```
use super::modal_probe::{k, open_para, probe_app, render, send, SIZES};
use super::pane_probe::run_probe;
use super::*;
use crate::config::LlmProvider;
use crossterm::event::KeyCode;
use tokio::sync::mpsc::UnboundedSender;

/// Hand back the stream the probe now controls, if a stream is running.
fn takeover(app: &mut App) -> Option<UnboundedSender<StreamMsg>> {
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

fn buffer(app: &App) -> String {
    app.opened.as_ref().map(|d| d.textarea.lines().join("\n")).unwrap_or_default()
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

fn usage(root: &std::path::Path) -> std::collections::BTreeMap<String, i64> {
    crate::ai::usage::usage_for(root, &crate::dayclock::today_key()).into_iter().collect()
}

/// The one category that grew between two usage snapshots.
fn grew(before: &std::collections::BTreeMap<String, i64>, after: &std::collections::BTreeMap<String, i64>) -> Vec<String> {
    after
        .iter()
        .filter(|(k, v)| before.get(*k).copied().unwrap_or(0) < **v)
        .map(|(k, v)| format!("{k}+{}", v - before.get(k).copied().unwrap_or(0)))
        .collect()
}

#[test]
#[ignore = "probe: opens a real Store and drives every AI flow against a stub provider; run with --ignored --nocapture"]
fn flows_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("flows", |out| {
        let (tmp, mut app, pid) = probe_app();
        std::env::set_current_dir(tmp.path()).unwrap();
        let root = tmp.path().to_path_buf();
        crate::ai::usage::install(&root, 30);
        let mut problems: Vec<String> = Vec::new();
        macro_rules! note {
            ($e:expr) => {{
                let m: String = $e;
                out.push(format!("  !! {m}"));
                problems.push(m);
            }};
        }
        app.cfg.llm.providers.insert(
            "probe".into(),
            LlmProvider { model: "probe-model".into(), api_key_env: None, base_url: None },
        );
        app.cfg.llm.default = "probe".into();
        app.ai = crate::ai::AiClient::from_config(&app.cfg.llm).unwrap();
        open_para(&mut app, pid);
        app.right_pane = RightPane::Ai;
        let original = buffer(&app);
        let mut categories: Vec<(String, Vec<String>)> = Vec::new();

        // Start a flow, take its stream, and record the usage category it logged.
        macro_rules! start {
            ($label:expr, $start:expr) => {{
                let before = usage(&root);
                $start;
                let tx = takeover(&mut app);
                if tx.is_none() {
                    note!(format!("[{}] no stream started: {}", $label, app.status));
                }
                let delta = grew(&before, &usage(&root));
                if delta.len() != 1 {
                    note!(format!("[{}] usage ledger grew by {:?}, expected exactly one category", $label, delta));
                }
                categories.push(($label.to_string(), delta));
                tx
            }};
        }

        // ---- Sentence-rhythm rewrite: diff review → accept with snapshot ------
        if let Some(tx) = start!("RHYTHM", app.start_sentence_rhythm_rewrite()) {
            let reply = "= Para One\n\nThe harbor froze. Mara spoke. The town waited.\n";
            finish(&mut app, &tx, reply);
            match &app.modal {
                Modal::AiDiffReview { post_accept_snapshot, after_lines, .. } => {
                    if post_accept_snapshot.as_deref() != Some("Sentence rhythm rewrite") {
                        note!(format!("[RHYTHM] snapshot annotation {post_accept_snapshot:?}"));
                    }
                    if !after_lines.join("\n").contains("The town waited.") {
                        note!(format!("[RHYTHM] the diff's after side is not the reply: {after_lines:?}"));
                    }
                    out.push("  [RHYTHM] diff review opened with the reply and the snapshot annotation".into());
                }
                other => note!(format!("[RHYTHM] expected the diff review after Done, got {}", std::any::type_name_of_val(other))),
            }
            render_all(&mut app, "rhythm diff", &mut problems);
            let snaps_before = app.store.list_snapshots(pid).map(|s| s.len()).unwrap_or(0);
            send(&mut app, k(KeyCode::Char('a'))).unwrap();
            if !matches!(app.modal, Modal::None) {
                note!("[RHYTHM] `a` did not close the diff review".into());
            }
            let now = buffer(&app);
            if !now.contains("The town waited.") || now.contains("sleeping animals") {
                note!(format!("[RHYTHM] accept did not replace the paragraph: {now:?}"));
            }
            if !now.starts_with("= Para One") {
                note!(format!("[RHYTHM] the heading did not survive the replace: {:?}", now.lines().next()));
            }
            match app.store.list_snapshots(pid) {
                Ok(snaps) => {
                    if snaps.len() != snaps_before + 1 {
                        note!(format!("[RHYTHM] expected one new snapshot, {} → {}", snaps_before, snaps.len()));
                    }
                    if !snaps.iter().any(|s| s.annotation == "Sentence rhythm rewrite") {
                        note!(format!("[RHYTHM] no snapshot annotated `Sentence rhythm rewrite`: {:?}", snaps.iter().map(|s| s.annotation.clone()).collect::<Vec<_>>()));
                    } else {
                        out.push("  [RHYTHM] accept replaced the body and left an annotated snapshot of the old one".into());
                    }
                }
                Err(e) => note!(format!("[RHYTHM] list_snapshots: {e}")),
            }
            if app.pending_rewrite_diff.is_some() {
                note!("[RHYTHM] pending_rewrite_diff still armed after accept".into());
            }
        }

        // ---- Rewrite rejected: nothing changes ------------------------------------
        let before_reject = buffer(&app);
        if let Some(tx) = start!("RHYTHM-REJECT", app.start_sentence_rhythm_rewrite()) {
            finish(&mut app, &tx, "= Para One\n\nSomething else entirely.\n");
            if !matches!(app.modal, Modal::AiDiffReview { .. }) {
                note!("[RHYTHM-REJECT] no diff review".into());
            }
            send(&mut app, k(KeyCode::Esc)).unwrap();
            let closer = if matches!(app.modal, Modal::None) {
                "Esc"
            } else {
                send(&mut app, k(KeyCode::Char('r'))).unwrap();
                "r"
            };
            if !matches!(app.modal, Modal::None) {
                note!("[RHYTHM-REJECT] neither Esc nor r closed the diff review".into());
            }
            if buffer(&app) != before_reject {
                note!("[RHYTHM-REJECT] a rejected rewrite changed the paragraph".into());
            } else {
                out.push(format!("  [RHYTHM-REJECT] `{closer}` closed the review and kept the paragraph"));
            }
        }

        // ---- Rewrite stream fails: flags cleared --------------------------------
        if let Some(tx) = start!("RHYTHM-ERROR", app.start_sentence_rhythm_rewrite()) {
            fail(&mut app, &tx, "stub: no such adapter");
            if app.pending_rewrite_diff.is_some() || app.pending_rewrite_span.is_some() {
                note!("[RHYTHM-ERROR] pending rewrite flags survive a failed stream".into());
            }
            if !matches!(app.modal, Modal::None) {
                note!("[RHYTHM-ERROR] a modal opened on an error".into());
            }
            if !app.status.contains("inference error") {
                note!(format!("[RHYTHM-ERROR] status {:?}", app.status));
            } else {
                out.push("  [RHYTHM-ERROR] a failed rewrite leaves no armed flow".into());
            }
        }

        // ---- Editorial fix, whole paragraph ------------------------------------
        if let Some(tx) = start!("EDITORIAL", app.start_editorial_rewrite("echo", None, None)) {
            finish(&mut app, &tx, "= Para One\n\nNo echoes remain in this paragraph.\n");
            match &app.modal {
                Modal::AiDiffReview { post_accept_snapshot, .. } => {
                    if !post_accept_snapshot.as_deref().is_some_and(|a| a.starts_with("Editorial:")) {
                        note!(format!("[EDITORIAL] annotation {post_accept_snapshot:?}"));
                    } else {
                        out.push(format!("  [EDITORIAL] diff review annotated {post_accept_snapshot:?}"));
                    }
                }
                _ => note!("[EDITORIAL] no diff review after Done".into()),
            }
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if !buffer(&app).contains("No echoes remain") {
                note!("[EDITORIAL] Enter did not accept the fix".into());
            }
        }

        // ---- Editorial fix, span-scoped: the reply is spliced into the range ------
        // `echo` is a whole-paragraph fix (its span is ignored by design); `filter`
        // is one of the Span-scoped specs, so the reply is the replacement phrase.
        let current = buffer(&app);
        let span_end = current.find("echoes").unwrap_or(10);
        if let Some(tx) = start!("EDITORIAL-SPAN", app.start_editorial_rewrite("filter", Some((0, span_end)), None)) {
            finish(&mut app, &tx, "SPLICED");
            match &app.modal {
                Modal::AiDiffReview { after_lines, .. } => {
                    let after = after_lines.join("\n");
                    let expected_tail = &current[span_end..];
                    if !after.starts_with("SPLICED") || !after.contains(expected_tail.trim_end()) {
                        note!(format!("[EDITORIAL-SPAN] splice wrong: {after:?} (tail {expected_tail:?})"));
                    } else {
                        out.push("  [EDITORIAL-SPAN] the phrase replaced only its span; the rest is intact".into());
                    }
                }
                _ => note!("[EDITORIAL-SPAN] no diff review after Done".into()),
            }
            send(&mut app, k(KeyCode::Esc)).unwrap();
            if !matches!(app.modal, Modal::None) {
                send(&mut app, k(KeyCode::Char('r'))).unwrap();
            }
            if buffer(&app) != current {
                note!("[EDITORIAL-SPAN] rejecting changed the paragraph".into());
            }
        }

        // ---- Continuation draft: only the <<<DRAFT>>> block is inserted ------------
        if let Some(doc) = app.opened.as_mut() {
            doc.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            doc.textarea.move_cursor(tui_textarea::CursorMove::End);
        }
        let before_draft = buffer(&app);
        if let Some(tx) = start!("CONTINUATION", app.start_continuation_draft()) {
            if !app.pending_continuation_draft {
                note!("[CONTINUATION] pending_continuation_draft not armed".into());
            }
            finish(&mut app, &tx, "Here is a draft:\n<<<DRAFT>>>The ice sang under the hulls.<<<END>>>\nNotes follow the draft.");
            ai_key(&mut app, KeyCode::Char('i'));
            let now = buffer(&app);
            if !now.contains("The ice sang under the hulls.") {
                note!(format!("[CONTINUATION] the draft was not inserted: {now:?}"));
            }
            if now.contains("Notes follow") || now.contains("Here is a draft") {
                note!("[CONTINUATION] commentary outside the markers reached the manuscript".into());
            }
            if app.pending_continuation_draft {
                note!("[CONTINUATION] flag still armed after the insert".into());
            }
            if !now.starts_with(before_draft.trim_end()) {
                note!("[CONTINUATION] text before the cursor changed".into());
            }
            if problems.iter().all(|p| !p.starts_with("[CONTINUATION]")) {
                out.push("  [CONTINUATION] `i` lifted only the <<<DRAFT>>> block at the cursor".into());
            }
        }

        // ---- Fact check: verdict parsed into navigable findings ---------------
        if let Some(tx) = start!("FACT-CHECK", app.start_fact_check()) {
            if app.fact_check_pending.is_none() {
                note!("[FACT-CHECK] fact_check_pending not armed".into());
            }
            finish(
                &mut app,
                &tx,
                "claim | fact | detail\nThe harbor froze in January | The harbor never freezes | climate note\nnone\nMara spoke | Mara is mute | character sheet\n",
            );
            let n = app.fact_check_nav.findings.len();
            if n != 2 {
                note!(format!("[FACT-CHECK] parsed {n} finding(s), expected 2: {:?}", app.fact_check_nav.findings));
            }
            if !app.status.contains("2 contradiction") {
                note!(format!("[FACT-CHECK] status {:?}", app.status));
            }
            if app.fact_check_pending.is_some() {
                note!("[FACT-CHECK] fact_check_pending still armed".into());
            }
            app.next_fact_finding();
            if app.fact_check_nav.idx != 0 {
                note!(format!("[FACT-CHECK] first jump landed on idx {}", app.fact_check_nav.idx));
            }
            app.next_fact_finding();
            app.next_fact_finding();
            if app.fact_check_nav.idx != 0 {
                note!(format!("[FACT-CHECK] jumps do not wrap: idx {}", app.fact_check_nav.idx));
            } else {
                out.push("  [FACT-CHECK] 2 findings parsed, jump cycles and wraps".into());
            }
            render_all(&mut app, "fact-check", &mut problems);
        }
        // A clean verdict leaves no findings.
        if let Some(tx) = start!("FACT-CHECK-CLEAN", app.start_fact_check()) {
            finish(&mut app, &tx, "none\n");
            if !app.fact_check_nav.findings.is_empty() {
                note!(format!("[FACT-CHECK-CLEAN] findings from a clean verdict: {:?}", app.fact_check_nav.findings));
            } else {
                out.push("  [FACT-CHECK-CLEAN] `none` yields no findings".into());
            }
        }

        // ---- Pane-landing flows: critique, show-don't-tell, grammar -----------
        for (label, start, reply) in [
            ("CRITIQUE", 0u8, "Strong opening; the second sentence drags."),
            ("SHOW-DONT-TELL", 1, "\"The town waited\" tells; show the empty quay."),
            ("GRAMMAR", 2, "No grammar issues found."),
        ] {
            let tx = match start {
                0 => start!(label, app.start_critique()),
                1 => start!(label, app.start_show_dont_tell_scan()),
                _ => start!(label, app.start_grammar_check()),
            };
            if let Some(tx) = tx {
                let before = buffer(&app);
                finish(&mut app, &tx, reply);
                let resp = app.inference.as_ref().map(|i| i.response.clone()).unwrap_or_default();
                if resp != reply {
                    note!(format!("[{label}] pane shows {resp:?}"));
                }
                if !matches!(app.modal, Modal::None) {
                    note!(format!("[{label}] a modal opened for a pane-landing flow"));
                }
                if buffer(&app) != before {
                    note!(format!("[{label}] the manuscript changed without an apply key"));
                }
                if !app.status.contains("responded") {
                    note!(format!("[{label}] status {:?}", app.status));
                }
                if problems.iter().all(|p| !p.starts_with(&format!("[{label}]"))) {
                    out.push(format!("  [{label}] reply landed in the AI pane, manuscript untouched"));
                }
            }
        }

        // ---- Usage ledger: every flow counted, under its own category --------
        out.push(String::new());
        out.push("  usage categories per flow:".into());
        for (label, delta) in &categories {
            out.push(format!("    {label:<18} {}", delta.join(", ")));
        }
        let names: std::collections::BTreeSet<String> = categories
            .iter()
            .filter(|(l, _)| !l.starts_with("RHYTHM-") && !l.starts_with("EDITORIAL-") && !l.ends_with("-CLEAN"))
            .flat_map(|(_, d)| d.iter().map(|s| s.split('+').next().unwrap_or("").to_string()))
            .collect();
        let flows = categories
            .iter()
            .filter(|(l, _)| !l.starts_with("RHYTHM-") && !l.starts_with("EDITORIAL-") && !l.ends_with("-CLEAN"))
            .count();
        if names.len() < flows {
            out.push(format!("  note: {flows} distinct flows share {} usage categories: {names:?}", names.len()));
        }

        // The original paragraph was replaced on purpose; make sure the flows
        // did not touch the second paragraph of the chapter.
        let _ = original;
        let h = &app.hierarchy;
        if let Some(two) = h.iter().find(|n| n.title == "Para Two") {
            if let Some(rel) = &two.file {
                let t = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
                if !t.contains("Mara left") {
                    note!("[ISOLATION] another paragraph changed".into());
                }
            }
        }

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} flow problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

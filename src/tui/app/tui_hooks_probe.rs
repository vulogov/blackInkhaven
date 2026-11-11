//! TUI-fired hooks probe (3.22) — the lifecycle hooks the editor itself fires
//! (`hook.on_status_promoted`, `hook.on_goal_hit`, `hook.on_streak_milestone`,
//! `hook.on_assemble`), driven through the real `App` on a trusted temp project
//! whose bootstrap stores every argument in a Bund variable the probe reads
//! back. The store-fired hooks (`on_create` … `on_delete`) are covered by
//! `scripting::hooks_probe`.
//!
//! Not driven here, and said so in the report: `hook.on_streak_break` needs a
//! day boundary to pass inside one session, and `hook.on_take` compiles with
//! typst and copies the PDF into the launch cwd.
//!
//! Adam, the policy and the active store are process-wide singletons set by
//! the first `Store::open`, so this runs alone:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture --exact tui::app::tui_hooks_probe::tui_hooks_probe
//! ```
//!
//! (`--exact`: a bare `hooks_probe` filter also matches `scripting::hooks_probe`,
//! and two probes sharing one Adam read each other's variables.)
#![cfg(test)]

use super::modal_probe::{open_para, probe_app_with};
use super::pane_probe::run_probe;
use super::*;

/// Each hook stores its arguments under `seen.*` (the top of the workbench is
/// the last argument, so the stores run right to left).
const BOOTSTRAP: &str = r#"
"hook.on_status_promoted" { "seen.status_to" swap var "seen.status_from" swap var "seen.status_uuid" swap var } register
"hook.on_goal_hit" { "seen.goal" swap var "seen.goal_words" swap var } register
"hook.on_streak_milestone" { "seen.milestone" swap var } register
"hook.on_streak_break" { "seen.streak_break" swap var } register
"hook.on_assemble" { "seen.assemble_files" swap var "seen.assemble_root" swap var "seen.assemble_slug" swap var "seen.assemble_uuid" swap var } register
"seen.boot" "yes" var
"#;

/// The value of Bund variable `name` rendered as text, or the eval error.
fn var(name: &str) -> Result<String, String> {
    let out = crate::scripting::eval(&format!("\"{name}\" var?")).map_err(|e| e.to_string())?;
    let v = out.top.ok_or_else(|| "nothing on the stack".to_string())?;
    Ok(crate::scripting::format_value(&v))
}

#[test]
#[ignore = "probe: builds the process-wide Adam VM through the editor; run alone with --ignored --nocapture"]
fn tui_hooks_probe() {
    run_probe("tui-hooks", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! check {
            ($cond:expr, $($arg:tt)*) => {{
                let msg = format!($($arg)*);
                if $cond { out.push(format!("  ok   {msg}")); } else { out.push(format!("  !!   {msg}")); problems.push(msg); }
            }};
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let (_tmp, mut app, pid) = probe_app_with(|cfg| {
            cfg.scripting.bootstrap = BOOTSTRAP.to_string();
            cfg.scripting.trust_decision = "trust".to_string();
            cfg.goals.daily_words = 3;
        });
        let book_id = app
            .hierarchy
            .iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .map(|n| n.id)
            .expect("user book");

        // The TUI builds Adam eagerly, and a trusted project runs the bootstrap.
        check!(crate::scripting::adam_initialised(), "the editor built Adam at startup");
        check!(var("seen.boot").as_deref() == Ok("yes"), "the bootstrap ran (seen.boot = {:?})", var("seen.boot"));

        // ---- on_status_promoted ( uuid from to -- ) ---------------------------
        let before = app.hierarchy.get(pid).and_then(|n| n.status.clone());
        let cycled = app.cycle_status_single(pid);
        check!(cycled.is_ok(), "cycle_status_single succeeded ({cycled:?})");
        app.reload_hierarchy();
        let after = app.hierarchy.get(pid).and_then(|n| n.status.clone());
        check!(before != after, "the status changed ({before:?} → {after:?})");
        check!(
            var("seen.status_uuid").as_deref() == Ok(pid.to_string().as_str()),
            "on_status_promoted saw the paragraph's uuid (got {:?})",
            var("seen.status_uuid")
        );
        let to = var("seen.status_to").unwrap_or_default();
        check!(
            after.as_deref().map(|s| s.to_ascii_lowercase()) == Some(to.clone()),
            "on_status_promoted's `to` matches the stored status ({to:?} vs {after:?})"
        );
        check!(
            !var("seen.status_from").unwrap_or_default().is_empty(),
            "on_status_promoted carried a `from` label ({:?})",
            var("seen.status_from")
        );

        // ---- on_streak_milestone needs history: seed six earlier writing days ----
        // Today's save (below) becomes the seventh consecutive day; the editor's
        // first progress snapshot (at open) saw no history, so 0 → 7 crosses the
        // first milestone.
        let now = crate::dayclock::now_secs();
        let seeded = crate::progress::with_active(|s| {
            for d in 1..=6 {
                s.record_event_at(now - d * 86_400, "save", pid, Some(book_id), 10, 100)?;
            }
            Ok(())
        });
        check!(matches!(seeded, Some(Ok(()))), "six earlier writing days seeded into the progress store ({seeded:?})");

        // ---- on_goal_hit ( today_words daily_goal -- ) via a real save ----------
        open_para(&mut app, pid);
        if let Some(doc) = app.opened.as_mut() {
            doc.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            doc.textarea.move_cursor(tui_textarea::CursorMove::End);
            doc.textarea.insert_str("\n\nSix more words land on this line today.\n");
        }
        let saved = app.save_current().map_err(|e| e.to_string());
        check!(saved.is_ok(), "save_current succeeded ({saved:?})");
        let goal_words = var("seen.goal_words");
        check!(
            goal_words.as_deref().map(|w| w.parse::<i64>().unwrap_or(0) >= 3) == Ok(true),
            "on_goal_hit fired with today's words ≥ goal (today_words {goal_words:?})"
        );
        check!(var("seen.goal").as_deref() == Ok("3"), "on_goal_hit carried the daily goal (got {:?})", var("seen.goal"));
        let streak = app.progress_cache.as_ref().map(|s| s.streak.days).unwrap_or(-1);
        check!(streak >= 7, "the progress snapshot counts the seven-day streak (got {streak})");
        check!(
            var("seen.milestone").as_deref() == Ok("7"),
            "on_streak_milestone fired for the 7-day crossing (got {:?})",
            var("seen.milestone")
        );
        check!(
            var("seen.streak_break").is_err(),
            "on_streak_break did not fire (the streak is alive)"
        );

        // A second save over the line must not re-fire the goal hook.
        let _ = crate::scripting::eval("\"seen.goal\" var-");
        if let Some(doc) = app.opened.as_mut() {
            doc.textarea.insert_str("More.");
        }
        let _ = app.save_current();
        check!(var("seen.goal").is_err(), "on_goal_hit does not re-fire while still over the line");

        // ---- on_assemble ( uuid slug root_typ files_written -- ) -----------------
        // The probe book was created straight in the store; the editor's
        // `add book` provisions the Typst skeleton (chapter + index/settings/
        // globals) that assembly reads, so do that first.
        if let Some(book_node) = app.hierarchy.get(book_id).cloned() {
            let provisioned = app.store.provision_user_book(&app.cfg, &book_node).map_err(|e| e.to_string());
            check!(provisioned.is_ok(), "the book's Typst skeleton is provisioned ({provisioned:?})");
            app.reload_hierarchy();
        }
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        app.pending_assembly = Some(book_id);
        app.run_pending_assembly(&mut terminal, book_id);
        check!(
            app.status.starts_with("Book assembly: wrote"),
            "assembly reported success ({})",
            app.status
        );
        check!(
            var("seen.assemble_uuid").as_deref() == Ok(book_id.to_string().as_str()),
            "on_assemble saw the book's uuid (got {:?})",
            var("seen.assemble_uuid")
        );
        let slug = app.hierarchy.get(book_id).map(|n| n.slug.clone()).unwrap_or_default();
        check!(
            var("seen.assemble_slug").as_deref() == Ok(slug.as_str()),
            "on_assemble saw the book's slug (got {:?})",
            var("seen.assemble_slug")
        );
        let root = var("seen.assemble_root").unwrap_or_default();
        check!(
            std::path::Path::new(&root).is_file(),
            "on_assemble's root .typ exists on disk ({root})"
        );
        check!(
            var("seen.assemble_files").map(|s| s.parse::<i64>().unwrap_or(0) > 0) == Ok(true),
            "on_assemble reported a positive file count (got {:?})",
            var("seen.assemble_files")
        );

        let depth = crate::scripting::with_adam(|b| b.vm.stack.current_stack_len()).unwrap_or(usize::MAX);
        check!(depth == 0, "the workbench is empty after the hooks ran (depth {depth})");

        out.push(String::new());
        out.push("  not driven: hook.on_streak_break (needs a day boundary), hook.on_take (typst + launch cwd)".into());
        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} hook problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

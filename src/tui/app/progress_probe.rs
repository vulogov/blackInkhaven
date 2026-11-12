//! Progress and goals probe (3.22) — the writing streak was 0 in every project
//! until the day-binning fix, so none of the surfaces that show it had ever
//! rendered a real one. This drives them through the real `App` on a temp
//! project with six seeded writing days plus today's save:
//!
//! * the progress snapshot, the Ctrl+V G progress modal and the writing-streak
//!   heatmap (streak, longest run, active days);
//! * the journal export's word-count section;
//! * the Bund words `ink.goals.streak` and `ink.goals.snapshot`, which open the
//!   progress store a second time while the editor holds it;
//! * day rollovers with the editor still open (`dayclock::shift_for_test`):
//!   after the first midnight the streak stands (today is pending) and the
//!   new day starts from a fresh baseline so `today_words` reads 0; after a
//!   second midnight that closed an unwritten day the streak drops to 0 and
//!   `hook.on_streak_break` fires with the old length; the next save over
//!   the line fires `hook.on_goal_hit` again and starts a new streak.
//!
//! Adam, the policy, the active store and the day clock are process-wide, so
//! this runs alone:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture --exact tui::app::progress_probe::progress_probe
//! ```
#![cfg(test)]

use super::modal_probe::{open_para, probe_app_with};
use super::pane_probe::run_probe;
use super::*;

const BOOTSTRAP: &str = r#"
"hook.on_goal_hit" { "seen.goal" swap var "seen.goal_words" swap var } register
"hook.on_streak_milestone" { "seen.milestone" swap var } register
"hook.on_streak_break" { "seen.streak_break" swap var } register
"#;

fn var(name: &str) -> Result<String, String> {
    let out = crate::scripting::eval(&format!("\"{name}\" var?")).map_err(|e| e.to_string())?;
    let v = out.top.ok_or_else(|| "nothing on the stack".to_string())?;
    Ok(crate::scripting::format_value(&v))
}

/// Evaluate a Bund expression that leaves a dict, and read `key` from it.
fn dict_key(expr: &str, key: &str) -> Result<String, String> {
    let out = crate::scripting::eval(expr).map_err(|e| e.to_string())?;
    let v = out.top.ok_or_else(|| "nothing on the stack".to_string())?;
    let d = v.cast_dict().map_err(|e| format!("not a dict: {e}"))?;
    d.get(key)
        .map(crate::scripting::format_value)
        .ok_or_else(|| format!("no key `{key}` in {:?}", d.keys().collect::<Vec<_>>()))
}

fn screen(app: &mut App) -> String {
    crate::probe_kit::render_text(140, 50, |f| app.draw(f)).unwrap_or_else(|e| format!("<render failed: {e}>"))
}

#[test]
#[ignore = "probe: process-wide Adam, progress store and day clock; run alone with --ignored --nocapture"]
fn progress_probe() {
    run_probe("progress", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! check {
            ($cond:expr, $($arg:tt)*) => {{
                let msg = format!($($arg)*);
                if $cond { out.push(format!("  ok   {msg}")); } else { out.push(format!("  !!   {msg}")); problems.push(msg); }
            }};
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let (tmp, mut app, pid) = probe_app_with(|cfg| {
            cfg.scripting.bootstrap = BOOTSTRAP.to_string();
            cfg.scripting.trust_decision = "trust".to_string();
            cfg.goals.daily_words = 3;
            cfg.goals.streak_grace_per_week = 0;
        });
        let root = tmp.path().to_path_buf();
        let book_id = app
            .hierarchy
            .iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .map(|n| n.id)
            .expect("user book");

        // ---- seed six earlier writing days: save events for the streak, and
        // baselines so the heatmap / sparkline see ten words on each of them.
        let now = crate::dayclock::now_secs();
        let today = crate::dayclock::today_days();
        let (per_book, project_total) = app.compute_word_totals_now();
        let book_total = per_book.iter().find(|(id, _)| *id == book_id).map(|(_, t)| *t).unwrap_or(0);
        let seeded = crate::progress::with_active(|s| {
            for d in 1..=6_i64 {
                s.record_event_at(now - d * 86_400, "save", pid, Some(book_id), 10, 100)?;
                s.record_baseline_at(today - d, crate::progress::store::PROJECT_SCOPE_BOOK_ID, project_total - 10 * d)?;
                s.record_baseline_at(today - d, book_id, book_total - 10 * d)?;
            }
            Ok(())
        });
        check!(matches!(seeded, Some(Ok(()))), "six earlier writing days seeded ({seeded:?})");

        // ---- today's save: streak 0 → 7, goal hit, milestone ----------------
        open_para(&mut app, pid);
        if let Some(doc) = app.opened.as_mut() {
            doc.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            doc.textarea.move_cursor(tui_textarea::CursorMove::End);
            doc.textarea.insert_str("\n\nFive more words today then.\n");
        }
        let saved = app.save_current().map_err(|e| e.to_string());
        check!(saved.is_ok(), "save_current succeeded ({saved:?})");
        let snap = app.progress_cache.clone().expect("progress cache");
        check!(snap.streak.days == 7, "snapshot streak is 7 days (got {})", snap.streak.days);
        check!(snap.streak.best == 7, "snapshot best streak is 7 (got {})", snap.streak.best);
        check!(snap.project.today_words >= 3, "today_words ≥ goal (got {})", snap.project.today_words);
        check!(snap.project.daily_goal == Some(3), "daily goal is 3 (got {:?})", snap.project.daily_goal);
        check!(
            snap.sparkline.len() == 30 && snap.sparkline[23..29].iter().all(|w| *w == 10),
            "sparkline shows ten words on each of the six seeded days (tail {:?})",
            snap.sparkline.iter().rev().take(8).collect::<Vec<_>>()
        );
        check!(var("seen.goal").as_deref() == Ok("3"), "on_goal_hit fired (goal {:?})", var("seen.goal"));
        check!(var("seen.milestone").as_deref() == Ok("7"), "on_streak_milestone fired (got {:?})", var("seen.milestone"));

        // ---- progress modal -----------------------------------------------------
        app.open_progress_modal();
        let s = screen(&mut app);
        check!(s.contains("streak: 7d"), "progress modal shows `streak: 7d`");
        check!(
            s.contains(&format!("words: {}/3", snap.project.today_words)),
            "progress modal shows today's words against the goal"
        );
        app.modal = Modal::None;

        // ---- writing-streak heatmap --------------------------------------------
        app.open_writing_streak_heatmap();
        check!(
            matches!(app.modal, Modal::WritingStreakHeatmap { .. }),
            "the heatmap opened (status: {})",
            app.status
        );
        let s = screen(&mut app);
        check!(s.contains("Writing streak"), "heatmap title is on screen");
        check!(s.contains("7-day current streak"), "heatmap footer counts the 7-day streak");
        check!(s.contains("7-day longest in window"), "heatmap footer's longest run is 7 days");
        check!(s.contains("7/91 days active"), "heatmap footer counts 7 active days");
        app.modal = Modal::None;

        // ---- journal export -----------------------------------------------------
        let journal = crate::tui::journal::compute_snapshot(app.progress_cache.as_ref(), &app.hierarchy, &root, 0, "");
        let md = crate::tui::journal::to_markdown(&journal);
        check!(md.contains("streak: 7 day(s)"), "journal markdown carries the streak");

        // ---- Bund words (second handle on progress.db while the editor holds one)
        let days = dict_key("ink.goals.streak", "days");
        check!(days.as_deref() == Ok("7"), "ink.goals.streak reports days = 7 (got {days:?})");
        let best = dict_key("ink.goals.streak", "best");
        check!(best.as_deref() == Ok("7"), "ink.goals.streak reports best = 7 (got {best:?})");
        let snap_streak = crate::scripting::eval("ink.goals.snapshot")
            .map_err(|e| e.to_string())
            .and_then(|o| o.top.ok_or_else(|| "empty".to_string()))
            .and_then(|v| v.cast_dict().map_err(|e| e.to_string()))
            .and_then(|d| d.get("streak").cloned().ok_or_else(|| "no streak".to_string()))
            .and_then(|v| v.cast_dict().map_err(|e| e.to_string()))
            .and_then(|d| d.get("days").map(crate::scripting::format_value).ok_or_else(|| "no days".to_string()));
        check!(snap_streak.as_deref() == Ok("7"), "ink.goals.snapshot carries streak.days = 7 (got {snap_streak:?})");

        // ---- day rollover with the editor open -----------------------------------
        // (`ink.goals.*` reinstall the configured boundary, so shift after them.)
        let _ = crate::scripting::eval("\"seen.goal\" var-");
        let goal_words_before = snap.project.today_words;
        crate::dayclock::shift_for_test(86_400);
        app.refresh_progress_cache();
        let next = app.progress_cache.clone().expect("progress cache");
        check!(next.streak.days == 7, "the morning after, the streak still stands (today pending; got {})", next.streak.days);
        check!(var("seen.streak_break").is_err(), "on_streak_break did not fire at midnight");
        check!(
            next.project.today_words == 0,
            "the new day starts at 0 words (got {}; yesterday ended at {goal_words_before})",
            next.project.today_words
        );
        check!(
            next.sparkline.len() == 30 && next.sparkline[28] == goal_words_before,
            "yesterday keeps its words in the sparkline after the rollover (got {:?})",
            next.sparkline.get(28)
        );
        check!(var("seen.goal").is_err(), "on_goal_hit did not fire on the empty new day");

        // A second midnight closes a day with no writing: that is the break.
        crate::dayclock::shift_for_test(86_400);
        app.refresh_progress_cache();
        let broken = app.progress_cache.clone().expect("progress cache");
        check!(broken.streak.days == 0, "after a missed day the streak reads 0 (got {})", broken.streak.days);
        check!(broken.streak.best == 7, "the best streak survives the break (got {})", broken.streak.best);
        check!(
            var("seen.streak_break").as_deref() == Ok("7"),
            "on_streak_break fired with the old length (got {:?})",
            var("seen.streak_break")
        );
        if let Some(doc) = app.opened.as_mut() {
            doc.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            doc.textarea.insert_str("\nFour words for tomorrow.\n");
        }
        let saved = app.save_current().map_err(|e| e.to_string());
        check!(saved.is_ok(), "save on the new day succeeded ({saved:?})");
        let after = app.progress_cache.clone().expect("progress cache");
        check!(
            (3..=6).contains(&after.project.today_words),
            "the new day's words count only the new save (got {})",
            after.project.today_words
        );
        check!(var("seen.goal").as_deref() == Ok("3"), "on_goal_hit fired again on the new day (got {:?})", var("seen.goal"));
        check!(after.streak.days == 1, "writing on the new day starts a 1-day streak (got {})", after.streak.days);
        crate::dayclock::shift_for_test(-2 * 86_400);

        let depth = crate::scripting::with_adam(|b| b.vm.stack.current_stack_len()).unwrap_or(usize::MAX);
        check!(depth == 0, "the workbench is empty after the hooks ran (depth {depth})");

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} progress problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

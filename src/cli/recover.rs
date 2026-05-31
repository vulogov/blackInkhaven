//! 1.2.15+ Phase R.2 — `inkhaven recover <crash.hjson>`.
//!
//! Reads a crash report produced by [`crate::crash`]
//! and walks the rescued-buffer manifest.  For each
//! buffer the user is prompted:
//!
//!   - `y` — apply the rescue (overwrite the on-disk
//!     file with the rescue bytes, after writing a
//!     `<original>.pre-recover-<UTC>` backup so the
//!     pre-rescue version is recoverable).
//!   - `N` (default) — skip this buffer.
//!   - `d` — show a unified diff between rescue and
//!     on-disk, then re-prompt.
//!
//! `--yes` skips the prompt and applies every rescue
//! whose rescue file exists.  `--keep` leaves the
//! crash report + rescue files in place; the default
//! behaviour is to move them into
//! `<project>/.inkhaven/recovered/` (or, when the
//! report has no project path, alongside the report
//! file).
//!
//! The recover CLI never reads the project's bdslib
//! database — it operates purely on disk paths.  Two
//! reasons: (1) bdslib may be locked by a fresh TUI
//! session the user just opened to investigate the
//! crash, (2) atomicity is simpler when the only
//! state touched is `.typ` files + the rescue
//! companions.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::crash::CrashReport;

/// Apply rescued buffers from a crash report back to
/// disk.
pub fn run(report_path: &Path, yes: bool, keep: bool) -> Result<()> {
    let body = std::fs::read_to_string(report_path)
        .with_context(|| format!("read crash report {}", report_path.display()))?;
    let report: CrashReport = serde_hjson::from_str(&body)
        .with_context(|| format!("parse crash report HJSON ({})", report_path.display()))?;

    println!(
        "Crash report: {}\n  inkhaven version : {}\n  panic at         : {}\n  generated at     : {}",
        report_path.display(),
        report.version,
        report
            .panic
            .location
            .as_deref()
            .unwrap_or("<unknown location>"),
        report.generated_at,
    );
    if let Some(ref msg) = Some(&report.panic.message) {
        if !msg.is_empty() {
            println!("  message          : {msg}");
        }
    }

    let project_root = report
        .project
        .path
        .as_deref()
        .map(Path::new);

    if report.rescued_buffers.is_empty() {
        println!("\nNo rescued buffers in this report — nothing to recover.");
        if !keep {
            move_report_only(report_path)?;
        }
        return Ok(());
    }

    println!(
        "\nFound {} rescued buffer(s).\n",
        report.rescued_buffers.len()
    );

    let stdin = std::io::stdin();
    let mut applied = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for (idx, b) in report.rescued_buffers.iter().enumerate() {
        let rescue_path = PathBuf::from(&b.rescue_path);
        let original = match project_root {
            Some(root) => root.join(&b.paragraph_rel_path),
            // No project path in the report — fall back
            // to deriving the original by stripping the
            // `.inkhaven-rescue` suffix from the rescue
            // path.  Same outcome as the explicit
            // rel-path join in the normal case.
            None => match rescue_path.to_string_lossy().strip_suffix(".inkhaven-rescue") {
                Some(s) => PathBuf::from(s),
                None => {
                    eprintln!(
                        "  [{}/{}] {}: rescue file name doesn't match the .inkhaven-rescue convention — skipping",
                        idx + 1,
                        report.rescued_buffers.len(),
                        b.paragraph_rel_path
                    );
                    errors += 1;
                    continue;
                }
            },
        };

        println!(
            "  [{}/{}] {}",
            idx + 1,
            report.rescued_buffers.len(),
            b.paragraph_rel_path
        );
        println!("       rescue : {} ({} bytes)", rescue_path.display(), b.bytes);

        // Verify the rescue file is still there.
        let rescue_body = match std::fs::read(&rescue_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("       rescue read failed: {e} — skipping");
                errors += 1;
                continue;
            }
        };

        let on_disk_meta = std::fs::metadata(&original);
        let on_disk_body = std::fs::read(&original);
        match (&on_disk_meta, &on_disk_body) {
            (Ok(meta), Ok(body)) => {
                if rescue_body == *body {
                    println!(
                        "       on-disk: {} ({} bytes, identical to rescue) — no action needed",
                        original.display(),
                        meta.len()
                    );
                    skipped += 1;
                    continue;
                }
                let delta = rescue_body.len() as i64 - body.len() as i64;
                println!(
                    "       on-disk: {} ({} bytes, delta {:+})",
                    original.display(),
                    meta.len(),
                    delta
                );
            }
            (Ok(_), Err(e)) => {
                println!(
                    "       on-disk: {} ({}; will create from rescue)",
                    original.display(),
                    e
                );
            }
            (Err(_), _) => {
                println!(
                    "       on-disk: {} (does not exist — will create from rescue)",
                    original.display()
                );
            }
        }

        let apply = if yes {
            true
        } else {
            prompt_apply(&stdin, &rescue_body, on_disk_body.as_deref().ok())?
        };

        if !apply {
            println!("       skipped.");
            skipped += 1;
            continue;
        }

        if let Err(e) = apply_rescue(&original, &rescue_body) {
            eprintln!("       apply failed: {e:#}");
            errors += 1;
            continue;
        }
        println!("       applied.");
        applied += 1;
    }

    println!(
        "\nDone: {applied} applied, {skipped} skipped, {errors} error(s).",
    );

    if !keep {
        let dest_dir = recovered_directory(project_root, report_path)?;
        move_to_recovered(report_path, &report, &dest_dir)?;
        println!("Moved report + rescue files into {}", dest_dir.display());
    }

    Ok(())
}

/// Prompt loop.  Returns true if the user picked
/// `y`/`Y`, false on `N`/`n`/empty.  `d` shows the
/// diff and re-prompts.
fn prompt_apply(
    stdin: &std::io::Stdin,
    rescue: &[u8],
    on_disk: Option<&[u8]>,
) -> Result<bool> {
    loop {
        print!("       apply? [y/N/diff]: ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        match line.trim() {
            "y" | "Y" => return Ok(true),
            "" | "n" | "N" => return Ok(false),
            "d" | "diff" => {
                print_diff(rescue, on_disk);
                continue;
            }
            other => {
                println!("       (didn't understand `{other}` — try y, N, or diff)");
            }
        }
    }
}

/// Minimal unified-diff-ish output.  We don't pull in
/// a diff dependency for one CLI path — a side-by-
/// side line-walk with `-` / `+` markers is enough for
/// the user to tell whether the rescue is what they
/// expect.
fn print_diff(rescue: &[u8], on_disk: Option<&[u8]>) {
    let rescue_str = String::from_utf8_lossy(rescue);
    let on_disk_str = on_disk.map(String::from_utf8_lossy).unwrap_or_default();
    let r_lines: Vec<&str> = rescue_str.lines().collect();
    let d_lines: Vec<&str> = on_disk_str.lines().collect();
    let max = r_lines.len().max(d_lines.len());
    println!("       --- diff (on-disk → rescue) ---");
    for i in 0..max {
        match (d_lines.get(i), r_lines.get(i)) {
            (Some(a), Some(b)) if a == b => {
                println!("         {a}");
            }
            (Some(a), Some(b)) => {
                println!("       - {a}");
                println!("       + {b}");
            }
            (Some(a), None) => {
                println!("       - {a}");
            }
            (None, Some(b)) => {
                println!("       + {b}");
            }
            (None, None) => {}
        }
    }
    println!("       --- end diff ---");
}

/// Apply one rescue: write the rollback `pre-recover`
/// backup, then atomically replace the original with
/// the rescue bytes.
fn apply_rescue(original: &Path, rescue_body: &[u8]) -> Result<()> {
    if let Some(parent) = original.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent {}", parent.display()))?;
    }

    // If the original is present, snapshot it as
    // `<original>.pre-recover-<UTC>` so the user can
    // get back to the pre-rescue version with `mv`.
    if original.exists() {
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let backup = {
            let mut s = original.as_os_str().to_os_string();
            s.push(format!(".pre-recover-{stamp}"));
            PathBuf::from(s)
        };
        std::fs::copy(original, &backup)
            .with_context(|| format!("snapshot original to {}", backup.display()))?;
    }

    crate::crash::write_atomic(original, rescue_body)
        .with_context(|| format!("write rescue to {}", original.display()))?;
    Ok(())
}

/// Resolve where to move the report + rescues on
/// success.  When the report names a project root we
/// always prefer `<project>/.inkhaven/recovered/`.
/// Otherwise we fall back to a sibling directory next
/// to the report itself (`<report parent>/recovered/`).
fn recovered_directory(
    project_root: Option<&Path>,
    report_path: &Path,
) -> Result<PathBuf> {
    let base = match project_root {
        Some(root) => root.join(".inkhaven/recovered"),
        None => report_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("recovered"),
    };
    std::fs::create_dir_all(&base)
        .with_context(|| format!("create {}", base.display()))?;
    Ok(base)
}

/// Move report + rescue companions into the
/// recovered dir.  Best-effort: failures log to stderr
/// but don't surface as fatal — the rescue itself
/// succeeded.
fn move_to_recovered(
    report_path: &Path,
    report: &CrashReport,
    dest_dir: &Path,
) -> Result<()> {
    let report_dest = dest_dir.join(
        report_path
            .file_name()
            .ok_or_else(|| anyhow!("report has no file name"))?,
    );
    if let Err(e) = std::fs::rename(report_path, &report_dest) {
        eprintln!(
            "  warning: couldn't move report ({}): {e}",
            report_path.display()
        );
    }
    for b in &report.rescued_buffers {
        let rescue = Path::new(&b.rescue_path);
        if !rescue.exists() {
            continue;
        }
        let name = match rescue.file_name() {
            Some(n) => n.to_os_string(),
            None => continue,
        };
        let dest = dest_dir.join(name);
        if let Err(e) = std::fs::rename(rescue, &dest) {
            eprintln!(
                "  warning: couldn't move rescue {}: {e}",
                rescue.display()
            );
        }
    }
    Ok(())
}

/// Used when the report had no rescued buffers — we
/// still want to clean up the report file unless
/// `--keep`.
fn move_report_only(report_path: &Path) -> Result<()> {
    let dest_dir = report_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("recovered");
    std::fs::create_dir_all(&dest_dir)
        .with_context(|| format!("create {}", dest_dir.display()))?;
    let name = report_path
        .file_name()
        .ok_or_else(|| anyhow!("report has no file name"))?;
    let dest = dest_dir.join(name);
    if let Err(e) = std::fs::rename(report_path, &dest) {
        eprintln!(
            "  warning: couldn't move report ({}): {e}",
            report_path.display()
        );
    } else {
        println!("Moved report into {}", dest_dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crash::{ActionRing, CrashReport};
    use crate::crash::report::{Environment, PanicContext, ProcessContext, ProjectContext};
    use crate::crash::rescue::RescueOutcome;

    fn write_test_report(
        dir: &Path,
        project_root: &Path,
        rescues: Vec<RescueOutcome>,
    ) -> PathBuf {
        let report = CrashReport {
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: "2026-05-31T14:00:00Z".to_string(),
            panic: PanicContext {
                message: "test panic".to_string(),
                location: Some("src/test.rs:1:1".to_string()),
                thread: "main".to_string(),
            },
            project: ProjectContext {
                path: Some(project_root.display().to_string()),
                open_book: Some("manuscript".to_string()),
                open_paragraph: Some("ch1/opening".to_string()),
                open_paragraph_rel_path: Some("manuscript/ch1/opening.typ".to_string()),
            },
            rescued_buffers: rescues,
            recent_actions: ActionRing::default(),
            environment: Environment::default(),
            process: ProcessContext::default(),
        };
        let body = serde_hjson::to_string(&report).unwrap();
        let report_path = dir.join("inkhaven-crash-test.hjson");
        std::fs::write(&report_path, body).unwrap();
        report_path
    }

    #[test]
    fn run_with_no_rescues_moves_report_when_not_keep() {
        let dir = std::env::temp_dir().join(format!(
            "ink-recover-empty-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("proj");
        std::fs::create_dir_all(&project).unwrap();

        let report_path = write_test_report(&dir, &project, vec![]);
        super::run(&report_path, true, false).unwrap();

        // Report file should have moved into <dir>/recovered/.
        assert!(!report_path.exists());
        assert!(dir.join("recovered").join("inkhaven-crash-test.hjson").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_applies_rescue_with_yes_flag() {
        let dir = std::env::temp_dir().join(format!(
            "ink-recover-yes-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("proj/manuscript/ch1")).unwrap();

        let project = dir.join("proj");
        let original = project.join("manuscript/ch1/opening.typ");
        std::fs::write(&original, "ON DISK BODY").unwrap();
        let rescue = project.join("manuscript/ch1/opening.typ.inkhaven-rescue");
        std::fs::write(&rescue, "RESCUED BODY").unwrap();

        let outcome = RescueOutcome {
            paragraph_rel_path: "manuscript/ch1/opening.typ".to_string(),
            rescue_path: rescue.display().to_string(),
            bytes: 12,
            cursor_row: 0,
            cursor_col: 4,
            mirror_captured_at: "2026-05-31T13:59:00Z".to_string(),
            error: None,
        };
        let report_path = write_test_report(&dir, &project, vec![outcome]);

        super::run(&report_path, true, false).unwrap();

        // Original has the rescue body.
        let after = std::fs::read_to_string(&original).unwrap();
        assert_eq!(after, "RESCUED BODY");
        // Pre-recover backup exists, naming starts with original.
        let parent_entries: Vec<_> = std::fs::read_dir(original.parent().unwrap())
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(
            parent_entries
                .iter()
                .any(|name| name.starts_with("opening.typ.pre-recover-")),
            "missing pre-recover backup; entries = {parent_entries:?}",
        );

        // Report + rescue should have moved to <project>/.inkhaven/recovered/.
        let recovered_dir = project.join(".inkhaven/recovered");
        assert!(recovered_dir.join("inkhaven-crash-test.hjson").exists());
        assert!(
            recovered_dir
                .join("opening.typ.inkhaven-rescue")
                .exists()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_skips_rescue_when_identical_to_disk() {
        let dir = std::env::temp_dir().join(format!(
            "ink-recover-identical-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("proj/manuscript/ch1")).unwrap();

        let project = dir.join("proj");
        let original = project.join("manuscript/ch1/opening.typ");
        let rescue = project.join("manuscript/ch1/opening.typ.inkhaven-rescue");
        std::fs::write(&original, "SAME BODY").unwrap();
        std::fs::write(&rescue, "SAME BODY").unwrap();

        let outcome = RescueOutcome {
            paragraph_rel_path: "manuscript/ch1/opening.typ".to_string(),
            rescue_path: rescue.display().to_string(),
            bytes: 9,
            cursor_row: 0,
            cursor_col: 0,
            mirror_captured_at: "2026-05-31T13:59:00Z".to_string(),
            error: None,
        };
        let report_path = write_test_report(&dir, &project, vec![outcome]);

        super::run(&report_path, true, false).unwrap();

        // No pre-recover backup should have been created
        // (we treated it as "no action needed").
        let parent_entries: Vec<_> = std::fs::read_dir(original.parent().unwrap())
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(
            !parent_entries
                .iter()
                .any(|name| name.starts_with("opening.typ.pre-recover-")),
            "should not back up identical body; entries = {parent_entries:?}",
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_keep_leaves_report_in_place() {
        let dir = std::env::temp_dir().join(format!(
            "ink-recover-keep-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("proj");
        std::fs::create_dir_all(&project).unwrap();

        let report_path = write_test_report(&dir, &project, vec![]);
        super::run(&report_path, true, true).unwrap();

        // --keep means the report stays put.
        assert!(report_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}

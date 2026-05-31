//! Serializable crash report.
//!
//! Holds everything we know about the moment a panic
//! fired, in a shape that round-trips through HJSON
//! and is consumed by the `inkhaven recover` CLI in
//! Phase R.2.
//!
//! Deliberately omitted from the report (per proposal
//! §5.5):
//!   * LLM prompts / responses (privacy + size),
//!   * search queries (privacy),
//!   * snapshot bodies (size),
//!   * full DB/index state (recover CLI re-derives
//!     from rescue buffers + disk).

use serde::{Deserialize, Serialize};

use super::actions::ActionRing;
use super::rescue::RescueOutcome;

/// Everything that travels in `inkhaven-crash-<ts>.hjson`.
///
/// Field order in the struct matches what we want to
/// read first when opening the file — version at the
/// top so format-evolution downgrades are detectable,
/// panic details next so the cause is obvious without
/// scrolling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Inkhaven version that wrote the report.
    /// Recover CLI checks compatibility against its
    /// own version.
    pub version: String,
    /// UTC ISO 8601 with seconds resolution.
    pub generated_at: String,
    pub panic: PanicContext,
    pub project: ProjectContext,
    pub rescued_buffers: Vec<RescueOutcome>,
    pub recent_actions: ActionRing,
    pub environment: Environment,
    pub process: ProcessContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PanicContext {
    /// Display string from
    /// `PanicInfo::payload`.  Truncated at 4 KB if
    /// huge (rare; usually one line).
    pub message: String,
    /// `file:line:col` from `PanicInfo::location` if
    /// available.  `None` for foreign panics that
    /// strip the location.
    pub location: Option<String>,
    /// Name of the thread that panicked.  Often
    /// `"main"` but worth recording for tokio-worker
    /// panics.
    pub thread: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectContext {
    /// Project directory.
    pub path: Option<String>,
    /// Open book slug (top-level user book) at panic
    /// time.  `None` if no paragraph was open.
    pub open_book: Option<String>,
    /// Open paragraph slug (full hierarchy path under
    /// the book).
    pub open_paragraph: Option<String>,
    /// Open paragraph file rel-path.  Lets the recover
    /// CLI find the source-of-truth file without
    /// re-walking the project tree.
    pub open_paragraph_rel_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Environment {
    /// `darwin`, `linux`, `windows`, etc.
    pub os_family: String,
    /// `std::env::consts::OS` + `std::env::consts::ARCH`.
    pub os_arch: String,
    /// `$TERM` value at process start.  Helps debug
    /// rendering issues that only appear in certain
    /// terminal emulators.
    pub term: Option<String>,
    /// Locale at process start (`$LANG`).
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessContext {
    pub pid: u32,
    /// Wall-clock at process start.  Lets the recover
    /// CLI compute uptime even after the fact.
    pub started_at: String,
}

impl CrashReport {
    /// Build the report from the panicinfo + the
    /// pre-captured context state + the per-buffer
    /// rescue outcomes.
    pub fn capture(
        info: &std::panic::PanicHookInfo<'_>,
        state: &super::CrashState,
        rescue_outcomes: &[RescueOutcome],
    ) -> Self {
        let message = panic_message(info);
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
        let thread = std::thread::current()
            .name()
            .unwrap_or("<unnamed>")
            .to_string();

        let panic = PanicContext {
            message,
            location,
            thread,
        };

        let project = ProjectContext {
            path: state.project_path.as_ref().map(|p| p.display().to_string()),
            open_book: state.open_book.clone(),
            open_paragraph: state.open_paragraph.clone(),
            open_paragraph_rel_path: state.open_paragraph_rel_path.clone(),
        };

        let environment = Environment {
            os_family: std::env::consts::FAMILY.to_string(),
            os_arch: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            term: std::env::var("TERM").ok(),
            lang: std::env::var("LANG").ok(),
        };

        let process = ProcessContext {
            pid: std::process::id(),
            started_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        };

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
            panic,
            project,
            rescued_buffers: rescue_outcomes.to_vec(),
            recent_actions: state.actions.clone(),
            environment,
            process,
        }
    }

    /// Serialise + atomic write.  HJSON for human
    /// readability; the recover CLI also accepts the
    /// JSON subset.
    pub fn write_atomic(&self, target: &std::path::Path) -> std::io::Result<()> {
        let body = serde_hjson::to_string(self).map_err(|e| {
            std::io::Error::other(format!("serialise CrashReport as HJSON: {e}"))
        })?;
        super::write_atomic(target, body.as_bytes())
    }
}

/// Best-effort extraction of the panic message.
/// Truncates at 4 KB so a runaway `format!` payload
/// can't blow up the report.
fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let raw = if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        format!("non-string panic payload: {info}")
    };
    if raw.len() > 4096 {
        let mut truncated = raw.chars().take(4000).collect::<String>();
        truncated.push_str("…[truncated]");
        truncated
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ActionRecord, CrashState};

    #[test]
    fn report_roundtrips_through_hjson() {
        let state = CrashState {
            project_path: Some(std::path::PathBuf::from("/proj")),
            open_book: Some("manuscript".into()),
            open_paragraph: Some("ch1/opening".into()),
            open_paragraph_rel_path: Some("manuscript/ch1/opening.typ".into()),
            actions: {
                let mut r = super::super::actions::ActionRing::default();
                r.push(ActionRecord::new("view.add_comment"));
                r.push(ActionRecord::with_detail("ai.continuation_draft", "anchors=3"));
                r
            },
            dirty_buffers: Default::default(),
        };
        // Build a minimal report without a real
        // PanicInfo — we can't easily construct one,
        // so test via direct struct construction.
        let report = CrashReport {
            version: "1.2.15".into(),
            generated_at: "2026-05-31T14:23:00Z".into(),
            panic: PanicContext {
                message: "called Option::unwrap() on None".into(),
                location: Some("src/foo.rs:42:7".into()),
                thread: "main".into(),
            },
            project: ProjectContext {
                path: state.project_path.as_ref().map(|p| p.display().to_string()),
                open_book: state.open_book.clone(),
                open_paragraph: state.open_paragraph.clone(),
                open_paragraph_rel_path: state.open_paragraph_rel_path.clone(),
            },
            rescued_buffers: vec![],
            recent_actions: state.actions.clone(),
            environment: Environment::default(),
            process: ProcessContext::default(),
        };

        let body = serde_hjson::to_string(&report).expect("serialize");
        let parsed: CrashReport = serde_hjson::from_str(&body).expect("parse");
        assert_eq!(parsed.version, "1.2.15");
        assert_eq!(parsed.panic.message, "called Option::unwrap() on None");
        assert_eq!(parsed.project.open_book.as_deref(), Some("manuscript"));
        assert_eq!(parsed.recent_actions.entries.len(), 2);
        assert_eq!(
            parsed.recent_actions.entries[1].action,
            "ai.continuation_draft"
        );
    }

    #[test]
    fn atomic_write_creates_target_file() {
        let dir = std::env::temp_dir().join(format!(
            "inkhaven-crash-report-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("report.hjson");

        let report = CrashReport {
            version: "1.2.15".into(),
            generated_at: "2026-05-31T14:23:00Z".into(),
            panic: PanicContext::default(),
            project: ProjectContext::default(),
            rescued_buffers: vec![],
            recent_actions: super::super::actions::ActionRing::default(),
            environment: Environment::default(),
            process: ProcessContext::default(),
        };
        report.write_atomic(&target).expect("write succeeds");
        assert!(target.exists());
        let body = std::fs::read_to_string(&target).unwrap();
        // HJSON doesn't quote keys.  Just check the
        // version field shows up by its value, not its
        // key syntax.
        assert!(
            body.contains("1.2.15"),
            "body should contain version string: {body}",
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

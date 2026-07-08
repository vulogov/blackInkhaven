//! TDOC-1 — verified code blocks.
//!
//! A code listing in the manuscript can be marked `verify` (in its fence info
//! string, e.g. ` ```rust verify `); [`extract_verifiable`] pulls those blocks out
//! and [`run_block`] runs each through the project-configured runner for its
//! language, so a stale example — one that no longer compiles against the current
//! toolchain — is caught deterministically. Zero AI, zero network; the only code
//! that runs is what the author's own config declares.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::config::DocsVerifyConfig;

/// One fenced code block extracted from a paragraph body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// The fence language (lower-cased), e.g. `rust`.
    pub lang: String,
    /// The code between the fences (trailing newline included per line).
    pub code: String,
    /// Whether the fence asked to be verified (`verify` flag, and not `no-verify`).
    pub verify: bool,
}

/// The result of running one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// The runner exited 0.
    Pass,
    /// The runner exited non-zero or timed out; carries its combined output.
    Fail { detail: String },
    /// No runner configured for the language (not an error).
    Skipped { reason: String },
    /// Could not run at all (temp file / spawn failure).
    Errored { reason: String },
}

/// Extract every fenced ` ```lang … ``` ` block from a paragraph body, with its
/// `verify` flag parsed from the fence info string. Pure.
pub fn extract_verifiable(body: &str) -> Vec<CodeBlock> {
    let mut out = Vec::new();
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        let Some(info) = line.trim_start().strip_prefix("```") else { continue };
        // Info string: first token is the language, the rest are flags
        // (space- or comma-separated).
        let mut tokens = info
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty());
        let lang = tokens.next().unwrap_or("").to_ascii_lowercase();
        let flags: Vec<String> = tokens.map(|t| t.to_ascii_lowercase()).collect();
        let verify = flags.iter().any(|f| f == "verify") && !flags.iter().any(|f| f == "no-verify");
        // Collect until the closing fence.
        let mut code = String::new();
        for l in lines.by_ref() {
            if l.trim_start().starts_with("```") {
                break;
            }
            code.push_str(l);
            code.push('\n');
        }
        if !lang.is_empty() {
            out.push(CodeBlock { lang, code, verify });
        }
    }
    out
}

static TEMP_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Run one block through its configured runner. Never panics; degrades to
/// `Skipped` (no runner) or `Errored` (couldn't run). The runner command is run
/// via `sh -c` with `{file}` → a temp file holding the code and `{dir}` → its
/// parent, its combined stdout+stderr captured, and a per-block wall-clock cap.
pub fn run_block(block: &CodeBlock, cfg: &DocsVerifyConfig) -> VerifyOutcome {
    let Some(cmd_tmpl) = cfg.runners.get(&block.lang) else {
        return VerifyOutcome::Skipped { reason: format!("no runner configured for `{}`", block.lang) };
    };
    let ext = cfg.extensions.get(&block.lang).map(String::as_str).unwrap_or("txt");
    let dir = std::env::temp_dir();
    let stem = format!("inkhaven-tdoc-{}-{}", std::process::id(), TEMP_SEQ.fetch_add(1, Ordering::Relaxed));
    let code_path: PathBuf = dir.join(format!("{stem}.{ext}"));
    let out_path: PathBuf = dir.join(format!("{stem}.out"));

    if let Err(e) = std::fs::write(&code_path, &block.code) {
        return VerifyOutcome::Errored { reason: format!("write temp file: {e}") };
    }
    let cmd = cmd_tmpl
        .replace("{file}", &code_path.to_string_lossy())
        .replace("{dir}", &dir.to_string_lossy());

    // Redirect stdout+stderr to a file (not a pipe) so a chatty runner can't
    // deadlock the poll loop below.
    let outcome = (|| -> VerifyOutcome {
        let out_file = match std::fs::File::create(&out_path) {
            Ok(f) => f,
            Err(e) => return VerifyOutcome::Errored { reason: format!("create output file: {e}") },
        };
        let err_file = match out_file.try_clone() {
            Ok(f) => f,
            Err(e) => return VerifyOutcome::Errored { reason: format!("clone output file: {e}") },
        };
        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(std::process::Stdio::null())
            .stdout(out_file)
            .stderr(err_file)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return VerifyOutcome::Errored { reason: format!("spawn runner: {e}") },
        };
        let deadline = Instant::now() + Duration::from_secs(cfg.timeout_seconds.max(1));
        let (status, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(s)) => break (Some(s), false),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break (None, true);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return VerifyOutcome::Errored { reason: format!("wait runner: {e}") },
            }
        };
        let detail = std::fs::read_to_string(&out_path).unwrap_or_default();
        let detail = tail(&detail, 40);
        match status {
            Some(s) if s.success() => VerifyOutcome::Pass,
            Some(_) => VerifyOutcome::Fail { detail },
            None if timed_out => VerifyOutcome::Fail {
                detail: format!("timed out after {}s\n{detail}", cfg.timeout_seconds),
            },
            None => VerifyOutcome::Errored { reason: "runner ended abnormally".into() },
        }
    })();

    let _ = std::fs::remove_file(&code_path);
    let _ = std::fs::remove_file(&out_path);
    outcome
}

/// Emit a failed-verification finding to the Output pane, anchored on `para_id`
/// so it colours the tree badge and answers the `t`/this-paragraph filter.
pub fn emit_finding(para_id: uuid::Uuid, lang: &str, detail: &str) {
    use crate::pane::output::{emit, kinds, Lifetime, Message, Severity};
    let msg = Message::new(
        kinds::DOC_VERIFY,
        Severity::Warning,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": format!("code example failed `{lang}` verification"),
            "category": "doc-verify",
            "lang": lang,
            "detail": detail,
        }),
    )
    .with_source_paragraph(para_id);
    emit(&msg);
}

/// The last `n` non-empty lines of a runner's output (the informative tail).
fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn cfg(runners: &[(&str, &str)]) -> DocsVerifyConfig {
        let mut c = DocsVerifyConfig { enabled: true, ..Default::default() };
        c.runners = runners.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        c
    }

    #[test]
    fn extract_parses_language_and_verify_flag() {
        let body = "#figure(caption: [x])[\n```rust verify\nfn main() {}\n```\n]\n\
                    prose\n```python\nprint(1)\n```\n```sh no-verify\necho hi\n```";
        let blocks = extract_verifiable(body);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].lang, "rust");
        assert!(blocks[0].verify);
        assert!(blocks[0].code.contains("fn main"));
        assert_eq!(blocks[1].lang, "python");
        assert!(!blocks[1].verify, "no flag → not verified");
        assert!(!blocks[2].verify, "no-verify wins");
    }

    #[test]
    fn run_block_reports_pass_fail_and_skip() {
        let c = cfg(&[("pass", "true"), ("boom", "false")]);
        let mk = |lang: &str| CodeBlock { lang: lang.into(), code: "x\n".into(), verify: true };
        assert_eq!(run_block(&mk("pass"), &c), VerifyOutcome::Pass);
        assert!(matches!(run_block(&mk("boom"), &c), VerifyOutcome::Fail { .. }));
        assert!(matches!(run_block(&mk("cobol"), &c), VerifyOutcome::Skipped { .. }));
    }

    #[test]
    fn run_block_substitutes_file_and_captures_output() {
        // Echo the temp file's contents; `grep` fails when the marker is absent.
        let mut c = DocsVerifyConfig { enabled: true, ..Default::default() };
        c.extensions = BTreeMap::from([("txt".to_string(), "txt".to_string())]);
        c.runners = BTreeMap::from([("txt".to_string(), "grep MARKER {file}".to_string())]);
        let hit = CodeBlock { lang: "txt".into(), code: "has MARKER here\n".into(), verify: true };
        let miss = CodeBlock { lang: "txt".into(), code: "nothing\n".into(), verify: true };
        assert_eq!(run_block(&hit, &c), VerifyOutcome::Pass);
        assert!(matches!(run_block(&miss, &c), VerifyOutcome::Fail { .. }));
    }

    #[test]
    fn run_block_times_out() {
        let mut c = DocsVerifyConfig { enabled: true, timeout_seconds: 1, ..Default::default() };
        c.runners = BTreeMap::from([("slow".to_string(), "sleep 5".to_string())]);
        let b = CodeBlock { lang: "slow".into(), code: "x\n".into(), verify: true };
        match run_block(&b, &c) {
            VerifyOutcome::Fail { detail } => assert!(detail.contains("timed out")),
            other => panic!("expected timeout Fail, got {other:?}"),
        }
    }
}

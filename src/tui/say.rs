//! Subprocess wrapper around macOS `/usr/bin/say`.
//!
//! Used by the TTS feature (Ctrl+B S read-aloud,
//! greeting + goodbye) after the tts-rs / AVFoundation
//! approach hit a per-process state bug on at least one
//! macOS version: the first `Tts::default()` engine could
//! speak once, but every subsequent `speak()` call on the
//! same instance (or a freshly-recreated one) returned
//! Ok with a new utterance id but produced no audio.
//!
//! Each `Say::speak` call spawns a brand-new subprocess.
//! No shared state across calls → no reuse bug.  The
//! macOS `say` binary itself wraps the same AVFoundation
//! engine, but each subprocess gets a fresh AVFoundation
//! context, side-stepping the per-process corruption.
//!
//! Trade-offs vs. an in-process tts crate:
//!   * Per-call latency: ~50-150 ms (subprocess startup +
//!     audio device init).  Imperceptible for a chord-
//!     triggered Ctrl+B S; the greeting / goodbye paths
//!     pay it once each.
//!   * No fine-grained progress callbacks — we can only
//!     poll `try_wait()` to detect process exit, not
//!     per-word events.  The playback modal already shows
//!     a spinner + elapsed time, which is enough.
//!   * Voice picking is by string match against
//!     `say -v "?"` output (the canonical macOS voice
//!     listing).
//!
//! Platform scope: macOS only.  Other platforms get a
//! "TTS is macOS-only in 1.2.9" modal from the caller —
//! we don't try to wrap a per-platform TTS abstraction
//! here.

use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Lifecycle wrapper for a single `say` subprocess.  Owns
/// the spawned `Child` until either (a) the user dismisses
/// the playback modal, (b) the process exits naturally, or
/// (c) the App is dropped at shutdown.  All three paths
/// call `stop()` for a clean teardown.
#[derive(Debug, Default)]
pub(super) struct Say {
    child: Option<Child>,
}

impl Say {
    /// Returns `Ok(())` when the host can run `say`.  The
    /// caller uses this to decide between "spawn and
    /// speak" vs "show the macOS-only modal".  Cheap:
    /// only checks `cfg!(target_os = "macos")` and the
    /// binary path's existence — no subprocess spawned.
    pub(super) fn available() -> Result<(), &'static str> {
        if !cfg!(target_os = "macos") {
            return Err("TTS is macOS-only in 1.2.9");
        }
        if !Path::new("/usr/bin/say").exists() {
            return Err("/usr/bin/say not found");
        }
        Ok(())
    }

    /// Speak `text` via `/usr/bin/say`.  Any prior
    /// subprocess this `Say` owns is killed first so a
    /// new `Ctrl+B S` during playback interrupts cleanly
    /// (the same UX the tts-rs `interrupt: true` flag was
    /// supposed to give us).  Text is passed via stdin to
    /// avoid command-line escaping issues with non-ASCII
    /// content (Russian, em-dashes, smart quotes, etc.).
    ///
    /// `voice` is the voice name as listed by `say -v "?"`;
    /// empty string falls back to the system default.
    /// `rate_wpm` is words-per-minute; `say` accepts an
    /// integer here (typical range 100-400, default
    /// ~175-220 per voice).  Pass `None` to let the voice
    /// pick its own default.
    pub(super) fn speak(
        &mut self,
        text: &str,
        voice: &str,
        rate_wpm: Option<u16>,
    ) -> std::io::Result<()> {
        // Kill any in-flight prior speech first.
        self.stop();
        let mut cmd = Command::new("/usr/bin/say");
        if !voice.is_empty() {
            cmd.arg("-v").arg(voice);
        }
        if let Some(r) = rate_wpm {
            cmd.arg("-r").arg(r.to_string());
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
            // stdin drops here, closing the pipe — `say`
            // sees EOF and starts producing audio.
        }
        self.child = Some(child);
        Ok(())
    }

    /// True while the spawned `say` is still running.
    /// Cheap (`try_wait` is a non-blocking waitpid).
    /// False when no child was spawned, after the child
    /// exits naturally, or after `stop()`.
    pub(super) fn is_speaking(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                // Reap immediately so we don't leave a
                // zombie in `child`.
                self.child = None;
                false
            }
            Err(_) => false,
        }
    }

    /// Kill the spawned `say` subprocess (if any) and
    /// reap it.  Idempotent.  Errors are swallowed —
    /// the worst case is a leaked zombie, which Drop
    /// handles via the same path on App teardown.
    pub(super) fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Enumerate installed voices by running
    /// `say -v "?"` and parsing the output.  Each line:
    ///
    ///   Milena (Enhanced)      ru-RU    # Sample text.
    ///
    /// The voice name extends from the start of the line
    /// to where two-or-more spaces precede the locale.
    /// Returns (name, locale, sample) tuples in the order
    /// `say` produced them.  On failure returns an empty
    /// list — callers fall back to "no voices known".
    pub(super) fn list_voices() -> Vec<(String, String, String)> {
        let output = match Command::new("/usr/bin/say")
            .arg("-v")
            .arg("?")
            .output()
        {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut out = Vec::new();
        for line in stdout.lines() {
            // Split on the first "# " (sample marker).
            let (head, sample) = match line.split_once("# ") {
                Some((a, b)) => (a.trim_end(), b.to_string()),
                None => (line.trim_end(), String::new()),
            };
            // Locale starts where `name<spaces>` ends —
            // we find the last whitespace run and split
            // there.  The trailing 5-char locale (e.g.
            // "ru-RU") may also be longer ("en-scotland")
            // so we split on whitespace + take last token.
            let mut parts: Vec<&str> = head.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let locale = parts.pop().unwrap_or("").to_string();
            let name = parts.join(" ");
            if name.is_empty() {
                continue;
            }
            out.push((name, locale, sample));
        }
        out
    }

    /// Substring-match `needle` against installed voice
    /// names (case-insensitive).  Prefers entries whose
    /// names also contain "Enhanced" or "Premium" so
    /// `"Milena"` auto-upgrades to `"Milena (Enhanced)"`
    /// when the premium variant is installed.  Returns
    /// the chosen voice name verbatim (suitable for the
    /// `-v` flag) or `None` when nothing matches.
    pub(super) fn pick_voice(needle: &str) -> Option<String> {
        if needle.is_empty() {
            return None;
        }
        let needle_lc = needle.to_lowercase();
        let voices = Self::list_voices();
        let mut best: Option<(String, bool, usize)> = None;
        for (name, _locale, _sample) in voices {
            if !name.to_lowercase().contains(&needle_lc) {
                continue;
            }
            let lc = name.to_lowercase();
            let enhanced =
                lc.contains("enhanced") || lc.contains("premium");
            let len = name.chars().count();
            let candidate = (name.clone(), enhanced, len);
            best = match best {
                None => Some(candidate),
                Some(prev) => {
                    // Prefer Enhanced; break ties on
                    // shorter name (so plain "Milena"
                    // doesn't lose to a random
                    // "Milena's Cousin").
                    let prev_score = (prev.1, std::cmp::Reverse(prev.2));
                    let new_score = (candidate.1, std::cmp::Reverse(candidate.2));
                    if new_score > prev_score {
                        Some(candidate)
                    } else {
                        Some(prev)
                    }
                }
            };
        }
        best.map(|(n, _, _)| n)
    }
}

impl Drop for Say {
    fn drop(&mut self) {
        self.stop();
    }
}

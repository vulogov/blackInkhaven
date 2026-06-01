//! Piper synthesis subprocess + playback dispatch.
//!
//! ## Synthesis
//!
//! `synth_to_wav(binary, voice_files, text, length_scale,
//! dest, timeout)` spawns:
//!
//! ```text
//! <binary> --model <voice.onnx> --output_file <dest>
//!          --length_scale <X.XX>
//! ```
//!
//! Text streams in on stdin (Piper supports stdin
//! reading natively — no command-line escaping needed
//! for non-ASCII).  Blocks until the subprocess exits
//! or `timeout` fires.
//!
//! ## WPM mapping
//!
//! 1.2.9's TTS surface uses words-per-minute as the
//! speed control because `say -r` accepts WPM directly.
//! Piper uses `--length_scale` instead — a multiplier on
//! sample-frame count, where 1.0 is neutral, smaller is
//! faster, larger is slower.
//!
//! Calibration (rough — Piper rates vary by voice +
//! quality tier): WPM ≈ 180 / length_scale.  We invert
//! that to map the existing `tts.speed` slider through
//! WPM into a length_scale Piper consumes.  Clamped to
//! `[0.5, 2.0]` so extreme settings remain
//! intelligible.
//!
//! ## Playback
//!
//! `select_play_command(custom, os)` returns a `Vec<String>`
//! template containing `{path}` placeholders.
//! `spawn_playback` substitutes the resolved WAV path
//! and spawns the subprocess.  Defaults per platform:
//!
//!   * macOS — `afplay {path}`
//!   * Linux — `paplay {path}` (PulseAudio); falls back
//!     to `aplay {path}` (ALSA) when paplay isn't on
//!     PATH.
//!   * Windows — `powershell -c "(New-Object
//!     Media.SoundPlayer '{path}').PlaySync()"`
//!
//! Users override via `tts.play_command`.  The override
//! is split on whitespace and `{path}` is substituted —
//! no shell intermediary, so quoting + escaping work
//! the same way `Command::new` always does.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::binary::{PiperOs, Platform};
use super::voice::VoiceFiles;
use super::PiperUnavailable;

/// Default synthesis timeout — 30 seconds, generous
/// enough for a long paragraph on a moderate-quality
/// voice but tight enough that a wedged Piper doesn't
/// stall the TUI indefinitely.
pub(crate) const DEFAULT_SYNTH_TIMEOUT: Duration =
    Duration::from_secs(30);

/// Calibration constant for the WPM ↔ length_scale
/// mapping.  See module docs.
pub(crate) const WPM_AT_LENGTH_SCALE_ONE: f32 = 180.0;

/// Convert `rate_wpm` to Piper's `--length_scale`.
/// Mapping: `length_scale = WPM_AT_LENGTH_SCALE_ONE /
/// wpm`.  Clamped to `[0.5, 2.0]` for intelligibility.
/// `None` returns `1.0` (Piper's natural rate).
pub(crate) fn wpm_to_length_scale(rate_wpm: Option<u16>) -> f32 {
    match rate_wpm {
        Some(wpm) if wpm > 0 => {
            (WPM_AT_LENGTH_SCALE_ONE / wpm as f32).clamp(0.5, 2.0)
        }
        _ => 1.0,
    }
}

/// Spawn `binary` to synthesise `text` using `voice`
/// into `dest`.  Blocks until exit / timeout / failure.
/// Returns the bytes written on success.  Cleans up
/// `dest` on failure.
pub(crate) fn synth_to_wav(
    binary: &Path,
    voice: &VoiceFiles,
    text: &str,
    rate_wpm: Option<u16>,
    dest: &Path,
    timeout: Duration,
) -> Result<u64, PiperUnavailable> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PiperUnavailable::DownloadFailed(format!(
                "mkdir synth dest {}: {e}",
                parent.display(),
            ))
        })?;
    }
    let length_scale = wpm_to_length_scale(rate_wpm);
    let mut cmd = Command::new(binary);
    cmd.arg("--model")
        .arg(&voice.onnx)
        .arg("--output_file")
        .arg(dest)
        .arg("--length_scale")
        .arg(format!("{length_scale:.2}"));
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "spawn piper {}: {e}",
            binary.display(),
        ))
    })?;

    // Stream text on stdin then close so Piper sees EOF
    // and starts synthesis.
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(e) = stdin.write_all(text.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(dest);
            return Err(PiperUnavailable::DownloadFailed(format!(
                "write piper stdin: {e}",
            )));
        }
        // Drop stdin to close the pipe.
    }

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let bytes = std::fs::metadata(dest)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    if bytes == 0 {
                        // Piper exited 0 but wrote
                        // nothing — treat as failure
                        // (catches a class of silent
                        // errors with bad voice
                        // models).
                        return Err(PiperUnavailable::DownloadFailed(
                            "piper exited 0 but produced no audio".into(),
                        ));
                    }
                    return Ok(bytes);
                }
                let mut stderr = String::new();
                if let Some(mut s) = child.stderr.take() {
                    use std::io::Read;
                    let _ = s.read_to_string(&mut stderr);
                }
                let _ = std::fs::remove_file(dest);
                return Err(PiperUnavailable::DownloadFailed(format!(
                    "piper exited {} — {}",
                    status.code().unwrap_or(-1),
                    stderr.trim(),
                )));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(dest);
                    return Err(PiperUnavailable::DownloadFailed(
                        format!(
                            "piper timed out after {}s",
                            timeout.as_secs(),
                        ),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = std::fs::remove_file(dest);
                return Err(PiperUnavailable::DownloadFailed(format!(
                    "piper wait: {e}",
                )));
            }
        }
    }
}

/// Resolved playback command template.  Argv-style;
/// `{path}` placeholders get substituted at spawn time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// Select the default playback command template.  See
/// module docs for the platform mapping.  `custom`
/// (from `tts.play_command`) overrides the default
/// entirely — splits on whitespace and treats the first
/// token as the program.
pub(crate) fn select_play_command(
    custom: Option<&str>,
    os: PiperOs,
) -> PlayCommand {
    if let Some(custom) = custom {
        let mut parts = custom.split_whitespace();
        let program = parts.next().unwrap_or("").to_string();
        let args = parts.map(String::from).collect();
        return PlayCommand { program, args };
    }
    match os {
        PiperOs::Darwin => PlayCommand {
            program: "afplay".into(),
            args: vec!["{path}".into()],
        },
        PiperOs::Linux => PlayCommand {
            program: "paplay".into(),
            args: vec!["{path}".into()],
        },
        PiperOs::Windows => PlayCommand {
            program: "powershell".into(),
            args: vec![
                "-NoProfile".into(),
                "-Command".into(),
                "(New-Object Media.SoundPlayer '{path}').PlaySync()".into(),
            ],
        },
    }
}

/// Spawn playback of `wav_path` using the platform
/// default (or `tts.play_command` override).  On Linux,
/// falls back from `paplay` to `aplay` when paplay isn't
/// on PATH.  Returns the spawned `Child` so the engine
/// can track + stop it.
pub(crate) fn spawn_playback(
    custom: Option<&str>,
    wav_path: &Path,
    platform: Platform,
) -> Result<Child, PiperUnavailable> {
    let mut command = select_play_command(custom, platform.os);
    // Linux fallback: paplay → aplay when paplay isn't
    // present.  Only kicks in for the default command;
    // a custom command is taken at face value.
    if custom.is_none()
        && matches!(platform.os, PiperOs::Linux)
        && command.program == "paplay"
        && which(&command.program).is_none()
    {
        if which("aplay").is_some() {
            command.program = "aplay".into();
        }
    }
    let path_str = wav_path.to_string_lossy().to_string();
    let resolved_args: Vec<String> = command
        .args
        .iter()
        .map(|a| a.replace("{path}", &path_str))
        .collect();
    let mut cmd = Command::new(&command.program);
    cmd.args(&resolved_args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.spawn().map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "spawn playback `{} {:?}`: {e}",
            command.program, resolved_args,
        ))
    })
}

/// Resolve a synthesis WAV staging path inside
/// `voices_dir` for the given voice.  Suffix includes a
/// unique nonce so concurrent speak() calls don't
/// trample each other.
pub(crate) fn synth_wav_path(voices_dir: &Path, voice_key: &str) -> PathBuf {
    // Use process-time nanos as a cheap unique nonce.
    // Collisions only matter if two threads in the same
    // process invoke speak() at the same nanosecond,
    // which is impossible in the single-threaded TUI
    // loop.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    voices_dir.join(format!(".synth-{voice_key}-{nonce}.wav"))
}

fn which(needle: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(needle);
        if std::fs::metadata(&candidate)
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WPM → length_scale ────────────────────────────

    #[test]
    fn wpm_180_maps_to_unity() {
        assert!((wpm_to_length_scale(Some(180)) - 1.0).abs() < 0.001);
    }

    #[test]
    fn wpm_above_180_yields_faster_length_scale() {
        // 270 wpm → 180/270 = 0.667
        let s = wpm_to_length_scale(Some(270));
        assert!(s < 1.0);
        assert!(s > 0.5);
    }

    #[test]
    fn wpm_below_180_yields_slower_length_scale() {
        // 120 wpm → 180/120 = 1.5
        let s = wpm_to_length_scale(Some(120));
        assert!(s > 1.0);
        assert!(s < 2.0);
    }

    #[test]
    fn wpm_extreme_low_clamped_to_2() {
        let s = wpm_to_length_scale(Some(50));
        assert!((s - 2.0).abs() < 0.001);
    }

    #[test]
    fn wpm_extreme_high_clamped_to_half() {
        let s = wpm_to_length_scale(Some(1000));
        assert!((s - 0.5).abs() < 0.001);
    }

    #[test]
    fn wpm_none_defaults_to_unity() {
        assert!((wpm_to_length_scale(None) - 1.0).abs() < 0.001);
    }

    #[test]
    fn wpm_zero_defaults_to_unity() {
        // 0 wpm would divide by zero; we treat it as
        // "no rate specified".
        assert!((wpm_to_length_scale(Some(0)) - 1.0).abs() < 0.001);
    }

    // ── select_play_command ───────────────────────────

    #[test]
    fn play_command_macos_default() {
        let cmd = select_play_command(None, PiperOs::Darwin);
        assert_eq!(cmd.program, "afplay");
        assert_eq!(cmd.args, vec!["{path}".to_string()]);
    }

    #[test]
    fn play_command_linux_default() {
        let cmd = select_play_command(None, PiperOs::Linux);
        assert_eq!(cmd.program, "paplay");
    }

    #[test]
    fn play_command_windows_default() {
        let cmd = select_play_command(None, PiperOs::Windows);
        assert_eq!(cmd.program, "powershell");
        // Must include the SoundPlayer line as one of
        // the args so the spawn substitution can find
        // {path}.
        let joined = cmd.args.join(" ");
        assert!(joined.contains("Media.SoundPlayer"));
        assert!(joined.contains("{path}"));
    }

    #[test]
    fn play_command_custom_overrides_everything() {
        let cmd = select_play_command(
            Some("mpv --no-video {path}"),
            PiperOs::Darwin,
        );
        assert_eq!(cmd.program, "mpv");
        assert_eq!(
            cmd.args,
            vec!["--no-video".to_string(), "{path}".to_string()],
        );
    }

    #[test]
    fn play_command_custom_handles_no_path_placeholder() {
        // A custom command that doesn't include {path}
        // is taken as-is; spawn_playback will then run
        // it without a path arg.  Edge case — user
        // responsibility, but we don't crash.
        let cmd = select_play_command(Some("/bin/true"), PiperOs::Linux);
        assert_eq!(cmd.program, "/bin/true");
        assert!(cmd.args.is_empty());
    }

    // ── synth_wav_path ────────────────────────────────

    #[test]
    fn synth_wav_path_lives_inside_voices_dir() {
        let voices_dir = Path::new("/voices");
        let p = synth_wav_path(voices_dir, "en_US-lessac-medium");
        assert!(p.starts_with(voices_dir));
        assert!(p.to_string_lossy().contains("en_US-lessac-medium"));
        assert!(p.to_string_lossy().ends_with(".wav"));
    }

    #[test]
    fn synth_wav_paths_are_unique_per_call() {
        let voices_dir = Path::new("/voices");
        let a = synth_wav_path(voices_dir, "v");
        // Sleep a nanosecond to guarantee a different
        // timestamp; on some platforms two same-nanosec
        // calls would collide.
        std::thread::sleep(Duration::from_nanos(1));
        let b = synth_wav_path(voices_dir, "v");
        assert_ne!(a, b);
    }

    // ── synth_to_wav (subprocess) ─────────────────────

    /// Write a tiny shell-script "piper" stand-in that
    /// accepts --model + --output_file + --length_scale,
    /// consumes stdin, and writes a 44-byte RIFF/WAVE
    /// header to the output path.  Lets us exercise the
    /// orchestration without a real Piper binary.
    fn make_fake_piper(dir: &Path) -> PathBuf {
        let path = dir.join("fake-piper");
        let script = r#"#!/bin/sh
OUT=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output_file) shift; OUT="$1" ;;
        --model|--length_scale) shift ;;
    esac
    shift
done
cat > /dev/null
printf 'RIFF$\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x00\x00\x00\x00' > "$OUT"
"#;
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    fn make_fake_failing_piper(dir: &Path) -> PathBuf {
        let path = dir.join("failing-piper");
        let script = r#"#!/bin/sh
cat > /dev/null
echo "fake piper failure" >&2
exit 7
"#;
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    fn make_voice_files(dir: &Path) -> VoiceFiles {
        let onnx = dir.join("v.onnx");
        let onnx_json = dir.join("v.onnx.json");
        std::fs::write(&onnx, b"fake-model").unwrap();
        std::fs::write(&onnx_json, b"{}").unwrap();
        VoiceFiles { onnx, onnx_json }
    }

    #[cfg(unix)]
    #[test]
    fn synth_writes_wav_with_fake_piper() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = make_fake_piper(tmp.path());
        let voice = make_voice_files(tmp.path());
        let dest = tmp.path().join("out.wav");
        let bytes = synth_to_wav(
            &binary,
            &voice,
            "hello world",
            Some(180),
            &dest,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(bytes > 0);
        let written = std::fs::read(&dest).unwrap();
        assert!(written.starts_with(b"RIFF"), "expected RIFF header");
        assert!(written.windows(4).any(|w| w == b"WAVE"));
    }

    #[cfg(unix)]
    #[test]
    fn synth_surfaces_subprocess_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = make_fake_failing_piper(tmp.path());
        let voice = make_voice_files(tmp.path());
        let dest = tmp.path().join("out.wav");
        let err = synth_to_wav(
            &binary,
            &voice,
            "hello",
            None,
            &dest,
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
        let msg = err.to_user_message();
        assert!(msg.contains("7") || msg.contains("piper"), "got: {msg}");
        // Failed synth must clean up the dest so we
        // don't leave a partial WAV.
        assert!(!dest.exists(), "expected dest cleaned on failure");
    }

    #[cfg(unix)]
    #[test]
    fn synth_errors_when_binary_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let voice = make_voice_files(tmp.path());
        let dest = tmp.path().join("out.wav");
        let err = synth_to_wav(
            Path::new("/nowhere/does-not-exist-piper"),
            &voice,
            "x",
            None,
            &dest,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }

    // ── spawn_playback ────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn spawn_playback_substitutes_path_placeholder() {
        // Use /bin/cp as a "playback" stand-in so we
        // can verify {path} was substituted.
        let tmp = tempfile::tempdir().unwrap();
        let wav = tmp.path().join("test.wav");
        std::fs::write(&wav, b"X").unwrap();
        let out = tmp.path().join("copy.wav");
        let custom =
            format!("/bin/cp {{path}} {}", out.to_string_lossy());
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let mut child =
            spawn_playback(Some(&custom), &wav, plat).unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
        assert!(out.exists());
        assert_eq!(std::fs::read(&out).unwrap(), b"X");
    }
}

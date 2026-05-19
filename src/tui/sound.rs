//! Typewriter-style sound effects driven by rodio. Two events fire
//! sounds: pressing Enter in the editor ("end of line, hit Enter") and
//! the editor pane losing focus ("remove page from machine"). The
//! waveforms are synthesised at runtime — no audio assets to ship — so
//! the binary stays self-contained.
//!
//! Initialisation is best-effort: a host without an audio device (CI,
//! a remote SSH session without sound forwarding) yields `None` from
//! `SoundPlayer::try_new` and every play call becomes a silent no-op.

use rodio::buffer::SamplesBuffer;
use rodio::source::Source;
use rodio::{OutputStream, OutputStreamHandle};

const SAMPLE_RATE: u32 = 44_100;

pub struct SoundPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    pub enabled: bool,
    pub volume: f32,
}

impl SoundPlayer {
    /// Try to grab the default output device. Returns `None` if rodio
    /// can't find one — callers treat that as "audio is unavailable on
    /// this host" and silently skip playback. Volume is clamped to
    /// `[0.0, 1.0]` so a misconfigured HJSON can't blow the speakers.
    pub fn try_new(enabled: bool, volume: f32) -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self {
            _stream: stream,
            handle,
            enabled,
            volume: volume.clamp(0.0, 1.0),
        })
    }

    pub fn play_enter(&self) {
        if !self.enabled {
            return;
        }
        let samples = synth_enter_click();
        self.play(samples);
    }

    pub fn play_focus_out(&self) {
        if !self.enabled {
            return;
        }
        let samples = synth_focus_out_clatter();
        self.play(samples);
    }

    fn play(&self, samples: Vec<f32>) {
        let buf = SamplesBuffer::new(1, SAMPLE_RATE, samples).amplify(self.volume);
        // play_raw consumes the source and plays it on a fresh sink
        // managed internally — fine for short one-shot SFX. Errors here
        // (e.g. transient device loss) are silently swallowed since a
        // missed typewriter click isn't worth surfacing.
        let _ = self.handle.play_raw(buf.convert_samples());
    }
}

/// A short "thock" — single low-mid frequency burst with rapid
/// exponential decay and a noise overlay so it doesn't sound like a
/// pure sine wave. ~80 ms total.
fn synth_enter_click() -> Vec<f32> {
    let duration_secs = 0.08;
    let n = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let mut out = Vec::with_capacity(n);
    let freq = 140.0_f32;
    let mut rng = LcgRand::new(0xC1AC_1234);
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let envelope = (-t * 55.0).exp();
        let tone = (t * freq * std::f32::consts::TAU).sin();
        let noise = (rng.next_unit() - 0.5) * 1.6;
        let v = (tone * 0.7 + noise * 0.5) * envelope * 0.55;
        out.push(v);
    }
    out
}

/// "Remove page from machine" — three quick clicks at descending
/// pitches, simulating the carriage release and paper rolling out.
/// ~280 ms total.
fn synth_focus_out_clatter() -> Vec<f32> {
    let mut out = Vec::new();
    // Three clicks at 180 / 130 / 90 Hz with 70 ms gaps.
    let click_specs = [(180.0_f32, 0.07), (130.0_f32, 0.08), (90.0_f32, 0.10)];
    let gap_secs = 0.02;
    let gap_samples = (SAMPLE_RATE as f32 * gap_secs) as usize;
    let mut rng = LcgRand::new(0xF0CB_5A75);
    for (freq, dur) in click_specs {
        let n = (SAMPLE_RATE as f32 * dur) as usize;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let envelope = (-t * 35.0).exp();
            let tone = (t * freq * std::f32::consts::TAU).sin();
            let noise = (rng.next_unit() - 0.5) * 1.4;
            let v = (tone * 0.55 + noise * 0.55) * envelope * 0.5;
            out.push(v);
        }
        for _ in 0..gap_samples {
            out.push(0.0);
        }
    }
    out
}

/// Tiny LCG so we don't take a `rand` dep just for two pinches of
/// white noise. Quality is fine for "make this sample sound less
/// like a pure sine."
struct LcgRand {
    state: u64,
}

impl LcgRand {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_u32(&mut self) -> u32 {
        // Numerical Recipes constants.
        self.state = self
            .state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.state >> 16) as u32
    }
    fn next_unit(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
}

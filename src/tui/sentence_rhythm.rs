//! 1.2.9+ — sentence-rhythm gauge.
//!
//! Splits the open paragraph into sentences (hand-
//! rolled walker with abbreviation suppression),
//! tokenises each one with `unicode-segmentation`
//! (consistent with the other word-counting code in
//! the project), and reports:
//!
//!   * N sentences
//!   * Mean word count
//!   * Standard deviation
//!   * Coefficient of variation (CV = stdev / mean)
//!   * A discrete `RhythmVerdict` mapped from the CV
//!   * The top-N shortest + longest sentences as
//!     outlier callouts
//!
//! Why CV (not stdev)?  CV normalises stdev by the
//! mean, so it's comparable across passages of
//! different overall pace.  A 10-word-mean passage
//! with stdev 5 reads similarly to a 20-word-mean
//! passage with stdev 10 — both have CV ~ 0.5.  Raw
//! stdev would mislead toward the long-sentence
//! passage being "more varied" when it's actually
//! the same rhythm at twice the tempo.
//!
//! Sentence segmentation is intentionally simple:
//! split on `.`/`!`/`?` followed by whitespace (or
//! end-of-text), tolerating trailing closing
//! quotes and parens, and suppressing splits inside
//! a configured list of common abbreviations
//! (Mr./Mrs./Dr./e.g./i.e./Ph.D./…).  Good enough
//! for literary text; deliberately not perfect — the
//! goal is a rhythm gauge, not a parser.

use unicode_segmentation::UnicodeSegmentation;

/// Discrete rhythm verdict.  Mapped from the
/// coefficient of variation; thresholds chosen
/// against Gary Provost's well-known "this sentence
/// has five words…" parable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhythmVerdict {
    /// Fewer than 3 sentences — can't meaningfully
    /// judge rhythm.  No-op-ish verdict.
    TooShort,
    /// CV < 0.25 — all sentences roughly the same
    /// length.  The Provost trap: "Listen to what is
    /// happening.  The writing is getting boring."
    Monotone,
    /// 0.25 ≤ CV < 0.45 — modest variation, slightly
    /// flat but readable.
    Steady,
    /// 0.45 ≤ CV < 0.80 — strong variation, good
    /// prose rhythm.  Most literary writing lives
    /// here.
    Varied,
    /// CV ≥ 0.80 — extreme variation (short
    /// fragments + long sentences mixed).
    /// Sometimes intentional (Hemingway, Cormac
    /// McCarthy); sometimes accidental.  Worth a
    /// look.
    Choppy,
}

impl RhythmVerdict {
    /// Map a CV value + sentence count to a verdict.
    pub fn from(cv: f64, n_sentences: usize) -> Self {
        if n_sentences < 3 {
            return Self::TooShort;
        }
        if cv < 0.25 {
            Self::Monotone
        } else if cv < 0.45 {
            Self::Steady
        } else if cv < 0.80 {
            Self::Varied
        } else {
            Self::Choppy
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TooShort => "TOO SHORT TO JUDGE",
            Self::Monotone => "MONOTONE",
            Self::Steady => "STEADY",
            Self::Varied => "VARIED",
            Self::Choppy => "CHOPPY",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Self::TooShort => "need at least 3 sentences to judge rhythm",
            Self::Monotone =>
                "sentences too uniform — drones · break it with a short one",
            Self::Steady =>
                "modest variation · workable but can sing louder",
            Self::Varied =>
                "strong variation · good prose rhythm",
            Self::Choppy =>
                "extreme variation — fragments + long sentences mixed",
        }
    }
}

/// One sentence + the metadata the modal needs to
/// surface it (line number, word count, short
/// preview).  Char-indexed; the row is the 1-based
/// editor row where the sentence STARTED.
#[derive(Debug, Clone)]
pub struct SentenceSample {
    /// 1-based row where the sentence opens.
    pub line_no: usize,
    /// Word count via UAX-#29 `unicode_words`.
    pub word_count: usize,
    /// First ~64 chars of the sentence for the
    /// outlier callout.
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct RhythmStats {
    /// Word count per sentence, paragraph order
    /// preserved.  Length = N.
    pub lengths: Vec<usize>,
    /// Mean word count.  Zero when N = 0.
    pub mean: f64,
    /// Population standard deviation.  Zero when N
    /// ≤ 1.
    pub stdev: f64,
    /// Coefficient of variation (stdev / mean).
    /// Zero when mean = 0.
    pub cv: f64,
    pub min: usize,
    pub max: usize,
    pub verdict: RhythmVerdict,
    /// Every sentence in paragraph order — used by
    /// the modal's per-sentence bar list.
    pub samples: Vec<SentenceSample>,
    /// Three shortest sentences, ascending by word
    /// count.  Cached so the renderer doesn't re-
    /// sort.
    pub shortest: Vec<SentenceSample>,
    /// Three longest sentences, descending by word
    /// count.
    pub longest: Vec<SentenceSample>,
}

impl RhythmStats {
    pub fn empty() -> Self {
        Self {
            lengths: Vec::new(),
            mean: 0.0,
            stdev: 0.0,
            cv: 0.0,
            min: 0,
            max: 0,
            verdict: RhythmVerdict::TooShort,
            samples: Vec::new(),
            shortest: Vec::new(),
            longest: Vec::new(),
        }
    }
}

/// Build a rhythm analysis from a paragraph's source
/// rows.  `lines` is the editor's row view (one
/// String per row); the analyser flattens them with
/// '\n' separators and then re-splits into sentences.
/// Empty input yields `RhythmStats::empty()`.
pub fn analyse(lines: &[String]) -> RhythmStats {
    if lines.is_empty() {
        return RhythmStats::empty();
    }
    // Flatten with '\n' so cross-row sentence
    // boundaries still split.  We track each
    // sentence's *starting* row by maintaining a
    // row-cursor through the flattened text.
    let mut text = String::new();
    let mut row_starts: Vec<usize> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        row_starts.push(text.chars().count());
        text.push_str(line);
        if i + 1 < lines.len() {
            text.push('\n');
        }
    }
    let sentences = split_sentences(&text);
    if sentences.is_empty() {
        return RhythmStats::empty();
    }
    let mut samples: Vec<SentenceSample> = Vec::with_capacity(sentences.len());
    for sent in &sentences {
        let word_count = sent.text.unicode_words().count();
        if word_count == 0 {
            continue;
        }
        // Map char_start back to a 1-based row by
        // walking row_starts.
        let row = row_for_char_index(&row_starts, sent.char_start);
        let preview: String = sent.text.chars().take(64).collect();
        samples.push(SentenceSample {
            line_no: row + 1,
            word_count,
            preview,
        });
    }
    if samples.is_empty() {
        return RhythmStats::empty();
    }
    let lengths: Vec<usize> = samples.iter().map(|s| s.word_count).collect();
    let n = lengths.len();
    let sum: usize = lengths.iter().sum();
    let mean = sum as f64 / n as f64;
    let variance: f64 = if n > 0 {
        lengths
            .iter()
            .map(|l| {
                let d = *l as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / n as f64
    } else {
        0.0
    };
    let stdev = variance.sqrt();
    let cv = if mean > 0.0 { stdev / mean } else { 0.0 };
    let min = *lengths.iter().min().unwrap_or(&0);
    let max = *lengths.iter().max().unwrap_or(&0);
    let verdict = RhythmVerdict::from(cv, n);

    let mut shortest: Vec<SentenceSample> = samples.clone();
    shortest.sort_by_key(|s| s.word_count);
    shortest.truncate(3);
    let mut longest: Vec<SentenceSample> = samples.clone();
    longest.sort_by(|a, b| b.word_count.cmp(&a.word_count));
    longest.truncate(3);

    RhythmStats {
        lengths,
        mean,
        stdev,
        cv,
        min,
        max,
        verdict,
        samples,
        shortest,
        longest,
    }
}

#[derive(Debug, Clone)]
struct RawSentence {
    /// Character index in the flattened text where
    /// the sentence starts.
    char_start: usize,
    /// Sentence text, trimmed.
    text: String,
}

/// Split prose into sentences.  Splits on `.`/`!`/`?`
/// when:
///   1. Optionally followed by closing quotes /
///      parens / additional terminators (consumed
///      greedily so `?!"` ends one sentence).
///   2. Then followed by whitespace OR end-of-text.
///   3. The trailing word isn't a known abbreviation
///      (Mr., Mrs., Dr., e.g., i.e., Ph.D., …).
fn split_sentences(text: &str) -> Vec<RawSentence> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences: Vec<RawSentence> = Vec::new();
    let mut buf = String::new();
    let mut start_char: usize = 0;
    let mut current_char: usize = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Strip leading whitespace from the buffer's
        // start_char so the recorded position points
        // at real text.
        if buf.is_empty() && c.is_whitespace() {
            i += 1;
            current_char += 1;
            start_char = current_char;
            continue;
        }
        buf.push(c);
        current_char += 1;
        if matches!(c, '.' | '!' | '?') {
            // Consume trailing closing quotes / parens
            // / extra terminators.
            let block_start = i;
            let mut j = i + 1;
            while j < chars.len()
                && matches!(
                    chars[j],
                    '.' | '!'
                        | '?'
                        | '"'
                        | '\''
                        | '”'
                        | '’'
                        | ')'
                        | ']'
                )
            {
                buf.push(chars[j]);
                j += 1;
                current_char += 1;
            }
            let followed_by_space = j >= chars.len() || chars[j].is_whitespace();
            // Ellipsis (`...` or longer) is a mid-
            // sentence pause, not a terminator.
            // Detect by looking at the consumed
            // terminator block: if it's purely dots
            // and length ≥ 2, suppress the split.
            let block: &[char] = &chars[block_start..j];
            let is_ellipsis =
                block.len() >= 2 && block.iter().all(|c| *c == '.');
            if followed_by_space
                && !is_ellipsis
                && !ends_with_abbreviation(buf.trim()) {
                let trimmed = buf.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(RawSentence {
                        char_start: start_char,
                        text: trimmed,
                    });
                }
                buf.clear();
                // Advance past the terminator block.
                i = j;
                start_char = current_char;
                continue;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    // Tail (unterminated final sentence — common in
    // literary fragments).
    let tail = buf.trim().to_string();
    if !tail.is_empty() {
        sentences.push(RawSentence {
            char_start: start_char,
            text: tail,
        });
    }
    sentences
}

/// Common English abbreviations whose terminal `.`
/// must NOT count as a sentence boundary.  Case-
/// insensitive match against the trailing
/// whitespace-delimited word of the current buffer.
const ABBREVIATIONS: &[&str] = &[
    "Mr.", "Mrs.", "Ms.", "Dr.", "Sr.", "Jr.", "St.", "Mt.", "Fr.", "Rev.",
    "Prof.", "Gen.", "Col.", "Maj.", "Lt.", "Capt.", "Sgt.", "Hon.", "Pres.",
    "Sen.", "Rep.", "Gov.",
    "Ph.D.", "M.D.", "B.A.", "M.A.", "B.S.", "M.S.", "Esq.",
    "e.g.", "i.e.", "etc.", "vs.", "viz.", "cf.", "et.", "al.",
    "Inc.", "Ltd.", "Co.", "Corp.",
    "No.", "Vol.", "p.", "pp.", "fig.", "ed.", "eds.",
];

fn ends_with_abbreviation(s: &str) -> bool {
    let last = s.split_whitespace().last().unwrap_or("");
    if last.is_empty() {
        return false;
    }
    ABBREVIATIONS.iter().any(|a| last.eq_ignore_ascii_case(a))
}

/// Map a flattened-text char index back to a row
/// index in the editor's row view.  Linear-scan
/// against `row_starts`; cheap for normal paragraph
/// sizes.
fn row_for_char_index(row_starts: &[usize], char_idx: usize) -> usize {
    let mut row = 0;
    for (i, start) in row_starts.iter().enumerate() {
        if *start <= char_idx {
            row = i;
        } else {
            break;
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_input_yields_empty_stats() {
        let r = analyse(&[]);
        assert_eq!(r.lengths.len(), 0);
        assert_eq!(r.verdict, RhythmVerdict::TooShort);
    }

    #[test]
    fn single_sentence_too_short() {
        let r = analyse(&lines("The cat sat on the mat."));
        assert_eq!(r.lengths, vec![6]);
        assert_eq!(r.verdict, RhythmVerdict::TooShort);
    }

    #[test]
    fn three_uniform_sentences_drones() {
        // Three sentences of exactly 5 words → stdev 0
        // → cv 0 → MONOTONE.
        let r = analyse(&lines(
            "I ate the red apple. I drank the cold milk. I read the old book.",
        ));
        assert_eq!(r.lengths, vec![5, 5, 5]);
        assert!((r.stdev - 0.0).abs() < 1e-9);
        assert_eq!(r.verdict, RhythmVerdict::Monotone);
    }

    #[test]
    fn varied_sentences_get_varied_verdict() {
        // Lengths roughly 3, 12, 5, 18, 4 — CV
        // should land in the Varied range.
        let r = analyse(&lines(
            "Bob ran fast. The morning fog clung to the cobblestones as he turned the corner. \
             He coughed. She had not slept in seventy hours and her hands shook against the rusted iron rail. \
             Then silence.",
        ));
        assert!(r.lengths.len() >= 5);
        assert!(matches!(
            r.verdict,
            RhythmVerdict::Varied | RhythmVerdict::Choppy
        ));
    }

    #[test]
    fn abbreviations_dont_split() {
        let r = analyse(&lines(
            "Dr. Smith arrived at noon. Mrs. Hale was already there. \
             They drank tea quietly.",
        ));
        // Without abbrev suppression this would be 5+
        // sentences; we want 3.
        assert_eq!(r.lengths.len(), 3);
    }

    #[test]
    fn ellipsis_and_terminator_combo() {
        // "She thought…" → single sentence ending in
        // ellipsis.  Then the next sentence.
        let r = analyse(&lines("She thought... it was over. He didn't agree."));
        assert_eq!(r.lengths.len(), 2);
    }

    #[test]
    fn closing_quotes_consumed_with_terminator() {
        // `said "Hello."` → one sentence; `said "Hello." Then` → two.
        let r = analyse(&lines(
            "He said \"Hello.\" Then she nodded. Then she walked away.",
        ));
        assert_eq!(r.lengths.len(), 3);
    }

    #[test]
    fn rhythm_verdict_thresholds() {
        // n < 3 → TooShort regardless of CV.
        assert_eq!(RhythmVerdict::from(0.5, 2), RhythmVerdict::TooShort);
        // n ≥ 3 thresholds.
        assert_eq!(RhythmVerdict::from(0.10, 5), RhythmVerdict::Monotone);
        assert_eq!(RhythmVerdict::from(0.30, 5), RhythmVerdict::Steady);
        assert_eq!(RhythmVerdict::from(0.55, 5), RhythmVerdict::Varied);
        assert_eq!(RhythmVerdict::from(1.10, 5), RhythmVerdict::Choppy);
    }

    #[test]
    fn line_no_tracks_starting_row() {
        let r = analyse(&lines(
            "First sentence here.\nSecond starts on row two.\nThird is row three.",
        ));
        assert_eq!(r.samples.len(), 3);
        assert_eq!(r.samples[0].line_no, 1);
        assert_eq!(r.samples[1].line_no, 2);
        assert_eq!(r.samples[2].line_no, 3);
    }

    #[test]
    fn outliers_picked_correctly() {
        let r = analyse(&lines(
            "Tiny. The medium one has six words here. \
             This is by far the very longest sentence in the whole paragraph with many many words.",
        ));
        assert_eq!(r.shortest[0].word_count, 1);
        assert!(r.longest[0].word_count >= 16);
    }
}

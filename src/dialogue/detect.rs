//! DIALOG-1 (D-P1) — convention-aware dialogue span extraction. Three
//! strategies (RFC §4): quote-pair (EN/DE), guillemet + em-dash (FR/RU), and
//! hybrid (ES = all three, additively, deduplicated).
//!
//! This phase extracts the speech spans (boundaries, cleaned text, terminal
//! punctuation). Attribution (who spoke) and tag-verb classification land in
//! D-P2 — except the FR inline tag, which is stripped here because it lives
//! *inside* the speech marks and would otherwise pollute the speech text.

use std::sync::OnceLock;

use regex::Regex;

use super::{AttributionConfidence, DialogueConvention, DialogueSpan, SpanForm};
use crate::prose::ProseLanguage;

/// Detect every dialogue span in one paragraph, ordered left to right with a
/// 0-based `span_index`. Attribution fields are left empty (filled in D-P2);
/// the FR inline tag, when present, is stripped and its verb stashed.
pub(crate) fn detect_spans(
    para_id: &str,
    text: &str,
    convention: DialogueConvention,
    lang: &ProseLanguage,
) -> Vec<DialogueSpan> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw: Vec<RawSpan> = Vec::new();

    match convention {
        DialogueConvention::QuotePair => {
            raw.extend(extract_quote_pairs(&chars, lang));
        }
        DialogueConvention::GuillemetsAndDash => {
            raw.extend(extract_guillemets(&chars));
            raw.extend(extract_em_dash(&chars));
        }
        DialogueConvention::Hybrid => {
            raw.extend(extract_quote_pairs(&chars, lang));
            raw.extend(extract_guillemets(&chars));
            raw.extend(extract_em_dash(&chars));
        }
    }

    // Order by start, then drop spans whose range overlaps an already-kept one
    // (the hybrid detectors are additive and can double-cover a span).
    raw.sort_by_key(|r| r.start);
    let mut kept: Vec<RawSpan> = Vec::new();
    for r in raw {
        if kept.last().is_some_and(|k| r.start < k.end) {
            continue;
        }
        kept.push(r);
    }

    let is_fr = matches!(convention, DialogueConvention::GuillemetsAndDash)
        && matches!(lang, ProseLanguage::Fr);
    let is_es = matches!(lang, ProseLanguage::Es);

    kept.into_iter()
        .enumerate()
        .map(|(i, r)| {
            let (speech, inline_tag) = if is_fr {
                strip_fr_inline_tag(&r.inner)
            } else {
                (r.inner.trim().to_string(), None)
            };
            let word_count = speech.split_whitespace().count() as u32;
            DialogueSpan {
                para_id: para_id.to_string(),
                span_index: i as u32,
                form: r.form,
                char_start: r.start,
                char_end: r.end,
                ends_question: ends_with_q(&speech, is_es),
                ends_exclamation: ends_with_excl(&speech, is_es),
                speech_text: speech,
                word_count,
                attribution_name: None,
                attribution_conf: AttributionConfidence::None,
                has_attribution_signal: false,
                tag_verb: inline_tag,
                tag_verb_class: None,
            }
        })
        .collect()
}

/// A detected span before it becomes a `DialogueSpan`: char-index range +
/// inner text + detection form.
struct RawSpan {
    start: usize,
    end: usize,
    inner: String,
    form: SpanForm,
}

/// Quote-pair openers→closers for a language. Straight `"` is handled
/// separately (toggle) since open == close.
fn quote_pairs_for(lang: &ProseLanguage) -> &'static [(char, char)] {
    match lang {
        ProseLanguage::De => &[('„', '“'), ('»', '«')],
        // EN, ES, and the Other fallback: curly double quotes.
        _ => &[('“', '”')],
    }
}

/// Convention A — speech between paired quotation marks. Inner marks of a
/// *different* kind are ignored (the outer pair wins). Unclosed openers emit a
/// span to end-of-paragraph (legitimate for speech split across paragraphs).
fn extract_quote_pairs(chars: &[char], lang: &ProseLanguage) -> Vec<RawSpan> {
    let pairs = quote_pairs_for(lang);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Straight double-quote: toggle to the next straight double-quote.
        if c == '"' {
            if let Some(j) = (i + 1..chars.len()).find(|&j| chars[j] == '"') {
                out.push(raw(chars, i, j, SpanForm::QuotePair));
                i = j + 1;
                continue;
            }
            out.push(raw_unclosed(chars, i, SpanForm::QuotePair));
            break;
        }
        if let Some(&(_, close)) = pairs.iter().find(|&&(open, _)| open == c) {
            if let Some(j) = (i + 1..chars.len()).find(|&j| chars[j] == close) {
                out.push(raw(chars, i, j, SpanForm::QuotePair));
                i = j + 1;
                continue;
            }
            out.push(raw_unclosed(chars, i, SpanForm::QuotePair));
            break;
        }
        i += 1;
    }
    out
}

/// Convention B (guillemet sub-convention) — `«…»`. FR/RU open with `«` and
/// close with `»` (the reverse of the DE book style, which is quote-pair).
fn extract_guillemets(chars: &[char]) -> Vec<RawSpan> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '«' {
            if let Some(j) = (i + 1..chars.len()).find(|&j| chars[j] == '»') {
                out.push(raw(chars, i, j, SpanForm::Guillemet));
                i = j + 1;
                continue;
            }
            out.push(raw_unclosed(chars, i, SpanForm::Guillemet));
            break;
        }
        i += 1;
    }
    out
}

/// Convention B (em-dash sub-convention) — a paragraph (or, after a hard line
/// break, a line) that begins with an em-dash opener `— ` is a speaker turn.
/// The speech runs to end-of-paragraph; the inline incise is stripped later
/// (FR) by `strip_fr_inline_tag`.
fn extract_em_dash(chars: &[char]) -> Vec<RawSpan> {
    let mut out = Vec::new();
    // Consider the paragraph and each post-newline line.
    let mut line_start = 0usize;
    let mut i = 0usize;
    while i <= chars.len() {
        let at_end = i == chars.len();
        if at_end || chars[i] == '\n' {
            // Emit if this line opened with an em-dash.
            let seg = &chars[line_start..i];
            let first = seg.iter().position(|c| !c.is_whitespace());
            if let Some(p) = first {
                if seg[p] == '—' || seg[p] == '–' {
                    // Speech = everything after the dash (+ following space).
                    let mut s = p + 1;
                    while s < seg.len() && (seg[s] == ' ' || seg[s] == '\u{00A0}') {
                        s += 1;
                    }
                    let inner: String = seg[s..].iter().collect();
                    if !inner.trim().is_empty() {
                        out.push(RawSpan {
                            start: line_start + p,
                            end: i,
                            inner,
                            form: SpanForm::EmDash,
                        });
                    }
                }
            }
            line_start = i + 1;
            i += 1;
            continue;
        }
        i += 1;
    }
    out
}

fn raw(chars: &[char], open_idx: usize, close_idx: usize, form: SpanForm) -> RawSpan {
    RawSpan {
        start: open_idx,
        end: close_idx + 1,
        inner: chars[open_idx + 1..close_idx].iter().collect(),
        form,
    }
}

fn raw_unclosed(chars: &[char], open_idx: usize, form: SpanForm) -> RawSpan {
    RawSpan {
        start: open_idx,
        end: chars.len(),
        inner: chars[open_idx + 1..].iter().collect(),
        form,
    }
}

/// Strip a French inline attribution tag (the *incise*) from a speech span:
/// `, dit-il,` / `, demanda-t-elle,` etc. Returns the cleaned speech and the
/// tag verb if one was found. Handles the euphonic `-t-` infix.
fn strip_fr_inline_tag(inner: &str) -> (String, Option<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // , verb (-t)? - pronoun ,
        Regex::new(r",\s*([\p{L}]+)(?:-t)?-(?:il|elle|ils|elles|on|je|tu|nous|vous)\s*,")
            .expect("fr inline tag regex")
    });
    if let Some(caps) = re.captures(inner) {
        let verb = caps.get(1).map(|m| m.as_str().to_string());
        let stripped = re.replace(inner, " ").to_string();
        // Collapse the double space left by the splice and trim.
        let cleaned = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        return (cleaned, verb);
    }
    (inner.trim().to_string(), None)
}

/// Terminal `?` — or, for ES, the presence of an inverted `¿` opener.
fn ends_with_q(speech: &str, is_es: bool) -> bool {
    let t = speech.trim_end();
    t.ends_with('?') || (is_es && speech.contains('¿'))
}

/// Terminal `!` — or, for ES, the presence of an inverted `¡` opener.
fn ends_with_excl(speech: &str, is_es: bool) -> bool {
    let t = speech.trim_end();
    t.ends_with('!') || (is_es && speech.contains('¡'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convention(lang: &ProseLanguage) -> DialogueConvention {
        super::super::dialogue_convention(lang)
    }

    fn detect(text: &str, lang: ProseLanguage) -> Vec<DialogueSpan> {
        detect_spans("p", text, convention(&lang), &lang)
    }

    #[test]
    fn en_curly_and_straight_quotes() {
        let s = detect("\u{201C}Hello,\u{201D} she said. \"Again?\"", ProseLanguage::En);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].speech_text, "Hello,");
        assert_eq!(s[0].form, SpanForm::QuotePair);
        assert!(s[1].ends_question);
        assert_eq!(s[1].speech_text, "Again?");
    }

    #[test]
    fn en_unclosed_quote_is_captured_not_flagged() {
        let s = detect("\u{201C}This runs on and on", ProseLanguage::En);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].speech_text, "This runs on and on");
    }

    #[test]
    fn en_word_count_and_exclamation() {
        let s = detect("\u{201C}Get out now!\u{201D}", ProseLanguage::En);
        assert_eq!(s[0].word_count, 3);
        assert!(s[0].ends_exclamation);
    }

    #[test]
    fn de_low_quotes_and_book_guillemets() {
        let s = detect("\u{201E}Guten Tag\u{201C}", ProseLanguage::De);
        assert_eq!(s[0].speech_text, "Guten Tag");
        let s2 = detect("\u{00BB}Komm her\u{00AB}", ProseLanguage::De);
        assert_eq!(s2[0].speech_text, "Komm her");
    }

    #[test]
    fn fr_guillemets_with_inline_tag_stripped() {
        let s = detect("\u{00AB} Bonjour, dit-il, comment\u{00A0}? \u{00BB}", ProseLanguage::Fr);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].form, SpanForm::Guillemet);
        // The "dit-il" incise is removed from the speech, captured as the tag.
        assert!(!s[0].speech_text.contains("dit-il"), "got: {}", s[0].speech_text);
        assert_eq!(s[0].tag_verb.as_deref(), Some("dit"));
    }

    #[test]
    fn fr_euphonic_t_infix_tag() {
        let (clean, verb) = strip_fr_inline_tag("Vraiment, demanda-t-elle, vraiment ?");
        assert_eq!(verb.as_deref(), Some("demanda"));
        assert!(!clean.contains("demanda-t-elle"));
    }

    #[test]
    fn fr_em_dash_opener() {
        let s = detect("\u{2014} Bonjour, comment allez-vous ?", ProseLanguage::Fr);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].form, SpanForm::EmDash);
        assert!(s[0].speech_text.starts_with("Bonjour"));
        assert!(s[0].ends_question);
    }

    #[test]
    fn ru_guillemets_and_dash_both_in_paragraph() {
        let s = detect("\u{2014} Привет\n\u{00AB}Пока\u{00BB}", ProseLanguage::Ru);
        assert_eq!(s.len(), 2);
        let forms: Vec<SpanForm> = s.iter().map(|x| x.form).collect();
        assert!(forms.contains(&SpanForm::EmDash));
        assert!(forms.contains(&SpanForm::Guillemet));
    }

    #[test]
    fn es_hybrid_all_three_forms_dedup() {
        // Quote-pair, guillemet, and an em-dash line — all detected, no overlap.
        let s = detect(
            "\u{201C}Hola\u{201D}\n\u{00AB}Adiós\u{00BB}\n\u{2014} Vamos",
            ProseLanguage::Es,
        );
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn es_inverted_question_and_exclamation() {
        let s = detect("\u{201C}\u{00BF}Qué pasa?\u{201D}", ProseLanguage::Es);
        assert!(s[0].ends_question);
        let s2 = detect("\u{201C}\u{00A1}Cuidado!\u{201D}", ProseLanguage::Es);
        assert!(s2[0].ends_exclamation);
    }
}

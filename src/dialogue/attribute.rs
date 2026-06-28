//! DIALOG-1 (D-P2) — attribution cascade. For each detected span, decide who
//! spoke it and how confident we are (RFC §6.2):
//!
//! 1. **Certain** — a named character within the name window *and* a dialogue
//!    verb within the verb window.
//! 2. **Inferred** — an action beat naming a character within the beat window,
//!    or a pronoun within the verb window with a named character carried over
//!    from the previous paragraph.
//! 3. **None** — no signal.
//!
//! A separate, more lenient flag (`has_attribution_signal`) drives the
//! zero-attribution *finding* (§5.1): any of name ≤60 tok / verb ≤15 / action
//! beat ≤30 / inline tag clears it, even when the span is not confidently
//! attributable to a specific character.
//!
//! Windows are token distances from the span boundary, defaulting to 60/15/30;
//! the pipeline threads config overrides in D-P4.

use super::{
    AttributionConfidence, DialogueLexicon, DialogueSpan, TagVerbClass, classify_tag_verb,
};
use crate::prose::ProseLanguage;

/// Token-distance windows for the attribution search (RFC §5.1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AttributionWindows {
    pub name: usize,
    pub verb: usize,
    pub beat: usize,
}

impl Default for AttributionWindows {
    fn default() -> Self {
        AttributionWindows { name: 60, verb: 15, beat: 30 }
    }
}

/// Attribute every span in a paragraph in place. `names` is the Characters-book
/// name set (any case); `prev_named` is the character most recently established
/// as speaking in the previous paragraph, for pronoun inference.
pub(crate) fn attribute_spans(
    spans: &mut [DialogueSpan],
    paragraph: &str,
    names: &[String],
    prev_named: Option<&str>,
    lex: &DialogueLexicon,
    lang: &ProseLanguage,
    win: AttributionWindows,
) {
    let toks = tokenize(paragraph);
    let pronouns = pronouns_for(lang);
    let names_lc: Vec<(String, Vec<String>)> = names
        .iter()
        .map(|n| (n.clone(), n.to_lowercase().split_whitespace().map(str::to_string).collect()))
        .filter(|(_, parts): &(String, Vec<String>)| !parts.is_empty())
        .collect();

    for span in spans.iter_mut() {
        attribute_one(span, &toks, &names_lc, &pronouns, lex, prev_named, win);
    }
}

struct Tok {
    lc: String,
    char_idx: usize,
}

/// Whitespace tokenizer that records each token's starting char index, with
/// surrounding punctuation trimmed from the lowercased form (so `Mara,` matches
/// `mara`).
fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 0usize;
    for (idx, ch) in text.chars().enumerate() {
        if ch.is_whitespace() {
            push_tok(&mut out, &mut cur, cur_start);
        } else {
            if cur.is_empty() {
                cur_start = idx;
            }
            cur.push(ch);
        }
    }
    push_tok(&mut out, &mut cur, cur_start);
    out
}

fn push_tok(out: &mut Vec<Tok>, cur: &mut String, start: usize) {
    if cur.is_empty() {
        return;
    }
    let lc: String = cur
        .to_lowercase()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string();
    if !lc.is_empty() {
        out.push(Tok { lc, char_idx: start });
    }
    cur.clear();
}

#[allow(clippy::too_many_arguments)]
fn attribute_one(
    span: &mut DialogueSpan,
    toks: &[Tok],
    names_lc: &[(String, Vec<String>)],
    pronouns: &[&str],
    lex: &super::DialogueLexicon,
    prev_named: Option<&str>,
    win: AttributionWindows,
) {
    // Token-index range covered by the span's own text (skip it — it's speech).
    let span_first = toks
        .iter()
        .position(|t| t.char_idx >= span.char_start)
        .unwrap_or(toks.len());
    let span_last = toks
        .iter()
        .rposition(|t| t.char_idx < span.char_end)
        .unwrap_or(0);
    let dist = |k: usize| -> Option<usize> {
        if k < span_first {
            Some(span_first - k)
        } else if k > span_last {
            Some(k - span_last)
        } else {
            None // inside the span
        }
    };

    // Nearest dialogue verb within the verb window (outside the span).
    let mut best_verb: Option<(usize, String, TagVerbClass)> = None;
    // Nearest pronoun within the verb window.
    let mut pronoun_near = false;
    for (k, t) in toks.iter().enumerate() {
        let Some(d) = dist(k) else { continue };
        if d <= win.verb {
            if let Some(class) = classify_tag_verb(&t.lc, lex) {
                if best_verb.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                    best_verb = Some((d, t.lc.clone(), class));
                }
            }
            if pronouns.contains(&t.lc.as_str()) {
                pronoun_near = true;
            }
        }
    }

    // Nearest named character within the name window.
    let mut best_name: Option<(usize, String)> = None; // (distance, canonical name)
    for (canonical, parts) in names_lc {
        if let Some((d, _)) = nearest_name_match(toks, parts, &dist) {
            if d <= win.name && best_name.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best_name = Some((d, canonical.clone()));
            }
        }
    }

    // Record the tag verb (a found verb, even if attribution stays None). Keep a
    // FR inline tag already on the span if no nearby verb beat it.
    if let Some((_, verb, class)) = &best_verb {
        span.tag_verb = Some(verb.clone());
        span.tag_verb_class = Some(*class);
    } else if let Some(inline) = span.tag_verb.clone() {
        span.tag_verb_class = classify_tag_verb(&inline, lex);
    }

    // Cascade.
    let name_within_beat = best_name.as_ref().is_some_and(|(d, _)| *d <= win.beat);
    let (conf, name) = if let Some((_, name)) = &best_name {
        if best_verb.is_some() {
            (AttributionConfidence::Certain, Some(name.clone())) // level 1
        } else if name_within_beat {
            (AttributionConfidence::Inferred, Some(name.clone())) // level 3 (action beat)
        } else {
            (AttributionConfidence::Inferred, Some(name.clone())) // name in window, no verb
        }
    } else if pronoun_near {
        match prev_named {
            Some(p) => (AttributionConfidence::Inferred, Some(p.to_string())), // level 2
            None => (AttributionConfidence::None, None),
        }
    } else {
        (AttributionConfidence::None, None)
    };

    span.attribution_conf = conf;
    span.attribution_name = name;
    // Lenient signal for the zero-attribution finding (§5.1).
    span.has_attribution_signal = span.attribution_conf != AttributionConfidence::None
        || span.tag_verb.is_some()
        || best_name.is_some();
}

/// Find the nearest occurrence (token distance) of a multi-token name run in
/// the token stream, outside the span. Returns `(distance, start_token_index)`.
fn nearest_name_match(
    toks: &[Tok],
    parts: &[String],
    dist: &impl Fn(usize) -> Option<usize>,
) -> Option<(usize, usize)> {
    if parts.is_empty() || toks.len() < parts.len() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    for start in 0..=toks.len() - parts.len() {
        let matches = parts
            .iter()
            .enumerate()
            .all(|(j, p)| &toks[start + j].lc == p);
        if !matches {
            continue;
        }
        let Some(d) = dist(start) else { continue };
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, start));
        }
    }
    best
}

/// Third-person subject pronouns per language (attribution-relevant).
fn pronouns_for(lang: &ProseLanguage) -> &'static [&'static str] {
    match lang {
        ProseLanguage::En => &["he", "she", "they"],
        ProseLanguage::Ru => &["он", "она", "они", "оно"],
        ProseLanguage::De => &["er", "sie", "es"],
        ProseLanguage::Fr => &["il", "elle", "ils", "elles", "on"],
        ProseLanguage::Es => &["él", "ella", "ellos", "ellas"],
        ProseLanguage::Other(_) => &["he", "she", "they"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::{detect_spans, dialogue_convention};

    fn run(text: &str, names: &[&str], prev: Option<&str>, lang: ProseLanguage) -> Vec<DialogueSpan> {
        let conv = dialogue_convention(&lang);
        let mut spans = detect_spans("p", text, conv, &lang);
        let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let lex = super::super::verbs::lexicon_for(&lang);
        attribute_spans(&mut spans, text, &names, prev, lex, &lang, AttributionWindows::default());
        spans
    }

    #[test]
    fn certain_named_tag_with_verb() {
        let s = run("\u{201C}Hello,\u{201D} said Mara.", &["Mara", "Aldric"], None, ProseLanguage::En);
        assert_eq!(s[0].attribution_conf, AttributionConfidence::Certain);
        assert_eq!(s[0].attribution_name.as_deref(), Some("Mara"));
        assert_eq!(s[0].tag_verb.as_deref(), Some("said"));
        assert_eq!(s[0].tag_verb_class, Some(TagVerbClass::Neutral));
        assert!(s[0].has_attribution_signal);
    }

    #[test]
    fn said_bookism_verb_class_recorded() {
        let s = run("\u{201C}No,\u{201D} Aldric whispered.", &["Aldric"], None, ProseLanguage::En);
        assert_eq!(s[0].tag_verb_class, Some(TagVerbClass::SaidBookism));
        assert_eq!(s[0].attribution_conf, AttributionConfidence::Certain);
    }

    #[test]
    fn inferred_pronoun_with_prior_named() {
        // No name in this paragraph; a pronoun + a carried-over speaker.
        let s = run("\u{201C}Maybe,\u{201D} he said.", &["Mara"], Some("Mara"), ProseLanguage::En);
        assert_eq!(s[0].attribution_conf, AttributionConfidence::Inferred);
        assert_eq!(s[0].attribution_name.as_deref(), Some("Mara"));
    }

    #[test]
    fn inferred_action_beat_name_no_verb() {
        // A name as an action-beat subject, no dialogue verb.
        let s = run("\u{201C}Stop.\u{201D} Mara raised her hand.", &["Mara"], None, ProseLanguage::En);
        assert_eq!(s[0].attribution_conf, AttributionConfidence::Inferred);
        assert_eq!(s[0].attribution_name.as_deref(), Some("Mara"));
    }

    #[test]
    fn none_when_no_signal() {
        let s = run("\u{201C}Who goes there?\u{201D}", &["Mara"], None, ProseLanguage::En);
        assert_eq!(s[0].attribution_conf, AttributionConfidence::None);
        assert!(!s[0].has_attribution_signal);
    }

    #[test]
    fn name_inside_span_is_not_attribution() {
        // The only "Mara" is *inside* the speech — must not attribute.
        let s = run("\u{201C}Mara, come here.\u{201D}", &["Mara"], None, ProseLanguage::En);
        assert_eq!(s[0].attribution_conf, AttributionConfidence::None);
        assert!(!s[0].has_attribution_signal);
    }

    #[test]
    fn multi_token_name() {
        let s = run("\u{201C}Aye.\u{201D} said Jon Snow.", &["Jon Snow"], None, ProseLanguage::En);
        assert_eq!(s[0].attribution_name.as_deref(), Some("Jon Snow"));
        assert_eq!(s[0].attribution_conf, AttributionConfidence::Certain);
    }
}

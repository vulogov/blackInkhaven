//! 1.3.1 SUBMISSION-1 P3 — submission-package generator metadata.
//!
//! Shared by the `inkhaven submission` CLI (P3.2) and the TUI generator
//! picker (P3.3): each variant maps to a draft title, a `submission-*`
//! slug (the `Submissions`-book paragraph + the prompt-override key), and a
//! built-in system prompt.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionKind {
    Query,
    SynopsisShort,
    SynopsisLong,
    Comps,
    Logline,
}

impl SubmissionKind {
    pub const ALL: [Self; 5] = [
        Self::Query,
        Self::SynopsisShort,
        Self::SynopsisLong,
        Self::Comps,
        Self::Logline,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Query => "submission-query",
            Self::SynopsisShort => "submission-synopsis-short",
            Self::SynopsisLong => "submission-synopsis-long",
            Self::Comps => "submission-comps",
            Self::Logline => "submission-logline",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Query => "Query Letter",
            Self::SynopsisShort => "Synopsis (short)",
            Self::SynopsisLong => "Synopsis (long)",
            Self::Comps => "Comp Titles",
            Self::Logline => "Logline & Pitch",
        }
    }

    pub fn builtin_system(self) -> &'static str {
        match self {
            Self::Query => "You are an expert query-letter writer. Using ONLY the supplied book \
information, draft a one-page query letter: a hook paragraph, a 1–2 paragraph mini-synopsis that \
does NOT reveal the ending, a short bio line (use a [BIO] placeholder), and a professional closing. \
~250–350 words. No markdown, no preamble — just the letter.",
            Self::SynopsisShort => "You are an expert synopsis writer. Using ONLY the supplied book \
information, write a ONE-PAGE synopsis (~500 words) covering the COMPLETE arc INCLUDING the ending \
(a synopsis spoils by design). Present tense, third person, plain prose — no preamble.",
            Self::SynopsisLong => "You are an expert synopsis writer. Using ONLY the supplied book \
information, write a 2–3 page synopsis (~1000–1500 words) covering the COMPLETE arc INCLUDING the \
ending. Present tense, third person; put each major character's name in CAPS on first mention. \
Plain prose, no preamble.",
            Self::Comps => "You are a well-read acquisitions assistant. Suggest 3–5 comparable \
published titles (comps) for this book. For each: 'Title by Author — one line on why it's \
comparable (tone / theme / readership)'. Prefer titles from roughly the last decade in the same \
category. These are SUGGESTIONS from general knowledge, NOT verified market data — do not invent \
sales figures or award claims. No preamble.",
            Self::Logline => "You are a pitch doctor. Using ONLY the supplied book information, write \
a single compelling logline (1–2 sentences) followed by a 2–3 sentence elevator pitch. No \
preamble, no markdown headers.",
        }
    }

    /// The user-prompt body wrapping the digest context.
    pub fn user_prompt(self, context: &str) -> String {
        format!(
            "BOOK INFORMATION:\n{context}\n\nWrite the {} now.",
            self.title().to_lowercase(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_is_complete_and_distinct() {
        let slugs: std::collections::BTreeSet<_> =
            SubmissionKind::ALL.iter().map(|k| k.slug()).collect();
        assert_eq!(slugs.len(), 5, "distinct slugs");
        for k in SubmissionKind::ALL {
            assert!(k.slug().starts_with("submission-"));
            assert!(!k.title().is_empty());
            assert!(k.builtin_system().len() > 40);
            assert!(k.user_prompt("CTX").contains("CTX"));
        }
        assert!(SubmissionKind::SynopsisLong.builtin_system().contains("2–3 page"));
        assert!(SubmissionKind::SynopsisShort.builtin_system().contains("ONE-PAGE"));
    }
}

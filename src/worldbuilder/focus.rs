//! WBLD-1 (WB-P0) — the worldbuilder's focus model and right-pane view enum.
//!
//! Four primary panes cycle with Tab (Facts → World → Query → Right → Facts);
//! Shift+Tab reverses. The confirmation overlay is *sticky* — it owns focus until
//! the author commits or discards, then focus returns to the Query prompt.

/// Which pane currently receives keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    /// Top-left: the Facts book tree.
    FactsPane,
    /// Bottom-left: the World book tree.
    WorldPane,
    /// Bottom: the full-width command / chat input.
    QueryPrompt,
    /// Right: Chat | Research | Map | Ledger.
    RightPane,
    /// Transient `/wfact`-style confirmation; sticky until resolved (WB-P7).
    #[allow(dead_code)]
    ConfirmationOverlay,
}

impl Focus {
    /// Tab: advance to the next primary pane. The overlay is sticky.
    pub(crate) fn next(self) -> Focus {
        match self {
            Focus::FactsPane => Focus::WorldPane,
            Focus::WorldPane => Focus::QueryPrompt,
            Focus::QueryPrompt => Focus::RightPane,
            Focus::RightPane => Focus::FactsPane,
            Focus::ConfirmationOverlay => Focus::ConfirmationOverlay,
        }
    }

    /// Shift+Tab: retreat to the previous primary pane. The overlay is sticky.
    pub(crate) fn prev(self) -> Focus {
        match self {
            Focus::FactsPane => Focus::RightPane,
            Focus::WorldPane => Focus::FactsPane,
            Focus::QueryPrompt => Focus::WorldPane,
            Focus::RightPane => Focus::QueryPrompt,
            Focus::ConfirmationOverlay => Focus::ConfirmationOverlay,
        }
    }
}

/// The view shown in the right pane; cycles with `Ctrl+R`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightPane {
    /// Streaming worldbuilding conversation (default).
    Chat,
    /// Real-world research sub-mode (system prompt shifted).
    Research,
    /// The last rendered map.
    Map,
    /// The magic-ledger editor.
    Ledger,
}

impl RightPane {
    /// `Ctrl+R`: cycle Chat → Research → Map → Ledger → Chat.
    pub(crate) fn next(self) -> RightPane {
        match self {
            RightPane::Chat => RightPane::Research,
            RightPane::Research => RightPane::Map,
            RightPane::Map => RightPane::Ledger,
            RightPane::Ledger => RightPane::Chat,
        }
    }

    /// The pane's short title.
    pub(crate) fn title(self) -> &'static str {
        match self {
            RightPane::Chat => "Chat",
            RightPane::Research => "Research",
            RightPane::Map => "Map",
            RightPane::Ledger => "Ledger",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_four_primaries_and_returns() {
        let mut f = Focus::FactsPane;
        f = f.next();
        assert_eq!(f, Focus::WorldPane);
        f = f.next();
        assert_eq!(f, Focus::QueryPrompt);
        f = f.next();
        assert_eq!(f, Focus::RightPane);
        f = f.next();
        assert_eq!(f, Focus::FactsPane); // full loop
    }

    #[test]
    fn shift_tab_is_the_inverse() {
        for f in [Focus::FactsPane, Focus::WorldPane, Focus::QueryPrompt, Focus::RightPane] {
            assert_eq!(f.next().prev(), f);
            assert_eq!(f.prev().next(), f);
        }
    }

    #[test]
    fn overlay_is_sticky() {
        assert_eq!(Focus::ConfirmationOverlay.next(), Focus::ConfirmationOverlay);
        assert_eq!(Focus::ConfirmationOverlay.prev(), Focus::ConfirmationOverlay);
    }

    #[test]
    fn right_pane_cycles() {
        let mut r = RightPane::Chat;
        r = r.next();
        assert_eq!(r, RightPane::Research);
        r = r.next();
        assert_eq!(r, RightPane::Map);
        r = r.next();
        assert_eq!(r, RightPane::Ledger);
        r = r.next();
        assert_eq!(r, RightPane::Chat);
    }
}

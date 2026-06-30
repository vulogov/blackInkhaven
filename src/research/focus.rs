//! RESRCH-1 — the focus model (RFC §5.3). Three primary targets cycled by
//! `Tab` / `Shift+Tab`; the confirmation overlay is a transient fourth that
//! takes focus automatically and returns to `QueryPrompt` on confirm/discard.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    FactsTree,
    QueryPrompt,
    AiChat,
    ConfirmationOverlay,
}

impl Focus {
    /// `Tab` — forward through the three primary panes (the overlay is excluded;
    /// it owns focus while active).
    pub(crate) fn next(self) -> Focus {
        match self {
            Focus::FactsTree => Focus::QueryPrompt,
            Focus::QueryPrompt => Focus::AiChat,
            Focus::AiChat => Focus::FactsTree,
            Focus::ConfirmationOverlay => Focus::ConfirmationOverlay,
        }
    }

    /// `Shift+Tab` — backward.
    pub(crate) fn prev(self) -> Focus {
        match self {
            Focus::FactsTree => Focus::AiChat,
            Focus::QueryPrompt => Focus::FactsTree,
            Focus::AiChat => Focus::QueryPrompt,
            Focus::ConfirmationOverlay => Focus::ConfirmationOverlay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_three_primary_panes() {
        assert_eq!(Focus::FactsTree.next(), Focus::QueryPrompt);
        assert_eq!(Focus::QueryPrompt.next(), Focus::AiChat);
        assert_eq!(Focus::AiChat.next(), Focus::FactsTree);
    }

    #[test]
    fn shift_tab_reverses() {
        assert_eq!(Focus::FactsTree.prev(), Focus::AiChat);
        assert_eq!(Focus::AiChat.prev(), Focus::QueryPrompt);
        assert_eq!(Focus::QueryPrompt.prev(), Focus::FactsTree);
    }

    #[test]
    fn overlay_is_sticky() {
        assert_eq!(Focus::ConfirmationOverlay.next(), Focus::ConfirmationOverlay);
        assert_eq!(Focus::ConfirmationOverlay.prev(), Focus::ConfirmationOverlay);
    }
}

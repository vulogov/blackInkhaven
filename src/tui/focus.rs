#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Editor,
    Ai,
    SearchBar,
    AiPrompt,
}

impl Focus {
    /// Cycle through the three main panes (Tree → Editor → Ai → Tree). Input
    /// bars are reachable only via their dedicated shortcuts.
    pub fn next(self) -> Self {
        match self {
            Focus::Tree => Focus::Editor,
            Focus::Editor => Focus::Ai,
            Focus::Ai => Focus::Tree,
            // From input bars, Tab defocuses back to Tree and then cycles.
            Focus::SearchBar | Focus::AiPrompt => Focus::Tree,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Focus::Tree => Focus::Ai,
            Focus::Editor => Focus::Tree,
            Focus::Ai => Focus::Editor,
            Focus::SearchBar | Focus::AiPrompt => Focus::Tree,
        }
    }

    pub fn is_input(self) -> bool {
        matches!(self, Focus::SearchBar | Focus::AiPrompt)
    }

    pub fn label(self) -> &'static str {
        match self {
            Focus::Tree => "Tree",
            Focus::Editor => "Editor",
            Focus::Ai => "AI",
            Focus::SearchBar => "Search",
            Focus::AiPrompt => "Prompt",
        }
    }
}

//! WBLD-1 (WB-P8) — the guided world interview.
//!
//! A fixed five-stage script (Sky · Land · People · Rules · Review) that walks
//! the author from an empty project to a first coherent frame. Each step is a
//! plain question whose answer fills a **shaping-command template** — the same
//! `/star`, `/tilt`, `/set …` commands the author could type by hand (WB-P4) — so
//! every recorded delta is schema-valid by construction and goes through the one
//! tested `Op` engine. The interview holds no logic beyond the script and a
//! cursor; the app parses each answer and accumulates the ops into the pending
//! delta, so the ★ score moves live and the author reviews everything at the end
//! with `/diff` before `/write`. It never generates prose: it only asks and records.

/// The five conversational stages (Review is the closing summary, not a step).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Sky,
    Land,
    People,
    Rules,
}

impl Stage {
    pub(super) fn label(self) -> &'static str {
        match self {
            Stage::Sky => "Sky",
            Stage::Land => "Land",
            Stage::People => "People",
            Stage::Rules => "Rules",
        }
    }
}

/// One interview question. `template` is a shaping-command with a single `{}`
/// placeholder the answer is substituted into (e.g. `"/star {}"`).
pub(super) struct Step {
    pub stage: Stage,
    pub prompt: &'static str,
    pub template: &'static str,
}

/// The interview script. Ordered by stage; every template is an existing WB-P4
/// shaping command, so answers can only produce valid `world.hjson` edits.
static SCRIPT: &[Step] = &[
    Step {
        stage: Stage::Sky,
        prompt: "What kind of star? (G Sun-like · K orange · M red dwarf)",
        template: "/star {}",
    },
    Step {
        stage: Stage::Sky,
        prompt: "Axial tilt in degrees? (Earth 23.4 — higher means harsher seasons)",
        template: "/tilt {}",
    },
    Step {
        stage: Stage::Sky,
        prompt: "Add a moon? (name, optional period in days — blank to skip)",
        template: "/moon {}",
    },
    Step {
        stage: Stage::Land,
        prompt: "How many continents? (e.g. 3)",
        template: "/set geology.generated.continents {}",
    },
    Step {
        stage: Stage::Land,
        prompt: "Sea level, 0..1? (Earth ≈ 0.6 — higher means more ocean)",
        template: "/set geology.generated.sea_level {}",
    },
    Step {
        stage: Stage::Land,
        prompt: "Mountains — active, quiet, or ancient?",
        template: "/set geology.generated.mountain_orogeny {}",
    },
    Step {
        stage: Stage::People,
        prompt: "Primary language? (e.g. English)",
        template: "/set primary_language {}",
    },
    Step {
        stage: Stage::People,
        prompt: "Name a nation? (name [era] [polity_kind] [traits…] — blank to skip)",
        template: "/nation {}",
    },
    Step {
        stage: Stage::Rules,
        prompt: "Is there magic in this world? (true/false)",
        template: "/set magic.enabled {}",
    },
];

/// The interview cursor over [`SCRIPT`].
pub(super) struct Interview {
    pos: usize,
}

impl Interview {
    pub(super) fn new() -> Interview {
        Interview { pos: 0 }
    }

    /// The step awaiting an answer, or `None` once the script is exhausted.
    pub(super) fn current(&self) -> Option<&'static Step> {
        SCRIPT.get(self.pos)
    }

    /// Move to the next question.
    pub(super) fn advance(&mut self) {
        self.pos += 1;
    }

    pub(super) fn done(&self) -> bool {
        self.pos >= SCRIPT.len()
    }

    /// `(current 1-based index, total)` for the progress banner.
    pub(super) fn progress(&self) -> (usize, usize) {
        (self.pos + 1, SCRIPT.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_has_exactly_one_placeholder_and_is_a_command() {
        for step in SCRIPT {
            assert_eq!(
                step.template.matches("{}").count(),
                1,
                "step `{}` must have one placeholder",
                step.prompt
            );
            assert!(step.template.starts_with('/'), "template must be a /command");
        }
    }

    #[test]
    fn cursor_walks_the_whole_script_then_reports_done() {
        let mut iv = Interview::new();
        assert_eq!(iv.progress(), (1, SCRIPT.len()));
        assert_eq!(iv.current().unwrap().stage, Stage::Sky);
        for _ in 0..SCRIPT.len() {
            assert!(!iv.done());
            iv.advance();
        }
        assert!(iv.done());
        assert!(iv.current().is_none());
    }

    #[test]
    fn stages_appear_in_sky_land_people_rules_order() {
        let mut last = 0usize;
        let order = |s: Stage| match s {
            Stage::Sky => 0,
            Stage::Land => 1,
            Stage::People => 2,
            Stage::Rules => 3,
        };
        for step in SCRIPT {
            let o = order(step.stage);
            assert!(o >= last, "stages must be non-decreasing");
            last = o;
        }
    }
}

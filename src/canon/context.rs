//! CL-P6 — grounded context via smysl `pack`.
//!
//! Fit the canon to a token budget for grounding an LLM (Book chat / F1 /
//! research), instead of best-hits truncation. Relevance is seeded softly via
//! smysl `SalienceRequest::role_weights` (the plan's HNSW seam — inkhaven's own
//! semantic retrieval picks the query-relevant decisions), then `pack` selects
//! within the budget **closure-complete** (a chosen decision pulls in its
//! grounds and rebuttals) and holds back `reserve` tokens for the prompt +
//! answer. The estimator is smysl's chars/4 — the same token estimate inkhaven
//! already uses — so no external tokenizer and no embedding dependency.
//!
//! Cost is constant in canon size: a bigger canon means better selection into
//! the same window, not a bigger prompt.

use anyhow::{anyhow, Result};
use smysl::{pack, salience, PackRequest, SalienceRequest, Uid};

use super::query::CanonView;
use super::CanonLedger;

/// A budget-bounded selection of canon decisions to ground an LLM on.
#[derive(Debug, Clone)]
pub struct PackedContext {
    pub views: Vec<CanonView>,
    pub used: u64,
    pub budget: u64,
    pub reserved: u64,
    pub dropped: usize,
}

impl PackedContext {
    /// Render the packed decisions as a plain context block for a prompt.
    pub fn to_prompt(&self) -> String {
        let mut s = String::new();
        for v in &self.views {
            let kind = v
                .kind
                .map(|k| k.schema_str().trim_start_matches("x.narrative/"))
                .unwrap_or("?");
            let commit = v.commitment.map(|c| format!(" [{c}]")).unwrap_or_default();
            let loc = v
                .locator
                .as_deref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            s.push_str(&format!("- ({kind}{commit}) {}{loc}\n", v.gist));
        }
        s
    }
}

impl CanonLedger {
    /// Pack canon decisions to a token budget for grounding. `relevant` names the
    /// query-relevant decisions (from inkhaven's semantic retrieval); they are
    /// seeded into salience so `pack` prefers them and their grounds/rebuttals,
    /// while `reserve` holds back room for the prompt + answer.
    pub fn pack_context(
        &self,
        relevant: &[Uid],
        budget: usize,
        reserve: usize,
    ) -> Result<PackedContext> {
        self.with_store(|s| {
            let mut request = SalienceRequest::default();
            request.role_weights = relevant.iter().map(|u| (*u, 1.0)).collect();
            let sal = salience(s, &request);

            let req = PackRequest::budget(budget as u64).reserving(reserve as u64);
            let packed = pack(s, &sal, &req).map_err(|e| anyhow!("canon pack: {e}"))?;

            let views: Vec<CanonView> = packed
                .selection
                .keys()
                .filter_map(|u| Self::view_from_unit(s, *u))
                .collect();
            Ok(PackedContext {
                views,
                used: packed.info.used,
                budget: packed.info.budget,
                reserved: packed.info.reserved,
                dropped: packed.info.dropped.len(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::NarrativeKind;
    use uuid::Uuid;

    #[test]
    fn packs_to_budget_closure_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x99);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "winter seals the harbour", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "the escape must wait for the thaw", n, "ch9", &[base])
            .unwrap();

        // Seed the plot-point as relevant, ample budget → it is packed, and its
        // ground (the world-fact) comes with it (closure-complete).
        let ctx = led.pack_context(&[plot], 4000, 500).unwrap();
        let uids: Vec<Uid> = ctx.views.iter().map(|v| v.uid).collect();
        assert!(uids.contains(&plot), "the relevant decision is packed");
        assert!(uids.contains(&base), "its ground comes with it (closure)");

        // Budget invariant + a usable prompt.
        assert!(ctx.used + ctx.reserved <= ctx.budget, "used + reserved <= budget");
        assert!(!ctx.to_prompt().is_empty());
    }
}

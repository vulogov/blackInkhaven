//! 1.3.0 PDF-1 P1 — creep (shingling) compensation math (RFC §8.2.3).
//! Pure.
//!
//! When folded sheets are nested, the inner leaves protrude at the
//! fore-edge and lose the most margin when the stack is trimmed flush.
//! Creep compensation pulls inner-sheet content **toward the spine** by a
//! growing amount so the trimmed margins stay even.

use super::CreepStrategy;

/// Inward creep shift in **mm** for sheet `i` (1-based from the
/// outermost) of a folded signature, paper caliper `thickness_mm`.
///
/// Zero on the outermost sheet, growing linearly inward — the basic
/// shingling model (each nested sheet adds one caliper of fore-edge
/// protrusion).  The *direction* (left vs right of the spine) is applied
/// by the emitter from each slot's column.  `Shingle` and `Pushout`
/// share this magnitude; they differ only in how it's applied
/// (whole-page translate vs content clip-and-shift) at emission.  A
/// non-folded style (stab / concertina) has no nesting → no creep.
pub fn creep_shift_mm(strategy: CreepStrategy, i: usize, thickness_mm: f32) -> f32 {
    match strategy {
        CreepStrategy::None => 0.0,
        CreepStrategy::Shingle | CreepStrategy::Pushout => {
            i.saturating_sub(1) as f32 * thickness_mm.max(0.0)
        }
    }
}

/// The maximum creep shift in a signature of `sheets` sheets — the value
/// surfaced in the imposition preview so the user can sanity-check before
/// printing.
pub fn max_creep_mm(strategy: CreepStrategy, sheets: usize, thickness_mm: f32) -> f32 {
    creep_shift_mm(strategy, sheets, thickness_mm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} !≈ {b}");
    }

    #[test]
    fn none_strategy_is_zero() {
        approx(creep_shift_mm(CreepStrategy::None, 4, 0.1), 0.0);
    }

    #[test]
    fn outermost_sheet_has_no_creep() {
        approx(creep_shift_mm(CreepStrategy::Shingle, 1, 0.1), 0.0);
    }

    #[test]
    fn grows_inward_proportional_to_caliper() {
        approx(creep_shift_mm(CreepStrategy::Shingle, 2, 0.1), 0.1);
        approx(creep_shift_mm(CreepStrategy::Shingle, 4, 0.1), 0.3);
        // pushout shares the magnitude
        approx(creep_shift_mm(CreepStrategy::Pushout, 4, 0.1), 0.3);
        // thicker stock → more creep
        approx(creep_shift_mm(CreepStrategy::Shingle, 4, 0.2), 0.6);
    }

    #[test]
    fn monotonic_non_decreasing_toward_the_inside() {
        let mut prev = -1.0;
        for i in 1..=8 {
            let s = creep_shift_mm(CreepStrategy::Shingle, i, 0.12);
            assert!(s >= prev, "creep must not decrease inward");
            prev = s;
        }
    }

    #[test]
    fn max_is_at_the_innermost_sheet() {
        approx(max_creep_mm(CreepStrategy::Shingle, 4, 0.1), 0.3);
        approx(max_creep_mm(CreepStrategy::None, 4, 0.1), 0.0);
    }

    #[test]
    fn negative_caliper_clamped() {
        approx(creep_shift_mm(CreepStrategy::Shingle, 4, -1.0), 0.0);
    }
}

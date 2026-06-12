//! 1.3.0 PDF-1 P1 — imposition layout: the pure page→position math (RFC
//! §8.2.2).  No I/O.  Given a binding style + signature size + page
//! count, assign every source page to a sheet/side/column slot.  The
//! correctness contract (RFC §13) is the property tests below: every
//! source page appears exactly once.

use super::{BindingStyle, BlankPolicy};

/// One imposed position on a sheet side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// 1-based source page, or `None` for a blank (padding).
    pub page: Option<usize>,
    /// 0-based column on this side: 0 = left, 1 = right (2-up); always 0
    /// for 1-up stab / concertina.
    pub column: usize,
}

/// One physical sheet (or leaf) with its front + back slot assignments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// 0-based signature index (0 for single-signature styles).
    pub signature: usize,
    /// 1-based sheet index within its signature, 1 = outermost.
    pub sheet_in_sig: usize,
    pub front: Vec<Slot>,
    pub back: Vec<Slot>,
}

/// The full imposition plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub style: BindingStyle,
    pub source_pages: usize,
    pub padded_pages: usize,
    pub columns_per_side: usize,
    pub sheets_per_signature: usize,
    pub signatures: usize,
    pub sheets: Vec<Sheet>,
}

/// Plan the imposition.  `sheets_per_signature` is honoured only for
/// `PerfectBound`; `SaddleStitch` uses one signature sized to fit;
/// `Stab` / `Concertina` are one leaf per two pages.
pub fn plan(
    style: BindingStyle,
    sheets_per_signature: usize,
    source_pages: usize,
    blank: BlankPolicy,
) -> Layout {
    let source = source_pages.max(1);
    match style {
        BindingStyle::SaddleStitch => {
            let s = source.div_ceil(4).max(1);
            build_folded(style, 1, s, source, blank)
        }
        BindingStyle::PerfectBound => {
            let s = sheets_per_signature.max(1);
            let per_sig = 4 * s;
            let n = source.div_ceil(per_sig).max(1);
            build_folded(style, n, s, source, blank)
        }
        BindingStyle::Stab | BindingStyle::Concertina => build_sequential(style, source, blank),
    }
}

/// Positions within a folded signature of `s` sheets (4s pages), 1-based
/// sheet `i` (1 = outermost): `(front-left, front-right, back-left,
/// back-right)`, each a 1-based position within the signature.
fn folded_positions(s: usize, i: usize) -> (usize, usize, usize, usize) {
    let fs = 4 * s;
    (fs - 2 * i + 2, 2 * i - 1, 2 * i, fs - 2 * i + 1)
}

fn build_folded(
    style: BindingStyle,
    n_sigs: usize,
    s: usize,
    source: usize,
    blank: BlankPolicy,
) -> Layout {
    let per_sig = 4 * s;
    let padded = n_sigs * per_sig;
    let mut sheets = Vec::with_capacity(n_sigs * s);
    for g in 0..n_sigs {
        let off = g * per_sig;
        for i in 1..=s {
            let (fl, fr, bl, br) = folded_positions(s, i);
            sheets.push(Sheet {
                signature: g,
                sheet_in_sig: i,
                front: vec![
                    slot(off + fl, source, padded, blank, 0),
                    slot(off + fr, source, padded, blank, 1),
                ],
                back: vec![
                    slot(off + bl, source, padded, blank, 0),
                    slot(off + br, source, padded, blank, 1),
                ],
            });
        }
    }
    Layout {
        style,
        source_pages: source,
        padded_pages: padded,
        columns_per_side: 2,
        sheets_per_signature: s,
        signatures: n_sigs,
        sheets,
    }
}

fn build_sequential(style: BindingStyle, source: usize, blank: BlankPolicy) -> Layout {
    let leaves = source.div_ceil(2).max(1);
    let padded = 2 * leaves;
    let mut sheets = Vec::with_capacity(leaves);
    for k in 0..leaves {
        sheets.push(Sheet {
            signature: 0,
            sheet_in_sig: k + 1,
            front: vec![slot(2 * k + 1, source, padded, blank, 0)],
            back: vec![slot(2 * k + 2, source, padded, blank, 0)],
        });
    }
    Layout {
        style,
        source_pages: source,
        padded_pages: padded,
        columns_per_side: 1,
        sheets_per_signature: 1,
        signatures: 1,
        sheets,
    }
}

fn slot(pos: usize, source: usize, padded: usize, blank: BlankPolicy, column: usize) -> Slot {
    Slot {
        page: map_page(pos, source, padded, blank),
        column,
    }
}

/// Map a 1-based imposed position to a source page (or `None` for a
/// blank), honouring where the padding blanks sit.
fn map_page(pos: usize, source: usize, padded: usize, blank: BlankPolicy) -> Option<usize> {
    if pos < 1 || pos > padded {
        return None;
    }
    let blanks = padded - source;
    match blank {
        BlankPolicy::Append | BlankPolicy::Error => (pos <= source).then_some(pos),
        BlankPolicy::Prepend => (pos > blanks).then(|| pos - blanks),
        BlankPolicy::Balance => {
            let front = blanks / 2;
            if pos <= front || pos - front > source {
                None
            } else {
                Some(pos - front)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every real source page appears exactly once across all slots; the
    /// blank count matches the padding.  (RFC §13 correctness contract.)
    fn assert_complete(layout: &Layout) {
        let mut seen: Vec<usize> = Vec::new();
        let mut blanks = 0;
        for sh in &layout.sheets {
            for slot in sh.front.iter().chain(sh.back.iter()) {
                match slot.page {
                    Some(p) => seen.push(p),
                    None => blanks += 1,
                }
            }
        }
        let set: BTreeSet<usize> = seen.iter().copied().collect();
        assert_eq!(set.len(), seen.len(), "a source page was imposed twice");
        let expected: BTreeSet<usize> = (1..=layout.source_pages).collect();
        assert_eq!(set, expected, "missing/extra page");
        assert_eq!(
            blanks,
            layout.padded_pages - layout.source_pages,
            "blank count mismatch"
        );
    }

    fn pages(side: &[Slot]) -> Vec<Option<usize>> {
        side.iter().map(|s| s.page).collect()
    }

    #[test]
    fn saddle_four_pages_is_classic_imposition() {
        let l = plan(BindingStyle::SaddleStitch, 1, 4, BlankPolicy::Append);
        assert_eq!(l.sheets.len(), 1);
        assert_eq!(l.padded_pages, 4);
        // front [4|1], back [2|3]
        assert_eq!(pages(&l.sheets[0].front), vec![Some(4), Some(1)]);
        assert_eq!(pages(&l.sheets[0].back), vec![Some(2), Some(3)]);
        assert_complete(&l);
    }

    #[test]
    fn saddle_eight_pages_two_nested_sheets() {
        let l = plan(BindingStyle::SaddleStitch, 1, 8, BlankPolicy::Append);
        assert_eq!(l.sheets.len(), 2);
        assert_eq!(pages(&l.sheets[0].front), vec![Some(8), Some(1)]); // outer
        assert_eq!(pages(&l.sheets[0].back), vec![Some(2), Some(7)]);
        assert_eq!(pages(&l.sheets[1].front), vec![Some(6), Some(3)]); // inner
        assert_eq!(pages(&l.sheets[1].back), vec![Some(4), Some(5)]);
        assert_complete(&l);
    }

    #[test]
    fn saddle_six_pages_pads_to_eight_append() {
        let l = plan(BindingStyle::SaddleStitch, 1, 6, BlankPolicy::Append);
        assert_eq!(l.padded_pages, 8);
        // positions 7,8 are blank (append) → on the outer sheet
        assert_eq!(pages(&l.sheets[0].front), vec![None, Some(1)]); // pos 8 blank
        assert_eq!(pages(&l.sheets[0].back), vec![Some(2), None]); // pos 7 blank
        assert_complete(&l);
    }

    #[test]
    fn perfect_bound_multi_signature() {
        // 16 pages, 2 sheets/sig (8pp signatures) → 2 signatures, 4 sheets.
        let l = plan(BindingStyle::PerfectBound, 2, 16, BlankPolicy::Append);
        assert_eq!(l.signatures, 2);
        assert_eq!(l.sheets.len(), 4);
        assert_eq!(l.sheets[2].signature, 1); // 3rd sheet is in 2nd signature
        assert_complete(&l);
    }

    #[test]
    fn stab_is_sequential_one_up() {
        let l = plan(BindingStyle::Stab, 1, 5, BlankPolicy::Append);
        assert_eq!(l.columns_per_side, 1);
        assert_eq!(l.sheets.len(), 3); // ceil(5/2)
        assert_eq!(pages(&l.sheets[0].front), vec![Some(1)]);
        assert_eq!(pages(&l.sheets[0].back), vec![Some(2)]);
        assert_eq!(pages(&l.sheets[2].front), vec![Some(5)]);
        assert_eq!(pages(&l.sheets[2].back), vec![None]); // page 6 blank
        assert_complete(&l);
    }

    #[test]
    fn prepend_and_balance_place_blanks() {
        let pre = plan(BindingStyle::SaddleStitch, 1, 6, BlankPolicy::Prepend);
        // blanks at positions 1,2 (front) → outer sheet front-right=pos1=blank
        assert_eq!(pages(&pre.sheets[0].front), vec![Some(6), None]);
        assert_complete(&pre);
        let bal = plan(BindingStyle::Stab, 1, 5, BlankPolicy::Balance);
        assert_complete(&bal); // 1 blank, front/2=0 → behaves like append here
    }

    #[test]
    fn property_every_page_once_across_styles_and_sizes() {
        for &style in &[
            BindingStyle::SaddleStitch,
            BindingStyle::PerfectBound,
            BindingStyle::Stab,
            BindingStyle::Concertina,
        ] {
            for pages in [1usize, 2, 3, 4, 7, 16, 17, 33, 100, 249] {
                for &blank in &[
                    BlankPolicy::Append,
                    BlankPolicy::Prepend,
                    BlankPolicy::Balance,
                ] {
                    let l = plan(style, 3, pages, blank);
                    assert_complete(&l);
                }
            }
        }
    }
}

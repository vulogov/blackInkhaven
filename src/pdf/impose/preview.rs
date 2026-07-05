//! 1.3.0 PDF-1 P1 — imposition preview (RFC App. D).  Pure: a summary of
//! the plan (signatures / sheets / blanks / creep) plus the first sheet's
//! schematic, rendered to text lines the `Ctrl+B I` overlay (and any
//! dry-run) display.

use super::super::geometry::{pt_to_mm, Size};
use super::layout::{self, Sheet, Slot};
use super::marks::MarkConfig;
use super::{BindingStyle, CreepStrategy, ImpositionParams};

/// Everything the preview shows, computed from the plan.
#[derive(Debug, Clone)]
pub struct ImpositionPreview {
    pub profile: String,
    pub source_pages: usize,
    pub style: BindingStyle,
    pub signature_pages: usize,
    pub sheets_per_signature: usize,
    pub signatures: usize,
    #[allow(dead_code)] // part of the complete plan; surfaced by --preview consumers
    pub total_sheets: usize,
    pub imposed_pages: usize,
    pub blanks: usize,
    pub sheet_size: Size,
    pub paper_thickness_mm: f32,
    pub creep: CreepStrategy,
    pub max_creep_mm: f32,
    pub marks: MarkConfig,
    pub first_sheet: Option<Sheet>,
}

pub fn build(profile: &str, source_pages: usize, params: &ImpositionParams) -> ImpositionPreview {
    let l = layout::plan(
        params.style,
        params.sheets_per_signature,
        source_pages,
        params.blank,
    );
    let total_sheets = l.sheets.len();
    let max_creep = super::creep::max_creep_mm(
        params.creep,
        l.sheets_per_signature,
        params.paper_thickness_mm,
    );
    ImpositionPreview {
        profile: profile.to_string(),
        source_pages,
        style: params.style,
        signature_pages: l.sheets_per_signature * if params.style.is_folded() { 4 } else { 2 },
        sheets_per_signature: l.sheets_per_signature,
        signatures: l.signatures,
        total_sheets,
        imposed_pages: total_sheets * 2,
        blanks: l.padded_pages.saturating_sub(l.source_pages),
        sheet_size: params.sheet_size,
        paper_thickness_mm: params.paper_thickness_mm,
        creep: params.creep,
        max_creep_mm: max_creep,
        marks: params.marks,
        first_sheet: l.sheets.into_iter().next(),
    }
}

impl ImpositionPreview {
    /// The preview body as text lines (RFC App. D).
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![
            format!("Profile        : {}", self.profile),
            format!("Source pages   : {}", self.source_pages),
            format!("Style          : {}", style_name(self.style)),
        ];
        if self.style.is_folded() {
            v.push(format!(
                "Signature      : {} pages ({} sheet{})",
                self.signature_pages,
                self.sheets_per_signature,
                plural(self.sheets_per_signature),
            ));
        }
        v.push(format!(
            "Signatures     : {}  ({} imposed page{}, +{} blank{})",
            self.signatures,
            self.imposed_pages,
            plural(self.imposed_pages),
            self.blanks,
            plural(self.blanks),
        ));
        v.push(format!(
            "Sheet size     : {:.0} × {:.0} pt  ({:.0} × {:.0} mm)",
            self.sheet_size.width,
            self.sheet_size.height,
            pt_to_mm(self.sheet_size.width),
            pt_to_mm(self.sheet_size.height),
        ));
        v.push(format!(
            "Paper / creep  : {:.2} mm · {}",
            self.paper_thickness_mm,
            creep_desc(self.creep, self.max_creep_mm),
        ));
        v.push(format!("Marks          : {}", marks_desc(&self.marks)));
        if let Some(sheet) = &self.first_sheet {
            v.push(String::new());
            v.push("Sheet 1 (signature 1, outermost):".to_string());
            v.push(format!("  Front: {}", fmt_side(&sheet.front)));
            v.push(format!("  Back:  {}", fmt_side(&sheet.back)));
        }
        v
    }
}

fn fmt_side(side: &[Slot]) -> String {
    side.iter()
        .map(|s| match s.page {
            Some(p) => format!("[{p:>4} ]"),
            None => "[  —  ]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn style_name(s: BindingStyle) -> &'static str {
    match s {
        BindingStyle::SaddleStitch => "saddle-stitch",
        BindingStyle::PerfectBound => "perfect-bound",
        BindingStyle::Concertina => "concertina",
        BindingStyle::Stab => "stab",
    }
}

fn creep_desc(c: CreepStrategy, max_mm: f32) -> String {
    match c {
        CreepStrategy::None => "no creep".to_string(),
        CreepStrategy::Shingle => format!("shingle, max {max_mm:.2} mm"),
        CreepStrategy::Pushout => format!("push-out, max {max_mm:.2} mm"),
    }
}

fn marks_desc(m: &MarkConfig) -> String {
    let mut on = Vec::new();
    if m.crop {
        on.push("crop");
    }
    if m.fold {
        on.push("fold");
    }
    if m.registration {
        on.push("registration");
    }
    if m.spine_marker {
        on.push("spine");
    }
    if m.signature_number {
        on.push("sig#");
    }
    if m.color_bar {
        on.push("color-bar");
    }
    if on.is_empty() {
        "none".to_string()
    } else {
        on.join(" · ")
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{BlankPolicy, CreepStrategy};

    fn params(style: BindingStyle) -> ImpositionParams {
        ImpositionParams {
            style,
            sheets_per_signature: 1,
            blank: BlankPolicy::Append,
            sheet_size: Size::from_mm(420.0, 297.0),
            creep: CreepStrategy::Shingle,
            paper_thickness_mm: 0.10,
            marks: MarkConfig::default(),
            crop_offset_mm: 5.0,
            fold_mark_length_mm: 8.0,
        }
    }

    #[test]
    fn saddle_8_summary_and_schematic() {
        let p = build("default", 8, &params(BindingStyle::SaddleStitch));
        assert_eq!(p.signatures, 1);
        assert_eq!(p.total_sheets, 2);
        assert_eq!(p.imposed_pages, 4);
        assert_eq!(p.blanks, 0);
        let text = p.lines().join("\n");
        assert!(text.contains("Style          : saddle-stitch"));
        assert!(text.contains("(420 × 297 mm)"));
        // first sheet front [8][1], back [2][7]
        assert!(text.contains("Front: [   8 ][   1 ]"), "got:\n{text}");
        assert!(text.contains("Back:  [   2 ][   7 ]"));
        assert!(text.contains("shingle, max"));
    }

    #[test]
    fn padding_blanks_reported() {
        let p = build("default", 6, &params(BindingStyle::SaddleStitch));
        assert_eq!(p.blanks, 2);
        let text = p.lines().join("\n");
        assert!(text.contains("+2 blanks"));
        assert!(text.contains("[  —  ]"), "blank cell shown: {text}");
    }

    #[test]
    fn stab_one_up_no_signature_line() {
        let p = build("default", 5, &params(BindingStyle::Stab));
        let text = p.lines().join("\n");
        assert!(!text.contains("Signature      :")); // 1-up has no folded signature line
        assert!(text.contains("Front: [   1 ]"));
    }
}

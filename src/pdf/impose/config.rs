//! 1.3.0 PDF-1 P1 — imposition HJSON config (RFC §8.2.1 / App. A).
//!
//! Deserialized through `Config::load_layered` — the same cascade as
//! every other setting (project `inkhaven.hjson` → global
//! `~/.config/inkhaven`, global wins), so a user can keep a house
//! imposition profile globally.  `resolve(profile)` maps a named profile
//! to runtime [`ImpositionParams`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::geometry::{page_size, Size};
use super::super::paper;
use super::marks::MarkConfig;
use super::{BindingStyle, BlankPolicy, CreepStrategy, ImpositionParams};

/// The top-level `imposition:` block: named profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpositionConfig {
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, ImpositionProfile>,
}

impl Default for ImpositionConfig {
    fn default() -> Self {
        Self {
            profiles: default_profiles(),
        }
    }
}

fn default_profiles() -> BTreeMap<String, ImpositionProfile> {
    let mut m = BTreeMap::new();
    m.insert("default".into(), ImpositionProfile::default());
    m.insert("chapbook".into(), ImpositionProfile::chapbook());
    // US-paper analogues (Tabloid 11×17 folds to two Letter-half pages) and
    // a heavy-book profile for thick perfect-bound titles.
    m.insert("us_perfect".into(), ImpositionProfile::us_perfect());
    m.insert("us_chapbook".into(), ImpositionProfile::us_chapbook());
    m.insert("thick".into(), ImpositionProfile::thick());
    // Home-binding favourite: a proper A5 codex from A4 sheets.
    m.insert("a5_book".into(), ImpositionProfile::a5_book());
    m
}

impl ImpositionConfig {
    pub fn resolve(&self, profile: &str) -> Result<ImpositionParams, String> {
        self.profiles
            .get(profile)
            .ok_or_else(|| {
                format!(
                    "imposition: unknown profile `{profile}` (have: {})",
                    self.profile_names()
                )
            })?
            .resolve()
    }
    pub fn profile_names(&self) -> String {
        self.profiles.keys().cloned().collect::<Vec<_>>().join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImpositionProfile {
    pub style: String,
    pub sheets_per_signature: usize,
    pub target_sheet_size: SheetSize,
    pub orientation: String,
    pub margins: Margins,
    pub creep: CreepConfig,
    pub marks: MarksConfig,
    pub blank_page_policy: String,
}

impl Default for ImpositionProfile {
    fn default() -> Self {
        Self {
            style: "perfect_bound".into(),
            sheets_per_signature: 4,
            target_sheet_size: SheetSize::Preset("A3".into()),
            orientation: "auto".into(),
            margins: Margins::default(),
            creep: CreepConfig::default(),
            marks: MarksConfig::default(),
            blank_page_policy: "append".into(),
        }
    }
}

impl ImpositionProfile {
    fn chapbook() -> Self {
        Self {
            style: "saddle_stitch".into(),
            target_sheet_size: SheetSize::Preset("A4".into()),
            creep: CreepConfig {
                enabled: false,
                ..CreepConfig::default()
            },
            marks: MarksConfig {
                registration: false,
                spine_marker: false,
                signature_number: false,
                ..MarksConfig::default()
            },
            blank_page_policy: "balance".into(),
            ..Self::default()
        }
    }

    /// US trade analogue of `default`: perfect-bound on Tabloid (11×17)
    /// sheets, which fold to two US-Letter-half pages per side.
    fn us_perfect() -> Self {
        Self {
            target_sheet_size: SheetSize::Preset("TABLOID".into()),
            ..Self::default()
        }
    }

    /// US analogue of `chapbook`: saddle-stitched on Tabloid sheets, no
    /// creep, minimal marks.
    fn us_chapbook() -> Self {
        Self {
            target_sheet_size: SheetSize::Preset("TABLOID".into()),
            ..Self::chapbook()
        }
    }

    /// The popular home-binding recipe: a proper **A5 codex** folded from
    /// A4 sheets.  Perfect-bound (gathered + glued/sewn signatures) rather
    /// than saddle-stitched, so it scales to a full-length book — the
    /// `chapbook` / `pdf booklet` saddle path tops out around 40 pages.
    /// Four A4 sheets per signature = 16 A5 pages, the standard home
    /// section; shingle creep keeps the fold square, and the
    /// signature-number mark lets you gather them in order by hand.
    /// (Source pages should already be A5 trim — imposition centres, it
    /// doesn't scale.)
    fn a5_book() -> Self {
        Self {
            style: "perfect_bound".into(),
            sheets_per_signature: 4,
            target_sheet_size: SheetSize::Preset("A4".into()),
            marks: MarksConfig {
                crop: true,
                fold: true,
                signature_number: true,
                registration: false,
                spine_marker: false,
                color_bar: false,
            },
            ..Self::default()
        }
    }

    /// Heavy perfect-bound book: large 8-sheet (32-page) signatures with
    /// push-out creep so the outer leaves don't bind short after trimming.
    fn thick() -> Self {
        Self {
            sheets_per_signature: 8,
            creep: CreepConfig {
                strategy: "pushout".into(),
                ..CreepConfig::default()
            },
            ..Self::default()
        }
    }

    pub fn resolve(&self) -> Result<ImpositionParams, String> {
        let style = BindingStyle::parse(&self.style)
            .ok_or_else(|| format!("imposition: bad style `{}`", self.style))?;
        let blank = BlankPolicy::parse(&self.blank_page_policy)
            .ok_or_else(|| format!("imposition: bad blank_page_policy `{}`", self.blank_page_policy))?;
        let base = self
            .target_sheet_size
            .resolve()
            .ok_or_else(|| format!("imposition: unknown target_sheet_size {:?}", self.target_sheet_size))?;
        let cols = style.columns_per_side();
        let sheet_size = match self.orientation.trim().to_ascii_lowercase().as_str() {
            "portrait" => base.portrait(),
            "landscape" => base.landscape(),
            // auto: folded 2-up needs a wide sheet; 1-up stays portrait.
            _ => {
                if cols == 2 {
                    base.landscape()
                } else {
                    base.portrait()
                }
            }
        };
        let creep = if self.creep.enabled {
            CreepStrategy::parse(&self.creep.strategy)
                .ok_or_else(|| format!("imposition: bad creep strategy `{}`", self.creep.strategy))?
        } else {
            CreepStrategy::None
        };
        let thickness = self
            .creep
            .thickness_mm_override
            .or_else(|| paper::paper_stock(&self.creep.paper_stock).map(|s| s.thickness_mm))
            .unwrap_or(0.1);

        Ok(ImpositionParams {
            style,
            sheets_per_signature: self.sheets_per_signature.max(1),
            blank,
            sheet_size,
            creep,
            paper_thickness_mm: thickness,
            marks: MarkConfig {
                crop: self.marks.crop,
                fold: self.marks.fold,
                registration: self.marks.registration,
                spine_marker: self.marks.spine_marker,
                signature_number: self.marks.signature_number,
                color_bar: self.marks.color_bar,
            },
            crop_offset_mm: self.margins.crop_offset_mm,
            fold_mark_length_mm: self.margins.fold_mark_length_mm,
        })
    }
}

/// A preset name (`"A3"`) or an explicit `{ width_mm, height_mm }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SheetSize {
    Preset(String),
    Custom { width_mm: f32, height_mm: f32 },
}

impl SheetSize {
    fn resolve(&self) -> Option<Size> {
        match self {
            SheetSize::Preset(name) => page_size(name),
            SheetSize::Custom { width_mm, height_mm } => Some(Size::from_mm(*width_mm, *height_mm)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Margins {
    pub bleed_mm: f32,
    pub crop_offset_mm: f32,
    pub fold_mark_length_mm: f32,
    pub gutter_mm: f32,
    pub outer_margin_mm: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            bleed_mm: 3.0,
            crop_offset_mm: 5.0,
            fold_mark_length_mm: 8.0,
            gutter_mm: 0.0,
            outer_margin_mm: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CreepConfig {
    pub enabled: bool,
    pub paper_stock: String,
    pub thickness_mm_override: Option<f32>,
    pub strategy: String,
}

impl Default for CreepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            paper_stock: paper::DEFAULT_INTERIOR.into(),
            thickness_mm_override: None,
            strategy: "shingle".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MarksConfig {
    pub crop: bool,
    pub fold: bool,
    pub registration: bool,
    pub spine_marker: bool,
    pub signature_number: bool,
    pub color_bar: bool,
}

impl Default for MarksConfig {
    fn default() -> Self {
        Self {
            crop: true,
            fold: true,
            registration: true,
            spine_marker: true,
            signature_number: true,
            color_bar: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_resolves() {
        let cfg = ImpositionConfig::default();
        let p = cfg.resolve("default").unwrap();
        assert_eq!(p.style, BindingStyle::PerfectBound);
        assert_eq!(p.sheets_per_signature, 4);
        assert!(matches!(p.creep, CreepStrategy::Shingle));
        // A3 landscape (auto, 2-up), uncoated_80gsm caliper.
        assert!(p.sheet_size.width > p.sheet_size.height);
        assert!((p.paper_thickness_mm - 0.100).abs() < 1e-4);
        assert!(p.marks.crop && !p.marks.color_bar);
    }

    #[test]
    fn chapbook_profile_is_saddle_no_creep() {
        let p = ImpositionConfig::default().resolve("chapbook").unwrap();
        assert_eq!(p.style, BindingStyle::SaddleStitch);
        assert!(matches!(p.creep, CreepStrategy::None));
        assert!(!p.marks.spine_marker);
    }

    #[test]
    fn unknown_profile_errors() {
        assert!(ImpositionConfig::default().resolve("nope").is_err());
    }

    #[test]
    fn us_profiles_use_tabloid_sheets() {
        let cfg = ImpositionConfig::default();
        // 11×17 in points = 792 × 1224; folded 2-up → landscape (wider).
        let p = cfg.resolve("us_perfect").unwrap();
        assert_eq!(p.style, BindingStyle::PerfectBound);
        assert!((p.sheet_size.width - 1224.0).abs() < 0.5, "Tabloid long edge across");
        assert!(p.sheet_size.width > p.sheet_size.height);
        let c = cfg.resolve("us_chapbook").unwrap();
        assert_eq!(c.style, BindingStyle::SaddleStitch);
        assert!(matches!(c.creep, CreepStrategy::None));
        assert!(!c.marks.spine_marker);
    }

    #[test]
    fn thick_profile_is_big_pushout_signatures() {
        let p = ImpositionConfig::default().resolve("thick").unwrap();
        assert_eq!(p.sheets_per_signature, 8);
        assert!(matches!(p.creep, CreepStrategy::Pushout));
    }

    #[test]
    fn profile_names_lists_all_builtins() {
        let names = ImpositionConfig::default().profile_names();
        for n in ["default", "chapbook", "us_perfect", "us_chapbook", "thick", "a5_book"] {
            assert!(names.contains(n), "`{n}` missing from {names}");
        }
    }

    #[test]
    fn a5_book_is_perfect_bound_a4_with_gather_marks() {
        let p = ImpositionConfig::default().resolve("a5_book").unwrap();
        assert_eq!(p.style, BindingStyle::PerfectBound);
        assert_eq!(p.sheets_per_signature, 4); // 16 A5 pages / signature
        // A4 landscape (auto, 2-up): long edge across = 842 pt.
        assert!((p.sheet_size.width - 842.0).abs() < 0.5, "A4 long edge across");
        assert!(p.sheet_size.width > p.sheet_size.height);
        assert!(p.marks.signature_number, "gather-in-order mark on");
        assert!(!p.marks.registration && !p.marks.spine_marker);
    }

    #[test]
    fn deserializes_from_json_with_custom_sheet_and_override() {
        let json = r#"{
            "profiles": {
                "art": {
                    "style": "saddle_stitch",
                    "target_sheet_size": { "width_mm": 320.0, "height_mm": 450.0 },
                    "orientation": "portrait",
                    "creep": { "enabled": true, "strategy": "pushout", "thickness_mm_override": 0.2 }
                }
            }
        }"#;
        let cfg: ImpositionConfig = serde_json::from_str(json).unwrap();
        let p = cfg.resolve("art").unwrap();
        assert_eq!(p.style, BindingStyle::SaddleStitch);
        assert!(matches!(p.creep, CreepStrategy::Pushout));
        assert!((p.paper_thickness_mm - 0.2).abs() < 1e-4);
        // custom 320×450 mm, forced portrait
        assert!(p.sheet_size.height > p.sheet_size.width);
    }

    #[test]
    fn bad_style_errors() {
        let mut cfg = ImpositionConfig::default();
        cfg.profiles.get_mut("default").unwrap().style = "origami".into();
        assert!(cfg.resolve("default").is_err());
    }
}

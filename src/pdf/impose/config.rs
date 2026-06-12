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

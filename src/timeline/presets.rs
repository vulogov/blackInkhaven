//! Precision enum + preset-calendar expansions.

use serde::{Deserialize, Serialize};

/// How exact the user's intent is for a `TimelinePoint`.
/// Coarser precisions widen the fuzz window the AI critique
/// uses for overlap detection.
///
/// Serialized as a lowercase string in JSON metadata so users
/// editing files by hand see a friendly form.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    Tick,
    Hour,
    Day,
    Week,
    Month,
    Season,
    Year,
}

impl Default for Precision {
    fn default() -> Self {
        Self::Day
    }
}

impl Precision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Season => "season",
            Self::Year => "year",
        }
    }

    /// Parse from CLI / Bund inputs (case-insensitive,
    /// accepts singular only — we match the on-disk form).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tick" => Some(Self::Tick),
            "hour" => Some(Self::Hour),
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "season" => Some(Self::Season),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_str() {
        for p in [
            Precision::Tick,
            Precision::Hour,
            Precision::Day,
            Precision::Week,
            Precision::Month,
            Precision::Season,
            Precision::Year,
        ] {
            assert_eq!(Precision::from_str(p.as_str()), Some(p));
        }
    }

    #[test]
    fn default_is_day() {
        assert_eq!(Precision::default(), Precision::Day);
    }

    #[test]
    fn from_str_case_insensitive() {
        assert_eq!(Precision::from_str("SEASON"), Some(Precision::Season));
        assert_eq!(Precision::from_str("  Year  "), Some(Precision::Year));
    }
}

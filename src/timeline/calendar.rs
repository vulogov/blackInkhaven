//! Calendar arithmetic — convert i64 ticks ↔ human-readable
//! strings per the user's HJSON `timeline.calendar` block.
//!
//! Stack model: a `Vec<UnitDef>` ordered base-first.
//! `units[0]` is the base unit (one tick = one of these);
//! `units[1]` is the next level up (`units[1].per_parent`
//! base units make one level-1 unit), and so on. The
//! top-level unit has `per_parent = 0` (unbounded).
//!
//! Display follows a `display_format` string with these
//! placeholders:
//!
//!   {year}        — top level value, signed
//!   {epoch_label} — appended when value ≥ 0
//!   {epoch_before_label} — appended when value < 0
//!   {month}       — month index (1-based)
//!   {month-name}  — looked up in unit's `names[]` if set,
//!                   otherwise falls back to numeric
//!   {day}         — day index (1-based)
//!   {hour}        — hour index (0-based; canonical clock form)
//!
//! Parser inverts the same shape: dotted segments
//! `Y[label].M[.D[.H]]` with optional month-name / season-name
//! substitution at the month slot.

use serde::{Deserialize, Serialize};

use super::presets::Precision;

/// A single tick count, signed so prequels (`-1A`) work.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelinePoint(pub i64);

impl TimelinePoint {
    pub fn from_ticks(t: i64) -> Self {
        Self(t)
    }
    pub fn ticks(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitDef {
    /// User-visible name (`day`, `month`, `year`, …).
    /// Lowercase by convention.
    pub name: String,
    /// How many of THIS unit make ONE of the parent unit.
    /// `0` on the top-most unit means "unbounded" (the
    /// chain ends here).
    #[serde(default)]
    pub per_parent: u32,
    /// Optional display names. Index 0 == "1" in human-
    /// readable terms (so months 1..=12 read names[0..=11]).
    /// Empty vec → use numeric form everywhere.
    #[serde(default)]
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonDef {
    pub name: String,
    /// 1-based month at which this season starts.
    pub start_month: u32,
    /// How many months the season covers.
    pub span_months: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseAlias {
    /// Literal string the user can type to mean a specific
    /// tick value. Useful for landmark dates ("Founding",
    /// "Day Zero", "BattleOfX").
    #[serde(rename = "match")]
    pub matches: String,
    pub ticks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    /// One of `gregorian` | `sols` | `custom`. Presets expand
    /// the block at load time; `custom` honours user values
    /// directly.
    #[serde(default = "default_preset")]
    pub preset: String,
    /// Name of the base unit (the unit one tick represents).
    /// Default `day`.
    #[serde(default = "default_base_unit")]
    pub base_unit: String,
    /// Unit stack, base-first.
    #[serde(default)]
    pub units: Vec<UnitDef>,
    /// Seasons (used by `Precision::Season` fuzz windows).
    #[serde(default)]
    pub seasons: Vec<SeasonDef>,
    /// Suffix appended to non-negative year values.
    #[serde(default)]
    pub epoch_label: String,
    /// Suffix appended to negative year values (absolute
    /// value displayed). Empty = reject negatives at parse.
    #[serde(default)]
    pub epoch_before_label: String,
    /// Format string for `Calendar::format`. Falls back to
    /// `"{year}{epoch_label}.{month}.{day}"` when empty.
    #[serde(default)]
    pub display_format: String,
    /// Landmark string aliases recognised by `parse`.
    #[serde(default)]
    pub parse_aliases: Vec<ParseAlias>,
}

fn default_preset() -> String {
    "custom".to_owned()
}
fn default_base_unit() -> String {
    "day".to_owned()
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            preset: default_preset(),
            base_unit: default_base_unit(),
            units: Vec::new(),
            seasons: Vec::new(),
            epoch_label: String::new(),
            epoch_before_label: String::new(),
            display_format: String::new(),
            parse_aliases: Vec::new(),
        }
    }
}

/// Pre-built calendar ready for arithmetic.
#[derive(Debug, Clone)]
pub struct Calendar {
    pub cfg: CalendarConfig,
    /// Cumulative ticks per unit at each level: ticks_per[i]
    /// = how many base-unit ticks one unit at level i
    /// represents. ticks_per[0] = 1 (base). Length matches
    /// `cfg.units.len()`.
    ticks_per: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub input: String,
    pub hint: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse `{}`: {}", self.input, self.hint)
    }
}

impl std::error::Error for ParseError {}

impl Calendar {
    /// Build from config; expands `preset = "sols"` /
    /// `"gregorian"` shortcuts in place. `custom` is taken
    /// as-is. Empty `units` → defaults to a single base unit
    /// with the same name as `base_unit`.
    pub fn from_config(mut cfg: CalendarConfig) -> Self {
        expand_preset(&mut cfg);
        if cfg.units.is_empty() {
            cfg.units.push(UnitDef {
                name: cfg.base_unit.clone(),
                per_parent: 0,
                names: Vec::new(),
            });
        }
        if cfg.display_format.is_empty() {
            cfg.display_format = default_display_format(&cfg);
        }
        // ticks_per[0] = 1 (base unit); ticks_per[i] = ticks_per[i-1]
        // * units[i].per_parent. Multiplies up the stack so that
        // a year is `per_parent_of_year * ticks_per_of_month` ticks.
        let mut ticks_per: Vec<i64> = Vec::with_capacity(cfg.units.len());
        if !cfg.units.is_empty() {
            ticks_per.push(1);
            for unit in cfg.units.iter().skip(1) {
                let prev = *ticks_per.last().unwrap();
                let per = unit.per_parent.max(1) as i64;
                ticks_per.push(prev.saturating_mul(per));
            }
        }
        Self { cfg, ticks_per }
    }

    pub fn unit_names(&self) -> Vec<&str> {
        self.cfg.units.iter().map(|u| u.name.as_str()).collect()
    }

    pub fn ticks_per(&self, unit: &str) -> Option<i64> {
        let idx = self.cfg.units.iter().position(|u| u.name == unit)?;
        Some(self.ticks_per[idx])
    }

    /// Add `n` of `unit` to `p`. Unknown unit names return
    /// `p` unchanged so callers can chain safely; the CLI
    /// validates names up-front. Special-case `"season"`
    /// resolves against `seasons[]` (which sit between
    /// months and years in user mental model).
    pub fn add_units(&self, p: TimelinePoint, n: i64, unit: &str) -> TimelinePoint {
        let unit_lower = unit.to_ascii_lowercase();
        if unit_lower == "season" {
            // One season = average of season span_months.
            let avg = if self.cfg.seasons.is_empty() {
                3
            } else {
                let sum: u32 = self.cfg.seasons.iter().map(|s| s.span_months).sum();
                (sum / self.cfg.seasons.len() as u32).max(1)
            };
            return TimelinePoint(
                p.0 + n * self.ticks_per("month").unwrap_or(1) * (avg as i64),
            );
        }
        match self.ticks_per(&unit_lower) {
            Some(per) => TimelinePoint(p.0 + n * per),
            None => p,
        }
    }

    /// Inclusive start, exclusive end of the precision's
    /// containing window.
    pub fn fuzz_window(
        &self,
        p: TimelinePoint,
        prec: Precision,
    ) -> (TimelinePoint, TimelinePoint) {
        let span = self.span_for_precision(prec);
        if span <= 1 {
            return (p, TimelinePoint(p.0 + 1));
        }
        // Floor p to the nearest multiple of span (towards -∞)
        // so the window contains p naturally.
        let floor_ticks = if p.0 >= 0 {
            (p.0 / span) * span
        } else {
            -(((-p.0) + span - 1) / span * span)
        };
        (
            TimelinePoint(floor_ticks),
            TimelinePoint(floor_ticks + span),
        )
    }

    fn span_for_precision(&self, prec: Precision) -> i64 {
        match prec {
            Precision::Tick => 1,
            Precision::Hour => self.ticks_per("hour").unwrap_or(1),
            Precision::Day => self.ticks_per("day").unwrap_or(1),
            Precision::Week => self.ticks_per("day").unwrap_or(1).saturating_mul(7),
            Precision::Month => self.ticks_per("month").unwrap_or(
                self.ticks_per("day").unwrap_or(1).saturating_mul(30),
            ),
            Precision::Season => {
                // Average season length in months × ticks/month.
                let months_per = self.ticks_per("month").unwrap_or(
                    self.ticks_per("day").unwrap_or(1).saturating_mul(30),
                );
                let avg_months = if self.cfg.seasons.is_empty() {
                    3
                } else {
                    let sum: u32 = self.cfg.seasons.iter().map(|s| s.span_months).sum();
                    (sum / self.cfg.seasons.len() as u32).max(1)
                };
                months_per.saturating_mul(avg_months as i64)
            }
            Precision::Year => self.ticks_per("year").unwrap_or(
                self.ticks_per("day").unwrap_or(1).saturating_mul(365),
            ),
        }
    }

    /// Format ticks as a human-readable string. `prec`
    /// truncates finer fields; e.g. `Year` precision emits
    /// only the year segment.
    pub fn format(&self, p: TimelinePoint, prec: Precision) -> String {
        let breakdown = self.decompose(p);
        let year = breakdown.iter().last().copied().unwrap_or(0);
        let month = breakdown.get(self.index_of("month")).copied().unwrap_or(1);
        let day = breakdown.get(self.index_of("day")).copied().unwrap_or(1);
        let hour = breakdown.get(self.index_of("hour")).copied().unwrap_or(0);

        let epoch_token = if year >= 0 {
            self.cfg.epoch_label.clone()
        } else {
            self.cfg.epoch_before_label.clone()
        };
        let year_abs = year.abs();

        // Substitute placeholders, then truncate by precision.
        // We render the full string then strip everything
        // after the precision's slot to keep things simple.
        let mut out = self.cfg.display_format.clone();
        out = out.replace("{year}", &year_abs.to_string());
        out = out.replace("{epoch_label}", &epoch_token);
        out = out.replace("{epoch_before_label}", &epoch_token);
        out = out.replace("{day}", &day.to_string());
        out = out.replace("{hour}", &format!("{hour:02}"));
        out = out.replace(
            "{month-name}",
            &self.month_name(month as usize).unwrap_or_else(|| month.to_string()),
        );
        out = out.replace("{month}", &month.to_string());

        // Truncate by precision — strip trailing segments
        // smaller than `prec`. We use the `.` separator as
        // the cut point since that's the canonical shape.
        truncate_by_precision(&out, prec)
    }

    /// Decompose ticks into the unit stack, base-first.
    /// `out[0]` = base remainder, `out[len-1]` = top-level.
    /// For ticks_per_year arithmetic, top level is signed
    /// (negative for pre-epoch), lower units are 1-based
    /// positive (so "1A.1.1" = year 1, month 1, day 1 from
    /// epoch).
    fn decompose(&self, p: TimelinePoint) -> Vec<i64> {
        let mut out: Vec<i64> = vec![0; self.cfg.units.len()];
        let total = p.0;
        if total >= 0 {
            let mut remaining = total;
            for i in (0..self.cfg.units.len()).rev() {
                let per = self.ticks_per[i];
                if per == 0 {
                    continue;
                }
                let value = remaining / per;
                remaining -= value * per;
                // Bottom three are 1-based for display; top
                // is 0-based + 1 to read "year 1" instead of
                // "year 0".
                if i == self.cfg.units.len() - 1 {
                    out[i] = value + 1; // year 1 = epoch
                } else {
                    out[i] = value + 1;
                }
            }
        } else {
            // Negative: mirror around -1 so the smallest
            // pre-epoch tick is `year -1, month 1, day 1`.
            let positive = -total - 1;
            let mut remaining = positive;
            for i in (0..self.cfg.units.len()).rev() {
                let per = self.ticks_per[i];
                if per == 0 {
                    continue;
                }
                let value = remaining / per;
                remaining -= value * per;
                if i == self.cfg.units.len() - 1 {
                    out[i] = -(value + 1);
                } else {
                    // Reverse subordinate so "year -1 day 1"
                    // is the latest tick BEFORE the epoch.
                    let max_minus_1 = self.cfg.units[i].per_parent as i64 - 1;
                    out[i] = max_minus_1 - value + 1;
                }
            }
        }
        out
    }

    fn month_name(&self, idx_one_based: usize) -> Option<String> {
        let unit = self.cfg.units.iter().find(|u| u.name == "month")?;
        if unit.names.is_empty() || idx_one_based == 0 {
            return None;
        }
        unit.names.get(idx_one_based - 1).cloned()
    }

    /// Map the top-most defined unit's name to a `Precision`.
    /// Falls back to `Year` when the calendar is "full" (has
    /// `year` defined) or to whatever the topmost unit maps to.
    fn top_unit_precision(&self) -> Precision {
        let top = self
            .cfg
            .units
            .last()
            .map(|u| u.name.as_str())
            .unwrap_or("year");
        match top {
            "hour" => Precision::Hour,
            "day" => Precision::Day,
            "week" => Precision::Week,
            "month" => Precision::Month,
            "year" => Precision::Year,
            _ => Precision::Year,
        }
    }

    fn index_of(&self, name: &str) -> usize {
        self.cfg
            .units
            .iter()
            .position(|u| u.name == name)
            .unwrap_or(usize::MAX)
    }

    /// Parse a user-typed string into `(TimelinePoint,
    /// inferred_precision)`. Walks two shapes:
    ///
    ///   * Alias match (`parse_aliases[].match` → ticks).
    ///   * Numeric / named segments separated by `.`, with
    ///     an optional epoch label suffix on the year.
    pub fn parse(&self, s: &str) -> Result<(TimelinePoint, Precision), ParseError> {
        let raw = s.trim();
        if raw.is_empty() {
            return Err(ParseError {
                input: s.to_owned(),
                hint: "empty timeline input".into(),
            });
        }
        // Alias pass.
        for alias in &self.cfg.parse_aliases {
            if alias.matches.eq_ignore_ascii_case(raw) {
                // Aliases are by convention day-precision
                // markers — they're landmark days, not
                // year-only spans.
                return Ok((TimelinePoint(alias.ticks), Precision::Day));
            }
        }

        // Strip an optional epoch label off the year segment.
        // The label may sit immediately after digits or be
        // separated by whitespace.
        let (year_str, body, is_before) = split_year_and_label(
            raw,
            &self.cfg.epoch_label,
            &self.cfg.epoch_before_label,
        );

        let year: i64 = year_str.parse().map_err(|_| ParseError {
            input: s.to_owned(),
            hint: format!("can't parse year segment `{year_str}`"),
        })?;
        let year = if is_before { -year.abs() } else { year };

        // Walk dotted segments under the year.
        let mut segments: Vec<&str> =
            body.split('.').map(str::trim).filter(|s| !s.is_empty()).collect();
        // First segment after the year is the month, then
        // day, then hour. Each is either numeric or a name
        // (month-name / season-name at month slot).
        let mut month: i64 = 1;
        let mut day: i64 = 1;
        let mut hour: i64 = 0;
        // Default precision = the top-most defined unit.
        // For sols (only `day`) the input `Sol 5` is a Day
        // statement, not a Year statement.
        let mut precision = self.top_unit_precision();

        if let Some(month_seg) = segments.first().copied() {
            // Try season name first (so "1A.spring" precision = Season).
            if let Some(season) = self.season_by_name_prefix(month_seg) {
                month = season.start_month as i64;
                day = 1;
                segments.remove(0);
                precision = Precision::Season;
            } else if let Some(month_idx) = self.month_index_by_name(month_seg) {
                month = month_idx as i64;
                segments.remove(0);
                precision = Precision::Month;
            } else if let Ok(n) = month_seg.parse::<i64>() {
                month = n;
                segments.remove(0);
                precision = Precision::Month;
            } else {
                return Err(ParseError {
                    input: s.to_owned(),
                    hint: format!(
                        "unknown month/season name `{month_seg}` (try numeric form, e.g. `{year}.3`)"
                    ),
                });
            }
        }
        if let Some(day_seg) = segments.first().copied() {
            let n: i64 = day_seg.parse().map_err(|_| ParseError {
                input: s.to_owned(),
                hint: format!("can't parse day segment `{day_seg}`"),
            })?;
            day = n;
            segments.remove(0);
            // Day overrides Season precision (user got specific).
            precision = Precision::Day;
        }
        if let Some(hour_seg) = segments.first().copied() {
            let n: i64 = hour_seg.parse().map_err(|_| ParseError {
                input: s.to_owned(),
                hint: format!("can't parse hour segment `{hour_seg}`"),
            })?;
            hour = n;
            segments.remove(0);
            precision = Precision::Hour;
        }
        if !segments.is_empty() {
            return Err(ParseError {
                input: s.to_owned(),
                hint: format!("trailing segments `{}` not understood", segments.join(".")),
            });
        }

        let ticks = self.compose(year, month, day, hour)?;
        Ok((TimelinePoint(ticks), precision))
    }

    fn season_by_name_prefix(&self, s: &str) -> Option<&SeasonDef> {
        let needle = s.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return None;
        }
        // Prefer exact match; fall back to prefix.
        let exact = self
            .cfg
            .seasons
            .iter()
            .find(|sd| sd.name.eq_ignore_ascii_case(&needle));
        if exact.is_some() {
            return exact;
        }
        self.cfg
            .seasons
            .iter()
            .find(|sd| sd.name.to_ascii_lowercase().starts_with(&needle))
    }

    fn month_index_by_name(&self, s: &str) -> Option<usize> {
        let unit = self.cfg.units.iter().find(|u| u.name == "month")?;
        if unit.names.is_empty() {
            return None;
        }
        let needle = s.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return None;
        }
        // Exact wins; prefix only when unambiguous.
        for (i, n) in unit.names.iter().enumerate() {
            if n.eq_ignore_ascii_case(&needle) {
                return Some(i + 1);
            }
        }
        let mut hits: Vec<usize> = Vec::new();
        for (i, n) in unit.names.iter().enumerate() {
            if n.to_ascii_lowercase().starts_with(&needle) {
                hits.push(i + 1);
            }
        }
        if hits.len() == 1 {
            Some(hits[0])
        } else {
            None
        }
    }

    fn compose(
        &self,
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
    ) -> Result<i64, ParseError> {
        // The "year" value is actually the value at the TOP
        // of the unit stack. For sols (only `day` defined)
        // the user's input `Sol 5` maps to a day value of 5,
        // not a year value of 5 with no smaller units.
        // Walk the stack top→bottom and place inputs.
        let top_idx = self.cfg.units.len().saturating_sub(1);
        let top_ticks = self.ticks_per.get(top_idx).copied().unwrap_or(1);

        let top_value = year;
        if top_value == 0 {
            return Err(ParseError {
                input: format!("{top_value}"),
                hint: "year 0 (top-unit value 0) doesn't exist — use `1` for the epoch or `-1` for the value before"
                    .into(),
            });
        }
        // Below-the-top inputs (month / day / hour) only
        // matter when those units exist in the stack.
        let ticks_per_month = self.ticks_per("month").unwrap_or(0);
        let ticks_per_day = self.ticks_per("day").unwrap_or(0);
        let ticks_per_hour = self.ticks_per("hour").unwrap_or(0);
        let m0 = (month - 1).max(0);
        let d0 = (day - 1).max(0);
        let h0 = hour.max(0);
        let within = ticks_per_month.saturating_mul(m0)
            + ticks_per_day.saturating_mul(d0)
            + ticks_per_hour.saturating_mul(h0);

        if top_value > 0 {
            Ok(top_ticks.saturating_mul(top_value - 1) + within)
        } else {
            // Negative top: ticks fall in
            // [-top_ticks*|n|, -top_ticks*(|n|-1) )
            let base = -(top_ticks.saturating_mul(top_value.abs()));
            Ok(base + within)
        }
    }
}

fn truncate_by_precision(s: &str, prec: Precision) -> String {
    // We only need to drop trailing dotted segments past the
    // precision's slot. Year = keep first segment; Month =
    // first two; Day = first three; Hour = first four.
    // Season is handled at parse-time (we composed it as
    // month-with-day=1) so for format purposes we keep two
    // segments.
    let keep = match prec {
        Precision::Year => 1,
        Precision::Season => 2,
        Precision::Month => 2,
        Precision::Day => 3,
        Precision::Hour => 4,
        Precision::Tick | Precision::Week => return s.to_owned(),
    };
    let mut parts: Vec<&str> = s.split('.').collect();
    if parts.len() > keep {
        parts.truncate(keep);
    }
    parts.join(".")
}

fn split_year_and_label<'a>(
    raw: &'a str,
    epoch: &str,
    before: &str,
) -> (String, &'a str, bool) {
    // First dotted segment is the year + optional label.
    let first_dot = raw.find('.').unwrap_or(raw.len());
    let year_seg = &raw[..first_dot];
    let rest = if first_dot < raw.len() {
        &raw[first_dot + 1..]
    } else {
        ""
    };
    // Try the longer label first to avoid swallowing the
    // shorter one as a prefix of digits.
    let mut yseg = year_seg.trim().to_owned();
    let mut is_before = false;

    // Try stripping the label from either end. Trailing
    // form (`1A`) is the canonical custom-calendar shape;
    // leading form (`Sol 5`) lets the sols preset use a more
    // natural display string. Whichever side strips cleanly
    // wins.
    let try_strip = |s: &str, label: &str| -> Option<String> {
        if label.is_empty() {
            return None;
        }
        let lower = s.to_ascii_lowercase();
        let llabel = label.to_ascii_lowercase();
        if lower.ends_with(&llabel) {
            return Some(s[..s.len() - label.len()].trim().to_owned());
        }
        if lower.starts_with(&llabel) {
            return Some(s[label.len()..].trim().to_owned());
        }
        None
    };

    if let Some(stripped) = try_strip(&yseg, before) {
        yseg = stripped;
        is_before = true;
    } else if let Some(stripped) = try_strip(&yseg, epoch) {
        yseg = stripped;
    }
    (yseg, rest, is_before)
}

fn default_display_format(cfg: &CalendarConfig) -> String {
    // Pick a format that covers every defined unit. If only
    // a base unit exists ("sols"-style), emit just that.
    let mut parts: Vec<&str> = Vec::new();
    let mut has = |name: &str| cfg.units.iter().any(|u| u.name == name);
    if has("year") {
        parts.push("{year}{epoch_label}");
    } else if has("day") {
        // sols-style: only days.
        return format!(
            "{}{{day}}",
            if cfg.epoch_label.is_empty() {
                String::new()
            } else {
                format!("{} ", cfg.epoch_label)
            }
        );
    } else {
        return "{year}".to_owned();
    }
    if has("month") {
        parts.push(".{month}");
    }
    if has("day") {
        parts.push(".{day}");
    }
    parts.join("")
}

fn expand_preset(cfg: &mut CalendarConfig) {
    let preset = cfg.preset.trim().to_ascii_lowercase();
    match preset.as_str() {
        "sols" => {
            if cfg.units.is_empty() {
                cfg.units.push(UnitDef {
                    name: "day".to_owned(),
                    per_parent: 0,
                    names: Vec::new(),
                });
            }
            if cfg.epoch_label.is_empty() {
                cfg.epoch_label = "Sol".to_owned();
            }
            if cfg.display_format.is_empty() {
                cfg.display_format = "Sol {day}".to_owned();
            }
        }
        "gregorian" => {
            if cfg.units.is_empty() {
                cfg.units = vec![
                    UnitDef {
                        name: "day".to_owned(),
                        per_parent: 0,
                        names: Vec::new(),
                    },
                    UnitDef {
                        name: "month".to_owned(),
                        per_parent: 30,
                        names: vec![
                            "January", "February", "March", "April",
                            "May", "June", "July", "August",
                            "September", "October", "November", "December",
                        ]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    },
                    UnitDef {
                        name: "year".to_owned(),
                        per_parent: 12,
                        names: Vec::new(),
                    },
                ];
            }
            if cfg.seasons.is_empty() {
                cfg.seasons = vec![
                    SeasonDef { name: "winter".into(), start_month: 12, span_months: 3 },
                    SeasonDef { name: "spring".into(), start_month: 3,  span_months: 3 },
                    SeasonDef { name: "summer".into(), start_month: 6,  span_months: 3 },
                    SeasonDef { name: "autumn".into(), start_month: 9,  span_months: 3 },
                ];
            }
            if cfg.display_format.is_empty() {
                cfg.display_format = "{year}{epoch_label}.{month}.{day}".to_owned();
            }
        }
        _ => {} // custom — leave user values alone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sols() -> Calendar {
        Calendar::from_config(CalendarConfig {
            preset: "sols".into(),
            ..Default::default()
        })
    }

    fn gregorian() -> Calendar {
        Calendar::from_config(CalendarConfig {
            preset: "gregorian".into(),
            ..Default::default()
        })
    }

    fn custom_aerin() -> Calendar {
        Calendar::from_config(CalendarConfig {
            preset: "custom".into(),
            base_unit: "day".into(),
            units: vec![
                UnitDef { name: "day".into(),   per_parent: 0,  names: vec![] },
                UnitDef { name: "month".into(), per_parent: 30, names: vec![
                    "Frostmoon".into(), "Snowfall".into(), "Greenstart".into(),
                    "Bloomtide".into(), "Highsun".into(), "Goldfall".into(),
                    "Mistwane".into(), "Stormrise".into(), "Coldgate".into(),
                    "Longnight".into(), "Hearthlit".into(), "Yearfall".into(),
                ] },
                UnitDef { name: "year".into(),  per_parent: 12, names: vec![] },
            ],
            seasons: vec![
                SeasonDef { name: "winter".into(), start_month: 1, span_months: 3 },
                SeasonDef { name: "spring".into(), start_month: 4, span_months: 3 },
                SeasonDef { name: "summer".into(), start_month: 7, span_months: 3 },
                SeasonDef { name: "autumn".into(), start_month: 10, span_months: 3 },
            ],
            epoch_label: "A".into(),
            epoch_before_label: "BA".into(),
            display_format: "{year}{epoch_label}.{month}.{day}".into(),
            parse_aliases: vec![ParseAlias { matches: "Founding".into(), ticks: 0 }],
        })
    }

    #[test]
    fn sols_format_simple() {
        let c = sols();
        assert_eq!(c.format(TimelinePoint(0), Precision::Day), "Sol 1");
        assert_eq!(c.format(TimelinePoint(142), Precision::Day), "Sol 143");
    }

    #[test]
    fn sols_parse_roundtrip() {
        let c = sols();
        let (p, prec) = c.parse("Sol 5").unwrap_or_else(|e| {
            // sols preset uses "Sol N" as the form
            panic!("parse failed: {e}");
        });
        assert_eq!(p, TimelinePoint(4)); // Sol 5 → 4 ticks past Sol 1
        assert_eq!(prec, Precision::Day);
    }

    #[test]
    fn aerin_year_only_precision() {
        let c = custom_aerin();
        let (p, prec) = c.parse("1A").unwrap();
        assert_eq!(p, TimelinePoint(0));
        assert_eq!(prec, Precision::Year);
        assert_eq!(c.format(p, prec), "1A");
    }

    #[test]
    fn aerin_full_form() {
        let c = custom_aerin();
        let (p, prec) = c.parse("1A.3.15").unwrap();
        assert_eq!(prec, Precision::Day);
        // year 1, month 3, day 15 = (3-1)*30 + (15-1) = 74
        assert_eq!(p, TimelinePoint(74));
        assert_eq!(c.format(p, prec), "1A.3.15");
    }

    #[test]
    fn aerin_month_by_name() {
        let c = custom_aerin();
        let (p, prec) = c.parse("1A.Greenstart").unwrap();
        assert_eq!(prec, Precision::Month);
        // year 1, month 3, day 1 = (3-1)*30 = 60
        assert_eq!(p, TimelinePoint(60));
    }

    #[test]
    fn aerin_month_by_prefix() {
        let c = custom_aerin();
        // "Frost" matches Frostmoon uniquely
        let (p, _) = c.parse("1A.Frost").unwrap();
        assert_eq!(p, TimelinePoint(0));
    }

    #[test]
    fn aerin_season_precision() {
        let c = custom_aerin();
        let (p, prec) = c.parse("3A.spring").unwrap();
        assert_eq!(prec, Precision::Season);
        // year 3, month 4 = (3-1)*360 + (4-1)*30 = 810
        assert_eq!(p, TimelinePoint(810));
    }

    #[test]
    fn aerin_alias_landmark() {
        let c = custom_aerin();
        let (p, prec) = c.parse("Founding").unwrap();
        assert_eq!(p, TimelinePoint(0));
        assert_eq!(prec, Precision::Day);
    }

    #[test]
    fn aerin_negative_year() {
        let c = custom_aerin();
        let (p, prec) = c.parse("-1BA").unwrap();
        assert_eq!(prec, Precision::Year);
        // year -1 starts at -1*360 = -360 ticks.
        assert_eq!(p, TimelinePoint(-360));
    }

    #[test]
    fn parse_error_year_zero() {
        let c = custom_aerin();
        let err = c.parse("0A.3.5").unwrap_err();
        assert!(err.hint.contains("year 0"));
    }

    #[test]
    fn parse_error_unknown_month_name() {
        let c = custom_aerin();
        let err = c.parse("1A.frosbun").unwrap_err();
        assert!(err.hint.contains("unknown month"));
    }

    #[test]
    fn gregorian_default_format_walks() {
        let c = gregorian();
        let (p, prec) = c.parse("2024.5.20").unwrap();
        assert_eq!(prec, Precision::Day);
        assert_eq!(c.format(p, prec), "2024.5.20");
    }

    #[test]
    fn add_units_walks_day_then_month() {
        let c = custom_aerin();
        let (p, _) = c.parse("1A.3.10").unwrap();
        let later = c.add_units(p, 5, "day");
        assert_eq!(c.format(later, Precision::Day), "1A.3.15");
        let next_month = c.add_units(p, 1, "month");
        assert_eq!(c.format(next_month, Precision::Day), "1A.4.10");
    }

    #[test]
    fn fuzz_window_day() {
        let c = custom_aerin();
        let (p, _) = c.parse("1A.3.10").unwrap();
        let (lo, hi) = c.fuzz_window(p, Precision::Day);
        assert_eq!(hi.0 - lo.0, 1);
        assert!(p.0 >= lo.0 && p.0 < hi.0);
    }

    #[test]
    fn fuzz_window_year() {
        let c = custom_aerin();
        let (p, _) = c.parse("1A.3.10").unwrap();
        let (lo, hi) = c.fuzz_window(p, Precision::Year);
        assert_eq!(hi.0 - lo.0, 360); // 12 months * 30 days
        assert!(p.0 >= lo.0 && p.0 < hi.0);
    }

    #[test]
    fn format_precision_truncates() {
        let c = custom_aerin();
        let (p, _) = c.parse("1A.3.15").unwrap();
        assert_eq!(c.format(p, Precision::Year), "1A");
        assert_eq!(c.format(p, Precision::Month), "1A.3");
        assert_eq!(c.format(p, Precision::Day), "1A.3.15");
    }
}

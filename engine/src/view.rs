//! What the city currently shows: which metric drives colour, what is filtered
//! out, and what is highlighted.
//!
//! Colour is the strongest channel available and it was permanently spent on
//! one metric (district purpose). Height, footprint, saturation, roof cap and
//! plinth are all taken, so a seventh simultaneous channel would be
//! unreadable. Making colour switchable gives every metric a full-strength
//! palette when it is asked for, and nothing has to compete.

use crate::hover::BuildingInfo;
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColorMode {
    /// What the district is for. The default: it is the only mode that says
    /// something about every building at once.
    #[default]
    Typology,
    /// Worst cyclomatic complexity among the building's functions.
    Complexity,
    /// Days since the last commit touching the file.
    Age,
    /// Commit churn percentile.
    Churn,
    /// How many other buildings depend on this one.
    FanIn,
    /// Martin's instability: 0 depended upon, 1 depends on others.
    Instability,
    /// Source language, for mixed repositories.
    Language,
}

impl ColorMode {
    pub fn parse(s: &str) -> ColorMode {
        match s {
            "complexity" => ColorMode::Complexity,
            "age" => ColorMode::Age,
            "churn" => ColorMode::Churn,
            "fanin" => ColorMode::FanIn,
            "instability" => ColorMode::Instability,
            "language" => ColorMode::Language,
            _ => ColorMode::Typology,
        }
    }
}

/// Predicates the view is restricted to. Everything that fails becomes
/// translucent rather than hidden, so the shape of the city is preserved and
/// matches are read in their real context.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub district: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub min_methods: u32,
    #[serde(default)]
    pub min_complexity: u32,
    #[serde(default)]
    pub min_churn_pct: f64,
    #[serde(default)]
    pub only_no_callers: bool,
    #[serde(default)]
    pub only_hubs: bool,
    #[serde(default)]
    pub only_cycles: bool,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.language.is_empty()
            && self.district.is_empty()
            && self.kind.is_empty()
            && self.min_methods == 0
            && self.min_complexity == 0
            && self.min_churn_pct <= 0.0
            && !self.only_no_callers
            && !self.only_hubs
            && !self.only_cycles
    }

    pub fn matches(&self, b: &BuildingInfo) -> bool {
        if !self.text.is_empty() && !b.name.to_lowercase().contains(&self.text.to_lowercase()) {
            return false;
        }
        if !self.language.is_empty() && b.language != self.language {
            return false;
        }
        if !self.district.is_empty() && b.district_name != self.district {
            return false;
        }
        if !self.kind.is_empty() && b.kind != self.kind {
            return false;
        }
        if b.num_methods < self.min_methods {
            return false;
        }
        if b.max_complexity < self.min_complexity {
            return false;
        }
        if b.churn_rank * 100.0 < self.min_churn_pct {
            return false;
        }
        // "No callers" is a question, not a verdict: a library's whole public
        // surface has no internal callers and is not dead.
        if self.only_no_callers && b.fan_in > 0 {
            return false;
        }
        if self.only_hubs && !b.hub {
            return false;
        }
        if self.only_cycles && b.cycle_id == 0 {
            return false;
        }
        true
    }
}

/// A sequential palette, pale to hot. Used by every numeric mode so that
/// "more" always looks the same regardless of which metric is selected.
fn ramp(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    // pale sand -> amber -> red
    const STOPS: [[f32; 3]; 4] = [
        [0.86, 0.85, 0.78],
        [0.88, 0.72, 0.36],
        [0.85, 0.44, 0.22],
        [0.72, 0.16, 0.17],
    ];
    let scaled = t * (STOPS.len() - 1) as f32;
    let i = scaled.floor() as usize;
    let j = (i + 1).min(STOPS.len() - 1);
    let f = scaled - i as f32;
    [
        STOPS[i][0] + (STOPS[j][0] - STOPS[i][0]) * f,
        STOPS[i][1] + (STOPS[j][1] - STOPS[i][1]) * f,
        STOPS[i][2] + (STOPS[j][2] - STOPS[i][2]) * f,
    ]
}

/// Categorical colour for a language, distinct from every typology hue.
fn language_rgb(lang: &str) -> [f32; 3] {
    match lang {
        "C" => [0.38, 0.55, 0.78],
        "Rust" => [0.80, 0.47, 0.28],
        "Go" => [0.35, 0.72, 0.76],
        _ => [0.62, 0.62, 0.64],
    }
}

/// Scale for the numeric modes, so one outlier does not flatten everything
/// else. Set once per city from the data actually present.
#[derive(Clone, Copy, Debug)]
pub struct Scales {
    pub complexity: f32,
    pub fan_in: f32,
}

impl Default for Scales {
    fn default() -> Self {
        Scales { complexity: 20.0, fan_in: 20.0 }
    }
}

/// The colour a building takes in the current mode.
pub fn color_for(mode: ColorMode, b: &BuildingInfo, typology_rgb: [f32; 3], s: Scales) -> [f32; 3] {
    match mode {
        ColorMode::Typology => typology_rgb,
        ColorMode::Complexity => ramp(b.max_complexity as f32 / s.complexity.max(1.0)),
        // Recent code is hot, old code is pale: the same direction as every
        // other numeric mode, where more means hotter.
        ColorMode::Age => ramp(1.0 - (b.age_days as f32 / 540.0).clamp(0.0, 1.0)),
        ColorMode::Churn => ramp(b.churn_rank as f32),
        ColorMode::FanIn => ramp(b.fan_in as f32 / s.fan_in.max(1.0)),
        ColorMode::Instability => ramp(b.instability as f32),
        ColorMode::Language => language_rgb(&b.language),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> BuildingInfo {
        BuildingInfo {
            name: "Widget".into(),
            kind: "type".into(),
            language: "C".into(),
            district_name: "Core".into(),
            num_methods: 4,
            max_complexity: 9,
            fan_in: 3,
            churn_rank: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn empty_filter_matches_everything() {
        let f = Filter::default();
        assert!(f.is_empty());
        assert!(f.matches(&info()));
    }

    #[test]
    fn predicates_compose_with_and() {
        let mut f = Filter { language: "C".into(), min_methods: 4, ..Default::default() };
        assert!(f.matches(&info()));
        f.min_methods = 5;
        assert!(!f.matches(&info()), "min_methods should exclude");
        f.min_methods = 4;
        f.language = "Rust".into();
        assert!(!f.matches(&info()), "language should exclude");
    }

    #[test]
    fn text_match_is_case_insensitive_substring() {
        let f = Filter { text: "wid".into(), ..Default::default() };
        assert!(f.matches(&info()));
        let f = Filter { text: "nope".into(), ..Default::default() };
        assert!(!f.matches(&info()));
    }

    #[test]
    fn no_callers_selects_only_unreferenced() {
        let f = Filter { only_no_callers: true, ..Default::default() };
        assert!(!f.matches(&info()));
        let mut b = info();
        b.fan_in = 0;
        assert!(f.matches(&b));
    }

    #[test]
    fn ramp_is_monotonic_and_bounded() {
        let a = ramp(0.0);
        let b = ramp(1.0);
        assert!(b[0] < a[0] || b[2] < a[2], "ramp should darken toward hot");
        for t in [-1.0, 0.0, 0.5, 1.0, 2.0] {
            for c in ramp(t) {
                assert!((0.0..=1.0).contains(&c), "channel out of range at t={t}");
            }
        }
    }
}

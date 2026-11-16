//! The parts of the backend's CityMap the engine reads. Everything else stays
//! in the frontend, which holds the full JSON.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Building {
    pub id: String,
    pub name: String,
    /// "type" (a struct/union/enum) or "module" (the free functions of a file).
    pub kind: String,
    pub dir: String,
    pub language: String,
    pub num_fields: u32,
    pub num_methods: u32,
    pub lines_of_code: u32,
    pub max_complexity: u32,
    pub fan_in: u32,
    pub instability: f64,
    pub hub: bool,
    /// 0..1 percentile of commit activity within this repository.
    pub churn_rank: f64,
    pub age_days: u32,
    /// Days between the repository's first commit and this file's first.
    pub born_day: u32,
    /// Dependency cycle this building belongs to, or 0.
    pub cycle_id: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct District {
    pub id: String,
    pub name: String,
    pub typology: String,
    pub buildings: Vec<Building>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DependencyEdge {
    pub source: String,
    pub target: String,
    pub weight: u32,
    pub in_cycle: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CityMap {
    pub districts: Vec<District>,
    pub dependencies: Vec<DependencyEdge>,
}

impl CityMap {
    pub fn building_count(&self) -> usize {
        self.districts.iter().map(|d| d.buildings.len()).sum()
    }
}

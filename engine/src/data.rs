use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Building {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// "type" (a struct/union/enum) or "module" (the free functions of a file).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub source_file: String,
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub num_fields: u32,
    #[serde(default)]
    pub num_methods: u32,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub lines_of_code: u32,
    #[serde(default)]
    pub max_complexity: u32,
    #[serde(default)]
    pub sum_complexity: u32,
    #[serde(default)]
    pub fan_in: u32,
    #[serde(default)]
    pub fan_out: u32,
    #[serde(default)]
    pub instability: f64,
    #[serde(default)]
    pub hub: bool,
    #[serde(default)]
    pub commit_churn: u32,
    /// 0..1 percentile of commit activity within this repository.
    #[serde(default)]
    pub churn_rank: f64,
    #[serde(default)]
    pub last_modified: String,
    #[serde(default)]
    pub age_days: u32,
    /// Days between the repository's first commit and this file's first.
    #[serde(default)]
    pub born_day: u32,
    #[serde(default)]
    pub primary_author: String,
    #[serde(default)]
    pub summary: String,
    /// "rule" | "llm" | "none" — how this building's methods were attributed.
    #[serde(default)]
    pub assoc_source: String,
    /// Dependency cycle this building belongs to, or 0.
    #[serde(default)]
    pub cycle_id: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct District {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub typology: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub buildings: Vec<Building>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DependencyEdge {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub weight: u32,
    /// "call" or "type".
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub in_cycle: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CityStats {
    #[serde(default)]
    pub total_buildings: u32,
    #[serde(default)]
    pub total_types: u32,
    #[serde(default)]
    pub total_modules: u32,
    #[serde(default)]
    pub total_methods: u32,
    #[serde(default)]
    pub total_files: u32,
    #[serde(default)]
    pub languages: std::collections::HashMap<String, u32>,
    #[serde(default)]
    pub total_loc: u32,
    #[serde(default)]
    pub methods_by_rule: u32,
    #[serde(default)]
    pub methods_by_llm: u32,
    #[serde(default)]
    pub orphans: u32,
    #[serde(default)]
    pub skipped_dirs: Vec<String>,
    #[serde(default)]
    pub llm_calls: u32,
    #[serde(default)]
    pub refined: bool,
    #[serde(default)]
    pub history_days: u32,
    #[serde(default)]
    pub first_commit: String,
    #[serde(default)]
    pub last_commit: String,
    #[serde(default)]
    pub cycles: Vec<Cycle>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Cycle {
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub size: u32,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub namespaces: u32,
    #[serde(default)]
    pub reliable: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CityMap {
    #[serde(default)]
    pub districts: Vec<District>,
    #[serde(default)]
    pub dependencies: Vec<DependencyEdge>,
    #[serde(default)]
    pub stats: CityStats,
}

impl CityMap {
    pub fn building_count(&self) -> usize {
        self.districts.iter().map(|d| d.buildings.len()).sum()
    }
}

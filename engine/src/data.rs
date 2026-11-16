use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Building {
    pub id: String,
    pub dir: String,
    pub num_fields: u32,
    pub num_methods: u32,
    pub lines_of_code: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct District {
    pub id: String,
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

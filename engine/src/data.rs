use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Building {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source_file: String,
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
    pub commit_churn: u32,
    #[serde(default)]
    pub last_modified: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct District {
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
pub struct Road {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub full_path: String,
    #[serde(default)]
    pub depth: u32,
    #[serde(default)]
    pub file_count: u32,
    #[serde(default)]
    pub buildings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DependencyEdge {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub weight: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CityMap {
    #[serde(default)]
    pub districts: Vec<District>,
    #[serde(default)]
    pub roads: Vec<Road>,
    #[serde(default)]
    pub dependencies: Vec<DependencyEdge>,
}

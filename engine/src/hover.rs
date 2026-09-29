use bevy::prelude::*;
use serde::Serialize;

/// Lightweight payload for the cursor tooltip. Kept small because it is
/// dispatched to JS on every hover change.
#[derive(Serialize, Clone, Debug, Default)]
pub struct HoverInfo {
    pub kind: String, // "building" | "district" | "link"
    pub name: String,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct BuildingInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source_file: String,
    pub dir: String,
    pub language: String,
    pub district_name: String,
    pub typology: String,
    pub num_fields: u32,
    pub num_methods: u32,
    pub lines_of_code: u32,
    pub max_complexity: u32,
    pub sum_complexity: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub instability: f64,
    pub hub: bool,
    pub commit_churn: u32,
    pub churn_rank: f64,
    pub last_modified: String,
    pub age_days: u32,
    pub born_day: u32,
    pub primary_author: String,
    pub summary: String,
    pub assoc_source: String,
    pub cycle_id: u32,
    pub fields: Vec<String>,
    pub methods: Vec<String>,
    pub calls: Vec<String>,
    pub called_by: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct DistrictInfo {
    pub id: String,
    pub name: String,
    pub typology: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub building_count: usize,
    pub total_lines_of_code: u32,
    pub total_methods: u32,
    pub top_buildings: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct CitySummary {
    pub total_buildings: u32,
    pub total_types: u32,
    pub total_modules: u32,
    pub total_methods: u32,
    pub total_files: u32,
    pub total_loc: u32,
    pub languages: Vec<(String, u32)>,
    pub cycles: Vec<CycleInfo>,
    pub history_days: u32,
    pub first_commit: String,
    pub last_commit: String,
    pub methods_by_rule: u32,
    pub methods_by_llm: u32,
    pub orphans: u32,
    pub llm_calls: u32,
    pub refined: bool,
    pub skipped_dirs: Vec<String>,
    pub districts: Vec<DistrictInfo>,
    /// Every building name, so the frontend can offer search without holding
    /// the whole city map.
    pub index: Vec<IndexEntry>,
}

/// The result of a path query, sent back so the panel can name the hops.
/// Highlighting alone leaves the user guessing at what was found.
#[derive(Serialize, Clone, Debug, Default)]
pub struct PathInfo {
    pub from_id: String,
    pub from_name: String,
    pub to_id: String,
    pub to_name: String,
    /// Building names in order, empty when no path exists.
    pub hops: Vec<String>,
}

/// A reported dependency cycle, for the panel.
#[derive(Serialize, Clone, Debug, Default)]
pub struct CycleInfo {
    pub id: u32,
    pub size: u32,
    pub namespaces: u32,
    pub members: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub district: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum SelectPayload {
    None,
    Building(Box<BuildingInfo>),
    District(Box<DistrictInfo>),
}

/// Marks a building entity and carries everything picking and selection need.
#[derive(Component)]
pub struct Pickable {
    pub info: Box<BuildingInfo>,
    pub district_idx: usize,
    /// Footprint in district-local space; picking rotates the ray into this
    /// frame once per district rather than per building.
    pub local_min: Vec2,
    pub local_max: Vec2,
    pub y_min: f32,
    pub y_max: f32,
    /// The building's colour under the typology view, kept so switching colour
    /// mode never has to re-read the city map.
    pub typology_rgb: [f32; 3],
}

#[derive(Component)]
pub struct PickableDistrict {
    pub info: Box<DistrictInfo>,
    pub polygon: Vec<(f64, f64)>,
    pub district_idx: usize,
}

#[derive(Component)]
pub struct DistrictLabel {
    pub world: Vec3,
    pub district_idx: usize,
}

#[derive(Resource, Default)]
pub struct SelectionState {
    pub hovered: Option<Entity>,
    pub selected: Option<Entity>,
    pub last_cursor: Option<Vec2>,
    pub last_camera: Option<(Vec3, Quat)>,
}

fn dispatch(event_name: &str, payload: &impl Serialize) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = serde_wasm_bindgen::to_value(payload) {
                let init = web_sys::CustomEventInit::new();
                init.set_detail(&val);
                init.set_bubbles(true);
                if let Ok(ev) = web_sys::CustomEvent::new_with_event_init_dict(event_name, &init) {
                    let _ = window.dispatch_event(&ev);
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (event_name, payload);
    }
}

pub fn dispatch_hover(p: &Option<HoverInfo>) {
    dispatch("repolis:hover", p);
}
pub fn dispatch_select(p: &SelectPayload) {
    dispatch("repolis:select", p);
}
pub fn dispatch_city(p: &CitySummary) {
    dispatch("repolis:city", p);
}
pub fn dispatch_path(p: &Option<PathInfo>) {
    dispatch("repolis:path", p);
}

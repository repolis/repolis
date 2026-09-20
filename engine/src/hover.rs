use bevy::prelude::*;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct BuildingHoverInfo {
    pub name: String,
    pub source_file: String,
    pub district_name: String,
    pub typology: String,
    pub num_fields: u32,
    pub num_methods: u32,
    pub lines_of_code: u32,
    pub commit_churn: u32,
    pub last_modified: String,
    pub summary: String,
    pub fields: Vec<String>,
    pub methods: Vec<String>,
    pub dependencies: Vec<String>,
    pub callers: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct DistrictHoverInfo {
    pub name: String,
    pub typology: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub building_count: usize,
    pub total_lines_of_code: u32,
    pub buildings: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct RoadHoverInfo {
    pub name: String,
    pub road_type: String,
    pub source: String,
    pub target: String,
    pub weight: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum HoverPayload {
    None,
    Building(BuildingHoverInfo),
    District(DistrictHoverInfo),
    Road(RoadHoverInfo),
}

#[derive(Component)]
pub struct PickableBuilding {
    pub info: BuildingHoverInfo,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub default_material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct PickableDistrict {
    pub info: DistrictHoverInfo,
    pub polygon: Vec<(f64, f64)>,
    pub default_material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct PickableRoad {
    pub info: RoadHoverInfo,
    pub points: Vec<(f64, f64)>,
    pub width: f32,
    pub default_material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub struct HoverState {
    pub hovered_entity: Option<Entity>,
    pub last_cursor_pos: Option<Vec2>,
    pub last_camera_transform: Option<(Vec3, Quat)>,
}

pub fn dispatch_hover_event(payload: &HoverPayload) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = serde_wasm_bindgen::to_value(payload) {
                let init = web_sys::CustomEventInit::new();
                init.set_detail(&val);
                init.set_bubbles(true);
                if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("repolis:hover", &init) {
                    let _ = window.dispatch_event(&event);
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = payload;
    }
}

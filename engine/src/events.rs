//! Engine -> UI events. Only ids cross the boundary: the frontend already
//! holds the city, so names and metrics are looked up there.

use bevy::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct Hover {
    pub kind: &'static str, // "building" | "district"
    pub id: String,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "id")]
pub enum SelectPayload {
    None,
    Building(String),
    District(String),
}

/// A path query's result; `hops` is empty when no path exists.
#[derive(Serialize)]
pub struct PathResult {
    pub from_id: String,
    pub to_id: String,
    pub hops: Vec<String>,
}

/// Marks a building entity and carries everything picking and selection need.
#[derive(Component)]
pub struct Pickable {
    pub district_idx: usize,
    pub building_idx: usize,
    /// World-space bounding box.
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Component)]
pub struct PickableDistrict {
    /// Ground rectangle, as (x, z).
    pub min: Vec2,
    pub max: Vec2,
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

pub fn dispatch_hover(p: &Option<Hover>) {
    dispatch("repolis:hover", p);
}
pub fn dispatch_select(p: &SelectPayload) {
    dispatch("repolis:select", p);
}
/// The city finished loading.
pub fn dispatch_city() {
    dispatch("repolis:city", &());
}
pub fn dispatch_path(p: &Option<PathResult>) {
    dispatch("repolis:path", p);
}

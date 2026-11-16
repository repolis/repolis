//! The wasm API. Calls from JS only queue work; systems pick it up next frame.

use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use crate::camera::CityCamera;
use crate::events::*;
use crate::path::shortest_path;
use crate::picking::frame_building;
use crate::view::{ColorMode, Filter};
use crate::{CityIndex, CityMap, SelectionState, ViewState};

// Test-only hook that feeds a city in without going through wasm bindings.
pub fn queue_city_for_test(city: CityMap) {
    PENDING_DATA.with(|c| *c.borrow_mut() = Some(city));
}

thread_local! {
    static CAMERA_OUT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static LABELS_OUT: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
    static PENDING_DATA: std::cell::RefCell<Option<CityMap>> = const { std::cell::RefCell::new(None) };
    // A queue, not a slot: restoring a URL issues several commands in one tick.
    static PENDING_COMMANDS: std::cell::RefCell<Vec<Command>> = const { std::cell::RefCell::new(Vec::new()) };
}

enum Command {
    Reset,
    Clear,
    Mode(ColorMode),
    Filter(Filter),
    Timeline(Option<u32>),
    Camera(String),
    Anchor(Option<String>),
    Path(String),
    Focus(String),
    Links(bool),
}

fn queue(cmd: Command) {
    PENDING_COMMANDS.with(|c| c.borrow_mut().push(cmd));
}

pub(crate) fn take_pending_city() -> Option<CityMap> {
    PENDING_DATA.with(|c| c.borrow_mut().take())
}

#[wasm_bindgen]
pub fn load_city_data(val: JsValue) {
    match serde_wasm_bindgen::from_value::<CityMap>(val) {
        Ok(city) => {
            web_sys::console::log_1(
                &format!(
                    "[engine] city: {} districts, {} buildings, {} edges",
                    city.districts.len(),
                    city.building_count(),
                    city.dependencies.len(),
                )
                .into(),
            );
            PENDING_DATA.with(|c| *c.borrow_mut() = Some(city));
        }
        Err(e) => {
            web_sys::console::error_1(&format!("[engine] failed to parse city data: {e:?}").into());
        }
    }
}

/// Focuses the camera on a building by id, and selects it.
#[wasm_bindgen]
pub fn focus_symbol(building_id: String) {
    queue(Command::Focus(building_id));
}

#[wasm_bindgen]
pub fn reset_camera() {
    queue(Command::Reset);
}

/// Switches which metric drives building colour.
#[wasm_bindgen]
pub fn set_color_mode(mode: String) {
    queue(Command::Mode(ColorMode::parse(&mode)));
}

/// Restricts the view. Everything not matching becomes translucent.
#[wasm_bindgen]
pub fn set_filter(json: String) {
    queue(Command::Filter(
        serde_json::from_str(&json).unwrap_or_default(),
    ));
}

/// Shows the city as it stood on the given day of its history, counted from
/// the first commit. An empty string returns to the present.
#[wasm_bindgen]
pub fn set_timeline(day: String) {
    queue(Command::Timeline(day.parse().ok()));
}

/// The current camera, as `focusX,focusZ,radius,alpha,beta`. Polled rather
/// than pushed: it changes every frame during a drag.
#[wasm_bindgen]
pub fn camera_state() -> String {
    CAMERA_OUT.with(|c| c.borrow().clone())
}

/// Screen position of each district label, as `x, y, alpha` per district in
/// city order; alpha 0 means hidden. Read by the frontend every frame.
#[wasm_bindgen]
pub fn label_positions() -> Vec<f32> {
    LABELS_OUT.with(|l| l.borrow().clone())
}

/// Restores a camera previously returned by `camera_state`.
#[wasm_bindgen]
pub fn set_camera_state(state: String) {
    queue(Command::Camera(state));
}

/// Pins the building a path query starts from. An empty id clears it.
#[wasm_bindgen]
pub fn set_path_anchor(building_id: String) {
    queue(Command::Anchor(
        Some(building_id).filter(|id| !id.is_empty()),
    ));
}

/// Traces the shortest dependency path from the pinned anchor to this
/// building, by hop count.
#[wasm_bindgen]
pub fn show_path_to(building_id: String) {
    queue(Command::Path(building_id));
}

/// Shows or hides every dependency arc at once. Hidden by default.
#[wasm_bindgen]
pub fn set_show_links(show: bool) {
    queue(Command::Links(show));
}

#[wasm_bindgen]
pub fn clear_selection() {
    queue(Command::Clear);
}

/// Publishes the camera once still, so the URL records where the user stopped
/// looking rather than every frame of the drag.
pub(crate) fn publish_camera(
    cam_q: Query<(&Transform, &CityCamera)>,
    time: Res<Time>,
    mut last: Local<(Vec3, f32, f64)>,
    mut settled: Local<bool>,
) {
    let Ok((_, cam)) = cam_q.get_single() else {
        return;
    };
    let now = time.elapsed_seconds_f64();
    let moved = (cam.focus - last.0).length() > 0.01 || (cam.radius - last.1).abs() > 0.01;
    if moved {
        *last = (cam.focus, cam.radius, now);
        *settled = false;
        return;
    }
    if *settled || now - last.2 < 0.35 {
        return;
    }
    *settled = true;
    let text = format!(
        "{:.1},{:.1},{:.1},{:.3},{:.3}",
        cam.focus.x, cam.focus.z, cam.radius, cam.alpha, cam.beta
    );
    CAMERA_OUT.with(|c| *c.borrow_mut() = text);
}

pub(crate) fn publish_labels(
    camera_q: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    index: Res<CityIndex>,
) {
    let Ok((camera, cam_tf)) = camera_q.get_single() else {
        return;
    };
    let far = index.radius.max(1.0) * 6.0;
    let out = index
        .labels
        .iter()
        .flat_map(|&world| {
            let dist = cam_tf.translation().distance(world);
            match camera.world_to_viewport(cam_tf, world) {
                // Hidden where labels would overlap into noise, and when
                // zoomed right into one building.
                Some(p) if dist > 12.0 && dist < far => {
                    [p.x, p.y, (1.0 - dist / far).clamp(0.25, 1.0)]
                }
                _ => [0.0; 3],
            }
        })
        .collect();
    LABELS_OUT.with(|l| *l.borrow_mut() = out);
}

pub(crate) fn consume_commands(
    mut interaction: ResMut<SelectionState>,
    mut state: ResMut<ViewState>,
    mut cam_q: Query<&mut CityCamera>,
    buildings: Query<(Entity, &Pickable)>,
    index: Res<CityIndex>,
) {
    let cmds = PENDING_COMMANDS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    for cmd in cmds {
        match cmd {
            Command::Reset => {
                if let Ok(mut cam) = cam_q.get_single_mut() {
                    cam.go_home();
                }
                interaction.selected = None;
                state.path.clear();
                state.path_anchor = None;
                state.dirty = true;
                dispatch_select(&SelectPayload::None);
                dispatch_path(&None);
            }
            Command::Clear => {
                interaction.selected = None;
                state.path.clear();
                state.dirty = true;
                dispatch_select(&SelectPayload::None);
            }
            Command::Links(show) => {
                state.show_links = show;
                state.dirty = true;
            }
            Command::Mode(mode) => {
                state.mode = mode;
                state.dirty = true;
            }
            Command::Filter(filter) => {
                state.filter = filter;
                state.dirty = true;
            }
            Command::Camera(arg) => {
                if let Ok(mut cam) = cam_q.get_single_mut() {
                    let n: Vec<f32> = arg.split(',').filter_map(|v| v.parse().ok()).collect();
                    if n.len() == 5 {
                        cam.focus = Vec3::new(n[0], 0.0, n[1]);
                        cam.target_focus = cam.focus;
                        cam.radius = n[2].clamp(2.5, 20000.0);
                        cam.target_radius = cam.radius;
                        cam.alpha = n[3];
                        cam.beta = n[4].clamp(0.06, 1.52);
                    }
                }
            }
            Command::Timeline(day) => {
                state.timeline = day;
                state.dirty = true;
            }
            Command::Anchor(id) => {
                state.path_anchor = id;
                state.path.clear();
                state.dirty = true;
                dispatch_path(&None);
            }
            Command::Path(to_id) => {
                state.path.clear();
                let result = state.path_anchor.clone().map(|from_id| {
                    let hops: Vec<String> = shortest_path(&index.links, &from_id, &to_id)
                        .into_iter()
                        .filter(|id| index.by_id.contains_key(id))
                        .collect();
                    state.path = hops.iter().map(|id| index.by_id[id]).collect();
                    PathResult {
                        from_id,
                        to_id,
                        hops,
                    }
                });
                state.dirty = true;
                dispatch_path(&result);
            }
            Command::Focus(id) => {
                let Some((entity, p)) = index.by_id.get(&id).and_then(|e| buildings.get(*e).ok())
                else {
                    continue;
                };
                interaction.selected = Some(entity);
                state.path.clear();
                state.dirty = true;

                if let Ok(mut cam) = cam_q.get_single_mut() {
                    frame_building(&mut cam, p);
                }

                dispatch_select(&SelectPayload::Building(id));
            }
        }
    }
}

use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use wasm_bindgen::prelude::*;

mod camera;
mod data;
mod events;
mod layout;
mod render;
mod treemap;
mod view;

pub use data::CityMap;

// Test-only hook that feeds a city in without going through wasm bindings.
pub fn queue_city_for_test(city: CityMap) {
    PENDING_DATA.with(|c| *c.borrow_mut() = Some(city));
}

use camera::{camera_controls, CityCamera};
use events::*;
use layout::{LayoutResult, Link};
use render::*;
use view::{ColorMode, Filter, Scales};

pub const SKY_COLOR: Color = Color::srgb(0.62, 0.71, 0.80);
const GROUND_Y: f32 = 0.0;
const DISTRICT_TOP: f32 = 0.35;
const STREET_Y: f32 = 0.40;
const BUILDING_BASE: f32 = 0.45;

#[derive(Component)]
struct CityElement;

#[derive(Component)]
struct LinkLayer;

#[derive(Component)]
struct HighlightLinkLayer;

#[derive(Resource, Default)]
struct CityIndex {
    city: CityMap,
    by_id: HashMap<String, Entity>,
    links: Vec<Link>,
    materials: HashMap<u32, MaterialSet>,
    faded: Option<Handle<StandardMaterial>>,
    radius: f32,
}

impl CityIndex {
    fn building(&self, p: &Pickable) -> &data::Building {
        &self.city.districts[p.district_idx].buildings[p.building_idx]
    }
}

// (normal, dimmed, highlighted) for one quantised building color
type MaterialSet = (
    Handle<StandardMaterial>,
    Handle<StandardMaterial>,
    Handle<StandardMaterial>,
);

#[cfg(test)]
mod path_tests {
    use super::*;

    fn links(pairs: &[(&str, &str)]) -> Vec<Link> {
        pairs
            .iter()
            .map(|(a, b)| Link {
                source: a.to_string(),
                target: b.to_string(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn finds_the_shortest_route_not_merely_a_route() {
        // A long way round exists; BFS must take the short one.
        let l = links(&[("a", "b"), ("b", "c"), ("c", "d"), ("a", "x"), ("x", "d")]);
        assert_eq!(shortest_path(&l, "a", "d"), vec!["a", "x", "d"]);
    }

    #[test]
    fn edges_are_followed_in_both_directions() {
        // Links are stored canonically, so direction must not matter.
        let l = links(&[("b", "a"), ("c", "b")]);
        assert_eq!(shortest_path(&l, "a", "c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn disconnected_pair_yields_nothing() {
        let l = links(&[("a", "b"), ("c", "d")]);
        assert!(shortest_path(&l, "a", "d").is_empty());
    }

    #[test]
    fn a_building_reaches_itself_in_zero_hops() {
        let l = links(&[("a", "b")]);
        assert_eq!(shortest_path(&l, "a", "a"), vec!["a"]);
    }

    #[test]
    fn unknown_endpoint_is_not_a_panic() {
        let l = links(&[("a", "b")]);
        assert!(shortest_path(&l, "a", "ghost").is_empty());
        assert!(shortest_path(&l, "ghost", "a").is_empty());
    }
}

// Everything the view is showing. One resource, so mode, filter, selection
// and path settle in a single pass instead of fighting over material handles.
#[derive(Resource, Default)]
struct ViewState {
    mode: ColorMode,
    filter: Filter,
    scales: Scales,
    // Buildings on the highlighted dependency path, in order.
    path: Vec<Entity>,
    // Where a path query starts. Separate from the selection: clicking the
    // second building replaces it, so "from the selection" would only ever
    // ask for a path from a building to itself.
    path_anchor: Option<String>,
    // Day of history to show the city as of, None for the present. Birth
    // dates come from the `git log` pass that already runs, so no commit is
    // checked out. It shows when each part came into existence, not how it
    // grew - that would need per-commit metrics.
    timeline: Option<u32>,
    dirty: bool,
}

thread_local! {
    static CAMERA_OUT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
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
}

fn queue(cmd: Command) {
    PENDING_COMMANDS.with(|c| c.borrow_mut().push(cmd));
}

// Registers every resource and system the city needs. Split out from
// `run_bevy_app` so a headless test can build the same schedule: Bevy only
// validates system parameters on the first run, not at compile time.
pub fn add_city_systems(app: &mut App) {
    app.init_resource::<SelectionState>()
        .init_resource::<CityIndex>()
        .init_resource::<ViewState>()
        .insert_resource(ClearColor(SKY_COLOR))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.80, 0.86, 0.96),
            // Low: shadows carry the depth cue, and more washes the massing
            // flat.
            brightness: 260.0,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                process_city_data,
                consume_commands,
                camera_controls,
                picking_system,
                refresh_view,
                update_labels,
                publish_camera,
            )
                .chain(),
        );
}

#[wasm_bindgen(start)]
pub fn run_bevy_app() {
    console_error_panic_hook::set_once();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Repolis".into(),
                    canvas: Some("#bevy-canvas".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WebGL2 too: WebGPU alone is a blank canvas in
                    // Firefox and older Safari.
                    backends: Some(Backends::BROWSER_WEBGPU | Backends::GL),
                    ..default()
                }),
                ..default()
            }),
    );
    add_city_systems(&mut app);
    app.run();
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

#[wasm_bindgen]
pub fn clear_selection() {
    queue(Command::Clear);
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            projection: Projection::Perspective(PerspectiveProjection {
                fov: camera::CAMERA_FOV,
                near: 0.5,
                far: 60000.0,
                ..default()
            }),
            transform: Transform::from_xyz(60.0, 70.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CityCamera::default(),
    ));

    // Height is the primary metric, and without cast shadows the massing
    // reads flat at every angle.
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 11000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.97, 0.92),
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, 0.6, 0.0)),
        cascade_shadow_config: CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 1600.0,
            first_cascade_far_bound: 220.0,
            ..default()
        }
        .build(),
        ..default()
    });

    commands.spawn((
        TextBundle::from_section(
            "loading...",
            TextStyle {
                font_size: 15.0,
                color: Color::srgba(1.0, 1.0, 1.0, 0.85),
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(12.0),
            ..default()
        }),
        HelpText,
    ));
}

#[derive(Component)]
struct HelpText;

/// Replaces characters Bevy's bundled subset font cannot draw. District names
/// come from a language model, so they carry dashes, quotes and accents.
fn ascii_only(s: &str) -> String {
    s.replace('\u{2026}', "...")
        .chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{00b7}' | '\u{2022}' => '-',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut index: ResMut<CityIndex>,
    mut interaction: ResMut<SelectionState>,
    mut view_state: ResMut<ViewState>,
    mut camera_q: Query<(&mut Transform, &mut CityCamera)>,
    existing: Query<Entity, With<CityElement>>,
    mut help_q: Query<&mut Text, With<HelpText>>,
) {
    let Some(city) = PENDING_DATA.with(|c| c.borrow_mut().take()) else {
        return;
    };

    // Before the index resets: the refinement pass re-sends the same repo,
    // and dropping the user back to the overview would undo their work.
    let had_city = index.radius > 0.0;

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }
    *interaction = SelectionState::default();
    *index = CityIndex::default();
    dispatch_hover(&None);
    dispatch_select(&SelectPayload::None);

    let result = layout::compute_layout(&city);
    index.radius = result.radius as f32;

    spawn_city(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut index,
        result,
        &city,
    );

    // Percentile, not maximum: libgit2's worst function is 129 and nearly
    // everything is under 15, which would flatten the whole ramp.
    {
        let mut cx: Vec<u32> = Vec::new();
        let mut fi: Vec<u32> = Vec::new();
        for d in &city.districts {
            for b in &d.buildings {
                cx.push(b.max_complexity);
                fi.push(b.fan_in);
            }
        }
        cx.sort_unstable();
        fi.sort_unstable();
        let p95 = |v: &Vec<u32>, fallback: f32| -> f32 {
            if v.is_empty() {
                return fallback;
            }
            (v[(v.len() * 95 / 100).min(v.len() - 1)] as f32).max(1.0)
        };
        view_state.scales = Scales {
            complexity: p95(&cx, 20.0),
            fan_in: p95(&fi, 20.0),
        };
        view_state.mode = ColorMode::Typology;
        view_state.filter = Filter::default();
        view_state.path.clear();
        view_state.timeline = None;
        view_state.path_anchor = None;
        view_state.dirty = true;
    }

    if !had_city {
        for (mut transform, mut cam) in camera_q.iter_mut() {
            cam.frame(Vec3::ZERO, index.radius * 2.35 + 60.0);
            cam.apply(&mut transform);
        }
    }

    for mut text in help_q.iter_mut() {
        // ASCII only: the bundled font has no middle dot or ellipsis.
        text.sections[0].value =
            "drag: orbit | scroll: zoom | WASD: pan | click: select | Esc: clear | R: reset".into();
    }

    index.city = city;
    dispatch_city();
}

fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    index: &mut CityIndex,
    result: LayoutResult,
    city: &CityMap,
) {
    let ground_radius = result.radius as f32 * 1.9 + 160.0;
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Circle::new(ground_radius).mesh().resolution(96)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.13, 0.14, 0.15),
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ..default()
        },
        CityElement,
    ));

    // One shared cube mesh and a few shared materials, so Bevy instances
    // them into a handful of draw calls.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let slab = |rect: &treemap::Rect, y0: f32, y1: f32| {
        let (x, z) = rect.center();
        Transform::from_xyz(x as f32, (y0 + y1) * 0.5, z as f32).with_scale(Vec3::new(
            rect.w as f32,
            y1 - y0,
            rect.d as f32,
        ))
    };

    let street_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.21, 0.23),
        perceptual_roughness: 0.98,
        ..default()
    });
    for street in &result.streets {
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: street_material.clone(),
                transform: slab(street, DISTRICT_TOP, STREET_Y),
                ..default()
            },
            CityElement,
        ));
    }

    for (i, d) in result.districts.iter().enumerate() {
        let district = &city.districts[i];
        let rgb = district_ground_rgb(&district.typology);
        let g = d.ground;
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                    perceptual_roughness: 0.95,
                    ..default()
                }),
                transform: slab(&g, GROUND_Y + 0.02, DISTRICT_TOP),
                ..default()
            },
            CityElement,
            PickableDistrict {
                min: Vec2::new(g.x as f32, g.z as f32),
                max: Vec2::new((g.x + g.w) as f32, (g.z + g.d) as f32),
                district_idx: i,
            },
        ));

        // One world-space label per district: without text, a district can
        // only be identified by hovering it.
        let (x, z) = g.center();
        commands.spawn((
            TextBundle::from_section(
                ascii_only(&format!("{} ({})", district.name, d.building_count)),
                TextStyle {
                    font_size: 15.0,
                    color: Color::srgb(1.0, 0.99, 0.94),
                    ..default()
                },
            )
            .with_style(Style {
                position_type: PositionType::Absolute,
                ..default()
            }),
            DistrictLabel {
                world: Vec3::new(x as f32, BUILDING_BASE + 6.0, z as f32),
            },
            CityElement,
        ));
    }

    for pb in &result.buildings {
        let district = &city.districts[pb.district_idx];
        let b = &district.buildings[pb.building_idx];
        let rgb = apply_age(typology_rgb(&district.typology), b.age_days);
        let handles = material_set(materials, &mut index.materials, rgb);

        let half = Vec3::new(pb.width as f32, 0.0, pb.depth as f32) * 0.5;
        let base = Vec3::new(pb.center_x as f32, BUILDING_BASE, pb.center_z as f32);
        let (min, max) = (base - half, base + half + Vec3::Y * pb.height as f32);
        let entity = commands
            .spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: handles.0.clone(),
                    transform: Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                    ..default()
                },
                Pickable {
                    district_idx: pb.district_idx,
                    building_idx: pb.building_idx,
                    min,
                    max,
                },
                CityElement,
            ))
            .id();
        index.by_id.insert(b.id.clone(), entity);
    }

    // One merged mesh for the overview, one reserved for the selection.
    let links = result.links;

    // Every dependency at once is unreadable: at rest show only the heaviest
    // cross-district links, and reveal a neighbourhood on selection.
    let mut overview: Vec<&layout::Link> = links.iter().filter(|l| l.inter_district).collect();
    overview.sort_by_key(|l| std::cmp::Reverse(l.weight));
    overview.truncate(90);

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&overview, 0.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgba(0.45, 0.78, 0.95, 0.30),
                emissive: LinearRgba::new(0.10, 0.35, 0.55, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
            ..default()
        },
        LinkLayer,
        CityElement,
    ));

    // Cycles are drawn permanently: they are rare, and they are the one thing
    // here a file tree cannot show at all.
    let cycle_links: Vec<&layout::Link> = links.iter().filter(|l| l.in_cycle).collect();
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&cycle_links, 0.6)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.30, 0.34),
                emissive: LinearRgba::new(3.4, 0.35, 0.45, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
            ..default()
        },
        CityElement,
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&[], 0.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.82, 0.25),
                emissive: LinearRgba::new(3.0, 2.0, 0.3, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
            ..default()
        },
        HighlightLinkLayer,
        CityElement,
    ));

    index.links = links;
}

// ---------------------------------------------------------------------------
// Picking and selection
// ---------------------------------------------------------------------------

/// Ray/AABB slab test.
fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;
    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let (lo, hi) = (min[axis], max[axis]);
        if d.abs() < 1e-7 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let mut t1 = (lo - o) * inv;
        let mut t2 = (hi - o) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        tmin = tmin.max(t1);
        tmax = tmax.min(t2);
        if tmin > tmax {
            return None;
        }
    }
    if tmax < 0.0 {
        return None;
    }
    Some(if tmin > 0.0 { tmin } else { tmax })
}

#[allow(clippy::too_many_arguments)]
fn picking_system(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    mut cam_focus_q: Query<&mut CityCamera>,
    buildings: Query<(Entity, &Pickable)>,
    districts: Query<(Entity, &PickableDistrict)>,
    mut interaction: ResMut<SelectionState>,
    mut state: ResMut<ViewState>,
    index: Res<CityIndex>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Escape)
        && (interaction.selected.is_some() || !state.path.is_empty())
    {
        interaction.selected = None;
        state.path.clear();
        state.dirty = true;
        dispatch_select(&SelectPayload::None);
    }

    let Some(window) = windows.iter().next() else {
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.get_single() else {
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        if interaction.hovered.take().is_some() {
            dispatch_hover(&None);
        }
        return;
    };

    let cam_pos = cam_tf.translation();
    let cam_rot = cam_tf.to_scale_rotation_translation().1;
    let clicked = mouse.just_pressed(MouseButton::Left);

    // Only when something moved, unless this is a click.
    let unchanged = interaction.last_cursor == Some(cursor)
        && interaction.last_camera == Some((cam_pos, cam_rot));
    if unchanged && !clicked {
        return;
    }
    interaction.last_cursor = Some(cursor);
    interaction.last_camera = Some((cam_pos, cam_rot));

    let Some(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let origin = ray.origin;
    let dir = *ray.direction;

    let mut best_t = f32::MAX;
    let mut best_building: Option<Entity> = None;
    for (entity, p) in buildings.iter() {
        if let Some(t) = ray_aabb(origin, dir, p.min, p.max) {
            if t < best_t {
                best_t = t;
                best_building = Some(entity);
            }
        }
    }

    let mut best_district: Option<Entity> = None;
    if best_building.is_none() && dir.y.abs() > 1e-5 {
        let t = (DISTRICT_TOP - origin.y) / dir.y;
        if t > 0.0 {
            let hit = (origin + dir * t).xz();
            best_district = districts
                .iter()
                .find(|(_, d)| hit.cmpge(d.min).all() && hit.cmple(d.max).all())
                .map(|(e, _)| e);
        }
    }

    // Hover: lightweight tooltip only.
    let hovered = best_building.or(best_district);
    if hovered != interaction.hovered {
        interaction.hovered = hovered;
        let payload = if let Some(p) = hovered.and_then(|e| buildings.get(e).ok()) {
            Some(Hover {
                kind: "building",
                id: index.building(p.1).id.clone(),
            })
        } else if let Some(d) = hovered.and_then(|e| districts.get(e).ok()) {
            let id = index.city.districts[d.1.district_idx].id.clone();
            Some(Hover {
                kind: "district",
                id,
            })
        } else {
            None
        };
        dispatch_hover(&payload);
    }

    if !clicked {
        return;
    }

    state.path.clear();
    state.dirty = true;

    match hovered {
        Some(e) if buildings.get(e).is_ok() => {
            let (_, p) = buildings.get(e).unwrap();
            interaction.selected = Some(e);

            if let Ok(mut cam) = cam_focus_q.get_single_mut() {
                frame_building(&mut cam, p);
            }

            dispatch_select(&SelectPayload::Building(index.building(p).id.clone()));
        }
        Some(e) if districts.get(e).is_ok() => {
            let (_, d) = districts.get(e).unwrap();
            interaction.selected = None;
            if let Ok(mut cam) = cam_focus_q.get_single_mut() {
                let center = (d.min + d.max) * 0.5;
                let span = (d.max - d.min).length().max(20.0);
                cam.frame_object(Vec3::new(center.x, 0.0, center.y), span);
            }
            let id = index.city.districts[d.district_idx].id.clone();
            dispatch_select(&SelectPayload::District(id));
        }
        _ => {
            interaction.selected = None;
            dispatch_select(&SelectPayload::None);
        }
    }
}

fn frame_building(cam: &mut CityCamera, p: &Pickable) {
    let size = (p.max - p.min).length().max(4.0);
    cam.frame_object((p.min + p.max) * 0.5, size);
}

/// Quantised, so lookalike buildings share a material and batch into one call.
fn rgb_key(rgb: [f32; 3]) -> u32 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u32) & 0x1f;
    (q(rgb[0]) << 10) | (q(rgb[1]) << 5) | q(rgb[2])
}

fn material_set(
    materials: &mut Assets<StandardMaterial>,
    cache: &mut HashMap<u32, MaterialSet>,
    rgb: [f32; 3],
) -> MaterialSet {
    cache
        .entry(rgb_key(rgb))
        .or_insert_with(|| {
            let m = make_building_materials(materials, rgb);
            (m.normal, m.dim, m.bright)
        })
        .clone()
}

/// Settles every building's appearance in one pass, since colour mode, filter,
/// selection and path all want the same material handle. Filtered-out
/// buildings turn translucent rather than hidden: the shape of the city is the
/// context that makes a match meaningful.
#[allow(clippy::too_many_arguments)]
fn refresh_view(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut index: ResMut<CityIndex>,
    mut state: ResMut<ViewState>,
    interaction: Res<SelectionState>,
    buildings: Query<(Entity, &Pickable)>,
    mut mats: Query<&mut Handle<StandardMaterial>>,
    highlight_q: Query<&Handle<Mesh>, With<HighlightLinkLayer>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    let index = &mut *index;

    let faded = index.faded.clone().unwrap_or_else(|| {
        let h = materials.add(StandardMaterial {
            base_color: Color::srgba(0.42, 0.43, 0.46, 0.16),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        });
        index.faded = Some(h.clone());
        h
    });

    // Neighbours of the selection, and the highlighted path.
    let mut related: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut chosen: Vec<&Link> = Vec::new();
    let mut selected_id = "";
    if let Some((_, p)) = interaction.selected.and_then(|e| buildings.get(e).ok()) {
        selected_id = &index.city.districts[p.district_idx].buildings[p.building_idx].id;
        for l in &index.links {
            if l.source == selected_id || l.target == selected_id {
                chosen.push(l);
                related.insert(if l.source == selected_id {
                    &l.target
                } else {
                    &l.source
                });
            }
        }
    }

    let on_path: std::collections::HashSet<Entity> = state.path.iter().copied().collect();
    if !on_path.is_empty() {
        // Replaces the neighbour highlight; both at once hides the answer.
        chosen.clear();
        let path_ids: Vec<&str> = state
            .path
            .iter()
            .filter_map(|e| buildings.get(*e).ok())
            .map(|(_, p)| {
                index.city.districts[p.district_idx].buildings[p.building_idx]
                    .id
                    .as_str()
            })
            .collect();
        for w in path_ids.windows(2) {
            chosen.extend(index.links.iter().filter(|l| {
                (l.source == w[0] && l.target == w[1]) || (l.source == w[1] && l.target == w[0])
            }));
        }
    }

    let filtering = !state.filter.is_empty();
    let focusing = interaction.selected.is_some() || !on_path.is_empty();

    let mut pending: Vec<(Entity, Handle<StandardMaterial>)> = Vec::new();
    for (entity, p) in buildings.iter() {
        let district = &index.city.districts[p.district_idx];
        let b = &district.buildings[p.building_idx];
        // Absent rather than faded: the timeline is for watching it get built.
        if let Some(day) = state.timeline {
            if b.born_day > day {
                pending.push((entity, faded.clone()));
                continue;
            }
        }
        let matched = !filtering || state.filter.matches(b, &district.name);
        let rgb = view::color_for(
            state.mode,
            b,
            typology_rgb(&district.typology),
            state.scales,
        );
        let rgb = apply_age_if_typology(state.mode, rgb, b.age_days);
        let set = material_set(&mut materials, &mut index.materials, rgb);

        let handle = if !matched {
            faded.clone()
        } else if !on_path.is_empty() {
            if on_path.contains(&entity) {
                set.2.clone()
            } else {
                set.1.clone()
            }
        } else if focusing {
            if b.id == selected_id {
                set.2.clone()
            } else if related.contains(b.id.as_str()) {
                set.0.clone()
            } else {
                set.1.clone()
            }
        } else {
            set.0.clone()
        };
        pending.push((entity, handle));
    }
    for (entity, handle) in pending {
        if let Ok(mut h) = mats.get_mut(entity) {
            *h = handle;
        }
    }

    if let Ok(mesh_handle) = highlight_q.get_single() {
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            *mesh = build_link_mesh(&chosen, 0.0);
        }
    }
}

/// Typology view only: in a numeric mode the ramp carries the meaning, and
/// fading it by age would corrupt it.
fn apply_age_if_typology(mode: ColorMode, rgb: [f32; 3], age_days: u32) -> [f32; 3] {
    if mode == ColorMode::Typology {
        apply_age(rgb, age_days)
    } else {
        rgb
    }
}

/// Publishes the camera once still, so the URL records where the user stopped
/// looking rather than every frame of the drag.
fn publish_camera(
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

/// Shortest dependency path, by hop count: "three steps away" is actionable
/// where a weighted cost is not.
fn shortest_path(links: &[Link], from: &str, to: &str) -> Vec<String> {
    if from == to {
        return vec![from.to_string()];
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for lr in links {
        adj.entry(lr.source.as_str())
            .or_default()
            .push(lr.target.as_str());
        adj.entry(lr.target.as_str())
            .or_default()
            .push(lr.source.as_str());
    }

    let mut prev: HashMap<&str, &str> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from);
    prev.insert(from, from);

    while let Some(cur) = queue.pop_front() {
        if cur == to {
            let mut path = vec![cur.to_string()];
            let mut at = cur;
            while prev[at] != at {
                at = prev[at];
                path.push(at.to_string());
            }
            path.reverse();
            return path;
        }
        for &next in adj.get(cur).into_iter().flatten() {
            if !prev.contains_key(next) {
                prev.insert(next, cur);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Labels and external commands
// ---------------------------------------------------------------------------

/// Projects each district label to screen space. Text lives in Bevy's UI layer
/// rather than being marshalled to JS every frame.
fn update_labels(
    camera_q: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    mut labels: Query<(&DistrictLabel, &mut Style, &mut Visibility, &mut Text)>,
    index: Res<CityIndex>,
) {
    let Ok((camera, cam_tf)) = camera_q.get_single() else {
        return;
    };
    let cam_pos = cam_tf.translation();

    for (label, mut style, mut vis, mut text) in labels.iter_mut() {
        let Some(screen) = camera.world_to_viewport(cam_tf, label.world) else {
            *vis = Visibility::Hidden;
            continue;
        };

        let dist = cam_pos.distance(label.world);
        // Fade out where labels would overlap into noise, and when zoomed
        // right into one building.
        let hide = dist > index.radius.max(1.0) * 6.0 || dist < 12.0;
        *vis = if hide {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };

        let alpha = (1.0 - (dist / (index.radius.max(1.0) * 6.0))).clamp(0.25, 1.0);
        text.sections[0].style.color = Color::srgba(1.0, 0.99, 0.94, alpha);

        let width = text.sections[0].value.chars().count() as f32 * 7.0;
        style.left = Val::Px(screen.x - width * 0.5);
        style.top = Val::Px(screen.y);
    }
}

fn consume_commands(
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

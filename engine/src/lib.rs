use bevy::core_pipeline::bloom::BloomSettings;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

mod camera;
mod data;
mod hover;
mod layout;
mod render;
mod treemap;
mod view;

// Re-exported so `examples/layout_check.rs` can verify the layout invariants
// against real city data without a browser.
pub use data::CityMap;

/// Test-only hook that feeds a city in without going through wasm bindings.
#[doc(hidden)]
pub fn queue_city_for_test(city: CityMap) {
    PENDING_DATA.with(|c| *c.borrow_mut() = Some(city));
}
pub use render::{
    apply_age as apply_age_public, churn_bucket as churn_bucket_public,
    color_key as color_key_public, typology_rgb as typology_rgb_public,
};
pub use layout::{
    compute_layout as compute_layout_public, point_in_convex as point_in_convex_public,
    polygon_area as polygon_area_public, LayoutResult,
};

use camera::{camera_controls, CityCamera};
use hover::*;
use render::*;
use view::{ColorMode, Filter, Scales};

pub const SKY_COLOR: Color = Color::srgb(0.62, 0.71, 0.80);
const GROUND_Y: f32 = 0.0;
const DISTRICT_TOP: f32 = 0.35;
const STREET_Y: f32 = 0.40;
const BUILDING_BASE: f32 = 0.45;

#[derive(Component)]
struct CityElement;

/// A roof cap or plinth belonging to `owner`. Kept as its own entity so it can
/// use a different material from the building body.
#[derive(Component)]
struct BuildingTrim {
    owner: Entity,
}

#[derive(Component)]
struct LinkLayer;

#[derive(Component)]
struct HighlightLinkLayer;

/// Everything picking and selection need, kept in one resource rather than
/// duplicated onto every entity.
#[derive(Resource, Default)]
struct CityIndex {
    districts: Vec<DistrictGeom>,
    by_id: HashMap<String, Entity>,
    links: Vec<LinkRef>,
    materials: HashMap<u32, MaterialSet>,
    faded: Option<Handle<StandardMaterial>>,
    radius: f32,
    center: Vec3,
}

/// (normal, dimmed, highlighted) for one quantised building colour.
type MaterialSet = (
    Handle<StandardMaterial>,
    Handle<StandardMaterial>,
    Handle<StandardMaterial>,
);

struct DistrictGeom {
    rot: f32,
    origin: Vec2,
    min: Vec2,
    max: Vec2,
}

struct LinkRef {
    source: String,
    target: String,
    index: usize,
}

#[cfg(test)]
mod path_tests {
    use super::*;

    fn links(pairs: &[(&str, &str)]) -> Vec<LinkRef> {
        pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| LinkRef {
                source: a.to_string(),
                target: b.to_string(),
                index: i,
            })
            .collect()
    }

    #[test]
    fn finds_the_shortest_route_not_merely_a_route() {
        // A long way round exists; BFS must take the short one.
        let l = links(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "d"),
            ("a", "x"),
            ("x", "d"),
        ]);
        assert_eq!(shortest_path(&l, "a", "d"), vec!["a", "x", "d"]);
    }

    #[test]
    fn edges_are_followed_in_both_directions() {
        // Links are stored canonically, so a path must not depend on which
        // way round the edge happened to be recorded.
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

/// Everything the current view is showing. One resource so that mode, filter,
/// selection and path all settle in a single pass rather than each fighting
/// over the same material handles.
#[derive(Resource, Default)]
struct ViewState {
    mode: ColorMode,
    filter: Filter,
    scales: Scales,
    /// Buildings on the highlighted dependency path, in order.
    path: Vec<Entity>,
    /// The building a path query starts from.
    ///
    /// Kept separate from the selection on purpose: clicking the second
    /// building replaces the selection, so a path query that read "from the
    /// selection" could only ever ask for a path from a building to itself.
    path_anchor: Option<String>,
    /// Day of the repository's history to show the city as of, or None for
    /// the present. A building appears on the day its file first appeared.
    ///
    /// This is the timeline done honestly and cheaply: file birth dates come
    /// out of the `git log` pass that already runs, so no commit has to be
    /// checked out or re-parsed. What it shows is when each part of the system
    /// came into existence - not how it grew, which would need per-commit
    /// metrics and is the expensive half.
    timeline: Option<u32>,
    dirty: bool,
}

#[derive(Resource, Default)]
struct LinkStore {
    links: Vec<layout::Link>,
}

thread_local! {
    static CAMERA_OUT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static PENDING_DATA: std::cell::RefCell<Option<CityMap>> = const { std::cell::RefCell::new(None) };
    static PENDING_FOCUS: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static PENDING_COMMAND: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Registers every resource and system the city needs.
///
/// Split out from `run_bevy_app` so a headless test can build the same
/// schedule: Bevy validates system parameters when the schedule is first run,
/// and conflicting queries panic there rather than at compile time.
pub fn add_city_systems(app: &mut App) {
    app.init_resource::<SelectionState>()
        .init_resource::<CityIndex>()
        .init_resource::<LinkStore>()
        .init_resource::<ViewState>()
        .insert_resource(ClearColor(SKY_COLOR))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.80, 0.86, 0.96),
            // Lower than before because real shadows now provide the depth
            // cue; the previous value washed the massing flat.
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
                        // WebGL2 as well as WebGPU. Pinning WebGPU only meant a
                        // silently blank canvas in Firefox and older Safari.
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
                    "[engine] city: {} districts, {} buildings, {} edges (refined: {})",
                    city.districts.len(),
                    city.building_count(),
                    city.dependencies.len(),
                    city.stats.refined
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

/// Focuses the camera on a building by id or exact name, and selects it.
#[wasm_bindgen]
pub fn focus_symbol(query: String) {
    PENDING_FOCUS.with(|c| *c.borrow_mut() = Some(query));
}

#[wasm_bindgen]
pub fn reset_camera() {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some("reset".into()));
}

/// Switches which metric drives building colour.
#[wasm_bindgen]
pub fn set_color_mode(mode: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("mode:{mode}")));
}

/// Restricts the view. Everything not matching becomes translucent.
#[wasm_bindgen]
pub fn set_filter(json: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("filter:{json}")));
}

/// Shows the city as it stood on the given day of its history, counted from
/// the first commit. An empty string returns to the present.
#[wasm_bindgen]
pub fn set_timeline(day: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("timeline:{day}")));
}

/// The current camera, as `focusX,focusZ,radius,alpha,beta`.
///
/// Read rather than pushed continuously: the camera changes every frame while
/// the mouse is down, and writing the URL that often would flood the history
/// and cost more than the feature is worth.
#[wasm_bindgen]
pub fn camera_state() -> String {
    CAMERA_OUT.with(|c| c.borrow().clone())
}

/// Restores a camera previously returned by `camera_state`.
#[wasm_bindgen]
pub fn set_camera_state(state: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("camera:{state}")));
}

/// Pins the building a path query starts from. An empty id clears it.
#[wasm_bindgen]
pub fn set_path_anchor(building_id: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("anchor:{building_id}")));
}

/// Traces the shortest dependency path from the pinned anchor to this
/// building, by hop count.
#[wasm_bindgen]
pub fn show_path_to(building_id: String) {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some(format!("path:{building_id}")));
}

#[wasm_bindgen]
pub fn clear_selection() {
    PENDING_COMMAND.with(|c| *c.borrow_mut() = Some("clear".into()));
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                // Required for bloom: without HDR every emissive value is
                // clamped at 1.0 and nothing can glow brighter than white.
                hdr: true,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: camera::CAMERA_FOV,
                near: 0.5,
                far: 60000.0,
                ..default()
            }),
            transform: Transform::from_xyz(60.0, 70.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        // Deliberately weak. Only the roof caps of high-churn buildings push
        // past the threshold, so bloom marks those and leaves the rest of the
        // city alone.
        BloomSettings {
            intensity: 0.12,
            ..BloomSettings::NATURAL
        },
        CityCamera::default(),
    ));

    // Shadows are the cheapest large improvement to legibility in the whole
    // scene: height is the primary metric, and without cast shadows the
    // massing reads flat at every camera angle.
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

/// Replaces characters Bevy's bundled subset font cannot draw.
///
/// District names come from a language model, so they can contain dashes,
/// quotes and accents that would otherwise render as empty boxes.
fn ascii_only(s: &str) -> String {
    // Multi-character substitutions first; the char map below cannot expand.
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
    mut store: ResMut<LinkStore>,
    mut interaction: ResMut<SelectionState>,
    mut view_state: ResMut<ViewState>,
    mut camera_q: Query<(&mut Transform, &mut CityCamera)>,
    existing: Query<Entity, With<CityElement>>,
    mut help_q: Query<&mut Text, With<HelpText>>,
) {
    let Some(city) = PENDING_DATA.with(|c| c.borrow_mut().take()) else {
        return;
    };

    // Capture this before the index is reset: a refinement pass re-sends the
    // same repository, and throwing the user back to the overview each time
    // would undo whatever they were looking at.
    let had_city = index.radius > 0.0;

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }
    *interaction = SelectionState::default();
    *index = CityIndex::default();
    dispatch_hover(&None);
    dispatch_select(&SelectPayload::None);

    let result = layout::compute_layout(&city);

    spawn_city(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut index,
        &mut store,
        &result,
        &city,
    );

    // Percentile rather than maximum: libgit2's worst function has complexity
    // 129 and almost everything else is under 15, so scaling to the maximum
    // would render the whole city in one flat colour.
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
            (v[(v.len() * 95) / 100.min(v.len() - 1)] as f32).max(1.0)
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

    index.radius = result.radius as f32;
    index.center = Vec3::new(result.center_x as f32, 0.0, result.center_z as f32);

    if !had_city {
        for (mut transform, mut cam) in camera_q.iter_mut() {
            cam.frame(index.center, index.radius * 2.35 + 60.0);
            cam.apply(&mut transform);
        }
    }

    for mut text in help_q.iter_mut() {
        // ASCII only: Bevy's bundled default font is a subset that has no
        // middle dot or ellipsis, so those rendered as empty boxes.
        text.sections[0].value =
            "drag: orbit | scroll: zoom | WASD: pan | click: select | Esc: clear | F: fly | R: reset".into();
    }

    dispatch_city(&build_summary(&city, &result));
}

fn build_summary(city: &CityMap, result: &LayoutResult) -> CitySummary {
    let mut districts = Vec::with_capacity(city.districts.len());
    let mut index = Vec::with_capacity(city.building_count());

    for d in &city.districts {
        let mut names: Vec<(&str, u32)> = d
            .buildings
            .iter()
            .map(|b| (b.name.as_str(), b.num_methods * 3 + b.num_fields))
            .collect();
        names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

        districts.push(DistrictInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            typology: d.typology.clone(),
            summary: d.summary.clone(),
            tags: d.tags.clone(),
            building_count: d.buildings.len(),
            total_lines_of_code: d.buildings.iter().map(|b| b.lines_of_code).sum(),
            total_methods: d.buildings.iter().map(|b| b.num_methods).sum(),
            top_buildings: names.iter().take(8).map(|(n, _)| n.to_string()).collect(),
        });

        for b in &d.buildings {
            index.push(IndexEntry {
                id: b.id.clone(),
                name: b.name.clone(),
                kind: b.kind.clone(),
                district: d.name.clone(),
            });
        }
    }

    let _ = result;
    let s = &city.stats;
    CitySummary {
        total_buildings: s.total_buildings,
        total_types: s.total_types,
        total_modules: s.total_modules,
        total_methods: s.total_methods,
        total_files: s.total_files,
        total_loc: s.total_loc,
        languages: {
            let mut v: Vec<(String, u32)> =
                s.languages.iter().map(|(k, n)| (k.clone(), *n)).collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            v
        },
        methods_by_rule: s.methods_by_rule,
        methods_by_llm: s.methods_by_llm,
        orphans: s.orphans,
        llm_calls: s.llm_calls,
        refined: s.refined,
        skipped_dirs: s.skipped_dirs.clone(),
        history_days: s.history_days,
        first_commit: s.first_commit.clone(),
        last_commit: s.last_commit.clone(),
        cycles: s
            .cycles
            .iter()
            .filter(|c| c.reliable && c.namespaces > 1)
            .map(|c| CycleInfo {
                id: c.id,
                size: c.size,
                namespaces: c.namespaces,
                members: c.members.clone(),
            })
            .collect(),
        districts,
        index,
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    index: &mut CityIndex,
    store: &mut LinkStore,
    result: &LayoutResult,
    city: &CityMap,
) {
    // Dependency lookups for the inspector, by building id.
    let mut calls: HashMap<&str, Vec<String>> = HashMap::new();
    let mut called_by: HashMap<&str, Vec<String>> = HashMap::new();
    let mut name_of: HashMap<&str, &str> = HashMap::new();
    for d in &city.districts {
        for b in &d.buildings {
            name_of.insert(b.id.as_str(), b.name.as_str());
        }
    }
    for e in &city.dependencies {
        if let Some(t) = name_of.get(e.target.as_str()) {
            calls.entry(e.source.as_str()).or_default().push((*t).to_string());
        }
        if let Some(s) = name_of.get(e.source.as_str()) {
            called_by.entry(e.target.as_str()).or_default().push((*s).to_string());
        }
    }

    let ground_radius = result.radius as f32 * 1.9 + 160.0;
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(circular_ground(ground_radius, 96)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.13, 0.14, 0.15),
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_xyz(result.center_x as f32, GROUND_Y, result.center_z as f32),
            ..default()
        },
        CityElement,
    ));

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mut cap_materials: HashMap<u32, Handle<StandardMaterial>> = HashMap::new();
    let plinth_material = make_plinth_material(materials);

    // District ground + street grid: one mesh per district instead of one
    // platform entity per building.
    for (i, d) in result.districts.iter().enumerate() {
        let rgb = district_ground_rgb(&d.typology);
        commands.spawn((
                PbrBundle {
                    mesh: meshes.add(polygon_prism(&d.polygon, DISTRICT_TOP, GROUND_Y + 0.02)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                        perceptual_roughness: 0.95,
                        ..default()
                    }),
                    ..default()
                },
                CityElement,
                PickableDistrict {
                    info: Box::new(district_info(city, i)),
                    polygon: d.polygon.clone(),
                    district_idx: i,
                },
            ));

        let mut quads = QuadBuilder::new();
        for s in result.streets.iter().filter(|s| s.district_idx == i) {
            // Rotate each block slab into world space about the district origin.
            let (sin_r, cos_r) = (d.rot as f32).sin_cos();
            let dx = s.local_x as f32 - d.origin_x as f32;
            let dz = s.local_z as f32 - d.origin_z as f32;
            let wx = d.origin_x as f32 + dx * cos_r - dz * sin_r;
            let wz = d.origin_z as f32 + dx * sin_r + dz * cos_r;
            quads.add_rect(wx, wz, s.width as f32, s.depth as f32, STREET_Y, d.rot as f32);
        }
        if !quads.is_empty() {
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(quads.build()),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(0.20, 0.21, 0.23),
                        perceptual_roughness: 0.98,
                        ..default()
                    }),
                    ..default()
                },
                CityElement,
            ));
        }

        index.districts.push(DistrictGeom {
            rot: d.rot as f32,
            origin: Vec2::new(d.origin_x as f32, d.origin_z as f32),
            min: Vec2::new(d.min_x as f32, d.min_z as f32),
            max: Vec2::new(d.max_x as f32, d.max_z as f32),
        });

        // One world-space label per district. This is the single largest
        // navigation win: previously the view contained no text at all, so a
        // district could only be identified by hovering it.
        commands.spawn((
            TextBundle::from_section(
                ascii_only(&format!("{} ({})", d.name, d.building_count)),
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
                world: Vec3::new(d.label_x as f32, BUILDING_BASE + 6.0, d.label_z as f32),
                district_idx: i,
            },
            CityElement,
        ));
    }

    // Buildings: one shared cube mesh and a small set of shared materials, so
    // Bevy instances them into a handful of draw calls.
    let by_district: Vec<&data::District> = city.districts.iter().collect();
    let mut raw: HashMap<&str, &data::Building> = HashMap::new();
    for d in &city.districts {
        for b in &d.buildings {
            raw.insert(b.id.as_str(), b);
        }
    }

    for pb in &result.buildings {
        let district = by_district.get(pb.district_idx).copied();
        let typology = district.map(|d| d.typology.as_str()).unwrap_or("unknown");
        let base = typology_rgb(typology);
        let rgb = apply_age(base, pb.age_days);
        let bucket = churn_bucket(pb.churn_rank);
        let handles = material_set(materials, &mut index.materials, rgb);

        let b = raw.get(pb.id.as_str()).copied();
        let info = BuildingInfo {
            id: pb.id.clone(),
            name: pb.name.clone(),
            kind: pb.kind.clone(),
            source_file: b.map(|x| x.source_file.clone()).unwrap_or_default(),
            dir: b.map(|x| x.dir.clone()).unwrap_or_default(),
            language: b.map(|x| x.language.clone()).unwrap_or_default(),
            district_name: district.map(|d| d.name.clone()).unwrap_or_default(),
            typology: typology.to_string(),
            num_fields: b.map(|x| x.num_fields).unwrap_or(0),
            num_methods: b.map(|x| x.num_methods).unwrap_or(0),
            lines_of_code: b.map(|x| x.lines_of_code).unwrap_or(0),
            max_complexity: b.map(|x| x.max_complexity).unwrap_or(0),
            sum_complexity: b.map(|x| x.sum_complexity).unwrap_or(0),
            fan_in: b.map(|x| x.fan_in).unwrap_or(0),
            fan_out: b.map(|x| x.fan_out).unwrap_or(0),
            instability: b.map(|x| x.instability).unwrap_or(0.0),
            hub: b.map(|x| x.hub).unwrap_or(false),
            commit_churn: b.map(|x| x.commit_churn).unwrap_or(0),
            churn_rank: pb.churn_rank,
            last_modified: b.map(|x| x.last_modified.clone()).unwrap_or_default(),
            age_days: pb.age_days,
            born_day: b.map(|x| x.born_day).unwrap_or(0),
            primary_author: b.map(|x| x.primary_author.clone()).unwrap_or_default(),
            summary: b.map(|x| x.summary.clone()).unwrap_or_default(),
            assoc_source: b.map(|x| x.assoc_source.clone()).unwrap_or_default(),
            cycle_id: b.map(|x| x.cycle_id).unwrap_or(0),
            fields: b.map(|x| x.fields.clone()).unwrap_or_default(),
            methods: b.map(|x| x.methods.clone()).unwrap_or_default(),
            calls: calls.get(pb.id.as_str()).cloned().unwrap_or_default(),
            called_by: called_by.get(pb.id.as_str()).cloned().unwrap_or_default(),
        };

        let half_w = (pb.width * 0.5) as f32;
        let half_d = (pb.depth * 0.5) as f32;
        let height = pb.height as f32;

        let entity = commands
            .spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: handles.0.clone(),
                    transform: Transform::from_xyz(
                        pb.center_x as f32,
                        BUILDING_BASE + height * 0.5,
                        pb.center_z as f32,
                    )
                    .with_rotation(Quat::from_rotation_y(
                        result.districts[pb.district_idx].rot as f32,
                    ))
                    .with_scale(Vec3::new(pb.width as f32, height, pb.depth as f32)),
                    ..default()
                },
                Pickable {
                    info: Box::new(info),
                    district_idx: pb.district_idx,
                    local_min: Vec2::new(pb.local_x as f32 - half_w, pb.local_z as f32 - half_d),
                    local_max: Vec2::new(pb.local_x as f32 + half_w, pb.local_z as f32 + half_d),
                    y_min: BUILDING_BASE,
                    y_max: BUILDING_BASE + height,
                    typology_rgb: base,
                },
                CityElement,
            ))
            .id();

        index.by_id.insert(pb.id.clone(), entity);

        let rot = Quat::from_rotation_y(result.districts[pb.district_idx].rot as f32);

        // Roof cap: the churn signal. Skipping the dullest quarter keeps the
        // marker meaningful and avoids ~25% of the extra entities.
        if bucket >= 1 {
            let cap_mat = cap_materials
                .entry(bucket)
                .or_insert_with(|| make_cap_material(materials, bucket))
                .clone();
            let thickness = (height * 0.055).clamp(0.4, 1.4);
            commands.spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: cap_mat,
                    transform: Transform::from_xyz(
                        pb.center_x as f32,
                        BUILDING_BASE + height + thickness * 0.5,
                        pb.center_z as f32,
                    )
                    .with_rotation(rot)
                    .with_scale(Vec3::new(
                        pb.width as f32 * 1.07,
                        thickness,
                        pb.depth as f32 * 1.07,
                    )),
                    ..default()
                },
                BuildingTrim { owner: entity },
                CityElement,
            ));
        }

        // Plinth: marks a file module rather than a type.
        if pb.kind == "module" {
            commands.spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: plinth_material.clone(),
                    transform: Transform::from_xyz(
                        pb.center_x as f32,
                        BUILDING_BASE + 0.16,
                        pb.center_z as f32,
                    )
                    .with_rotation(rot)
                    .with_scale(Vec3::new(
                        pb.width as f32 * 1.22,
                        0.32,
                        pb.depth as f32 * 1.22,
                    )),
                    ..default()
                },
                BuildingTrim { owner: entity },
                CityElement,
            ));
        }
    }

    // Links: one merged mesh for the overview, one empty mesh reserved for the
    // selection highlight.
    let links: Vec<layout::Link> = result
        .links
        .iter()
        .map(|l| layout::Link {
            points: l.points.clone(),
            width: l.width,
            weight: l.weight,
            fwd: l.fwd,
            rev: l.rev,
            kind: l.kind.clone(),
            in_cycle: l.in_cycle,
            source: l.source.clone(),
            target: l.target.clone(),
            source_name: l.source_name.clone(),
            target_name: l.target_name.clone(),
            inter_district: l.inter_district,
        })
        .collect();

    for (i, l) in links.iter().enumerate() {
        index.links.push(LinkRef {
            source: l.source.clone(),
            target: l.target.clone(),
            index: i,
        });
    }

    // Showing every dependency at once is unreadable, so the resting view is
    // the heaviest cross-district links only; selecting a building reveals its
    // full neighbourhood.
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

    // Dependency cycles are drawn permanently and in their own colour. They
    // are rare, they are the one thing here that a file tree cannot show at
    // all, and hiding them behind a selection would defeat the point.
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

    store.links = links;
}

fn district_info(city: &CityMap, i: usize) -> DistrictInfo {
    let Some(d) = city.districts.get(i) else {
        return DistrictInfo::default();
    };
    let mut names: Vec<(&str, u32)> = d
        .buildings
        .iter()
        .map(|b| (b.name.as_str(), b.num_methods * 3 + b.num_fields))
        .collect();
    names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    DistrictInfo {
        id: d.id.clone(),
        name: d.name.clone(),
        typology: d.typology.clone(),
        summary: d.summary.clone(),
        tags: d.tags.clone(),
        building_count: d.buildings.len(),
        total_lines_of_code: d.buildings.iter().map(|b| b.lines_of_code).sum(),
        total_methods: d.buildings.iter().map(|b| b.num_methods).sum(),
        top_buildings: names.iter().take(8).map(|(n, _)| n.to_string()).collect(),
    }
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

fn point_in_polygon(px: f64, pz: f64, poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, zi) = poly[i];
        let (xj, zj) = poly[j];
        if ((zi > pz) != (zj > pz)) && (px < (xj - xi) * (pz - zi) / (zj - zi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
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
    if keys.just_pressed(KeyCode::Escape) && (interaction.selected.is_some() || !state.path.is_empty()) {
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

    // Recompute only when something actually moved, unless this is a click.
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

    // Two-level cull: test each district's bounding box first, then only the
    // buildings inside the districts the ray actually crosses. The previous
    // version swept every building AABB on every cursor movement.
    let mut candidate_districts = vec![false; index.districts.len()];
    let mut any_candidate = false;
    for (i, g) in index.districts.iter().enumerate() {
        let min = Vec3::new(g.min.x, 0.0, g.min.y);
        let max = Vec3::new(g.max.x, BUILDING_BASE + 90.0, g.max.y);
        if ray_aabb(origin, dir, min, max).is_some() {
            candidate_districts[i] = true;
            any_candidate = true;
        }
    }
    if !any_candidate {
        if interaction.hovered.take().is_some() {
            dispatch_hover(&None);
        }
        return;
    }

    let mut best_t = f32::MAX;
    let mut best_building: Option<Entity> = None;

    for (entity, p) in buildings.iter() {
        if !candidate_districts.get(p.district_idx).copied().unwrap_or(false) {
            continue;
        }
        let Some(g) = index.districts.get(p.district_idx) else {
            continue;
        };
        // Rotate the ray into the district's local frame so rotated footprints
        // stay exact axis-aligned boxes.
        let (o, d) = to_local(origin, dir, g.origin, g.rot);
        let min = Vec3::new(p.local_min.x, p.y_min, p.local_min.y);
        let max = Vec3::new(p.local_max.x, p.y_max, p.local_max.y);
        if let Some(t) = ray_aabb(o, d, min, max) {
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
            let hit = origin + dir * t;
            for (entity, d) in districts.iter() {
                if candidate_districts.get(d.district_idx).copied().unwrap_or(false)
                    && point_in_polygon(hit.x as f64, hit.z as f64, &d.polygon)
                {
                    best_district = Some(entity);
                    break;
                }
            }
        }
    }

    // Hover: lightweight tooltip only.
    let hovered = best_building.or(best_district);
    if hovered != interaction.hovered {
        interaction.hovered = hovered;
        let payload = match hovered {
            Some(e) => {
                if let Ok((_, p)) = buildings.get(e) {
                    Some(HoverInfo {
                        kind: "building".into(),
                        name: p.info.name.clone(),
                        detail: format!(
                            "{} \u{00b7} {} fields \u{00b7} {} functions",
                            p.info.kind, p.info.num_fields, p.info.num_methods
                        ),
                    })
                } else if let Ok((_, d)) = districts.get(e) {
                    Some(HoverInfo {
                        kind: "district".into(),
                        name: d.info.name.clone(),
                        detail: format!("{} entities \u{00b7} {}", d.info.building_count, d.info.typology),
                    })
                } else {
                    None
                }
            }
            None => None,
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
                let g = &index.districts[p.district_idx];
                let center = to_world(
                    Vec2::new(
                        (p.local_min.x + p.local_max.x) * 0.5,
                        (p.local_min.y + p.local_max.y) * 0.5,
                    ),
                    g.origin,
                    g.rot,
                );
                let footprint = (p.local_max - p.local_min).length();
                let height = p.y_max - p.y_min;
                let size = footprint.max(height).max(4.0);
                cam.frame_object(
                    Vec3::new(center.x, (p.y_min + p.y_max) * 0.5, center.y),
                    size,
                );
            }

            dispatch_select(&SelectPayload::Building(p.info.clone()));
        }
        Some(e) if districts.get(e).is_ok() => {
            let (_, d) = districts.get(e).unwrap();
            interaction.selected = None;
            if let Ok(mut cam) = cam_focus_q.get_single_mut() {
                let g = &index.districts[d.district_idx];
                let center = (g.min + g.max) * 0.5;
                let span = (g.max - g.min).length().max(20.0);
                cam.frame_object(Vec3::new(center.x, 0.0, center.y), span);
            }
            dispatch_select(&SelectPayload::District(d.info.clone()));
        }
        _ => {
            interaction.selected = None;
            dispatch_select(&SelectPayload::None);
        }
    }
}

fn to_local(origin: Vec3, dir: Vec3, pivot: Vec2, rot: f32) -> (Vec3, Vec3) {
    let (s, c) = rot.sin_cos();
    let ox = origin.x - pivot.x;
    let oz = origin.z - pivot.y;
    (
        Vec3::new(
            pivot.x + ox * c + oz * s,
            origin.y,
            pivot.y + -ox * s + oz * c,
        ),
        Vec3::new(dir.x * c + dir.z * s, dir.y, -dir.x * s + dir.z * c),
    )
}

fn to_world(local: Vec2, pivot: Vec2, rot: f32) -> Vec2 {
    let (s, c) = rot.sin_cos();
    let dx = local.x - pivot.x;
    let dz = local.y - pivot.y;
    Vec2::new(pivot.x + dx * c - dz * s, pivot.y + dx * s + dz * c)
}

/// Quantised key so two buildings that look the same share one material, and
/// Bevy can batch them into one draw call.
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

/// Settles every building's appearance from the current view state.
///
/// Colour mode, filter, selection and path all want to change the same
/// material handle, so they are resolved in one place and in one order rather
/// than by three systems overwriting each other. Filtered-out buildings become
/// translucent instead of hidden: the shape of the city is the context that
/// makes a match meaningful.
#[allow(clippy::too_many_arguments)]
fn refresh_view(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut index: ResMut<CityIndex>,
    mut state: ResMut<ViewState>,
    interaction: Res<SelectionState>,
    store: Res<LinkStore>,
    buildings: Query<(Entity, &Pickable)>,
    mut mats: Query<&mut Handle<StandardMaterial>>,
    mut trim: Query<(&BuildingTrim, &mut Visibility)>,
    highlight_q: Query<&Handle<Mesh>, With<HighlightLinkLayer>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;

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
    let mut related: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut chosen: Vec<&layout::Link> = Vec::new();
    let mut selected_id = String::new();
    if let Some(sel) = interaction.selected {
        if let Ok((_, p)) = buildings.get(sel) {
            selected_id = p.info.id.clone();
            for lr in &index.links {
                if lr.source == selected_id || lr.target == selected_id {
                    if let Some(link) = store.links.get(lr.index) {
                        chosen.push(link);
                        related.insert(
                            if lr.source == selected_id { lr.target.clone() } else { lr.source.clone() },
                        );
                    }
                }
            }
        }
    }

    let on_path: std::collections::HashSet<Entity> = state.path.iter().copied().collect();
    if !on_path.is_empty() {
        // The path replaces the neighbour highlight: showing both at once
        // makes it impossible to see which arcs are the answer.
        chosen.clear();
        let path_ids: Vec<String> = state
            .path
            .iter()
            .filter_map(|e| buildings.get(*e).ok().map(|(_, p)| p.info.id.clone()))
            .collect();
        for w in path_ids.windows(2) {
            for lr in &index.links {
                if (lr.source == w[0] && lr.target == w[1]) || (lr.source == w[1] && lr.target == w[0]) {
                    if let Some(link) = store.links.get(lr.index) {
                        chosen.push(link);
                    }
                }
            }
        }
    }

    let filtering = !state.filter.is_empty();
    let focusing = interaction.selected.is_some() || !on_path.is_empty();
    let mut visible_trim: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    let mut pending: Vec<(Entity, Handle<StandardMaterial>)> = Vec::new();
    for (entity, p) in buildings.iter() {
        // A building that did not exist yet is simply absent, not faded: the
        // point of the timeline is to watch the city get built.
        if let Some(day) = state.timeline {
            if p.info.born_day > day {
                pending.push((entity, faded.clone()));
                continue;
            }
        }
        let matched = !filtering || state.filter.matches(&p.info);
        let rgb = view::color_for(state.mode, &p.info, p.typology_rgb, state.scales);
        let rgb = apply_age_if_typology(state.mode, rgb, p.info.age_days);
        let set = material_set(&mut materials, &mut index.materials, rgb);

        let handle = if !matched {
            faded.clone()
        } else if !on_path.is_empty() {
            if on_path.contains(&entity) {
                visible_trim.insert(entity);
                set.2.clone()
            } else {
                set.1.clone()
            }
        } else if focusing {
            if p.info.id == selected_id {
                visible_trim.insert(entity);
                set.2.clone()
            } else if related.contains(&p.info.id) {
                visible_trim.insert(entity);
                set.0.clone()
            } else {
                set.1.clone()
            }
        } else {
            visible_trim.insert(entity);
            set.0.clone()
        };
        pending.push((entity, handle));
    }
    for (entity, handle) in pending {
        if let Ok(mut h) = mats.get_mut(entity) {
            *h = handle;
        }
    }

    // Roof caps carry their own emissive material, so a dimmed or filtered
    // building would otherwise keep glowing.
    for (t, mut vis) in trim.iter_mut() {
        *vis = if visible_trim.contains(&t.owner) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if let Ok(mesh_handle) = highlight_q.get_single() {
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            *mesh = build_link_mesh(&chosen, 0.0);
        }
    }
    let _ = &mut commands;
}

/// Age desaturation belongs to the typology view only. In a numeric mode the
/// ramp already carries the meaning, and fading it by age would corrupt it.
fn apply_age_if_typology(mode: ColorMode, rgb: [f32; 3], age_days: u32) -> [f32; 3] {
    if mode == ColorMode::Typology {
        apply_age(rgb, age_days)
    } else {
        rgb
    }
}

/// Publishes the camera once it has settled.
///
/// Waiting for stillness means the URL records where the user stopped looking,
/// not every intermediate frame of the drag that got them there.
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

/// Shortest dependency path between two buildings, by hop count.
///
/// Hops rather than edge weight: "three steps away" is a statement anyone can
/// act on, where a weighted cost is not.
fn shortest_path(links: &[LinkRef], from: &str, to: &str) -> Vec<String> {
    if from == to {
        return vec![from.to_string()];
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for lr in links {
        adj.entry(lr.source.as_str()).or_default().push(lr.target.as_str());
        adj.entry(lr.target.as_str()).or_default().push(lr.source.as_str());
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
        // Fade labels out when the camera is far enough that they would
        // overlap into noise, and when zoomed right into one building.
        let hide = dist > index.radius.max(1.0) * 6.0 || dist < 12.0;
        *vis = if hide { Visibility::Hidden } else { Visibility::Inherited };

        let alpha = (1.0 - (dist / (index.radius.max(1.0) * 6.0))).clamp(0.25, 1.0);
        text.sections[0].style.color = Color::srgba(1.0, 0.99, 0.94, alpha);

        let width = text.sections[0].value.chars().count() as f32 * 7.0;
        style.left = Val::Px(screen.x - width * 0.5);
        style.top = Val::Px(screen.y);
        let _ = label.district_idx;
    }
}

#[allow(clippy::too_many_arguments)]
fn consume_commands(
    mut interaction: ResMut<SelectionState>,
    mut state: ResMut<ViewState>,
    mut cam_q: Query<&mut CityCamera>,
    buildings: Query<(Entity, &Pickable)>,
    index: Res<CityIndex>,
) {
    if let Some(cmd) = PENDING_COMMAND.with(|c| c.borrow_mut().take()) {
        let (verb, arg) = match cmd.split_once(':') {
            Some((v, a)) => (v, a),
            None => (cmd.as_str(), ""),
        };
        match verb {
            "reset" => {
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
            "clear" => {
                interaction.selected = None;
                state.path.clear();
                state.dirty = true;
                dispatch_select(&SelectPayload::None);
            }
            "mode" => {
                state.mode = ColorMode::parse(arg);
                state.dirty = true;
            }
            "filter" => {
                state.filter = serde_json::from_str(arg).unwrap_or_default();
                state.dirty = true;
            }
            "camera" => {
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
            "timeline" => {
                state.timeline = if arg.is_empty() {
                    None
                } else {
                    arg.parse::<u32>().ok()
                };
                state.dirty = true;
            }
            "anchor" => {
                state.path_anchor = if arg.is_empty() { None } else { Some(arg.to_string()) };
                state.path.clear();
                state.dirty = true;
                dispatch_path(&None);
            }
            "path" => {
                state.path.clear();
                let mut result = None;
                if let Some(from_id) = state.path_anchor.clone() {
                    let ids = shortest_path(&index.links, &from_id, arg);
                    let mut hops = Vec::new();
                    for id in &ids {
                        if let Some(e) = index.by_id.get(id) {
                            state.path.push(*e);
                            if let Ok((_, p)) = buildings.get(*e) {
                                hops.push(p.info.name.clone());
                            }
                        }
                    }
                    let name_of = |id: &str| -> String {
                        index
                            .by_id
                            .get(id)
                            .and_then(|e| buildings.get(*e).ok())
                            .map(|(_, p)| p.info.name.clone())
                            .unwrap_or_default()
                    };
                    result = Some(PathInfo {
                        from_name: name_of(&from_id),
                        from_id,
                        to_name: name_of(arg),
                        to_id: arg.to_string(),
                        hops,
                    });
                }
                state.dirty = true;
                dispatch_path(&result);
            }
            _ => {}
        }
    }

    let Some(query) = PENDING_FOCUS.with(|c| c.borrow_mut().take()) else {
        return;
    };

    // Exact id first, then exact name, then a case-insensitive substring.
    let mut found: Option<(Entity, &Pickable)> = None;
    for (e, p) in buildings.iter() {
        if p.info.id == query {
            found = Some((e, p));
            break;
        }
    }
    if found.is_none() {
        let lower = query.to_lowercase();
        let mut best: Option<(Entity, &Pickable, usize)> = None;
        for (e, p) in buildings.iter() {
            let name = p.info.name.to_lowercase();
            let score = if name == lower {
                0
            } else if name.starts_with(&lower) {
                1
            } else if name.contains(&lower) {
                2
            } else {
                continue;
            };
            if best.as_ref().map(|(_, _, s)| score < *s).unwrap_or(true) {
                best = Some((e, p, score));
            }
        }
        found = best.map(|(e, p, _)| (e, p));
    }

    let Some((entity, p)) = found else {
        return;
    };

    interaction.selected = Some(entity);
    state.path.clear();
    state.dirty = true;

    if let (Ok(mut cam), Some(g)) = (cam_q.get_single_mut(), index.districts.get(p.district_idx)) {
        let center = to_world(
            Vec2::new(
                (p.local_min.x + p.local_max.x) * 0.5,
                (p.local_min.y + p.local_max.y) * 0.5,
            ),
            g.origin,
            g.rot,
        );
        let footprint = (p.local_max - p.local_min).length();
        let height = p.y_max - p.y_min;
        let size = footprint.max(height).max(4.0);
        cam.frame_object(
            Vec3::new(center.x, (p.y_min + p.y_max) * 0.5, center.y),
            size,
        );
    }

    dispatch_select(&SelectPayload::Building(p.info.clone()));
}

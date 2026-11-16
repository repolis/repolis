//! Entry point: the Bevy app, the shared resources and city (re)loading.
//! The wasm API lives in `api`, everything else in one module per concern.

use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

mod api;
mod appearance;
mod camera;
mod data;
mod events;
mod layout;
mod path;
mod picking;
mod render;
mod spawn;
mod treemap;
mod view;

pub use api::queue_city_for_test;
pub use data::CityMap;

use api::{consume_commands, publish_camera, take_pending_city};
use appearance::refresh_view;
use camera::{camera_controls, CityCamera};
use events::*;
use layout::Link;
use picking::picking_system;
use render::BuildingMaterials;
use spawn::spawn_city;
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
    materials: HashMap<u32, BuildingMaterials>,
    faded: Option<Handle<StandardMaterial>>,
    radius: f32,
}

impl CityIndex {
    fn building(&self, p: &Pickable) -> &data::Building {
        &self.city.districts[p.district_idx].buildings[p.building_idx]
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
    show_links: bool,
    dirty: bool,
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
    let Some(city) = take_pending_city() else {
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

    *view_state = ViewState {
        scales: Scales::from_city(&city),
        dirty: true,
        ..default()
    };

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

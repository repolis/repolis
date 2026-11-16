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
mod picking;
mod render;
mod spawn;
mod treemap;

pub use api::queue_city_for_test;
pub use data::CityMap;

use api::{consume_commands, publish_camera, publish_labels, take_pending_city};
use appearance::{apply_appearance, Appearance, ShowLinks};
use camera::{camera_controls, CityCamera};
use events::*;
use layout::Link;
use picking::{picking_system, Hovered};
use render::BuildingMaterials;
use spawn::spawn_city;

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

/// What the systems need to know about the loaded city.
#[derive(Resource, Default)]
struct CityIndex {
    links: Vec<Link>,
    /// Where each district's label sits, in district order.
    labels: Vec<Vec3>,
    materials: HashMap<u32, BuildingMaterials>,
    faded: Option<Handle<StandardMaterial>>,
    radius: f32,
}

// Registers every resource and system the city needs. Split out from
// `run_bevy_app` so a headless test can build the same schedule: Bevy only
// validates system parameters on the first run, not at compile time.
pub fn add_city_systems(app: &mut App) {
    app.init_resource::<Hovered>()
        .init_resource::<CityIndex>()
        .init_resource::<Appearance>()
        .init_resource::<ShowLinks>()
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
                apply_appearance,
                publish_labels,
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

}

#[allow(clippy::too_many_arguments)]
fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut index: ResMut<CityIndex>,
    mut hovered: ResMut<Hovered>,
    mut look: ResMut<Appearance>,
    mut camera_q: Query<(&mut Transform, &mut CityCamera)>,
    existing: Query<Entity, With<CityElement>>,
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
    *hovered = Hovered::default();
    *index = CityIndex::default();
    dispatch_hover(None);
    dispatch_select(None);

    let result = layout::compute_layout(&city);
    index.radius = result.radius;
    spawn_city(&mut commands, &mut meshes, &mut materials, &mut index, result);

    // The frontend sends the new appearance once it hears about the city.
    *look = Appearance::default();

    if !had_city {
        for (mut transform, mut cam) in camera_q.iter_mut() {
            cam.frame(Vec3::ZERO, index.radius * 2.35 + 60.0);
            cam.apply(&mut transform);
        }
    }

    dispatch_city();
}

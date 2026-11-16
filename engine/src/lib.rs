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
pub use render::FacadeMaterial;

use api::{consume_commands, publish_camera, publish_labels, take_pending_city};
use appearance::{apply_appearance, Appearance, ShowLinks};
use camera::{camera_controls, CityCamera};
use events::*;
use layout::Link;
use picking::{picking_system, Hovered};
use render::{BuildingMaterials, FACADE_SHADER};
use spawn::spawn_city;

pub const SKY_COLOR: Color = Color::srgb(0.62, 0.71, 0.80);
// Brighter than the sky because it gets tonemapped and the sky does not; the
// two must match where the ground fades out, or the horizon shows a seam.
const FOG_COLOR: Color = Color::srgb(0.72, 0.85, 1.0);
const GROUND_Y: f32 = 0.0;
const DISTRICT_TOP: f32 = 0.35;
const PAVEMENT_Y: f32 = 0.40;
const BUILDING_BASE: f32 = 0.45;

#[derive(Component)]
struct CityElement;

#[derive(Component)]
struct LinkLayer;

#[derive(Component)]
struct HighlightLinkLayer;

#[derive(Resource, Default)]
struct CityIndex {
    links: Vec<Link>,
    labels: Vec<Vec3>,
    materials: HashMap<u32, BuildingMaterials>,
    faded: Option<Handle<FacadeMaterial>>,
    radius: f32,
}

// Split out so a headless test can run the same schedule (Bevy checks params at runtime).
pub fn add_city_systems(app: &mut App) {
    app.init_resource::<Hovered>()
        .init_resource::<CityIndex>()
        .init_resource::<Appearance>()
        .init_resource::<ShowLinks>()
        .insert_resource(ClearColor(SKY_COLOR))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.80, 0.86, 0.96),
            // Low: shadows carry the depth cue, and more washes the massing flat.
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
                    // WebGL2 too: WebGPU alone is a blank canvas in Firefox and older Safari.
                    backends: Some(Backends::BROWSER_WEBGPU | Backends::GL),
                    ..default()
                }),
                ..default()
            }),
    );
    bevy::asset::load_internal_asset!(app, FACADE_SHADER, "shaders/facade.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<FacadeMaterial>::default());
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
        // The distances follow the city; see `city_fog`.
        city_fog(200.0),
        CityCamera::default(),
    ));

    // Without cast shadows the height-based massing reads flat.
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 11000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.87),
            ..default()
        },
        // Low afternoon sun: long shadows fall across the streets.
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.6, 0.0)),
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

fn home_distance(radius: f32) -> f32 {
    radius * 2.35 + 60.0
}

/// Starts past the city's near half and hides the ground's edge before the camera reaches it.
fn city_fog(radius: f32) -> FogSettings {
    let home = home_distance(radius);
    FogSettings {
        color: FOG_COLOR,
        directional_light_color: Color::srgba(1.0, 0.9, 0.7, 0.4),
        falloff: FogFalloff::Linear {
            start: home + radius * 0.5,
            end: home + radius * 5.0,
        },
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut facades: ResMut<Assets<FacadeMaterial>>,
    mut index: ResMut<CityIndex>,
    mut hovered: ResMut<Hovered>,
    mut look: ResMut<Appearance>,
    mut camera_q: Query<(&mut Transform, &mut CityCamera, &mut FogSettings)>,
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
    spawn_city(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut facades,
        &mut index,
        result,
    );

    // The frontend sends the new appearance once it hears about the city.
    *look = Appearance::default();

    for (mut transform, mut cam, mut fog) in camera_q.iter_mut() {
        *fog = city_fog(index.radius);
        if !had_city {
            cam.frame(Vec3::ZERO, home_distance(index.radius));
            cam.apply(&mut transform);
        }
    }

    dispatch_city();
}

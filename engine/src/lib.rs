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
mod scene;
mod spawn;
mod treemap;

pub use api::queue_city_for_test;
pub use data::CityMap;
pub use render::FacadeMaterial;

use api::{consume_commands, publish_camera, publish_labels};
use appearance::{apply_appearance, Appearance, ShowLinks};
use camera::camera_controls;
use layout::Link;
use picking::{picking_system, Hovered};
use render::BuildingMaterials;
use scene::{process_city_data, setup_scene, SKY_COLOR};

const GROUND_Y: f32 = 0.0;
const DISTRICT_TOP: f32 = 0.35;
const PAVEMENT_Y: f32 = 0.40;
const BUILDING_BASE: f32 = 0.45;

#[derive(Component)]
struct CityElement;

#[derive(Component)]
struct LinkLayer;

#[derive(Component)]
struct HighlightLinkLayer {
    cycles: bool,
}

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
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.80, 0.86, 0.96),
            // Low: shadows carry the depth cue, and more washes the massing flat.
            brightness: 260.0,
            ..default()
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
                render_creation: RenderCreation::from(WgpuSettings {
                    // WebGL2 too: WebGPU alone is a blank canvas in Firefox and older Safari.
                    backends: Some(Backends::BROWSER_WEBGPU | Backends::GL),
                    ..default()
                }),
                ..default()
            }),
    );
    bevy::asset::embedded_asset!(app, "shaders/facade.wesl");
    app.add_plugins(MaterialPlugin::<FacadeMaterial>::default());
    add_city_systems(&mut app);
    app.run();
}

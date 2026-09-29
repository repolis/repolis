//! Boots the real schedule headlessly and runs frames.
//!
//! Bevy validates system parameters the first time a schedule runs, so
//! conflicting queries are a runtime panic rather than a compile error. Every
//! system in this app touches several overlapping queries (buildings,
//! materials, meshes, two views of the camera), which is exactly the shape
//! that trips that check - and in a wasm build the only symptom would be a
//! blank canvas.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::render::mesh::MeshPlugin;
use bevy::window::WindowPlugin;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        WindowPlugin {
            primary_window: Some(Window::default()),
            ..default()
        },
        TransformPlugin,
        HierarchyPlugin,
        bevy::input::InputPlugin,
        MeshPlugin,
    ));
    // Registered by PbrPlugin in the real app, which needs a GPU.
    app.init_asset::<StandardMaterial>();
    engine::add_city_systems(&mut app);
    app
}

#[test]
fn schedule_runs_without_system_conflicts() {
    let mut app = headless_app();
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn loading_a_city_spawns_and_respawns_cleanly() {
    let mut app = headless_app();
    app.update();

    let city = sample_city();
    engine::queue_city_for_test(city.clone());
    app.update();

    let count = app
        .world_mut()
        .query::<&Transform>()
        .iter(app.world())
        .count();
    assert!(count > 0, "loading a city spawned nothing");

    // The refinement pass re-sends the same repository; a second load must
    // tear down the first city rather than stacking on top of it.
    engine::queue_city_for_test(city);
    app.update();
    app.update();

    let after = app
        .world_mut()
        .query::<&Transform>()
        .iter(app.world())
        .count();
    assert_eq!(count, after, "reloading the same city changed the entity count");
}

fn sample_city() -> engine::CityMap {
    let json = r#"{
      "districts": [
        {"id":"d0","name":"Core","typology":"core","summary":"","tags":[],
         "buildings":[
           {"id":"a.c::Foo","name":"Foo","kind":"type","source_file":"a.c","dir":"src",
            "num_fields":4,"num_methods":6,"fields":["a","b","c","d"],"methods":["f","g"],
            "lines_of_code":40,"commit_churn":3,"churn_rank":0.8,"age_days":10},
           {"id":"a.c::<module>","name":"a.c","kind":"module","source_file":"a.c","dir":"src",
            "num_fields":1,"num_methods":2,"fields":[],"methods":["h"],"lines_of_code":80}
         ]},
        {"id":"d1","name":"Data","typology":"data","summary":"","tags":[],
         "buildings":[
           {"id":"b.c::Bar","name":"Bar","kind":"type","source_file":"b.c","dir":"src/io",
            "num_fields":9,"num_methods":14,"fields":[],"methods":[],
            "lines_of_code":120,"churn_rank":0.2,"age_days":600}
         ]}
      ],
      "dependencies":[{"source":"a.c::Foo","target":"b.c::Bar","weight":3,"kind":"call"}],
      "stats":{"total_buildings":3}
    }"#;
    serde_json::from_str(json).expect("sample city parses")
}

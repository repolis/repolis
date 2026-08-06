// ═══════════════════════════════════════════════════════════════════
// CodeCity Rendering Engine
// ═══════════════════════════════════════════════════════════════════
//
// This Wasm module receives a CityMap JSON from the Go backend and
// produces a 3D city visualization using Bevy.
//
// Architecture:
//   1. Data models (serde)     — deserialize backend JSON
//   2. Layout engine           — Skyline packing + Manhattan grid
//   3. Bevy renderer           — spawn 3D meshes from layout
//
// The layout algorithms run entirely in WebAssembly, keeping the
// backend focused on code analysis and the renderer self-contained.

mod layout;
mod data;

use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use data::CityMap;
use layout::LayoutResult;

// ═══════════════════════════════════════════════════════════════════
// Bevy Resources
// ═══════════════════════════════════════════════════════════════════

/// Marker: the city has been spawned, don't re-process.
#[derive(Resource, Default)]
struct CitySpawned(bool);

/// Marker component for controlling the camera in the 3D scene.
#[derive(Component)]
struct CityCamera {
    speed: f32,
    rot_speed: f32,
}

impl Default for CityCamera {
    fn default() -> Self {
        Self {
            speed: 120.0,
            rot_speed: 1.5,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Wasm entry points (called from JavaScript)
// ═══════════════════════════════════════════════════════════════════

#[wasm_bindgen(start)]
pub fn run_bevy_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CodeCity Engine".into(),
                canvas: Some("#bevy-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .init_resource::<CitySpawned>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (process_city_data, camera_controls))
        .run();
}

#[wasm_bindgen]
pub fn load_city_data(val: JsValue) {
    match serde_wasm_bindgen::from_value::<CityMap>(val) {
        Ok(city_map) => {
            let total_buildings: usize = city_map
                .districts
                .iter()
                .map(|d| d.buildings.len())
                .sum();

            web_sys::console::log_1(
                &format!(
                    "[engine] Received {} districts, {} buildings, {} roads",
                    city_map.districts.len(),
                    total_buildings,
                    city_map.roads.len(),
                )
                .into(),
            );

            // Store for the Bevy Update system to pick up.
            // Since we can't access Bevy resources from outside the ECS,
            // we use a thread-local to bridge the gap.
            PENDING_DATA.with(|cell| {
                *cell.borrow_mut() = Some(city_map);
            });
        }
        Err(e) => {
            web_sys::console::log_1(
                &format!("[engine] Failed to parse city data: {:?}", e).into(),
            );
        }
    }
}

// Thread-local bridge between JS calls and the Bevy ECS.
thread_local! {
    static PENDING_DATA: std::cell::RefCell<Option<CityMap>> = std::cell::RefCell::new(None);
}

// ═══════════════════════════════════════════════════════════════════
// Bevy Systems
// ═══════════════════════════════════════════════════════════════════

/// Initial scene: camera + directional light. No geometry until
/// city data arrives.
fn setup_scene(mut commands: Commands) {
    // Camera — orbiting view
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(40.0, 50.0, 80.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CityCamera::default(),
    ));

    // Directional sunlight
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_rotation(
            Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0),
        ),
        ..default()
    });

    // Ambient light so shadows aren't pitch black
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
    });
}

/// Checks if new city data has been loaded from JS. If so,
/// runs the layout algorithms and spawns the 3D city.
fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<CitySpawned>,
    mut camera_query: Query<&mut Transform, With<CityCamera>>,
) {
    if spawned.0 {
        return;
    }

    // Try to grab pending data from the JS bridge
    let city_data = PENDING_DATA.with(|cell| cell.borrow_mut().take());
    let city_map = match city_data {
        Some(data) => data,
        None => return,
    };

    web_sys::console::log_1(&"[engine] Computing layout...".into());

    // ── Run layout algorithms ────────────────────────────────
    let result = layout::compute_layout(&city_map);

    web_sys::console::log_1(
        &format!(
            "[engine] Layout complete: {} districts placed",
            result.districts.len()
        )
        .into(),
    );

    // ── Spawn 3D geometry ────────────────────────────────────
    spawn_city(&mut commands, &mut meshes, &mut materials, &result);

    // ── Reposition Camera to frame the entire city automatically ─────
    let total_width = result
        .districts
        .iter()
        .map(|d| (d.pos_x + d.width) as f32)
        .fold(0.0_f32, f32::max);
    let total_depth = result
        .districts
        .iter()
        .map(|d| (d.pos_z + d.depth) as f32)
        .fold(0.0_f32, f32::max);
    let max_height = result
        .districts
        .iter()
        .flat_map(|d| d.buildings.iter())
        .map(|b| b.height as f32)
        .fold(10.0_f32, f32::max);

    let center_x = total_width / 2.0;
    let center_z = total_depth / 2.0;
    let view_dist = f32::max(total_width, total_depth) * 0.85 + 60.0;
    let view_height = max_height * 0.8 + f32::max(total_width, total_depth) * 0.45 + 50.0;

    for mut cam_tf in camera_query.iter_mut() {
        *cam_tf = Transform::from_xyz(center_x, view_height, center_z + view_dist)
            .looking_at(Vec3::new(center_x, 0.0, center_z), Vec3::Y);
    }

    spawned.0 = true;
    web_sys::console::log_1(&"[engine] City spawned and camera reoriented!".into());
}

/// Interactive camera movement: WASD for translation, Arrows / Mouse drag for rotation, Shift for speed boost.
fn camera_controls(
    mut query: Query<(&mut Transform, &CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();
    for (mut tf, cam) in query.iter_mut() {
        let mut speed = cam.speed;
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            speed *= 3.0;
        }
        if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::AltLeft) {
            speed *= 0.3;
        }

        let mut velocity = Vec3::ZERO;
        let forward = tf.rotation * Vec3::NEG_Z;
        let right = tf.rotation * Vec3::X;

        // Translation Controls
        if keys.pressed(KeyCode::KeyW) {
            velocity += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            velocity -= forward;
        }
        if keys.pressed(KeyCode::KeyA) {
            velocity -= right;
        }
        if keys.pressed(KeyCode::KeyD) {
            velocity += right;
        }
        if keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::KeyE) {
            velocity += Vec3::Y;
        }
        if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyC) {
            velocity -= Vec3::Y;
        }

        if velocity != Vec3::ZERO {
            tf.translation += velocity.normalize() * speed * delta;
        }

        // Rotation via Keyboard arrows
        let mut yaw_delta = 0.0;
        let mut pitch_delta = 0.0;

        if keys.pressed(KeyCode::ArrowLeft) {
            yaw_delta += cam.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            yaw_delta -= cam.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowUp) {
            pitch_delta += cam.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            pitch_delta -= cam.rot_speed * delta;
        }

        // Rotation via Mouse Drag (Left or Right Click)
        for ev in mouse_motion.read() {
            if mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right) {
                yaw_delta -= ev.delta.x * 0.004;
                pitch_delta -= ev.delta.y * 0.004;
            }
        }

        if yaw_delta != 0.0 || pitch_delta != 0.0 {
            let (yaw, pitch, _roll) = tf.rotation.to_euler(EulerRot::YXZ);
            let new_yaw = yaw + yaw_delta;
            let new_pitch = (pitch + pitch_delta).clamp(-1.54, 1.54); // Prevent gimbal lock
            tf.rotation = Quat::from_euler(EulerRot::YXZ, new_yaw, new_pitch, 0.0);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 3D City Spawning
// ═══════════════════════════════════════════════════════════════════

/// Typology → district platform color
fn typology_color(typology: &str) -> Color {
    match typology {
        "core"      => Color::srgb(0.35, 0.38, 0.42), // steel gray
        "data"      => Color::srgb(0.18, 0.42, 0.25), // forest green
        "network"   => Color::srgb(0.15, 0.30, 0.55), // ocean blue
        "security"  => Color::srgb(0.55, 0.15, 0.15), // crimson
        "interface" => Color::srgb(0.60, 0.50, 0.20), // gold
        "utility"   => Color::srgb(0.50, 0.45, 0.35), // sand
        "config"    => Color::srgb(0.30, 0.32, 0.38), // slate
        "test"      => Color::srgb(0.35, 0.20, 0.50), // purple
        "example"   => Color::srgb(0.25, 0.45, 0.50), // teal
        _           => Color::srgb(0.25, 0.25, 0.25), // dark gray
    }
}

/// Typology → building color (lighter variant of district color)
fn building_color(typology: &str) -> Color {
    match typology {
        "core"      => Color::srgb(0.55, 0.58, 0.65),
        "data"      => Color::srgb(0.30, 0.60, 0.40),
        "network"   => Color::srgb(0.30, 0.50, 0.75),
        "security"  => Color::srgb(0.75, 0.30, 0.30),
        "interface" => Color::srgb(0.80, 0.70, 0.35),
        "utility"   => Color::srgb(0.70, 0.65, 0.50),
        "config"    => Color::srgb(0.50, 0.52, 0.58),
        "test"      => Color::srgb(0.55, 0.35, 0.70),
        "example"   => Color::srgb(0.40, 0.65, 0.70),
        _           => Color::srgb(0.45, 0.45, 0.50),
    }
}

fn spawn_city(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    result: &LayoutResult,
) {
    let platform_height = 0.4;

    for district in &result.districts {
        // ── District platform ────────────────────────────────
        let platform_color = typology_color(&district.typology);
        commands.spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(
                district.width as f32,
                platform_height,
                district.depth as f32,
            )),
            material: materials.add(StandardMaterial {
                base_color: platform_color,
                perceptual_roughness: 0.9,
                ..default()
            }),
            transform: Transform::from_xyz(
                (district.pos_x + district.width / 2.0) as f32,
                platform_height / 2.0,
                (district.pos_z + district.depth / 2.0) as f32,
            ),
            ..default()
        });

        // ── Buildings ────────────────────────────────────────
        let bldg_color = building_color(&district.typology);
        let bldg_material = materials.add(StandardMaterial {
            base_color: bldg_color,
            perceptual_roughness: 0.6,
            ..default()
        });

        for building in &district.buildings {
            let half_h = building.height as f32 / 2.0;
            commands.spawn(PbrBundle {
                mesh: meshes.add(Cuboid::new(
                    building.width as f32,
                    building.height as f32,
                    building.depth as f32,
                )),
                material: bldg_material.clone(),
                transform: Transform::from_xyz(
                    (building.pos_x + building.width / 2.0) as f32,
                    platform_height + half_h,
                    (building.pos_z + building.depth / 2.0) as f32,
                ),
                ..default()
            });
        }
    }

    // ── Ground plane (road surface between districts) ────────
    let total_width = result.districts.iter()
        .map(|d| (d.pos_x + d.width) as f32)
        .fold(0.0_f32, f32::max)
        + 20.0;
    let total_depth = result.districts.iter()
        .map(|d| (d.pos_z + d.depth) as f32)
        .fold(0.0_f32, f32::max)
        + 20.0;

    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::new(total_width, 0.05, total_depth)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.12, 0.14),
            perceptual_roughness: 1.0,
            ..default()
        }),
        transform: Transform::from_xyz(
            total_width / 2.0 - 10.0,
            0.025,
            total_depth / 2.0 - 10.0,
        ),
        ..default()
    });
}

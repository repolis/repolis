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

/// Marker: the city has been spawned at least once.
#[derive(Resource, Default)]
struct CitySpawned(bool);

/// Marker component for any spawned 3D geometry belonging to the current city map.
#[derive(Component)]
struct CityElement;

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
    existing_elements: Query<Entity, With<CityElement>>,
) {
    // Try to grab pending data from the JS bridge
    let city_data = PENDING_DATA.with(|cell| cell.borrow_mut().take());
    let city_map = match city_data {
        Some(data) => data,
        None => return,
    };

    // Despawn any existing city geometry before generating the new scene
    for entity in existing_elements.iter() {
        commands.entity(entity).despawn_recursive();
    }

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
    // Organic layout can have negative coordinates, so compute true bounding box
    let all_buildings: Vec<&layout::PlacedBuilding> = result
        .districts
        .iter()
        .flat_map(|d| d.buildings.iter())
        .collect();

    let (mut min_x, mut min_z, mut max_x, mut max_z) = if all_buildings.is_empty() {
        (-50.0_f64, -50.0_f64, 50.0_f64, 50.0_f64)
    } else {
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN)
    };
    for b in &all_buildings {
        if b.pos_x < min_x { min_x = b.pos_x; }
        if b.pos_z < min_z { min_z = b.pos_z; }
        if b.pos_x + b.width > max_x { max_x = b.pos_x + b.width; }
        if b.pos_z + b.depth > max_z { max_z = b.pos_z + b.depth; }
    }
    for r in &result.roads {
        if r.start_x < min_x { min_x = r.start_x; }
        if r.start_z < min_z { min_z = r.start_z; }
        if r.end_x > max_x { max_x = r.end_x; }
        if r.end_z > max_z { max_z = r.end_z; }
        if r.end_x < min_x { min_x = r.end_x; }
        if r.end_z < min_z { min_z = r.end_z; }
        if r.start_x > max_x { max_x = r.start_x; }
        if r.start_z > max_z { max_z = r.start_z; }
    }
    for p in &result.platforms {
        if p.pos_x < min_x { min_x = p.pos_x; }
        if p.pos_z < min_z { min_z = p.pos_z; }
        if p.pos_x + p.width > max_x { max_x = p.pos_x + p.width; }
        if p.pos_z + p.depth > max_z { max_z = p.pos_z + p.depth; }
    }

    let extent_x = (max_x - min_x) as f32;
    let extent_z = (max_z - min_z) as f32;
    let center_x = (min_x + max_x) as f32 / 2.0;
    let center_z = (min_z + max_z) as f32 / 2.0;
    let max_height = all_buildings
        .iter()
        .map(|b| b.height as f32)
        .fold(10.0_f32, f32::max);

    let view_dist = f32::max(extent_x, extent_z) * 0.85 + 60.0;
    let view_height = max_height * 0.8 + f32::max(extent_x, extent_z) * 0.45 + 50.0;

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
    let road_top: f32 = 0.10;
    let platform_top: f32 = 0.22;

    // ── Compute bounding box for ground plane ────────────────
    let (mut min_x, mut min_z, mut max_x, mut max_z) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for d in &result.districts {
        for b in &d.buildings {
            if b.pos_x < min_x { min_x = b.pos_x; }
            if b.pos_z < min_z { min_z = b.pos_z; }
            if b.pos_x + b.width > max_x { max_x = b.pos_x + b.width; }
            if b.pos_z + b.depth > max_z { max_z = b.pos_z + b.depth; }
        }
    }
    for r in &result.roads {
        for &x in &[r.start_x, r.end_x] {
            if x < min_x { min_x = x; }
            if x > max_x { max_x = x; }
        }
        for &z in &[r.start_z, r.end_z] {
            if z < min_z { min_z = z; }
            if z > max_z { max_z = z; }
        }
    }
    for p in &result.platforms {
        if p.pos_x < min_x { min_x = p.pos_x; }
        if p.pos_z < min_z { min_z = p.pos_z; }
        if p.pos_x + p.width > max_x { max_x = p.pos_x + p.width; }
        if p.pos_z + p.depth > max_z { max_z = p.pos_z + p.depth; }
    }
    // Fallback for empty scenes
    if min_x > max_x {
        min_x = -50.0; max_x = 50.0; min_z = -50.0; max_z = 50.0;
    }
    let pad = 40.0;
    let gw = (max_x - min_x) as f32 + pad * 2.0;
    let gd = (max_z - min_z) as f32 + pad * 2.0;
    let gcx = (min_x + max_x) as f32 / 2.0;
    let gcz = (min_z + max_z) as f32 / 2.0;

    // ── Ground plane (dark urban base) ───────────────────────
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(gw, 0.04, gd)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.08, 0.09, 0.11),
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_xyz(gcx, 0.02, gcz),
            ..default()
        },
        CityElement,
    ));

    // ── Road segments (urban streets & highways) ─────────────
    let road_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.19, 0.22),
        perceptual_roughness: 0.95,
        ..default()
    });

    for road in &result.roads {
        let dx = road.end_x - road.start_x;
        let dz = road.end_z - road.start_z;
        let length = (dx * dx + dz * dz).sqrt() as f32;
        if length < 0.01 {
            continue;
        }
        let angle = (dx as f32).atan2(dz as f32);
        let cx = (road.start_x + road.end_x) as f32 / 2.0;
        let cz = (road.start_z + road.end_z) as f32 / 2.0;

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(road.width as f32, road_top, length)),
                material: road_material.clone(),
                transform: Transform::from_xyz(cx, road_top / 2.0, cz)
                    .with_rotation(Quat::from_rotation_y(angle)),
                ..default()
            },
            CityElement,
        ));
    }

    // ── Raised Sidewalk Platforms (City Blocks) ──────────────
    for platform in &result.platforms {
        let plat_color = typology_color(&platform.typology);
        let plat_material = materials.add(StandardMaterial {
            base_color: plat_color,
            perceptual_roughness: 0.8,
            ..default()
        });

        let thickness = platform_top;
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(
                    platform.width as f32,
                    thickness,
                    platform.depth as f32,
                )),
                material: plat_material,
                transform: Transform::from_xyz(
                    (platform.pos_x + platform.width / 2.0) as f32,
                    thickness / 2.0,
                    (platform.pos_z + platform.depth / 2.0) as f32,
                ),
                ..default()
            },
            CityElement,
        ));
    }

    // ── Buildings (colored by district typology) ─────────────
    for district in &result.districts {
        let bldg_color = building_color(&district.typology);
        let bldg_material = materials.add(StandardMaterial {
            base_color: bldg_color,
            perceptual_roughness: 0.55,
            ..default()
        });

        for building in &district.buildings {
            let half_h = building.height as f32 / 2.0;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(
                        building.width as f32,
                        building.height as f32,
                        building.depth as f32,
                    )),
                    material: bldg_material.clone(),
                    transform: Transform::from_xyz(
                        (building.pos_x + building.width / 2.0) as f32,
                        platform_top + half_h,
                        (building.pos_z + building.depth / 2.0) as f32,
                    ),
                    ..default()
                },
                CityElement,
            ));
        }
    }
}

use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use data::CityMap;
use layout::LayoutResult;

mod data;
mod layout;

#[derive(Resource, Default)]
struct CitySpawned(bool);

#[derive(Component)]
struct CityElement;

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
                .map(|district| district.buildings.len())
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

            PENDING_DATA.with(|cell| {
                *cell.borrow_mut() = Some(city_map);
            });
        }
        Err(error) => {
            web_sys::console::log_1(
                &format!("[engine] Failed to parse city data: {:?}", error).into(),
            );
        }
    }
}

thread_local! {
    static PENDING_DATA: std::cell::RefCell<Option<CityMap>> = std::cell::RefCell::new(None);
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(40.0, 50.0, 80.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CityCamera::default(),
    ));

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

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
    });
}

fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<CitySpawned>,
    mut camera_query: Query<&mut Transform, With<CityCamera>>,
    existing_elements: Query<Entity, With<CityElement>>,
) {
    let city_data = PENDING_DATA.with(|cell| cell.borrow_mut().take());
    let city_map = match city_data {
        Some(data) => data,
        None => return,
    };

    for entity in existing_elements.iter() {
        commands.entity(entity).despawn_recursive();
    }

    let result = layout::compute_layout(&city_map);

    spawn_city(&mut commands, &mut meshes, &mut materials, &result);

    let all_buildings: Vec<&layout::PlacedBuilding> = result
        .districts
        .iter()
        .flat_map(|district| district.buildings.iter())
        .collect();

    let (mut min_x, mut min_z, mut max_x, mut max_z) = if all_buildings.is_empty() {
        (-50.0_f64, -50.0_f64, 50.0_f64, 50.0_f64)
    } else {
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN)
    };

    for building in &all_buildings {
        if building.pos_x < min_x { min_x = building.pos_x; }
        if building.pos_z < min_z { min_z = building.pos_z; }
        if building.pos_x + building.width > max_x { max_x = building.pos_x + building.width; }
        if building.pos_z + building.depth > max_z { max_z = building.pos_z + building.depth; }
    }
    for road in &result.roads {
        if road.start_x < min_x { min_x = road.start_x; }
        if road.start_z < min_z { min_z = road.start_z; }
        if road.end_x > max_x { max_x = road.end_x; }
        if road.end_z > max_z { max_z = road.end_z; }
        if road.end_x < min_x { min_x = road.end_x; }
        if road.end_z < min_z { min_z = road.end_z; }
        if road.start_x > max_x { max_x = road.start_x; }
        if road.start_z > max_z { max_z = road.start_z; }
    }
    for platform in &result.platforms {
        if platform.pos_x < min_x { min_x = platform.pos_x; }
        if platform.pos_z < min_z { min_z = platform.pos_z; }
        if platform.pos_x + platform.width > max_x { max_x = platform.pos_x + platform.width; }
        if platform.pos_z + platform.depth > max_z { max_z = platform.pos_z + platform.depth; }
    }

    let extent_x = (max_x - min_x) as f32;
    let extent_z = (max_z - min_z) as f32;
    let center_x = (min_x + max_x) as f32 / 2.0;
    let center_z = (min_z + max_z) as f32 / 2.0;
    let max_height = all_buildings
        .iter()
        .map(|building| building.height as f32)
        .fold(10.0_f32, f32::max);

    let view_dist = f32::max(extent_x, extent_z) * 0.85 + 60.0;
    let view_height = max_height * 0.8 + f32::max(extent_x, extent_z) * 0.45 + 50.0;

    for mut transform in camera_query.iter_mut() {
        *transform = Transform::from_xyz(center_x, view_height, center_z + view_dist)
            .looking_at(Vec3::new(center_x, 0.0, center_z), Vec3::Y);
    }

    spawned.0 = true;
}

fn camera_controls(
    mut query: Query<(&mut Transform, &CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();
    for (mut transform, camera) in query.iter_mut() {
        let mut speed = camera.speed;
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            speed *= 3.0;
        }
        if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::AltLeft) {
            speed *= 0.3;
        }

        let mut velocity = Vec3::ZERO;
        let forward = transform.rotation * Vec3::NEG_Z;
        let right = transform.rotation * Vec3::X;

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
            transform.translation += velocity.normalize() * speed * delta;
        }

        let mut yaw_delta = 0.0;
        let mut pitch_delta = 0.0;

        if keys.pressed(KeyCode::ArrowLeft) {
            yaw_delta += camera.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            yaw_delta -= camera.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowUp) {
            pitch_delta += camera.rot_speed * delta;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            pitch_delta -= camera.rot_speed * delta;
        }

        for ev in mouse_motion.read() {
            if mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right) {
                yaw_delta -= ev.delta.x * 0.004;
                pitch_delta -= ev.delta.y * 0.004;
            }
        }

        if yaw_delta != 0.0 || pitch_delta != 0.0 {
            let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
            let new_yaw = yaw + yaw_delta;
            let new_pitch = (pitch + pitch_delta).clamp(-1.54, 1.54);
            transform.rotation = Quat::from_euler(EulerRot::YXZ, new_yaw, new_pitch, 0.0);
        }
    }
}

fn typology_color(typology: &str) -> Color {
    match typology {
        "core"      => Color::srgb(0.35, 0.38, 0.42),
        "data"      => Color::srgb(0.18, 0.42, 0.25),
        "network"   => Color::srgb(0.15, 0.30, 0.55),
        "security"  => Color::srgb(0.55, 0.15, 0.15),
        "interface" => Color::srgb(0.60, 0.50, 0.20),
        "utility"   => Color::srgb(0.50, 0.45, 0.35),
        "config"    => Color::srgb(0.30, 0.32, 0.38),
        "test"      => Color::srgb(0.35, 0.20, 0.50),
        "example"   => Color::srgb(0.25, 0.45, 0.50),
        _           => Color::srgb(0.25, 0.25, 0.25),
    }
}

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
    let road_height: f32 = 0.15;
    let platform_top: f32 = 0.22;

    let (mut min_x, mut min_z, mut max_x, mut max_z) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for district in &result.districts {
        for building in &district.buildings {
            if building.pos_x < min_x { min_x = building.pos_x; }
            if building.pos_z < min_z { min_z = building.pos_z; }
            if building.pos_x + building.width > max_x { max_x = building.pos_x + building.width; }
            if building.pos_z + building.depth > max_z { max_z = building.pos_z + building.depth; }
        }
    }
    for road in &result.roads {
        for &x in &[road.start_x, road.end_x] {
            if x < min_x { min_x = x; }
            if x > max_x { max_x = x; }
        }
        for &z in &[road.start_z, road.end_z] {
            if z < min_z { min_z = z; }
            if z > max_z { max_z = z; }
        }
    }
    for platform in &result.platforms {
        if platform.pos_x < min_x { min_x = platform.pos_x; }
        if platform.pos_z < min_z { min_z = platform.pos_z; }
        if platform.pos_x + platform.width > max_x { max_x = platform.pos_x + platform.width; }
        if platform.pos_z + platform.depth > max_z { max_z = platform.pos_z + platform.depth; }
    }
    
    if min_x > max_x {
        min_x = -50.0; max_x = 50.0; min_z = -50.0; max_z = 50.0;
    }
    
    let pad = 50.0;
    let ground_width = (max_x - min_x) as f32 + pad * 2.0;
    let ground_depth = (max_z - min_z) as f32 + pad * 2.0;
    let ground_center_x = (min_x + max_x) as f32 / 2.0;
    let ground_center_z = (min_z + max_z) as f32 / 2.0;

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(ground_width, 0.04, ground_depth)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.06, 0.07, 0.06),
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_xyz(ground_center_x, 0.02, ground_center_z),
            ..default()
        },
        CityElement,
    ));

    let road_light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.18, 0.28),
        emissive: LinearRgba::new(0.02, 0.05, 0.10, 1.0),
        perceptual_roughness: 0.75,
        ..default()
    });
    
    let road_medium = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.32, 0.42),
        emissive: LinearRgba::new(0.04, 0.15, 0.22, 1.0),
        perceptual_roughness: 0.55,
        ..default()
    });
    
    let road_heavy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.48, 0.58),
        emissive: LinearRgba::new(0.08, 0.30, 0.40, 1.0),
        perceptual_roughness: 0.35,
        ..default()
    });

    for road in &result.roads {
        let delta_x = road.end_x - road.start_x;
        let delta_z = road.end_z - road.start_z;
        let length = (delta_x * delta_x + delta_z * delta_z).sqrt() as f32;
        
        if length < 0.01 {
            continue;
        }
        
        let angle = (delta_x as f32).atan2(delta_z as f32);
        let center_x = (road.start_x + road.end_x) as f32 / 2.0;
        let center_z = (road.start_z + road.end_z) as f32 / 2.0;

        let width = road.width as f32;
        let material = if width >= 8.0 {
            road_heavy.clone()
        } else if width >= 4.0 {
            road_medium.clone()
        } else {
            road_light.clone()
        };

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(width, road_height, length)),
                material,
                transform: Transform::from_xyz(center_x, road_height / 2.0, center_z)
                    .with_rotation(Quat::from_rotation_y(angle)),
                ..default()
            },
            CityElement,
        ));
    }

    for platform in &result.platforms {
        let platform_color = typology_color(&platform.typology);
        let platform_material = materials.add(StandardMaterial {
            base_color: platform_color,
            perceptual_roughness: 0.8,
            ..default()
        });

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(
                    platform.width as f32,
                    platform_top,
                    platform.depth as f32,
                )),
                material: platform_material,
                transform: Transform::from_xyz(
                    (platform.pos_x + platform.width / 2.0) as f32,
                    platform_top / 2.0,
                    (platform.pos_z + platform.depth / 2.0) as f32,
                ),
                ..default()
            },
            CityElement,
        ));
    }

    for district in &result.districts {
        let building_color = building_color(&district.typology);
        let building_material = materials.add(StandardMaterial {
            base_color: building_color,
            perceptual_roughness: 0.55,
            ..default()
        });

        for building in &district.buildings {
            let half_height = building.height as f32 / 2.0;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(
                        building.width as f32,
                        building.height as f32,
                        building.depth as f32,
                    )),
                    material: building_material.clone(),
                    transform: Transform::from_xyz(
                        (building.pos_x + building.width / 2.0) as f32,
                        platform_top + half_height,
                        (building.pos_z + building.depth / 2.0) as f32,
                    ),
                    ..default()
                },
                CityElement,
            ));
        }
    }
}

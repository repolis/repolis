use bevy::prelude::*;
use bevy::pbr::{CascadeShadowConfigBuilder, ScreenSpaceAmbientOcclusionSettings, FogSettings, FogFalloff};
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
struct CameraStateText;

#[derive(PartialEq)]
enum CameraMode {
    Orbit,
    Free,
}

#[derive(Component)]
struct CityCamera {
    mode: CameraMode,
    speed: f32,
    rot_speed: f32,
    focus: Vec3,
    radius: f32,
    alpha: f32,
    beta: f32,
}

impl Default for CityCamera {
    fn default() -> Self {
        Self {
            mode: CameraMode::Orbit,
            speed: 120.0,
            rot_speed: 1.5,
            focus: Vec3::ZERO,
            radius: 100.0,
            alpha: std::f32::consts::FRAC_PI_4,
            beta: std::f32::consts::FRAC_PI_6,
        }
    }
}

#[wasm_bindgen(start)]
pub fn run_bevy_app() {
    let sky_color = Color::srgb(0.60, 0.70, 0.80);

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
        .insert_resource(ClearColor(sky_color))
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
    let sky_color = Color::srgb(0.60, 0.70, 0.80);

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(40.0, 50.0, 80.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CityCamera::default(),
        FogSettings {
            color: sky_color,
            falloff: FogFalloff::Linear {
                start: 100.0,
                end: 800.0,
            },
            ..default()
        },
        ScreenSpaceAmbientOcclusionSettings::default(),
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.98, 0.95), // Clean, slightly warm sunlight
            ..default()
        },
        transform: Transform::from_rotation(
            Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0),
        ),
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 20.0,
            maximum_distance: 800.0,
            ..default()
        }
        .into(),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0, // Neutral white ambient light to avoid blue tinting
    });
    
    // UI Text for Camera State
    commands.spawn((
        TextBundle::from_section(
            "Camera: ORBIT (Press F to toggle)",
            TextStyle {
                font_size: 20.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
        CameraStateText,
    ));
}

fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<CitySpawned>,
    mut camera_query: Query<(&mut Transform, &mut CityCamera)>,
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
    
    let view_dist = f32::max(extent_x, extent_z) * 0.85 + 60.0;

    for (mut transform, mut camera) in camera_query.iter_mut() {
        camera.focus = Vec3::new(center_x, 0.0, center_z);
        camera.radius = view_dist;
        camera.alpha = std::f32::consts::FRAC_PI_4;
        camera.beta = std::f32::consts::FRAC_PI_6;
        
        let x = camera.focus.x + camera.radius * camera.alpha.sin() * camera.beta.cos();
        let y = camera.focus.y + camera.radius * camera.beta.sin();
        let z = camera.focus.z + camera.radius * camera.alpha.cos() * camera.beta.cos();
        
        *transform = Transform::from_xyz(x, y, z).looking_at(camera.focus, Vec3::Y);
    }

    spawned.0 = true;
}

fn camera_controls(
    mut query: Query<(&mut Transform, &mut CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel: EventReader<bevy::input::mouse::MouseWheel>,
    time: Res<Time>,
    mut text_query: Query<&mut Text, With<CameraStateText>>,
) {
    let delta = time.delta_seconds();
    let toggle_pressed = keys.just_pressed(KeyCode::KeyF);
    
    for (mut transform, mut camera) in query.iter_mut() {
        if toggle_pressed {
            if camera.mode == CameraMode::Orbit {
                camera.mode = CameraMode::Free;
                for mut text in text_query.iter_mut() {
                    text.sections[0].value = "Camera: FREECAM (Press F to toggle)".to_string();
                }
            } else {
                camera.mode = CameraMode::Orbit;
                for mut text in text_query.iter_mut() {
                    text.sections[0].value = "Camera: ORBIT (Press F to toggle)".to_string();
                }
                let offset = transform.translation - camera.focus;
                camera.radius = offset.length();
                camera.alpha = offset.x.atan2(offset.z);
                camera.beta = (offset.y / camera.radius).asin().clamp(0.01, std::f32::consts::PI / 2.0 - 0.01);
            }
        }

        let mut mouse_delta = Vec2::ZERO;
        for ev in mouse_motion.read() {
            if mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right) {
                mouse_delta += ev.delta;
            }
        }
        
        let mut scroll = 0.0;
        for ev in mouse_wheel.read() {
            scroll += ev.y;
        }

        if camera.mode == CameraMode::Orbit {
            if mouse_delta != Vec2::ZERO {
                camera.alpha -= mouse_delta.x * 0.005;
                camera.beta += mouse_delta.y * 0.005;
                camera.beta = camera.beta.clamp(0.01, std::f32::consts::PI / 2.0 - 0.01);
            }
            if scroll != 0.0 {
                camera.radius -= scroll * (camera.radius * 0.1).max(5.0);
                camera.radius = camera.radius.clamp(10.0, 5000.0);
            }
            
            let x = camera.focus.x + camera.radius * camera.alpha.sin() * camera.beta.cos();
            let y = camera.focus.y + camera.radius * camera.beta.sin();
            let z = camera.focus.z + camera.radius * camera.alpha.cos() * camera.beta.cos();
            
            *transform = Transform::from_xyz(x, y, z).looking_at(camera.focus, Vec3::Y);
        } else {
            let mut speed = camera.speed;
            if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { speed *= 3.0; }
            if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::AltLeft) { speed *= 0.3; }

            let mut velocity = Vec3::ZERO;
            let forward = transform.rotation * Vec3::NEG_Z;
            let right = transform.rotation * Vec3::X;

            if keys.pressed(KeyCode::KeyW) { velocity += forward; }
            if keys.pressed(KeyCode::KeyS) { velocity -= forward; }
            if keys.pressed(KeyCode::KeyA) { velocity -= right; }
            if keys.pressed(KeyCode::KeyD) { velocity += right; }
            if keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::KeyE) { velocity += Vec3::Y; }
            if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyC) { velocity -= Vec3::Y; }

            if velocity != Vec3::ZERO {
                transform.translation += velocity.normalize() * speed * delta;
            }

            let mut yaw_delta = 0.0;
            let mut pitch_delta = 0.0;

            if keys.pressed(KeyCode::ArrowLeft) { yaw_delta += camera.rot_speed * delta; }
            if keys.pressed(KeyCode::ArrowRight) { yaw_delta -= camera.rot_speed * delta; }
            if keys.pressed(KeyCode::ArrowUp) { pitch_delta += camera.rot_speed * delta; }
            if keys.pressed(KeyCode::ArrowDown) { pitch_delta -= camera.rot_speed * delta; }

            yaw_delta -= mouse_delta.x * 0.004;
            pitch_delta -= mouse_delta.y * 0.004;

            if yaw_delta != 0.0 || pitch_delta != 0.0 {
                let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
                let new_yaw = yaw + yaw_delta;
                let new_pitch = (pitch + pitch_delta).clamp(-1.54, 1.54);
                transform.rotation = Quat::from_euler(EulerRot::YXZ, new_yaw, new_pitch, 0.0);
            }
        }
    }
}

fn hash_to_index(name: &str, max: usize) -> usize {
    let mut hash: u64 = 5381;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    (hash as usize) % max
}

fn typology_color(typology: &str, _name: &str) -> Color {
    if typology == "unknown" || typology.is_empty() {
        return Color::srgb(0.35, 0.35, 0.35); // Concrete grey platform
    }
    match typology {
        "district_base" => Color::srgb(0.25, 0.26, 0.28), // Very dark concrete for district base
        "core"      => Color::srgb(0.35, 0.38, 0.42),
        "data"      => Color::srgb(0.18, 0.42, 0.25),
        "network"   => Color::srgb(0.15, 0.30, 0.55),
        "security"  => Color::srgb(0.55, 0.15, 0.15),
        "interface" => Color::srgb(0.60, 0.50, 0.20),
        "utility"   => Color::srgb(0.50, 0.45, 0.35),
        "config"    => Color::srgb(0.30, 0.32, 0.38),
        "test"      => Color::srgb(0.35, 0.20, 0.50),
        "example"   => Color::srgb(0.25, 0.45, 0.50),
        _           => Color::srgb(0.35, 0.35, 0.35),
    }
}

fn building_color(typology: &str, name: &str) -> Color {
    if typology == "unknown" || typology.is_empty() {
        // A curated palette of realistic architectural colors
        let palette = [
            Color::srgb(0.85, 0.85, 0.85), // White concrete
            Color::srgb(0.70, 0.70, 0.70), // Light grey
            Color::srgb(0.50, 0.55, 0.60), // Steel / Glass bluish-grey
            Color::srgb(0.40, 0.40, 0.40), // Dark grey
            Color::srgb(0.80, 0.78, 0.75), // Warm off-white
        ];
        return palette[hash_to_index(name, palette.len())];
    }
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
        _           => Color::srgb(0.70, 0.70, 0.70),
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
    
    // Spawn a practically infinite ground plane
    let ground_center_x = (min_x + max_x) as f32 / 2.0;
    let ground_center_z = (min_z + max_z) as f32 / 2.0;

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(20000.0, 0.04, 20000.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.20, 0.20, 0.20), // Dark asphalt
                perceptual_roughness: 0.95,
                ..default()
            }),
            transform: Transform::from_xyz(ground_center_x, 0.02, ground_center_z),
            ..default()
        },
        CityElement,
    ));

    let data_stream_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.8, 1.0), 
        emissive: Color::linear_rgb(0.0, 4.0, 5.0).into(), // Glowing cyan
        perceptual_roughness: 0.2,
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
        
        // Data streams shouldn't be massive 8-unit wide roads. 
        // We scale them down to look like thick fiber optic cables.
        let stream_width = (road.width as f32) * 0.15;
        // Float them slightly above the ground/platforms
        let stream_height = 0.3; 

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(stream_width, 0.05, length)),
                material: data_stream_material.clone(),
                transform: Transform::from_xyz(center_x, stream_height, center_z)
                    .with_rotation(Quat::from_rotation_y(angle)),
                ..default()
            },
            CityElement,
        ));
    }

    for platform in &result.platforms {
        let is_district_base = platform.typology == "district_base";
        let current_platform_top = if is_district_base { 0.12 } else { platform_top };
        
        let platform_color = typology_color(&platform.typology, "platform_color");
        let platform_material = materials.add(StandardMaterial {
            base_color: platform_color,
            perceptual_roughness: 0.9,
            ..default()
        });

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(
                    platform.width as f32,
                    current_platform_top,
                    platform.depth as f32,
                )),
                material: platform_material,
                transform: Transform::from_xyz(
                    (platform.pos_x + platform.width / 2.0) as f32,
                    current_platform_top / 2.0,
                    (platform.pos_z + platform.depth / 2.0) as f32,
                ),
                ..default()
            },
            CityElement,
        ));
    }

    for district in &result.districts {
        for building in &district.buildings {
            let b_color = building_color(&district.typology, &building.name);
            let building_material = materials.add(StandardMaterial {
                base_color: b_color,
                perceptual_roughness: 0.8,
                ..default()
            });

            let half_height = building.height as f32 / 2.0;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(
                        building.width as f32,
                        building.height as f32,
                        building.depth as f32,
                    )),
                    material: building_material,
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

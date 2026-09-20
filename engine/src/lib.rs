use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::settings::{Backends, RenderCreation, WgpuFeatures, WgpuSettings};
use bevy::render::RenderPlugin;
use wasm_bindgen::prelude::*;

pub const SKY_COLOR: Color = Color::srgb(0.60, 0.70, 0.80);

use std::collections::HashMap;

use data::CityMap;
use layout::LayoutResult;
use hover::*;

mod data;
mod layout;
mod hover;

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
    console_error_panic_hook::set_once();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "CodeCity Engine".into(),
                        canvas: Some("#bevy-canvas".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: Some(Backends::BROWSER_WEBGPU),
                        features: WgpuFeatures::empty(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .init_resource::<CitySpawned>()
        .init_resource::<HoverState>()
        .insert_resource(ClearColor(SKY_COLOR))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (process_city_data, camera_controls, hover_picking_system))
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
            projection: Projection::Perspective(PerspectiveProjection {
                near: 0.5,
                far: 100000.0,
                ..default()
            }),
            transform: Transform::from_xyz(40.0, 50.0, 80.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CityCamera::default(),
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 14000.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 0.98, 0.95), // Clean, slightly warm sunlight
            ..default()
        },
        transform: Transform::from_rotation(
            Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0),
        ),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.92, 0.94, 0.98),
        brightness: 450.0, // Neutral ambient light for crisp architectural clarity
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
    mut hover_state: ResMut<HoverState>,
    mut camera_query: Query<(&mut Transform, &mut CityCamera)>,
    existing_elements: Query<Entity, With<CityElement>>,
) {
    let city_data = PENDING_DATA.with(|cell| cell.borrow_mut().take());
    let city_map = match city_data {
        Some(data) => data,
        None => return,
    };

    hover_state.hovered_entity = None;
    dispatch_hover_event(&HoverPayload::None);

    for entity in existing_elements.iter() {
        commands.entity(entity).despawn_recursive();
    }

    let result = layout::compute_layout(&city_map);
    spawn_city(&mut commands, &mut meshes, &mut materials, &result, &city_map);

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
        for &(px, pz) in &road.points {
            if px < min_x { min_x = px; }
            if pz < min_z { min_z = pz; }
            if px > max_x { max_x = px; }
            if pz > max_z { max_z = pz; }
        }
    }
    for platform in &result.platforms {
        if platform.polygon.is_empty() {
            if platform.pos_x < min_x { min_x = platform.pos_x; }
            if platform.pos_z < min_z { min_z = platform.pos_z; }
            if platform.pos_x + platform.width > max_x { max_x = platform.pos_x + platform.width; }
            if platform.pos_z + platform.depth > max_z { max_z = platform.pos_z + platform.depth; }
        } else {
            for &(px, pz) in &platform.polygon {
                if px < min_x { min_x = px; }
                if pz < min_z { min_z = pz; }
                if px > max_x { max_x = px; }
                if pz > max_z { max_z = pz; }
            }
        }
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

fn color_to_key(color: &Color) -> u32 {
    let linear = color.to_linear();
    let r = (linear.red * 255.0).clamp(0.0, 255.0) as u32;
    let g = (linear.green * 255.0).clamp(0.0, 255.0) as u32;
    let b = (linear.blue * 255.0).clamp(0.0, 255.0) as u32;
    let a = (linear.alpha * 255.0).clamp(0.0, 255.0) as u32;
    (r << 24) | (g << 16) | (b << 8) | a
}

fn spawn_city(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    result: &LayoutResult,
    city_map: &CityMap,
) {
    let platform_top: f32 = 0.22;

    // Shared unit cube mesh for all buildings, straight roads, and rectangular platforms
    // Allows Bevy's PBR engine to batch and GPU-instance thousands of entities into ~10 draw calls
    let unit_cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // Material caches to share identical StandardMaterial handles
    let mut building_material_cache: HashMap<u32, (Handle<StandardMaterial>, Handle<StandardMaterial>)> = HashMap::new();
    let mut platform_material_cache: HashMap<String, (Handle<StandardMaterial>, Handle<StandardMaterial>)> = HashMap::new();

    // Index dependencies
    let mut outgoing_deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming_deps: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &city_map.dependencies {
        outgoing_deps.entry(edge.source.clone()).or_default().push(edge.target.clone());
        incoming_deps.entry(edge.target.clone()).or_default().push(edge.source.clone());
    }

    // Index building data
    let mut building_data_map: HashMap<&str, (&data::Building, &data::District)> = HashMap::new();
    for district in &city_map.districts {
        for b in &district.buildings {
            building_data_map.insert(b.name.as_str(), (b, district));
        }
    }

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
        for &(px, pz) in &road.points {
            if px < min_x { min_x = px; }
            if pz < min_z { min_z = pz; }
            if px > max_x { max_x = px; }
            if pz > max_z { max_z = pz; }
        }
    }
    for platform in &result.platforms {
        if platform.polygon.is_empty() {
            if platform.pos_x < min_x { min_x = platform.pos_x; }
            if platform.pos_z < min_z { min_z = platform.pos_z; }
            if platform.pos_x + platform.width > max_x { max_x = platform.pos_x + platform.width; }
            if platform.pos_z + platform.depth > max_z { max_z = platform.pos_z + platform.depth; }
        } else {
            for &(px, pz) in &platform.polygon {
                if px < min_x { min_x = px; }
                if pz < min_z { min_z = pz; }
                if px > max_x { max_x = px; }
                if pz > max_z { max_z = pz; }
            }
        }
    }

    if min_x > max_x {
        min_x = -50.0; max_x = 50.0; min_z = -50.0; max_z = 50.0;
    }

    // Spawn circular ground plane and horizon fog fade
    let ground_center_x = (min_x + max_x) as f32 / 2.0;
    let ground_center_z = (min_z + max_z) as f32 / 2.0;

    let mut max_dist_sq: f32 = 0.0;
    let mut check_point = |px: f32, pz: f32| {
        let dx = px - ground_center_x;
        let dz = pz - ground_center_z;
        let d2 = dx * dx + dz * dz;
        if d2 > max_dist_sq {
            max_dist_sq = d2;
        }
    };

    for district in &result.districts {
        for b in &district.buildings {
            check_point(b.pos_x as f32, b.pos_z as f32);
            check_point((b.pos_x + b.width) as f32, b.pos_z as f32);
            check_point(b.pos_x as f32, (b.pos_z + b.depth) as f32);
            check_point((b.pos_x + b.width) as f32, (b.pos_z + b.depth) as f32);
        }
    }
    for road in &result.roads {
        for &(px, pz) in &road.points {
            check_point(px as f32, pz as f32);
        }
    }
    for platform in &result.platforms {
        if platform.polygon.is_empty() {
            check_point(platform.pos_x as f32, platform.pos_z as f32);
            check_point((platform.pos_x + platform.width) as f32, platform.pos_z as f32);
            check_point(platform.pos_x as f32, (platform.pos_z + platform.depth) as f32);
            check_point((platform.pos_x + platform.width) as f32, (platform.pos_z + platform.depth) as f32);
        } else {
            for &(px, pz) in &platform.polygon {
                check_point(px as f32, pz as f32);
            }
        }
    }

    let raw_radius = max_dist_sq.sqrt().max(50.0);
    let city_radius = raw_radius + 30.0;
    let r_inner = city_radius;
    let transition_width = (city_radius * 0.35).clamp(80.0, 300.0);
    let r_outer = r_inner + transition_width;
    let r_far = 60000.0_f32;

    // 1. Lit asphalt circular ground under the city
    let ground_mesh = generate_circular_ground_mesh(r_outer + 50.0, 128);
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(ground_mesh),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.20, 0.20, 0.20), // Dark asphalt
                perceptual_roughness: 0.95,
                cull_mode: None,
                double_sided: true,
                ..default()
            }),
            transform: Transform::from_xyz(ground_center_x, 0.02, ground_center_z),
            ..default()
        },
        CityElement,
    ));

    // 2. Circular fog fade and horizon skirt (alpha 0.0 inside city, fading to 1.0 sky color at edge)
    let fog_mesh = generate_circular_fog_mesh(r_inner, r_outer, r_far, SKY_COLOR, 128);
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(fog_mesh),
            material: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                double_sided: true,
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_xyz(ground_center_x, 0.022, ground_center_z),
            ..default()
        },
        CityElement,
    ));

    let default_road_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.8, 1.0),
        emissive: Color::linear_rgb(0.0, 4.0, 5.0).into(), // Glowing cyan
        perceptual_roughness: 0.2,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let hover_road_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.2),
        emissive: Color::linear_rgb(8.0, 6.0, 1.0).into(), // Intense electric amber-gold glow
        perceptual_roughness: 0.1,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    // Spawn continuous, curved road meshes along the agent paths
    for road in &result.roads {
        let stream_width = (road.width as f32) * 0.15;
        let stream_height = 0.28;

        let road_info = RoadHoverInfo {
            name: road.name.clone(),
            road_type: road.road_type.clone(),
            source: road.source.clone(),
            target: road.target.clone(),
            weight: road.weight,
        };

        let road_points = if road.points.len() >= 2 {
            road.points.clone()
        } else {
            vec![(road.start_x, road.start_z), (road.end_x, road.end_z)]
        };

        let pickable = PickableRoad {
            info: road_info,
            points: road_points,
            width: stream_width,
            default_material: default_road_material.clone(),
            hover_material: hover_road_material.clone(),
        };

        if road.points.len() >= 2 {
            let mesh = generate_curved_road_mesh(&road.points, stream_width, stream_height);
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material: default_road_material.clone(),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                pickable,
                CityElement,
            ));
        } else {
            let delta_x = road.end_x - road.start_x;
            let delta_z = road.end_z - road.start_z;
            let length = (delta_x * delta_x + delta_z * delta_z).sqrt() as f32;
            if length < 0.01 {
                continue;
            }
            let angle = (delta_x as f32).atan2(delta_z as f32);
            let center_x = (road.start_x + road.end_x) as f32 / 2.0;
            let center_z = (road.start_z + road.end_z) as f32 / 2.0;

            commands.spawn((
                PbrBundle {
                    mesh: unit_cube_mesh.clone(),
                    material: default_road_material.clone(),
                    transform: Transform::from_xyz(center_x, stream_height, center_z)
                        .with_rotation(Quat::from_rotation_y(angle))
                        .with_scale(Vec3::new(stream_width, 0.05, length)),
                    ..default()
                },
                pickable,
                CityElement,
            ));
        }
    }

    // Spawn organic Voronoi district platforms and rectangular building lot platforms
    for platform in &result.platforms {
        let (platform_material, hover_platform_material) = platform_material_cache
            .entry(platform.typology.clone())
            .or_insert_with(|| {
                let platform_color = typology_color(&platform.typology, "platform_color");
                let plat_linear = platform_color.to_linear();
                let def = materials.add(StandardMaterial {
                    base_color: platform_color,
                    perceptual_roughness: 0.9,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                let hov = materials.add(StandardMaterial {
                    base_color: Color::srgb(
                        (plat_linear.red * 1.4).min(1.0),
                        (plat_linear.green * 1.4).min(1.0),
                        (plat_linear.blue * 1.4).min(1.0),
                    ),
                    emissive: Color::linear_rgb(0.3, 0.6, 1.0).into(), // Cyber blue illuminated base
                    perceptual_roughness: 0.4,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                (def, hov)
            })
            .clone();

        let district_data = city_map.districts.get(platform.district_idx);
        let district_info = if let Some(d) = district_data {
            let total_loc: u32 = d.buildings.iter().map(|b| b.lines_of_code).sum();
            let b_names: Vec<String> = d.buildings.iter().map(|b| b.name.clone()).collect();
            DistrictHoverInfo {
                name: d.name.clone(),
                typology: d.typology.clone(),
                summary: d.summary.clone(),
                tags: d.tags.clone(),
                building_count: d.buildings.len(),
                total_lines_of_code: total_loc,
                buildings: b_names,
            }
        } else {
            DistrictHoverInfo {
                name: platform.typology.clone(),
                typology: platform.typology.clone(),
                ..default()
            }
        };

        let poly = if platform.polygon.len() >= 3 {
            platform.polygon.clone()
        } else {
            vec![
                (platform.pos_x, platform.pos_z),
                (platform.pos_x + platform.width, platform.pos_z),
                (platform.pos_x + platform.width, platform.pos_z + platform.depth),
                (platform.pos_x, platform.pos_z + platform.depth),
            ]
        };

        let pickable = PickableDistrict {
            info: district_info,
            polygon: poly,
            default_material: platform_material.clone(),
            hover_material: hover_platform_material,
        };

        if platform.polygon.len() >= 3 {
            let mesh = generate_polygon_prism_mesh(&platform.polygon, 0.12);
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material: platform_material,
                    transform: Transform::IDENTITY,
                    ..default()
                },
                pickable,
                CityElement,
            ));
        } else {
            commands.spawn((
                PbrBundle {
                    mesh: unit_cube_mesh.clone(),
                    material: platform_material,
                    transform: Transform::from_xyz(
                        (platform.pos_x + platform.width / 2.0) as f32,
                        platform_top / 2.0,
                        (platform.pos_z + platform.depth / 2.0) as f32,
                    )
                    .with_scale(Vec3::new(
                        platform.width as f32,
                        platform_top,
                        platform.depth as f32,
                    )),
                    ..default()
                },
                pickable,
                CityElement,
            ));
        }
    }

    // Spawn buildings with shared unit cube mesh and batched materials
    let fallback_b = data::Building::default();
    let fallback_d = data::District::default();
    for district in &result.districts {
        for building in &district.buildings {
            let b_color = building_color(&district.typology, &building.name);
            let color_key = color_to_key(&b_color);

            let (building_material, hover_building_material) = building_material_cache
                .entry(color_key)
                .or_insert_with(|| {
                    let b_linear = b_color.to_linear();
                    let def = materials.add(StandardMaterial {
                        base_color: b_color,
                        perceptual_roughness: 0.8,
                        ..default()
                    });
                    let hov = materials.add(StandardMaterial {
                        base_color: Color::srgb(
                            (b_linear.red * 1.5).min(1.0),
                            (b_linear.green * 1.5).min(1.0),
                            (b_linear.blue * 1.5).min(1.0),
                        ),
                        emissive: Color::linear_rgb(0.6, 1.4, 2.2).into(), // High-tech cyan edge/emissive glow
                        perceptual_roughness: 0.25,
                        ..default()
                    });
                    (def, hov)
                })
                .clone();

            let half_height = building.height as f32 / 2.0;
            let center_x = (building.pos_x + building.width / 2.0) as f32;
            let center_y = platform_top + half_height;
            let center_z = (building.pos_z + building.depth / 2.0) as f32;
            let half_w = (building.width / 2.0) as f32;
            let half_d = (building.depth / 2.0) as f32;

            let aabb_min = Vec3::new(center_x - half_w, platform_top, center_z - half_d);
            let aabb_max = Vec3::new(center_x + half_w, platform_top + building.height as f32, center_z + half_d);

            let (b_data, d_data) = building_data_map.get(building.name.as_str()).copied().unwrap_or((&fallback_b, &fallback_d));

            let building_info = BuildingHoverInfo {
                name: building.name.clone(),
                source_file: b_data.source_file.clone(),
                district_name: d_data.name.clone(),
                typology: d_data.typology.clone(),
                num_fields: b_data.num_fields,
                num_methods: b_data.num_methods,
                lines_of_code: b_data.lines_of_code,
                commit_churn: b_data.commit_churn,
                last_modified: b_data.last_modified.clone(),
                summary: b_data.summary.clone(),
                fields: b_data.fields.clone(),
                methods: b_data.methods.clone(),
                dependencies: outgoing_deps.get(&building.name).cloned().unwrap_or_default(),
                callers: incoming_deps.get(&building.name).cloned().unwrap_or_default(),
            };

            let pickable = PickableBuilding {
                info: building_info,
                aabb_min,
                aabb_max,
                default_material: building_material.clone(),
                hover_material: hover_building_material,
            };

            commands.spawn((
                PbrBundle {
                    mesh: unit_cube_mesh.clone(),
                    material: building_material,
                    transform: Transform::from_xyz(center_x, center_y, center_z)
                        .with_scale(Vec3::new(
                            building.width as f32,
                            building.height as f32,
                            building.depth as f32,
                        )),
                    ..default()
                },
                pickable,
                CityElement,
            ));
        }
    }
}

fn generate_curved_road_mesh(points: &[(f64, f64)], width: f32, height: f32) -> Mesh {
    let n = points.len();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * 2);
    let mut indices: Vec<u32> = Vec::with_capacity((n - 1) * 6);

    let half_w = width * 0.5;
    let mut cum_dist = 0.0_f32;

    for i in 0..n {
        let (tx, tz) = if i == 0 {
            let dx = (points[1].0 - points[0].0) as f32;
            let dz = (points[1].1 - points[0].1) as f32;
            let len = (dx * dx + dz * dz).sqrt().max(1e-4);
            (dx / len, dz / len)
        } else if i == n - 1 {
            let dx = (points[n - 1].0 - points[n - 2].0) as f32;
            let dz = (points[n - 1].1 - points[n - 2].1) as f32;
            let len = (dx * dx + dz * dz).sqrt().max(1e-4);
            (dx / len, dz / len)
        } else {
            let dx = (points[i + 1].0 - points[i - 1].0) as f32;
            let dz = (points[i + 1].1 - points[i - 1].1) as f32;
            let len = (dx * dx + dz * dz).sqrt().max(1e-4);
            (dx / len, dz / len)
        };

        if i > 0 {
            let seg_dx = (points[i].0 - points[i - 1].0) as f32;
            let seg_dz = (points[i].1 - points[i - 1].1) as f32;
            cum_dist += (seg_dx * seg_dx + seg_dz * seg_dz).sqrt();
        }

        let nx = -tz * half_w;
        let nz = tx * half_w;

        let px = points[i].0 as f32;
        let pz = points[i].1 as f32;

        positions.push([px + nx, height, pz + nz]);
        positions.push([px - nx, height, pz - nz]);

        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);

        uvs.push([0.0, cum_dist * 0.1]);
        uvs.push([1.0, cum_dist * 0.1]);

        if i < n - 1 {
            let a = (i * 2) as u32;
            let b = a + 1;
            let c = a + 2;
            let d = a + 3;

            indices.push(a);
            indices.push(b);
            indices.push(d);

            indices.push(a);
            indices.push(d);
            indices.push(c);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn generate_polygon_prism_mesh(polygon: &[(f64, f64)], height: f32) -> Mesh {
    let k = polygon.len();
    let cx = polygon.iter().map(|p| p.0).sum::<f64>() / (k as f64);
    let cz = polygon.iter().map(|p| p.1).sum::<f64>() / (k as f64);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // 1. Top face
    let center_idx = positions.len() as u32;
    positions.push([cx as f32, height, cz as f32]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for p in polygon {
        positions.push([p.0 as f32, height, p.1 as f32]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([(p.0 - cx) as f32 * 0.05 + 0.5, (p.1 - cz) as f32 * 0.05 + 0.5]);
    }

    for i in 0..k {
        let curr = center_idx + 1 + (i as u32);
        let next = center_idx + 1 + (((i + 1) % k) as u32);
        indices.push(center_idx);
        indices.push(curr);
        indices.push(next);
    }

    // 2. Side skirts
    let ground_y = 0.02_f32;
    for i in 0..k {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % k];

        let dx = (p2.0 - p1.0) as f32;
        let dz = (p2.1 - p1.1) as f32;
        let edge_len = (dx * dx + dz * dz).sqrt().max(1e-4);
        let nx = dz / edge_len;
        let nz = -dx / edge_len;

        let base_idx = positions.len() as u32;
        positions.push([p1.0 as f32, height, p1.1 as f32]);
        positions.push([p2.0 as f32, height, p2.1 as f32]);
        positions.push([p1.0 as f32, ground_y, p1.1 as f32]);
        positions.push([p2.0 as f32, ground_y, p2.1 as f32]);

        for _ in 0..4 {
            normals.push([nx, 0.0, nz]);
        }
        uvs.push([0.0, 1.0]);
        uvs.push([1.0, 1.0]);
        uvs.push([0.0, 0.0]);
        uvs.push([1.0, 0.0]);

        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 1);

        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn generate_circular_ground_mesh(radius: f32, segments: usize) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(segments + 1);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(segments + 1);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(segments + 1);
    let mut indices: Vec<u32> = Vec::with_capacity(segments * 3);

    // Center vertex
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    let step = std::f32::consts::TAU / (segments as f32);
    for i in 0..segments {
        let angle = (i as f32) * step;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([x * 0.05 + 0.5, z * 0.05 + 0.5]);
    }

    for i in 0..segments {
        let curr = (i + 1) as u32;
        let next = ((i + 1) % segments + 1) as u32;
        indices.push(0);
        indices.push(curr);
        indices.push(next);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn generate_circular_fog_mesh(
    r_inner: f32,
    r_outer: f32,
    r_far: f32,
    sky_color: Color,
    segments: usize,
) -> Mesh {
    let sky_linear = sky_color.to_linear();
    let (sr, sg, sb) = (sky_linear.red, sky_linear.green, sky_linear.blue);

    let transition_rings = 16;
    let mut rings: Vec<(f32, f32)> = Vec::new();

    // Inner edge: alpha = 0.0 (completely clear over the city)
    rings.push((r_inner, 0.0));

    // Smooth transition from r_inner to r_outer
    for i in 1..=transition_rings {
        let t = (i as f32) / (transition_rings as f32);
        let r = r_inner + t * (r_outer - r_inner);
        // Smoothstep: 3t^2 - 2t^3
        let alpha = t * t * (3.0 - 2.0 * t);
        rings.push((r, alpha));
    }

    // Skirt rings extending to the horizon / far clipping plane
    rings.push((r_outer + 200.0, 1.0));
    rings.push((r_outer + 1000.0, 1.0));
    rings.push((r_outer + 5000.0, 1.0));
    rings.push((20000.0_f32.max(r_outer + 10000.0), 1.0));
    rings.push((r_far, 1.0));

    let num_rings = rings.len();
    let total_vertices = num_rings * segments;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_vertices);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_vertices);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_vertices);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_vertices);

    let step = std::f32::consts::TAU / (segments as f32);

    for &(r, alpha) in &rings {
        for s in 0..segments {
            let angle = (s as f32) * step;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let x = r * cos_a;
            let z = r * sin_a;

            positions.push([x, 0.0, z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.5 + 0.5 * cos_a * (r / r_far), 0.5 + 0.5 * sin_a * (r / r_far)]);
            colors.push([sr, sg, sb, alpha]);
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((num_rings - 1) * segments * 6);

    for k in 0..(num_rings - 1) {
        let row_curr = (k * segments) as u32;
        let row_next = ((k + 1) * segments) as u32;

        for s in 0..segments {
            let s_next = (s + 1) % segments;

            let v00 = row_curr + s as u32;
            let v01 = row_curr + s_next as u32;
            let v10 = row_next + s as u32;
            let v11 = row_next + s_next as u32;

            // Quad split into two CCW triangles viewed from +Y
            indices.push(v00);
            indices.push(v10);
            indices.push(v01);

            indices.push(v01);
            indices.push(v10);
            indices.push(v11);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn hover_picking_system(
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    buildings_query: Query<(Entity, &PickableBuilding)>,
    roads_query: Query<(Entity, &PickableRoad)>,
    districts_query: Query<(Entity, &PickableDistrict)>,
    mut mat_query: Query<&mut Handle<StandardMaterial>>,
    mut hover_state: ResMut<HoverState>,
) {
    let Some(window) = windows.iter().next() else { return; };
    let Some(cursor_pos) = window.cursor_position() else {
        if let Some(prev_e) = hover_state.hovered_entity.take() {
            revert_entity_material(prev_e, &buildings_query, &roads_query, &districts_query, &mut mat_query);
            dispatch_hover_event(&HoverPayload::None);
        }
        hover_state.last_cursor_pos = None;
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else { return; };

    let cam_translation = camera_transform.translation();
    let cam_rotation = camera_transform.to_scale_rotation_translation().1;

    let cursor_unchanged = hover_state.last_cursor_pos == Some(cursor_pos);
    let camera_unchanged = hover_state.last_camera_transform == Some((cam_translation, cam_rotation));

    if cursor_unchanged && camera_unchanged {
        return;
    }

    hover_state.last_cursor_pos = Some(cursor_pos);
    hover_state.last_camera_transform = Some((cam_translation, cam_rotation));

    let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else { return; };

    let ray_origin = ray.origin;
    let ray_dir = *ray.direction;

    let mut closest_t = f32::MAX;
    let mut best_building: Option<Entity> = None;
    let mut best_road: Option<Entity> = None;
    let mut best_district: Option<Entity> = None;

    // 1. Test buildings (Ray-AABB) - zero allocations during loop
    for (entity, building) in buildings_query.iter() {
        if let Some(t) = ray_aabb_intersection(ray_origin, ray_dir, building.aabb_min, building.aabb_max) {
            if t < closest_t {
                closest_t = t;
                best_building = Some(entity);
                best_road = None;
                best_district = None;
            }
        }
    }

    // 2. Test roads (horizontal plane y = 0.28)
    if ray_dir.y.abs() > 1e-4 {
        let t_road = (0.28 - ray_origin.y) / ray_dir.y;
        if t_road > 0.0 && t_road < closest_t {
            let hit_pt = ray_origin + ray_dir * t_road;
            let p = Vec2::new(hit_pt.x, hit_pt.z);
            for (entity, road) in roads_query.iter() {
                let threshold = (road.width * 0.5 + 1.2).max(1.5);
                let thresh_sq = threshold * threshold;
                let mut hit = false;
                for i in 0..(road.points.len().saturating_sub(1)) {
                    let a = Vec2::new(road.points[i].0 as f32, road.points[i].1 as f32);
                    let b = Vec2::new(road.points[i + 1].0 as f32, road.points[i + 1].1 as f32);
                    if dist_sq_to_segment(p, a, b) <= thresh_sq {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    // Small priority bias (-0.02) so roads render over district plane
                    let effective_t = t_road - 0.02;
                    if effective_t < closest_t {
                        closest_t = effective_t;
                        best_road = Some(entity);
                        best_building = None;
                        best_district = None;
                    }
                }
            }
        }
    }

    // 3. Test districts (horizontal plane y = 0.22)
    if ray_dir.y.abs() > 1e-4 {
        let t_plat = (0.22 - ray_origin.y) / ray_dir.y;
        if t_plat > 0.0 && t_plat < closest_t {
            let hit_pt = ray_origin + ray_dir * t_plat;
            for (entity, district) in districts_query.iter() {
                if is_point_in_polygon(hit_pt.x as f64, hit_pt.z as f64, &district.polygon) {
                    if t_plat < closest_t {
                        closest_t = t_plat;
                        best_district = Some(entity);
                        best_building = None;
                        best_road = None;
                    }
                }
            }
        }
    }

    // Check if hovered entity changed
    let winning_entity = best_building.or(best_road).or(best_district);
    if winning_entity != hover_state.hovered_entity {
        // Revert previous entity material
        if let Some(prev_e) = hover_state.hovered_entity {
            revert_entity_material(prev_e, &buildings_query, &roads_query, &districts_query, &mut mat_query);
        }

        // Apply new hover
        if let Some(e) = best_building {
            if let Ok((_, building)) = buildings_query.get(e) {
                if let Ok(mut mat) = mat_query.get_mut(e) {
                    *mat = building.hover_material.clone();
                }
                dispatch_hover_event(&HoverPayload::Building(building.info.clone()));
            }
            hover_state.hovered_entity = Some(e);
        } else if let Some(e) = best_road {
            if let Ok((_, road)) = roads_query.get(e) {
                if let Ok(mut mat) = mat_query.get_mut(e) {
                    *mat = road.hover_material.clone();
                }
                dispatch_hover_event(&HoverPayload::Road(road.info.clone()));
            }
            hover_state.hovered_entity = Some(e);
        } else if let Some(e) = best_district {
            if let Ok((_, district)) = districts_query.get(e) {
                if let Ok(mut mat) = mat_query.get_mut(e) {
                    *mat = district.hover_material.clone();
                }
                dispatch_hover_event(&HoverPayload::District(district.info.clone()));
            }
            hover_state.hovered_entity = Some(e);
        } else {
            hover_state.hovered_entity = None;
            dispatch_hover_event(&HoverPayload::None);
        }
    }
}

fn revert_entity_material(
    entity: Entity,
    buildings_query: &Query<(Entity, &PickableBuilding)>,
    roads_query: &Query<(Entity, &PickableRoad)>,
    districts_query: &Query<(Entity, &PickableDistrict)>,
    mat_query: &mut Query<&mut Handle<StandardMaterial>>,
) {
    if let Ok((_, building)) = buildings_query.get(entity) {
        if let Ok(mut mat) = mat_query.get_mut(entity) {
            *mat = building.default_material.clone();
        }
    } else if let Ok((_, road)) = roads_query.get(entity) {
        if let Ok(mut mat) = mat_query.get_mut(entity) {
            *mat = road.default_material.clone();
        }
    } else if let Ok((_, district)) = districts_query.get(entity) {
        if let Ok(mut mat) = mat_query.get_mut(entity) {
            *mat = district.default_material.clone();
        }
    }
}

fn ray_aabb_intersection(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_x = if dir.x.abs() > 1e-6 { 1.0 / dir.x } else { 1e6 * dir.x.signum() };
    let inv_y = if dir.y.abs() > 1e-6 { 1.0 / dir.y } else { 1e6 * dir.y.signum() };
    let inv_z = if dir.z.abs() > 1e-6 { 1.0 / dir.z } else { 1e6 * dir.z.signum() };

    let t1_x = (min.x - origin.x) * inv_x;
    let t2_x = (max.x - origin.x) * inv_x;
    let (tmin_x, tmax_x) = if t1_x < t2_x { (t1_x, t2_x) } else { (t2_x, t1_x) };

    let t1_y = (min.y - origin.y) * inv_y;
    let t2_y = (max.y - origin.y) * inv_y;
    let (tmin_y, tmax_y) = if t1_y < t2_y { (t1_y, t2_y) } else { (t2_y, t1_y) };

    let tmin = tmin_x.max(tmin_y);
    let tmax = tmax_x.min(tmax_y);

    if tmin > tmax {
        return None;
    }

    let t1_z = (min.z - origin.z) * inv_z;
    let t2_z = (max.z - origin.z) * inv_z;
    let (tmin_z, tmax_z) = if t1_z < t2_z { (t1_z, t2_z) } else { (t2_z, t1_z) };

    let tmin_final = tmin.max(tmin_z);
    let tmax_final = tmax.min(tmax_z);

    if tmin_final > tmax_final || tmax_final < 0.0 {
        return None;
    }

    Some(if tmin_final > 0.0 { tmin_final } else { tmax_final })
}

fn dist_sq_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let ab_len_sq = ab.length_squared();
    if ab_len_sq < 1e-6 {
        return ap.length_squared();
    }
    let t = (ap.dot(ab) / ab_len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length_squared()
}

fn is_point_in_polygon(px: f64, pz: f64, polygon: &[(f64, f64)]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, zi) = polygon[i];
        let (xj, zj) = polygon[j];
        let intersect = ((zi > pz) != (zj > pz))
            && (px < (xj - xi) * (pz - zi) / (zj - zi + 1e-12) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

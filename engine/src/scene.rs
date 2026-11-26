use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

use crate::api::take_pending_city;
use crate::appearance::Appearance;
use crate::camera::{CityCamera, CAMERA_FOV};
use crate::events::*;
use crate::layout::compute_layout;
use crate::picking::Hovered;
use crate::render::FacadeMaterial;
use crate::spawn::spawn_city;
use crate::{CityElement, CityIndex};

pub(crate) const SKY_COLOR: Color = Color::srgb(0.62, 0.71, 0.80);
// Brighter than the sky because it gets tonemapped and the sky does not; the
// two must match where the ground fades out, or the horizon shows a seam.
const FOG_COLOR: Color = Color::srgb(0.72, 0.85, 1.0);

pub(crate) fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: CAMERA_FOV,
            near: 0.5,
            far: 60000.0,
            ..default()
        }),
        Transform::from_xyz(60.0, 70.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        // The distances follow the city; see `city_fog`.
        city_fog(200.0),
        CityCamera::default(),
    ));

    // Without cast shadows the height-based massing reads flat.
    commands.spawn((
        DirectionalLight {
            illuminance: 11000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.87),
            ..default()
        },
        // Low afternoon sun: long shadows fall across the streets.
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.6, 0.0)),
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 1600.0,
            first_cascade_far_bound: 220.0,
            ..default()
        }
        .build(),
    ));
}

fn home_distance(radius: f32) -> f32 {
    radius * 2.35 + 60.0
}

/// Starts past the city's near half and hides the ground's edge before the camera reaches it.
fn city_fog(radius: f32) -> DistanceFog {
    let home = home_distance(radius);
    DistanceFog {
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
pub(crate) fn process_city_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut facades: ResMut<Assets<FacadeMaterial>>,
    mut index: ResMut<CityIndex>,
    mut hovered: ResMut<Hovered>,
    mut look: ResMut<Appearance>,
    mut camera_q: Query<(&mut Transform, &mut CityCamera, &mut DistanceFog)>,
    existing: Query<Entity, With<CityElement>>,
) {
    let Some(city) = take_pending_city() else {
        return;
    };

    // Before the index resets: the refinement pass re-sends the same repo,
    // and dropping the user back to the overview would undo their work.
    let had_city = index.radius > 0.0;

    for e in existing.iter() {
        commands.entity(e).despawn();
    }
    *hovered = Hovered::default();
    *index = CityIndex::default();
    dispatch_hover(None);
    dispatch_select(None);

    let result = compute_layout(&city);
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

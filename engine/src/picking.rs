//! Hover and click: the nearest box under the cursor, a building or a
//! district's ground slab.

use bevy::math::bounding::{Aabb3d, RayCast3d};
use bevy::prelude::*;

use crate::camera::CityCamera;
use crate::events::*;

/// A box the cursor can hit, and what it stands for.
#[derive(Component)]
pub struct Pickable {
    pub target: Target,
    pub min: Vec3,
    pub max: Vec3,
}

/// What the cursor was over last frame, so hover is only reported on change.
#[derive(Resource, Default)]
pub struct Hovered(pub Option<Target>);

pub(crate) fn picking_system(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    mut city_cam: Query<&mut CityCamera>,
    pickables: Query<&Pickable>,
    mut hovered: ResMut<Hovered>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        dispatch_select(None);
    }
    let (Ok(window), Ok((camera, cam_tf))) = (windows.get_single(), camera_q.get_single()) else {
        return;
    };

    let hit = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(cam_tf, cursor))
        .and_then(|ray| nearest(ray, &pickables));
    let target = hit.map(|p| p.target);

    if target != hovered.0 {
        hovered.0 = target;
        dispatch_hover(target);
    }

    if mouse.just_pressed(MouseButton::Left) {
        if let (Some(p), Ok(mut cam)) = (hit, city_cam.get_single_mut()) {
            frame(&mut cam, p);
        }
        dispatch_select(target);
    }
}

/// The first box the ray enters. Buildings stand on their district's slab, so
/// a building under the cursor is always nearer than the ground beneath it.
fn nearest<'a>(ray: Ray3d, pickables: &'a Query<&Pickable>) -> Option<&'a Pickable> {
    let cast = RayCast3d::from_ray(ray, f32::MAX);
    pickables
        .iter()
        .filter_map(|p| {
            let aabb = Aabb3d {
                min: p.min.into(),
                max: p.max.into(),
            };
            cast.aabb_intersection_at(&aabb).map(|t| (t, p))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

/// Moves the camera so the box fills a comfortable part of the view.
pub(crate) fn frame(cam: &mut CityCamera, p: &Pickable) {
    cam.frame_object((p.min + p.max) * 0.5, (p.max - p.min).length().max(4.0));
}

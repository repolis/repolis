//! Hover and click: a ray from the cursor against building boxes, then the
//! district ground.

use bevy::math::bounding::{Aabb3d, RayCast3d};
use bevy::prelude::*;

use crate::camera::CityCamera;
use crate::events::*;
use crate::{CityIndex, DISTRICT_TOP};

#[allow(clippy::too_many_arguments)]
pub(crate) fn picking_system(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<CityCamera>>,
    mut cam_focus_q: Query<&mut CityCamera>,
    buildings: Query<(Entity, &Pickable)>,
    districts: Query<(Entity, &PickableDistrict)>,
    mut interaction: ResMut<SelectionState>,
    index: Res<CityIndex>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        dispatch_select(&SelectPayload::None);
    }

    let Some(window) = windows.iter().next() else {
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.get_single() else {
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        if interaction.hovered.take().is_some() {
            dispatch_hover(&None);
        }
        return;
    };

    let cam_pos = cam_tf.translation();
    let cam_rot = cam_tf.to_scale_rotation_translation().1;
    let clicked = mouse.just_pressed(MouseButton::Left);

    // Only when something moved, unless this is a click.
    let unchanged = interaction.last_cursor == Some(cursor)
        && interaction.last_camera == Some((cam_pos, cam_rot));
    if unchanged && !clicked {
        return;
    }
    interaction.last_cursor = Some(cursor);
    interaction.last_camera = Some((cam_pos, cam_rot));

    let Some(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let cast = RayCast3d::from_ray(ray, f32::MAX);
    let best_building = buildings
        .iter()
        .filter_map(|(e, p)| {
            let aabb = Aabb3d {
                min: p.min.into(),
                max: p.max.into(),
            };
            cast.aabb_intersection_at(&aabb).map(|t| (t, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e)| e);

    let ground = InfinitePlane3d::new(Vec3::Y);
    let best_district = best_building
        .is_none()
        .then(|| ray.intersect_plane(Vec3::Y * DISTRICT_TOP, ground))
        .flatten()
        .and_then(|t| {
            let hit = ray.get_point(t).xz();
            districts
                .iter()
                .find(|(_, d)| hit.cmpge(d.min).all() && hit.cmple(d.max).all())
        })
        .map(|(e, _)| e);

    // Hover: lightweight tooltip only.
    let hovered = best_building.or(best_district);
    if hovered != interaction.hovered {
        interaction.hovered = hovered;
        let payload = if let Some(p) = hovered.and_then(|e| buildings.get(e).ok()) {
            Some(Hover {
                kind: "building",
                id: index.building(p.1).id.clone(),
            })
        } else if let Some(d) = hovered.and_then(|e| districts.get(e).ok()) {
            let id = index.city.districts[d.1.district_idx].id.clone();
            Some(Hover {
                kind: "district",
                id,
            })
        } else {
            None
        };
        dispatch_hover(&payload);
    }

    if !clicked {
        return;
    }

    match hovered {
        Some(e) if buildings.get(e).is_ok() => {
            let (_, p) = buildings.get(e).unwrap();
            if let Ok(mut cam) = cam_focus_q.get_single_mut() {
                frame_building(&mut cam, p);
            }

            dispatch_select(&SelectPayload::Building(index.building(p).id.clone()));
        }
        Some(e) if districts.get(e).is_ok() => {
            let (_, d) = districts.get(e).unwrap();
            if let Ok(mut cam) = cam_focus_q.get_single_mut() {
                let center = (d.min + d.max) * 0.5;
                let span = (d.max - d.min).length().max(20.0);
                cam.frame_object(Vec3::new(center.x, 0.0, center.y), span);
            }
            let id = index.city.districts[d.district_idx].id.clone();
            dispatch_select(&SelectPayload::District(id));
        }
        _ => dispatch_select(&SelectPayload::None),
    }
}

pub(crate) fn frame_building(cam: &mut CityCamera, p: &Pickable) {
    let size = (p.max - p.min).length().max(4.0);
    cam.frame_object((p.min + p.max) * 0.5, size);
}

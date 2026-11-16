//! Turns a layout into entities: ground, streets, districts, buildings, arcs.

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::appearance::material_set;
use crate::events::Target;
use crate::layout::{LayoutResult, Link};
use crate::picking::Pickable;
use crate::render::*;
use crate::{CityElement, CityIndex, HighlightLinkLayer, LinkLayer};
use crate::{BUILDING_BASE, DISTRICT_TOP, GROUND_Y, STREET_Y};

/// The box over a ground rectangle between two heights, as (min, max).
fn slab(r: Rect, y0: f32, y1: f32) -> (Vec3, Vec3) {
    (
        Vec3::new(r.min.x, y0, r.min.y),
        Vec3::new(r.max.x, y1, r.max.y),
    )
}

/// Places and scales the shared unit cube to fill a box.
fn box_transform((min, max): (Vec3, Vec3)) -> Transform {
    Transform::from_translation((min + max) * 0.5).with_scale(max - min)
}

pub(crate) fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    index: &mut CityIndex,
    result: LayoutResult,
) {
    let ground_radius = result.radius * 1.9 + 160.0;
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Circle::new(ground_radius).mesh().resolution(96)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.13, 0.14, 0.15),
                perceptual_roughness: 1.0,
                ..default()
            }),
            transform: Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ..default()
        },
        CityElement,
    ));

    // One shared cube mesh and a few shared materials, so Bevy instances
    // them into a handful of draw calls.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let street_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.21, 0.23),
        perceptual_roughness: 0.98,
        ..default()
    });
    for &street in &result.streets {
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: street_material.clone(),
                transform: box_transform(slab(street, DISTRICT_TOP, STREET_Y)),
                ..default()
            },
            CityElement,
        ));
    }

    for (i, &ground) in result.districts.iter().enumerate() {
        let (min, max) = slab(ground, GROUND_Y + 0.02, DISTRICT_TOP);
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                // Its own material: the ground colour is set in place.
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.21),
                    perceptual_roughness: 0.95,
                    ..default()
                }),
                transform: box_transform((min, max)),
                ..default()
            },
            Pickable {
                target: Target::District(i),
                min,
                max,
            },
            CityElement,
        ));
        let c = ground.center();
        index.labels.push(Vec3::new(c.x, BUILDING_BASE + 6.0, c.y));
    }

    // Neutral until the frontend's first appearance arrives, a frame later.
    let neutral = material_set(materials, &mut index.materials, [0.62, 0.62, 0.64]).normal;
    for b in &result.buildings {
        let base = Vec3::new(b.center.x, BUILDING_BASE, b.center.y);
        let half = Vec3::new(b.size.x, 0.0, b.size.z) * 0.5;
        let (min, max) = (base - half, base + half + Vec3::Y * b.size.y);
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: neutral.clone(),
                transform: box_transform((min, max)),
                ..default()
            },
            Pickable {
                target: Target::Building(b.order),
                min,
                max,
            },
            CityElement,
        ));
    }

    // Every dependency at once is unreadable, so all arcs start hidden behind
    // one toggle; a selection reveals its own neighbourhood regardless.
    let links = result.links;
    let all: Vec<&Link> = links.iter().collect();
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&all, 0.0)),
            visibility: Visibility::Hidden,
            material: link_material(
                materials,
                Color::srgba(0.45, 0.78, 0.95, 0.30),
                LinearRgba::new(0.10, 0.35, 0.55, 1.0),
            ),
            ..default()
        },
        LinkLayer,
        CityElement,
    ));

    // Cycles in red on top: they are the one thing here a file tree cannot
    // show at all.
    let cycle_links: Vec<&Link> = links.iter().filter(|l| l.in_cycle).collect();
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&cycle_links, 0.6)),
            visibility: Visibility::Hidden,
            material: link_material(
                materials,
                Color::srgb(1.0, 0.30, 0.34),
                LinearRgba::new(3.4, 0.35, 0.45, 1.0),
            ),
            ..default()
        },
        LinkLayer,
        CityElement,
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&[], 0.0)),
            material: link_material(
                materials,
                Color::srgb(1.0, 0.82, 0.25),
                LinearRgba::new(3.0, 2.0, 0.3, 1.0),
            ),
            ..default()
        },
        HighlightLinkLayer,
        CityElement,
    ));

    index.links = links;
}

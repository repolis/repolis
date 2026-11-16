//! Turns a layout into entities: ground, streets, districts, buildings, arcs.

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::appearance::material_set;
use crate::data::CityMap;
use crate::events::*;
use crate::layout::{self, LayoutResult};
use crate::render::*;
use crate::{treemap, CityElement, CityIndex, HighlightLinkLayer, LinkLayer};
use crate::{BUILDING_BASE, DISTRICT_TOP, GROUND_Y, STREET_Y};

pub(crate) fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    index: &mut CityIndex,
    result: LayoutResult,
    city: &CityMap,
) {
    let ground_radius = result.radius as f32 * 1.9 + 160.0;
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
    let slab = |rect: &treemap::Rect, y0: f32, y1: f32| {
        let (x, z) = rect.center();
        Transform::from_xyz(x as f32, (y0 + y1) * 0.5, z as f32).with_scale(Vec3::new(
            rect.w as f32,
            y1 - y0,
            rect.d as f32,
        ))
    };

    let street_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.21, 0.23),
        perceptual_roughness: 0.98,
        ..default()
    });
    for street in &result.streets {
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: street_material.clone(),
                transform: slab(street, DISTRICT_TOP, STREET_Y),
                ..default()
            },
            CityElement,
        ));
    }

    // Neutral until the frontend's first appearance arrives, a frame later.
    let neutral = material_set(materials, &mut index.materials, [0.62, 0.62, 0.64]).normal;

    for (i, &g) in result.districts.iter().enumerate() {
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                // Its own material: the ground colour is set in place.
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.21),
                    perceptual_roughness: 0.95,
                    ..default()
                }),
                transform: slab(&g, GROUND_Y + 0.02, DISTRICT_TOP),
                ..default()
            },
            CityElement,
            PickableDistrict {
                min: Vec2::new(g.x as f32, g.z as f32),
                max: Vec2::new((g.x + g.w) as f32, (g.z + g.d) as f32),
                district_idx: i,
            },
        ));

        let (x, z) = g.center();
        index.labels.push(Vec3::new(x as f32, BUILDING_BASE + 6.0, z as f32));
    }

    // Each district's first building in city order.
    let offsets: Vec<usize> = city
        .districts
        .iter()
        .scan(0, |n, d| {
            let start = *n;
            *n += d.buildings.len();
            Some(start)
        })
        .collect();

    for pb in &result.buildings {
        let b = &city.districts[pb.district_idx].buildings[pb.building_idx];

        let half = Vec3::new(pb.width as f32, 0.0, pb.depth as f32) * 0.5;
        let base = Vec3::new(pb.center_x as f32, BUILDING_BASE, pb.center_z as f32);
        let (min, max) = (base - half, base + half + Vec3::Y * pb.height as f32);
        let entity = commands
            .spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: neutral.clone(),
                    transform: Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                    ..default()
                },
                Pickable {
                    district_idx: pb.district_idx,
                    building_idx: pb.building_idx,
                    order: offsets[pb.district_idx] + pb.building_idx,
                    min,
                    max,
                },
                CityElement,
            ))
            .id();
        index.by_id.insert(b.id.clone(), entity);
    }

    // Every dependency at once is unreadable, so all arcs start hidden behind
    // one toggle; a selection reveals its own neighbourhood regardless.
    let links = result.links;
    let all: Vec<&layout::Link> = links.iter().collect();
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
    let cycle_links: Vec<&layout::Link> = links.iter().filter(|l| l.in_cycle).collect();
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

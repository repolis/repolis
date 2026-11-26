use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::appearance::material_set;
use crate::events::Target;
use crate::layout::{LayoutResult, Link};
use crate::picking::Pickable;
use crate::render::*;
use crate::{CityElement, CityIndex, HighlightLinkLayer, LinkLayer};
use crate::{BUILDING_BASE, DISTRICT_TOP, GROUND_Y, PAVEMENT_Y};

struct Slab<'a> {
    rect: Rect,
    bottom: f32,
    top: f32,
    material: &'a Handle<StandardMaterial>,
}

fn slab_corners(r: Rect, y0: f32, y1: f32) -> (Vec3, Vec3) {
    (
        Vec3::new(r.min.x, y0, r.min.y),
        Vec3::new(r.max.x, y1, r.max.y),
    )
}

fn box_transform((min, max): (Vec3, Vec3)) -> Transform {
    Transform::from_translation((min + max) * 0.5).with_scale(max - min)
}

pub(crate) fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    facades: &mut Assets<FacadeMaterial>,
    index: &mut CityIndex,
    result: LayoutResult,
) {
    // Far past the city, so the fog swallows its edge.
    let ground_radius = result.radius * 8.0 + 400.0;
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(ground_radius).mesh().resolution(96))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.065, 0.075),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        CityElement,
    ));

    // Shared cube and materials so Bevy batches them into a few draw calls.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let pavement = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.36, 0.37),
        perceptual_roughness: 0.98,
        ..default()
    });
    let paint = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.74, 0.62),
        perceptual_roughness: 1.0,
        ..default()
    });
    // Lines start at the ground so one height serves avenues and streets.
    let blocks = result.blocks.iter().map(|&rect| Slab {
        rect,
        bottom: DISTRICT_TOP,
        top: PAVEMENT_Y,
        material: &pavement,
    });
    let lane_lines = result.lane_lines.iter().map(|&rect| Slab {
        rect,
        bottom: GROUND_Y,
        top: DISTRICT_TOP + 0.02,
        material: &paint,
    });
    for slab in blocks.chain(lane_lines) {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(slab.material.clone()),
            box_transform(slab_corners(slab.rect, slab.bottom, slab.top)),
            CityElement,
        ));
    }

    for (i, &ground) in result.districts.iter().enumerate() {
        let (min, max) = slab_corners(ground, GROUND_Y + 0.02, DISTRICT_TOP);
        commands.spawn((
            Mesh3d(cube.clone()),
            // Own material: the ground colour is set in place.
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.2, 0.21),
                perceptual_roughness: 0.95,
                ..default()
            })),
            box_transform((min, max)),
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
    let neutral = material_set(facades, &mut index.materials, [0.62, 0.62, 0.64]).normal;
    let building = meshes.add(building_mesh());
    for b in &result.buildings {
        let base = Vec3::new(b.center.x, BUILDING_BASE, b.center.y);
        let half = Vec3::new(b.size.x, 0.0, b.size.z) * 0.5;
        let (min, max) = (base - half, base + half + Vec3::Y * b.size.y);
        commands.spawn((
            Mesh3d(building.clone()),
            MeshMaterial3d(neutral.clone()),
            box_transform((min, max)),
            Pickable {
                target: Target::Building(b.order),
                min,
                max,
            },
            CityElement,
        ));
    }

    // All arcs start hidden: every dependency at once is unreadable.
    let links = result.links;
    let all: Vec<&Link> = links.iter().collect();
    commands.spawn((
        Mesh3d(meshes.add(build_link_mesh(&all, 0.0, &ALL_LINKS))),
        MeshMaterial3d(link_material(materials, 0.30)),
        Visibility::Hidden,
        LinkLayer,
        CityElement,
    ));

    // Cycles in red on top: the one thing a file tree cannot show.
    let cycle_links: Vec<&Link> = links.iter().filter(|l| l.in_cycle).collect();
    commands.spawn((
        Mesh3d(meshes.add(build_link_mesh(&cycle_links, 0.6, &CYCLE_LINKS))),
        MeshMaterial3d(link_material(materials, 1.0)),
        Visibility::Hidden,
        LinkLayer,
        CityElement,
    ));

    for cycles in [false, true] {
        commands.spawn((
            Mesh3d(meshes.add(build_link_mesh(&[], 0.0, &HIGHLIGHT_LINKS))),
            MeshMaterial3d(link_material(materials, 1.0)),
            HighlightLinkLayer { cycles },
            CityElement,
        ));
    }

    index.links = links;
}

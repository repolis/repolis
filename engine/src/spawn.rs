//! Turns a layout into entities: ground, streets, districts, buildings, arcs.

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::appearance::material_set;
use crate::data::CityMap;
use crate::events::*;
use crate::layout::{self, LayoutResult};
use crate::render::*;
use crate::view::{self, ColorMode, Scales};
use crate::{treemap, CityElement, CityIndex, HighlightLinkLayer, LinkLayer};
use crate::{BUILDING_BASE, DISTRICT_TOP, GROUND_Y, STREET_Y};

/// Replaces characters Bevy's bundled subset font cannot draw. District names
/// come from a language model, so they carry dashes, quotes and accents.
fn ascii_only(s: &str) -> String {
    s.replace('\u{2026}', "...")
        .chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{00b7}' | '\u{2022}' => '-',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

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

    for (i, d) in result.districts.iter().enumerate() {
        let district = &city.districts[i];
        let rgb = district_ground_rgb(&district.typology);
        let g = d.ground;
        commands.spawn((
            PbrBundle {
                mesh: cube.clone(),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
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

        // One world-space label per district: without text, a district can
        // only be identified by hovering it.
        let (x, z) = g.center();
        commands.spawn((
            TextBundle::from_section(
                ascii_only(&format!("{} ({})", district.name, d.building_count)),
                TextStyle {
                    font_size: 15.0,
                    color: Color::srgb(1.0, 0.99, 0.94),
                    ..default()
                },
            )
            .with_style(Style {
                position_type: PositionType::Absolute,
                ..default()
            }),
            DistrictLabel {
                world: Vec3::new(x as f32, BUILDING_BASE + 6.0, z as f32),
            },
            CityElement,
        ));
    }

    for pb in &result.buildings {
        let district = &city.districts[pb.district_idx];
        let b = &district.buildings[pb.building_idx];
        let rgb = view::color_for(
            ColorMode::Typology,
            b,
            &district.typology,
            Scales::default(),
        );
        let handles = material_set(materials, &mut index.materials, rgb);

        let half = Vec3::new(pb.width as f32, 0.0, pb.depth as f32) * 0.5;
        let base = Vec3::new(pb.center_x as f32, BUILDING_BASE, pb.center_z as f32);
        let (min, max) = (base - half, base + half + Vec3::Y * pb.height as f32);
        let entity = commands
            .spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: handles.normal,
                    transform: Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                    ..default()
                },
                Pickable {
                    district_idx: pb.district_idx,
                    building_idx: pb.building_idx,
                    min,
                    max,
                },
                CityElement,
            ))
            .id();
        index.by_id.insert(b.id.clone(), entity);
    }

    // One merged mesh for the overview, one reserved for the selection.
    let links = result.links;

    // Every dependency at once is unreadable: at rest show only the heaviest
    // cross-district links, and reveal a neighbourhood on selection.
    let mut overview: Vec<&layout::Link> = links.iter().filter(|l| l.inter_district).collect();
    overview.sort_by_key(|l| std::cmp::Reverse(l.weight));
    overview.truncate(90);

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&overview, 0.0)),
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

    // Cycles are drawn permanently: they are rare, and they are the one thing
    // here a file tree cannot show at all.
    let cycle_links: Vec<&layout::Link> = links.iter().filter(|l| l.in_cycle).collect();
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(build_link_mesh(&cycle_links, 0.6)),
            material: link_material(
                materials,
                Color::srgb(1.0, 0.30, 0.34),
                LinearRgba::new(3.4, 0.35, 0.45, 1.0),
            ),
            ..default()
        },
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

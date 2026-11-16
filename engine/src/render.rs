//! Mesh construction and the visual encoding.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::layout::Link;

/// Hue carries the district's typology. The channels the city uses are
/// documented in the on-screen legend: height = methods, footprint = fields,
/// hue = typology, saturation = age.
pub fn typology_rgb(typology: &str) -> [f32; 3] {
    match typology {
        "core" => [0.46, 0.55, 0.72],
        "data" => [0.30, 0.62, 0.42],
        "network" => [0.28, 0.52, 0.78],
        "security" => [0.78, 0.35, 0.33],
        "interface" => [0.85, 0.66, 0.30],
        "utility" => [0.68, 0.62, 0.50],
        "config" => [0.52, 0.54, 0.62],
        "test" => [0.58, 0.40, 0.74],
        "example" => [0.38, 0.68, 0.72],
        _ => [0.62, 0.62, 0.64],
    }
}

pub fn district_ground_rgb(typology: &str) -> [f32; 3] {
    let c = typology_rgb(typology);
    [c[0] * 0.30 + 0.05, c[1] * 0.30 + 0.05, c[2] * 0.30 + 0.06]
}

/// Age buckets, in days. Five distinguishable shades compare better across a
/// city than a smooth ramp, and keep materials bounded: a continuous ramp
/// produced 131 of them on libgit2.
const AGE_BUCKETS: [u32; 4] = [45, 180, 400, 900];

pub fn age_bucket(age_days: u32) -> u32 {
    let mut b = 0;
    for &edge in AGE_BUCKETS.iter() {
        if age_days >= edge {
            b += 1;
        }
    }
    b
}

/// Age desaturates toward concrete grey: code untouched for years is fully
/// weathered, recent code keeps its district's hue.
pub fn apply_age(rgb: [f32; 3], age_days: u32) -> [f32; 3] {
    let t = age_bucket(age_days) as f32 / AGE_BUCKETS.len() as f32;
    let grey = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
    let mix = 0.55 * t;
    [
        rgb[0] * (1.0 - mix) + grey * mix,
        rgb[1] * (1.0 - mix) + grey * mix,
        rgb[2] * (1.0 - mix) + grey * mix,
    ]
}

pub struct BuildingMaterials {
    pub normal: Handle<StandardMaterial>,
    pub dim: Handle<StandardMaterial>,
    pub bright: Handle<StandardMaterial>,
}

pub fn make_building_materials(
    materials: &mut Assets<StandardMaterial>,
    rgb: [f32; 3],
) -> BuildingMaterials {
    let normal = materials.add(StandardMaterial {
        base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        perceptual_roughness: 0.78,
        metallic: 0.02,
        ..default()
    });
    let dim = materials.add(StandardMaterial {
        base_color: Color::srgb(rgb[0] * 0.30, rgb[1] * 0.30, rgb[2] * 0.32),
        perceptual_roughness: 0.95,
        ..default()
    });
    let bright = materials.add(StandardMaterial {
        base_color: Color::srgb(
            (rgb[0] * 1.35).min(1.0),
            (rgb[1] * 1.35).min(1.0),
            (rgb[2] * 1.35).min(1.0),
        ),
        emissive: LinearRgba::new(0.55, 1.15, 1.7, 1.0),
        perceptual_roughness: 0.3,
        ..default()
    });
    BuildingMaterials { normal, dim, bright }
}

/// Builds one mesh containing every arc, so the whole dependency graph is a
/// single draw call rather than one entity per link. The highlighted subset is
/// rebuilt separately into a second, small mesh.
pub fn build_link_mesh(links: &[&Link], vertical: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for link in links {
        let pts = &link.points;
        if pts.len() < 2 {
            continue;
        }
        let half = (link.width as f32) * 0.5;
        let base = positions.len() as u32;

        for i in 0..pts.len() {
            let (px, py, pz) = pts[i];
            let (nx, nz) = tangent_normal(pts, i);
            positions.push([
                px as f32 + nx * half,
                py as f32 + vertical,
                pz as f32 + nz * half,
            ]);
            positions.push([
                px as f32 - nx * half,
                py as f32 + vertical,
                pz as f32 - nz * half,
            ]);
            normals.push([0.0, 1.0, 0.0]);
            normals.push([0.0, 1.0, 0.0]);
            let t = i as f32 / (pts.len() - 1) as f32;
            uvs.push([0.0, t]);
            uvs.push([1.0, t]);

            if i + 1 < pts.len() {
                let a = base + (i as u32) * 2;
                indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn tangent_normal(pts: &[(f64, f64, f64)], i: usize) -> (f32, f32) {
    let (ax, _, az) = pts[i.saturating_sub(1)];
    let (bx, _, bz) = pts[(i + 1).min(pts.len() - 1)];
    let dx = (bx - ax) as f32;
    let dz = (bz - az) as f32;
    let len = (dx * dx + dz * dz).sqrt().max(1e-4);
    (-dz / len, dx / len)
}

//! Mesh construction and the visual encoding.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::layout::Link;

/// Normal, dimmed and highlighted variants of one quantised building colour.
#[derive(Clone)]
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

/// Unlit and translucent, so arcs read as overlays rather than structure.
pub fn link_material(
    materials: &mut Assets<StandardMaterial>,
    base_color: Color,
    emissive: LinearRgba,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color,
        emissive,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
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

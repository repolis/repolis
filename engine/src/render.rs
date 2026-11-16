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
    BuildingMaterials {
        normal,
        dim,
        bright,
    }
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
        let base = positions.len() as u32;
        for (i, &p) in pts.iter().enumerate() {
            // Half the ribbon's width, sideways to the arc on the ground plane.
            let along = pts[(i + 1).min(pts.len() - 1)] - pts[i.saturating_sub(1)];
            let side = along.xz().normalize_or_zero().perp() * (link.width * 0.5);
            let side = Vec3::new(side.x, 0.0, side.y);
            let p = p + Vec3::Y * vertical;
            positions.extend([(p + side).to_array(), (p - side).to_array()]);
            normals.extend([[0.0, 1.0, 0.0]; 2]);
            let t = i as f32 / (pts.len() - 1) as f32;
            uvs.extend([[0.0, t], [1.0, t]]);
            if i + 1 < pts.len() {
                let a = base + (i as u32) * 2;
                indices.extend([a, a + 1, a + 3, a, a + 3, a + 2]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

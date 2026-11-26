use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::layout::{Direction, Link};

pub const FACADE_SHADER: Handle<Shader> =
    Handle::weak_from_u128(0x5e1f_a3ad_e000_4b1d_9c3a_7f02_b11d_0001);

/// Floor height and window column width, in world units.
const FACADE_GRID: Vec4 = Vec4::new(1.3, 1.5, 0.0, 0.0);

/// Draws floors and windows on walls in the shader, so the whole city stays one cube mesh.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Facade {
    #[uniform(100)]
    grid: Vec4,
}

impl MaterialExtension for Facade {
    fn fragment_shader() -> ShaderRef {
        FACADE_SHADER.into()
    }
}

pub type FacadeMaterial = ExtendedMaterial<StandardMaterial, Facade>;

pub fn facade(base: StandardMaterial) -> FacadeMaterial {
    ExtendedMaterial {
        base,
        extension: Facade { grid: FACADE_GRID },
    }
}

/// Unit cube whose walls all run u across and v bottom to top, which the
/// facade shader relies on; Bevy's cuboid orients each face differently.
pub fn building_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut face = |normal: Vec3, right: Vec3, up: Vec3| {
        let base = positions.len() as u32;
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let p = normal * 0.5 + right * (u - 0.5) + up * (v - 0.5);
            positions.push(p.to_array());
            normals.push(normal.to_array());
            uvs.push([u, v]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    face(Vec3::Z, Vec3::X, Vec3::Y);
    face(Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y);
    face(Vec3::X, Vec3::NEG_Z, Vec3::Y);
    face(Vec3::NEG_X, Vec3::Z, Vec3::Y);
    face(Vec3::Y, Vec3::X, Vec3::NEG_Z);
    face(Vec3::NEG_Y, Vec3::X, Vec3::Z);

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

#[derive(Clone)]
pub struct BuildingMaterials {
    pub normal: Handle<FacadeMaterial>,
    pub dim: Handle<FacadeMaterial>,
    pub bright: Handle<FacadeMaterial>,
}

pub fn make_building_materials(
    materials: &mut Assets<FacadeMaterial>,
    rgb: [f32; 3],
) -> BuildingMaterials {
    let normal = materials.add(facade(StandardMaterial {
        base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        perceptual_roughness: 0.78,
        metallic: 0.02,
        ..default()
    }));
    let dim = materials.add(facade(StandardMaterial {
        base_color: Color::srgb(rgb[0] * 0.30, rgb[1] * 0.30, rgb[2] * 0.32),
        perceptual_roughness: 0.95,
        ..default()
    }));
    let bright = materials.add(facade(StandardMaterial {
        base_color: Color::srgb(
            (rgb[0] * 1.35).min(1.0),
            (rgb[1] * 1.35).min(1.0),
            (rgb[2] * 1.35).min(1.0),
        ),
        emissive: LinearRgba::new(0.55, 1.15, 1.7, 1.0),
        perceptual_roughness: 0.3,
        ..default()
    }));
    BuildingMaterials {
        normal,
        dim,
        bright,
    }
}

pub struct LinkColors(pub Color, pub Color);

pub const ALL_LINKS: LinkColors =
    LinkColors(Color::srgb(0.45, 0.78, 0.95), Color::srgb(0.12, 0.30, 0.80));
pub const CYCLE_LINKS: LinkColors =
    LinkColors(Color::srgb(1.0, 0.55, 0.58), Color::srgb(0.85, 0.08, 0.14));
pub const HIGHLIGHT_LINKS: LinkColors =
    LinkColors(Color::srgb(1.0, 0.90, 0.45), Color::srgb(0.95, 0.32, 0.08));

pub fn link_material(
    materials: &mut Assets<StandardMaterial>,
    alpha: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE.with_alpha(alpha),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

const TAPER_AT_CALLEE: f32 = 0.8;

fn tangent_at(points: &[Vec3], i: usize) -> Vec3 {
    points[(i + 1).min(points.len() - 1)] - points[i.saturating_sub(1)]
}

fn toward_callee(direction: Direction, t: f32) -> f32 {
    match direction {
        Direction::Forward => t,
        Direction::Backward => 1.0 - t,
        Direction::Both => 0.5,
    }
}

pub fn build_link_mesh(links: &[&Link], vertical: f32, colors: &LinkColors) -> Mesh {
    let (from, to) = (LinearRgba::from(colors.0), LinearRgba::from(colors.1));
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut vertex_colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for link in links {
        let pts = &link.points;
        if pts.len() < 2 {
            continue;
        }
        let base = positions.len() as u32;
        for (i, &p) in pts.iter().enumerate() {
            let t = i as f32 / (pts.len() - 1) as f32;
            let d = toward_callee(link.direction, t);
            let taper = if link.direction == Direction::Both {
                1.0
            } else {
                1.0 - TAPER_AT_CALLEE * d
            };
            let half = link.width * 0.5 * taper;
            let side = tangent_at(pts, i).xz().normalize_or_zero().perp() * half;
            let side = Vec3::new(side.x, 0.0, side.y);
            let p = p + Vec3::Y * vertical;
            positions.extend([(p + side).to_array(), (p - side).to_array()]);
            normals.extend([[0.0, 1.0, 0.0]; 2]);
            uvs.extend([[0.0, t], [1.0, t]]);
            vertex_colors.extend([from.mix(&to, d).to_f32_array(); 2]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

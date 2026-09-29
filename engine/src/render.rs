//! Mesh construction and the visual encoding.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::layout::Link;

/// Hue carries the district's typology. The five channels the city uses are
/// documented in the on-screen legend: height = methods, footprint = fields,
/// hue = typology, saturation = age, roof glow = churn.
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

/// Quantised, so identical appearances share one material handle and batch
/// into a single draw call. `module` is deliberately absent: nothing
/// downstream read it, so it doubled the materials for byte-identical
/// appearances. Modules are distinguished by geometry instead.
pub fn color_key(rgb: [f32; 3], churn_bucket: u32) -> u32 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u32) & 0x1f;
    (q(rgb[0]) << 16) | (q(rgb[1]) << 11) | (q(rgb[2]) << 6) | churn_bucket
}

pub fn churn_bucket(churn_rank: f64) -> u32 {
    (churn_rank.clamp(0.0, 1.0) * 3.999) as u32
}

/// Roof cap for a churn bucket: slab colour and glow. Emissive goes above 1.0
/// on purpose - the camera is HDR and bloom only picks up what exceeds the
/// threshold, which is what makes a busy building glow rather than look warm.
pub fn make_cap_material(materials: &mut Assets<StandardMaterial>, bucket: u32) -> Handle<StandardMaterial> {
    let (base, emissive) = match bucket {
        3 => ([1.00, 0.55, 0.16], LinearRgba::new(5.2, 1.9, 0.35, 1.0)),
        2 => ([0.94, 0.52, 0.18], LinearRgba::new(2.2, 0.85, 0.16, 1.0)),
        _ => ([0.72, 0.46, 0.24], LinearRgba::new(0.55, 0.22, 0.05, 1.0)),
    };
    materials.add(StandardMaterial {
        base_color: Color::srgb(base[0], base[1], base[2]),
        emissive,
        perceptual_roughness: 0.45,
        ..default()
    })
}

/// Plinth under a "module" building, so file modules read differently from
/// types at a glance.
pub fn make_plinth_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.20),
        perceptual_roughness: 0.95,
        ..default()
    })
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
    // Churn is a roof cap now (see `make_cap_material`): at full strength a
    // facade tint was 0.55 of red on an already warm colour, and invisible.
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

/// A flat polygon with a vertical skirt, used for district ground.
pub fn polygon_prism(polygon: &[(f64, f64)], top: f32, bottom: f32) -> Mesh {
    let k = polygon.len();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if k < 3 {
        return Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    }

    let cx = polygon.iter().map(|p| p.0).sum::<f64>() / k as f64;
    let cz = polygon.iter().map(|p| p.1).sum::<f64>() / k as f64;

    positions.push([cx as f32, top, cz as f32]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);
    for p in polygon {
        positions.push([p.0 as f32, top, p.1 as f32]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([((p.0 - cx) * 0.02 + 0.5) as f32, ((p.1 - cz) * 0.02 + 0.5) as f32]);
    }
    // Wound front-facing from +Y, so back-face culling can stay on.
    for i in 0..k {
        let curr = 1 + i as u32;
        let next = 1 + ((i + 1) % k) as u32;
        indices.extend_from_slice(&[0, next, curr]);
    }

    for i in 0..k {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % k];
        let dx = (p2.0 - p1.0) as f32;
        let dz = (p2.1 - p1.1) as f32;
        let len = (dx * dx + dz * dz).sqrt().max(1e-4);
        let (nx, nz) = (dz / len, -dx / len);

        let b = positions.len() as u32;
        positions.push([p1.0 as f32, top, p1.1 as f32]);
        positions.push([p2.0 as f32, top, p2.1 as f32]);
        positions.push([p1.0 as f32, bottom, p1.1 as f32]);
        positions.push([p2.0 as f32, bottom, p2.1 as f32]);
        for _ in 0..4 {
            normals.push([nx, 0.0, nz]);
        }
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]]);
        indices.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Merges all of a district's street slabs into one mesh, rather than a
/// platform entity per building - 434 extra pickables on a mid-sized repo.
pub struct QuadBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl QuadBuilder {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn add_rect(&mut self, cx: f32, cz: f32, w: f32, d: f32, y: f32, rot: f32) {
        let (s, c) = rot.sin_cos();
        let hw = w * 0.5;
        let hd = d * 0.5;
        let corners = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)];
        let base = self.positions.len() as u32;
        for (i, (lx, lz)) in corners.iter().enumerate() {
            self.positions
                .push([cx + lx * c - lz * s, y, cz + lx * s + lz * c]);
            self.normals.push([0.0, 1.0, 0.0]);
            self.uvs.push(match i {
                0 => [0.0, 0.0],
                1 => [1.0, 0.0],
                2 => [1.0, 1.0],
                _ => [0.0, 1.0],
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 3, base + 2, base, base + 2, base + 1]);
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

impl Default for QuadBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn circular_ground(radius: f32, segments: usize) -> Mesh {
    let mut positions = vec![[0.0f32, 0.0, 0.0]];
    let mut normals = vec![[0.0f32, 1.0, 0.0]];
    let mut uvs = vec![[0.5f32, 0.5]];
    let mut indices: Vec<u32> = Vec::with_capacity(segments * 3);

    let step = std::f32::consts::TAU / segments as f32;
    for i in 0..segments {
        let a = i as f32 * step;
        let (x, z) = (radius * a.cos(), radius * a.sin());
        positions.push([x, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([x * 0.01 + 0.5, z * 0.01 + 0.5]);
    }
    for i in 0..segments {
        let curr = (i + 1) as u32;
        let next = ((i + 1) % segments + 1) as u32;
        indices.extend_from_slice(&[0, next, curr]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

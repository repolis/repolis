use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::events::Target;
use crate::layout::Link;
use crate::picking::Pickable;
use crate::render::*;
use crate::{CityIndex, HighlightLinkLayer, LinkLayer};

const DIM: u8 = 1;
const BRIGHT: u8 = 2;
const FADED: u8 = 3;

/// Everything per building or district is in city order.
#[derive(Resource, Default)]
pub(crate) struct Appearance {
    /// rgb per building.
    pub colors: Vec<f32>,
    /// 0 normal, 1 dim, 2 bright, 3 faded, per building.
    pub states: Vec<u8>,
    /// rgb per district ground.
    pub grounds: Vec<f32>,
    /// Arcs to highlight, as consecutive pairs of building indices.
    pub links: Vec<u32>,
}

#[derive(Resource, Default)]
pub(crate) struct ShowLinks(pub bool);

/// Quantised, so lookalike buildings share a material and batch into one call.
fn rgb_key(rgb: [f32; 3]) -> u32 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u32) & 0x1f;
    (q(rgb[0]) << 10) | (q(rgb[1]) << 5) | q(rgb[2])
}

pub(crate) fn material_set(
    materials: &mut Assets<FacadeMaterial>,
    cache: &mut HashMap<u32, BuildingMaterials>,
    rgb: [f32; 3],
) -> BuildingMaterials {
    cache
        .entry(rgb_key(rgb))
        .or_insert_with(|| make_building_materials(materials, rgb))
        .clone()
}

fn rgb_at(v: &[f32], i: usize) -> Option<[f32; 3]> {
    v.get(i * 3..i * 3 + 3).map(|c| [c[0], c[1], c[2]])
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_appearance(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut facades: ResMut<Assets<FacadeMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut index: ResMut<CityIndex>,
    look: Res<Appearance>,
    show: Res<ShowLinks>,
    mut buildings: Query<(&Pickable, &mut MeshMaterial3d<FacadeMaterial>)>,
    grounds: Query<(&Pickable, &MeshMaterial3d<StandardMaterial>)>,
    highlight_q: Query<(&HighlightLinkLayer, &Mesh3d)>,
    mut link_layers: Query<&mut Visibility, With<LinkLayer>>,
) {
    if show.is_changed() {
        for mut vis in &mut link_layers {
            *vis = if show.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    if !look.is_changed() {
        return;
    }
    let index = &mut *index;

    // Translucent rather than hidden: the shape of the city is the context
    // that makes a filter match meaningful.
    let faded = index
        .faded
        .get_or_insert_with(|| {
            facades.add(facade(StandardMaterial {
                base_color: Color::srgba(0.42, 0.43, 0.46, 0.16),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                ..default()
            }))
        })
        .clone();

    for (p, mut handle) in &mut buildings {
        let Target::Building(i) = p.target else {
            continue;
        };
        let Some(rgb) = rgb_at(&look.colors, i) else {
            continue;
        };
        let set = material_set(&mut facades, &mut index.materials, rgb);
        handle.0 = match look.states.get(i).copied() {
            Some(DIM) => set.dim,
            Some(BRIGHT) => set.bright,
            Some(FADED) => faded.clone(),
            _ => set.normal,
        };
    }
    // Each ground owns its material, so the colour is set in place.
    for (p, handle) in &grounds {
        if let Target::District(i) = p.target {
            if let (Some(rgb), Some(mut m)) =
                (rgb_at(&look.grounds, i), materials.get_mut(&handle.0))
            {
                m.base_color = Color::srgb(rgb[0], rgb[1], rgb[2]);
            }
        }
    }

    let pairs: HashSet<(usize, usize)> = look
        .links
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[a, b]| (a.min(b) as usize, a.max(b) as usize))
        .collect();
    for (layer, mesh_handle) in &highlight_q {
        let chosen: Vec<&Link> = index
            .links
            .iter()
            .filter(|l| l.in_cycle == layer.cycles && pairs.contains(&(l.source, l.target)))
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = if layer.cycles {
                build_link_mesh(&chosen, 0.6, &CYCLE_LINKS)
            } else {
                build_link_mesh(&chosen, 0.0, &HIGHLIGHT_LINKS)
            };
        }
    }
}

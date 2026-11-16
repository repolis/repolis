//! Which material each building wears, given colour mode, filter, timeline,
//! selection and path.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::events::*;
use crate::layout::Link;
use crate::render::*;
use crate::view;
use crate::{CityIndex, HighlightLinkLayer, LinkLayer, ViewState};

/// Quantised, so lookalike buildings share a material and batch into one call.
fn rgb_key(rgb: [f32; 3]) -> u32 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u32) & 0x1f;
    (q(rgb[0]) << 10) | (q(rgb[1]) << 5) | q(rgb[2])
}

pub(crate) fn material_set(
    materials: &mut Assets<StandardMaterial>,
    cache: &mut HashMap<u32, BuildingMaterials>,
    rgb: [f32; 3],
) -> BuildingMaterials {
    cache
        .entry(rgb_key(rgb))
        .or_insert_with(|| make_building_materials(materials, rgb))
        .clone()
}

/// Settles every building's appearance in one pass, since colour mode, filter,
/// selection and path all want the same material handle. Filtered-out
/// buildings turn translucent rather than hidden: the shape of the city is the
/// context that makes a match meaningful.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_view(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut index: ResMut<CityIndex>,
    mut state: ResMut<ViewState>,
    interaction: Res<SelectionState>,
    buildings: Query<(Entity, &Pickable)>,
    mut mats: Query<&mut Handle<StandardMaterial>>,
    highlight_q: Query<&Handle<Mesh>, With<HighlightLinkLayer>>,
    mut link_layers: Query<&mut Visibility, With<LinkLayer>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    let index = &mut *index;

    let faded = index.faded.clone().unwrap_or_else(|| {
        let h = materials.add(StandardMaterial {
            base_color: Color::srgba(0.42, 0.43, 0.46, 0.16),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        });
        index.faded = Some(h.clone());
        h
    });

    // Neighbours of the selection, and the highlighted path.
    let mut related: HashSet<&str> = std::collections::HashSet::new();
    let mut chosen: Vec<&Link> = Vec::new();
    let mut selected_id = "";
    if let Some((_, p)) = interaction.selected.and_then(|e| buildings.get(e).ok()) {
        selected_id = &index.city.districts[p.district_idx].buildings[p.building_idx].id;
        for l in &index.links {
            if l.source == selected_id || l.target == selected_id {
                chosen.push(l);
                related.insert(if l.source == selected_id {
                    &l.target
                } else {
                    &l.source
                });
            }
        }
    }

    let on_path: HashSet<Entity> = state.path.iter().copied().collect();
    if !on_path.is_empty() {
        // Replaces the neighbour highlight; both at once hides the answer.
        chosen.clear();
        let path_ids: Vec<&str> = state
            .path
            .iter()
            .filter_map(|e| buildings.get(*e).ok())
            .map(|(_, p)| {
                index.city.districts[p.district_idx].buildings[p.building_idx]
                    .id
                    .as_str()
            })
            .collect();
        for w in path_ids.windows(2) {
            chosen.extend(index.links.iter().filter(|l| {
                (l.source == w[0] && l.target == w[1]) || (l.source == w[1] && l.target == w[0])
            }));
        }
    }

    let filtering = !state.filter.is_empty();
    let focusing = interaction.selected.is_some() || !on_path.is_empty();

    let mut pending: Vec<(Entity, Handle<StandardMaterial>)> = Vec::new();
    for (entity, p) in buildings.iter() {
        let district = &index.city.districts[p.district_idx];
        let b = &district.buildings[p.building_idx];
        // Absent rather than faded: the timeline is for watching it get built.
        if let Some(day) = state.timeline {
            if b.born_day > day {
                pending.push((entity, faded.clone()));
                continue;
            }
        }
        let matched = !filtering || state.filter.matches(b, &district.name);
        let rgb = view::color_for(state.mode, b, &district.typology, state.scales);
        let set = material_set(&mut materials, &mut index.materials, rgb);

        let handle = if !matched {
            faded.clone()
        } else if !on_path.is_empty() {
            if on_path.contains(&entity) {
                set.bright
            } else {
                set.dim
            }
        } else if focusing {
            if b.id == selected_id {
                set.bright
            } else if related.contains(b.id.as_str()) {
                set.normal
            } else {
                set.dim
            }
        } else {
            set.normal
        };
        pending.push((entity, handle));
    }
    for (entity, handle) in pending {
        if let Ok(mut h) = mats.get_mut(entity) {
            *h = handle;
        }
    }

    for mut vis in link_layers.iter_mut() {
        *vis = if state.show_links {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mesh_handle) = highlight_q.get_single() {
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            *mesh = build_link_mesh(&chosen, 0.0);
        }
    }
}

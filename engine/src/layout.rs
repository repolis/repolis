use crate::data::CityMap;
use crate::treemap::{area, inset_by_share, squarify};
use bevy::math::{Rect, Vec2, Vec3};
use std::collections::HashMap;

const MIN_FOOTPRINT: f32 = 4.0;
const MIN_HEIGHT: f32 = 3.0;
const LOT_MARGIN: f32 = 1.1;
const STREET_WIDTH: f32 = 4.5;
const DISTRICT_PADDING: f32 = 7.0;
const DISTRICT_GAP: f32 = 8.0;

pub struct PlacedBuilding {
    /// Position in city order.
    pub order: usize,
    pub district: usize,
    /// Footprint centre on the ground plane (world x, z).
    pub center: Vec2,
    pub size: Vec3,
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    #[default]
    Forward,
    Backward,
    Both,
}

#[derive(Default)]
pub struct Link {
    pub points: Vec<Vec3>,
    pub width: f32,
    pub in_cycle: bool,
    pub direction: Direction,
    /// Building indices, in city order.
    pub source: usize,
    pub target: usize,
}

#[derive(Default)]
pub struct LayoutResult {
    /// Each district's ground, in city order.
    pub districts: Vec<Rect>,
    pub buildings: Vec<PlacedBuilding>,
    pub blocks: Vec<Rect>,
    pub lane_lines: Vec<Rect>,
    pub links: Vec<Link>,
    pub radius: f32,
}

struct Sized<'a> {
    order: usize,
    district: usize,
    dir: &'a str,
    size: Vec3,
}

impl Sized<'_> {
    fn lot_area(&self) -> f32 {
        (self.size.x + LOT_MARGIN * 2.0) * (self.size.z + LOT_MARGIN * 2.0)
    }
}

fn hash01(name: &str, seed: u64) -> f32 {
    let mut h: u64 = seed;
    for b in name.bytes() {
        h = h.wrapping_mul(1099511628211).wrapping_add(b as u64);
    }
    ((h >> 11) % 100_000) as f32 / 100_000.0
}

/// Above `cap`, a value grows only by the `power` of its excess, so one huge
/// type cannot dwarf the city.
fn soften(v: f32, cap: f32, power: f32) -> f32 {
    if v > cap {
        cap + (v - cap).powf(power)
    } else {
        v
    }
}

/// The city is centred on the origin.
pub fn compute_layout(city: &CityMap) -> LayoutResult {
    let sized = compute_dimensions(city);
    if sized.is_empty() {
        return LayoutResult {
            radius: 60.0,
            ..Default::default()
        };
    }

    let mut footprint = vec![0.0; city.districts.len()];
    for s in &sized {
        footprint[s.district] += s.lot_area();
    }
    let needed: Vec<f32> = footprint
        .iter()
        .zip(&city.districts)
        .map(|(&lots, d)| district_area_needed(lots, d.buildings.len()))
        .collect();

    let half = needed.iter().sum::<f32>().sqrt() * 0.5;
    let cells = squarify(&needed, Rect::new(-half, -half, half, half));

    let mut districts = Vec::with_capacity(cells.len());
    let mut buildings = Vec::new();
    let mut blocks = Vec::new();
    let mut lane_lines = Vec::new();
    centre_lines(&cells, Rect::new(-half, -half, half, half), &mut lane_lines);
    for (i, &cell) in cells.iter().enumerate() {
        let ground = inset_by_share(cell, 0.08, 1.0, DISTRICT_GAP);
        // Scaled, not fixed: a fixed inset ate a one-building district's whole plot.
        let plot = inset_by_share(ground, 0.07, 0.5, DISTRICT_PADDING);
        let mine: Vec<&Sized> = sized.iter().filter(|s| s.district == i).collect();
        pack_district(&mine, plot, &mut buildings, &mut blocks, &mut lane_lines);
        districts.push(ground);
    }

    let links = build_links(city, &buildings);
    LayoutResult {
        districts,
        buildings,
        blocks,
        lane_lines,
        links,
        radius: half.max(50.0),
    }
}

/// Budget for streets, gutters and gaps too, or plots fit only the footprints.
fn district_area_needed(lot_area: f32, building_count: usize) -> f32 {
    let count = building_count.max(1) as f32;
    let side = (lot_area * 1.45 + count * STREET_WIDTH * STREET_WIDTH).sqrt() + DISTRICT_GAP * 2.0;
    (side * side).max(400.0)
}

fn footprint_side(num_fields: u32) -> f32 {
    soften(
        MIN_FOOTPRINT + (num_fields as f32).powf(0.58) * 2.4,
        26.0,
        0.5,
    )
}

fn building_height(num_methods: u32, lines_of_code: u32) -> f32 {
    let raw = if num_methods > 0 {
        MIN_HEIGHT + (num_methods as f32).powf(0.72) * 3.4
    } else if lines_of_code > 1 {
        MIN_HEIGHT + (lines_of_code as f32).powf(0.5) * 0.35
    } else {
        MIN_HEIGHT
    };
    soften(raw, 60.0, 0.55)
}

fn compute_dimensions(city: &CityMap) -> Vec<Sized<'_>> {
    let mut out = Vec::new();
    for (d, district) in city.districts.iter().enumerate() {
        for b in &district.buildings {
            let side = footprint_side(b.num_fields);
            let height = building_height(b.num_methods, b.lines_of_code);

            // Keep towers from becoming needles.
            let side = side.max(height * 0.2 + 2.2);
            let aspect = 0.78 + hash01(&b.id, 10) * 0.44;

            out.push(Sized {
                order: out.len(),
                district: d,
                dir: if b.dir.is_empty() { "root" } else { &b.dir },
                size: Vec3::new(side * aspect, height, side / aspect),
            });
        }
    }
    out
}

fn pack_district(
    mine: &[&Sized],
    plot: Rect,
    buildings: &mut Vec<PlacedBuilding>,
    blocks: &mut Vec<Rect>,
    lane_lines: &mut Vec<Rect>,
) {
    if mine.is_empty() || area(plot) <= 1.0 {
        return;
    }

    let mut by_dir: HashMap<&str, Vec<&Sized>> = HashMap::new();
    for s in mine {
        by_dir.entry(s.dir).or_default().push(s);
    }
    let mut dirs: Vec<Vec<&Sized>> = by_dir.into_values().collect();
    dirs.sort_by_key(|in_dir| in_dir[0].dir);

    let dir_areas: Vec<f32> = dirs
        .iter()
        .map(|in_dir| in_dir.iter().map(|s| s.lot_area()).sum::<f32>().max(1.0))
        .collect();

    let cells = squarify(&dir_areas, plot);
    centre_lines(&cells, plot, lane_lines);
    for (in_dir, block) in dirs.iter().zip(cells) {
        let inner = inset_by_share(block, 0.09, 0.25, STREET_WIDTH * 0.5);
        if area(inner) <= 1e-6 {
            continue;
        }
        blocks.push(inner);

        let lot_areas: Vec<f32> = in_dir.iter().map(|s| s.lot_area()).collect();
        for (s, lot) in in_dir.iter().zip(squarify(&lot_areas, inner)) {
            let cell = inset_by_share(lot, 0.12, 0.05, LOT_MARGIN);
            if cell.width() <= 1e-4 || cell.height() <= 1e-4 {
                continue;
            }
            // Shrink to fit so the building never leaves its lot.
            buildings.push(PlacedBuilding {
                order: s.order,
                district: s.district,
                center: cell.center(),
                size: Vec3::new(
                    s.size.x.min(cell.width()).max(0.4),
                    s.size.y,
                    s.size.z.min(cell.height()).max(0.4),
                ),
            });
        }
    }
}

/// Edges on `bounds` face open ground and get no centre line.
fn centre_lines(cells: &[Rect], bounds: Rect, out: &mut Vec<Rect>) {
    for c in cells {
        let top_left = Vec2::new(c.min.x, c.max.y);
        let bottom_right = Vec2::new(c.max.x, c.min.y);
        for (from, to) in [
            (c.min, bottom_right),
            (top_left, c.max),
            (c.min, top_left),
            (bottom_right, c.max),
        ] {
            if !edge_on_bounds(from, to, bounds) {
                out.push(Rect::from_corners(from, to).inflate(0.09));
            }
        }
    }
}

fn edge_on_bounds(from: Vec2, to: Vec2, bounds: Rect) -> bool {
    let near_either = |v: f32, a: f32, b: f32| (v - a).abs() < 0.01 || (v - b).abs() < 0.01;
    if from.x == to.x {
        near_either(from.x, bounds.min.x, bounds.max.x)
    } else {
        near_either(from.y, bounds.min.y, bounds.max.y)
    }
}

#[derive(Default)]
struct MergedEdge {
    weight: u32,
    in_cycle: bool,
    forward: bool,
    backward: bool,
}

impl MergedEdge {
    fn direction(&self) -> Direction {
        match (self.forward, self.backward) {
            (true, true) => Direction::Both,
            (false, true) => Direction::Backward,
            _ => Direction::Forward,
        }
    }
}

/// Arcs, not ground roads: a road centre to centre would cross every building between.
fn build_links(city: &CityMap, buildings: &[PlacedBuilding]) -> Vec<Link> {
    let ids: Vec<&str> = city
        .districts
        .iter()
        .flat_map(|d| d.buildings.iter().map(|b| b.id.as_str()))
        .collect();
    let placed: HashMap<&str, &PlacedBuilding> =
        buildings.iter().map(|b| (ids[b.order], b)).collect();

    // Collapse reciprocal edges; A->B and B->A would overlap exactly.
    let mut merged: HashMap<(usize, usize), MergedEdge> = HashMap::new();
    for e in &city.dependencies {
        let (Some(source), Some(target)) =
            (placed.get(e.source.as_str()), placed.get(e.target.as_str()))
        else {
            continue;
        };
        if source.order == target.order {
            continue;
        }
        let low = source.order.min(target.order);
        let high = source.order.max(target.order);
        let edge = merged.entry((low, high)).or_default();
        edge.weight += e.weight.max(1);
        edge.in_cycle |= e.in_cycle;
        if source.order < target.order {
            edge.forward = true;
        } else {
            edge.backward = true;
        }
    }

    let by_order: HashMap<usize, &PlacedBuilding> =
        buildings.iter().map(|b| (b.order, b)).collect();
    let mut links: Vec<Link> = merged
        .into_iter()
        .map(|((low, high), edge)| Link {
            points: arc(by_order[&low], by_order[&high]),
            width: (0.45 + (edge.weight as f32).ln_1p() * 0.5).min(3.0),
            in_cycle: edge.in_cycle,
            direction: edge.direction(),
            source: low,
            target: high,
        })
        .collect();
    links.sort_by_key(|l| (l.source, l.target));
    links
}

fn arc(src: &PlacedBuilding, tgt: &PlacedBuilding) -> Vec<Vec3> {
    let inter = src.district != tgt.district;
    let start = Vec3::new(src.center.x, src.size.y + 1.0, src.center.y);
    let end = Vec3::new(tgt.center.x, tgt.size.y + 1.0, tgt.center.y);
    let span = src.center.distance(tgt.center);
    let mut control = (start + end) * 0.5;
    control.y = src.size.y.max(tgt.size.y) + 4.0 + span * if inter { 0.18 } else { 0.28 };

    let steps = if inter { 20 } else { 12 };
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            start.lerp(control, t).lerp(control.lerp(end, t), t)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Building, District};

    fn city(sizes: &[(&str, usize)]) -> CityMap {
        let mut c = CityMap::default();
        for (di, (name, n)) in sizes.iter().enumerate() {
            let mut d = District {
                id: format!("d{di}"),
                ..Default::default()
            };
            for i in 0..*n {
                d.buildings.push(Building {
                    id: format!("{name}/f{}.c::T{i}", i % 4),
                    dir: format!("{name}/sub{}", i % 3),
                    num_fields: (i % 17) as u32,
                    num_methods: (i % 11) as u32,
                    lines_of_code: 10 + i as u32,
                });
            }
            c.districts.push(d);
        }
        c
    }

    fn footprint(b: &PlacedBuilding) -> Rect {
        Rect::from_center_size(b.center, Vec2::new(b.size.x, b.size.z))
    }

    /// Core invariant: no two buildings overlap, whatever the district sizes.
    #[test]
    fn no_building_overlaps_even_with_extreme_size_spread() {
        let c = city(&[
            ("huge", 221),
            ("mid", 40),
            ("tiny", 2),
            ("small", 7),
            ("m2", 60),
        ]);
        let r = compute_layout(&c);
        assert_eq!(r.buildings.len(), c.building_count());
        for (i, a) in r.buildings.iter().enumerate() {
            for b in &r.buildings[i + 1..] {
                let overlap = footprint(a).intersect(footprint(b));
                assert!(
                    overlap.width() <= 1e-3 || overlap.height() <= 1e-3,
                    "{} overlaps {}",
                    a.order,
                    b.order
                );
            }
        }
    }

    #[test]
    fn every_building_sits_inside_its_district() {
        let c = city(&[("a", 30), ("b", 12), ("c", 80)]);
        let r = compute_layout(&c);
        for b in &r.buildings {
            let g = r.districts[b.district];
            let f = footprint(b);
            assert!(
                f.min.cmpge(g.min - 1e-3).all() && f.max.cmple(g.max + 1e-3).all(),
                "building escaped district {}",
                b.district
            );
        }
    }

    #[test]
    fn district_area_scales_with_content() {
        let c = city(&[("big", 150), ("small", 6)]);
        let r = compute_layout(&c);
        let (big, small) = (area(r.districts[0]), area(r.districts[1]));
        assert!(big > small * 4.0, "big={big} small={small}");
    }

    #[test]
    fn handles_degenerate_input() {
        assert!(compute_layout(&CityMap::default()).buildings.is_empty());
        assert_eq!(compute_layout(&city(&[("solo", 1)])).buildings.len(), 1);
    }

    #[test]
    fn links_keep_their_direction() {
        let mut c = city(&[("a", 3)]);
        let edge = |s: usize, t: usize| crate::data::DependencyEdge {
            source: format!("a/f{s}.c::T{s}"),
            target: format!("a/f{t}.c::T{t}"),
            weight: 1,
            ..Default::default()
        };
        c.dependencies = vec![edge(0, 1), edge(2, 0), edge(1, 2), edge(2, 1)];
        let dirs: Vec<_> = compute_layout(&c)
            .links
            .iter()
            .map(|l| (l.source, l.target, l.direction))
            .collect();
        assert_eq!(
            dirs,
            [
                (0, 1, Direction::Forward),
                (0, 2, Direction::Backward),
                (1, 2, Direction::Both),
            ]
        );
    }
}

//! Places districts, directory blocks and buildings as three nested squarified
//! treemaps, so nothing can overlap and everything stays axis aligned.

use crate::data::CityMap;
use crate::treemap::{squarify, Item, Rect};
use std::collections::HashMap;

const MIN_FOOTPRINT: f64 = 4.0;
const MIN_HEIGHT: f64 = 3.0;
// Gap left around each building lot; becomes the alley between buildings.
const LOT_MARGIN: f64 = 1.1;
// Gap left around each directory block; becomes a street.
const STREET_WIDTH: f64 = 4.5;
// Gap between a district's ground and its built-up area.
const DISTRICT_PADDING: f64 = 7.0;
// Open land between neighbouring districts.
const DISTRICT_GAP: f64 = 8.0;

pub struct PlacedBuilding {
    pub center_x: f64,
    pub center_z: f64,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub district_idx: usize,
    pub building_idx: usize,
}

/// A dependency drawn as an elevated arc rather than a ground road.
#[derive(Default)]
pub struct Link {
    pub points: Vec<(f64, f64, f64)>,
    pub width: f64,
    /// Both ends sit in the same reported dependency cycle.
    pub in_cycle: bool,
    pub source: String,
    pub target: String,
}

#[derive(Default)]
pub struct LayoutResult {
    /// Each district's ground, in city order.
    pub districts: Vec<Rect>,
    pub buildings: Vec<PlacedBuilding>,
    /// One slab per directory block.
    pub streets: Vec<Rect>,
    pub links: Vec<Link>,
    pub radius: f64,
}

struct Sized {
    district_idx: usize,
    building_idx: usize,
    width: f64,
    depth: f64,
    height: f64,
    dir: String,
}

impl Sized {
    fn lot_area(&self) -> f64 {
        (self.width + LOT_MARGIN * 2.0) * (self.depth + LOT_MARGIN * 2.0)
    }
}

fn hash01(name: &str, seed: u64) -> f64 {
    let mut h: u64 = seed;
    for b in name.bytes() {
        h = h.wrapping_mul(1099511628211).wrapping_add(b as u64);
    }
    ((h >> 11) % 100_000) as f64 / 100_000.0
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

    // Area each district must accommodate, including its streets and padding.
    let n = city.districts.len();
    let mut needed = vec![0.0_f64; n];
    for s in &sized {
        needed[s.district_idx] += s.lot_area();
    }
    let items: Vec<Item> = needed
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let count = city.districts[i].buildings.len().max(1) as f64;
            // Streets, gutters and the gap to the neighbours are real area:
            // budget for them, or the plot is sized for the footprints alone.
            let area = a * 1.45 + count * STREET_WIDTH * STREET_WIDTH;
            let side = area.sqrt() + DISTRICT_GAP * 2.0;
            Item {
                key: i,
                area: (side * side).max(400.0),
            }
        })
        .collect();

    let side = items.iter().map(|i| i.area).sum::<f64>().sqrt();
    let mut cells = vec![Rect::new(0.0, 0.0, 0.0, 0.0); n];
    for (i, cell) in squarify(&items, Rect::new(-side * 0.5, -side * 0.5, side, side)) {
        cells[i] = cell;
    }

    let mut districts = Vec::with_capacity(n);
    let mut buildings = Vec::new();
    let mut streets = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        let ground = cell.inset((cell.w.min(cell.d) * 0.08).clamp(1.0, DISTRICT_GAP));
        // Scaled: a fixed inset ate a one-building district's whole plot.
        let plot = ground.inset((ground.w.min(ground.d) * 0.07).clamp(0.5, DISTRICT_PADDING));
        let mine: Vec<&Sized> = sized.iter().filter(|s| s.district_idx == i).collect();
        pack_district(&mine, &plot, &mut buildings, &mut streets);
        districts.push(ground);
    }

    let links = build_links(city, &buildings);
    LayoutResult {
        districts,
        buildings,
        streets,
        links,
        radius: (side * 0.5).max(50.0),
    }
}

fn compute_dimensions(city: &CityMap) -> Vec<Sized> {
    let mut out = Vec::new();
    for (d_idx, district) in city.districts.iter().enumerate() {
        for (b_idx, b) in district.buildings.iter().enumerate() {
            // Footprint is state (fields), height is behaviour (methods).
            let raw_side = MIN_FOOTPRINT + (b.num_fields as f64).powf(0.58) * 2.4;
            let side = if raw_side > 26.0 {
                26.0 + (raw_side - 26.0).powf(0.5)
            } else {
                raw_side
            };

            let raw_height = if b.num_methods > 0 {
                MIN_HEIGHT + (b.num_methods as f64).powf(0.72) * 3.4
            } else if b.lines_of_code > 1 {
                MIN_HEIGHT + (b.lines_of_code as f64).powf(0.5) * 0.35
            } else {
                MIN_HEIGHT
            };
            let height = if raw_height > 60.0 {
                60.0 + (raw_height - 60.0).powf(0.55)
            } else {
                raw_height
            };

            // Keep towers from becoming needles.
            let final_side = side.max(height * 0.2 + 2.2);
            let aspect = 0.78 + hash01(&b.id, 10) * 0.44;

            out.push(Sized {
                district_idx: d_idx,
                building_idx: b_idx,
                width: final_side * aspect,
                depth: final_side / aspect,
                height,
                dir: if b.dir.is_empty() {
                    "root".to_string()
                } else {
                    b.dir.clone()
                },
            });
        }
    }
    out
}

fn pack_district(
    mine: &[&Sized],
    plot: &Rect,
    buildings: &mut Vec<PlacedBuilding>,
    streets: &mut Vec<Rect>,
) {
    if mine.is_empty() || plot.area() <= 1.0 {
        return;
    }

    // One block per source directory, sized by its total footprint.
    let mut by_dir: HashMap<&str, Vec<&Sized>> = HashMap::new();
    for s in mine {
        by_dir.entry(s.dir.as_str()).or_default().push(s);
    }
    let mut dir_keys: Vec<&str> = by_dir.keys().copied().collect();
    dir_keys.sort_unstable();

    let dir_items: Vec<Item> = dir_keys
        .iter()
        .enumerate()
        .map(|(i, d)| Item {
            key: i,
            area: by_dir[d].iter().map(|s| s.lot_area()).sum::<f64>().max(1.0),
        })
        .collect();

    for (dir_key, block) in squarify(&dir_items, *plot) {
        let in_dir = &by_dir[dir_keys[dir_key]];
        // The street between blocks, proportional so a block never disappears
        // into its own gutter.
        let street = (block.w.min(block.d) * 0.09).clamp(0.25, STREET_WIDTH * 0.5);
        let inner = block.inset(street);
        if inner.area() <= 1e-6 {
            continue;
        }
        streets.push(block);

        // One lot per building inside its directory's block.
        let items: Vec<Item> = in_dir
            .iter()
            .enumerate()
            .map(|(i, s)| Item {
                key: i,
                area: s.lot_area(),
            })
            .collect();

        for (k, lot) in squarify(&items, inner) {
            let s = in_dir[k];
            let margin = (lot.w.min(lot.d) * 0.12).clamp(0.05, LOT_MARGIN);
            let cell = lot.inset(margin);
            if cell.w <= 1e-4 || cell.d <= 1e-4 {
                continue;
            }

            // Shrink to fit when the lot is smaller. Either way the building
            // stays inside its lot, so no two can touch.
            let (x, z) = cell.center();
            buildings.push(PlacedBuilding {
                center_x: x,
                center_z: z,
                width: s.width.min(cell.w).max(0.4),
                height: s.height,
                depth: s.depth.min(cell.d).max(0.4),
                district_idx: s.district_idx,
                building_idx: s.building_idx,
            });
        }
    }
}

/// Dependency links, drawn as arcs above the rooftops. A ground road runs
/// centre to centre, so it starts under its source building, ends under its
/// target and crosses everything between; an arc cannot intersect the city and
/// leaves the ground plane to the street grid.
fn build_links(city: &CityMap, buildings: &[PlacedBuilding]) -> Vec<Link> {
    let id_of = |b: &PlacedBuilding| {
        city.districts[b.district_idx].buildings[b.building_idx]
            .id
            .as_str()
    };
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, b) in buildings.iter().enumerate() {
        index.insert(id_of(b), i);
    }

    // Collapse reciprocal edges; A->B and B->A would overlap exactly.
    let mut merged: HashMap<(usize, usize), (u32, bool)> = HashMap::new();
    for e in &city.dependencies {
        let (Some(&a), Some(&b)) = (index.get(e.source.as_str()), index.get(e.target.as_str()))
        else {
            continue;
        };
        if a == b {
            continue;
        }
        let slot = merged.entry((a.min(b), a.max(b))).or_default();
        slot.0 += e.weight.max(1);
        slot.1 |= e.in_cycle;
    }

    let mut keys: Vec<((usize, usize), (u32, bool))> = merged.into_iter().collect();
    keys.sort_by(|x, y| y.1 .0.cmp(&x.1 .0).then(x.0.cmp(&y.0)));

    let mut links = Vec::with_capacity(keys.len());
    for ((a, b), (weight, in_cycle)) in keys {
        let src = &buildings[a];
        let tgt = &buildings[b];
        let inter = src.district_idx != tgt.district_idx;

        let (sx, sz) = (src.center_x, src.center_z);
        let (tx, tz) = (tgt.center_x, tgt.center_z);
        let span = (tx - sx).hypot(tz - sz);
        let lift = src.height.max(tgt.height) + 4.0 + span * if inter { 0.18 } else { 0.28 };

        // Quadratic arc, sampled just densely enough to read as a curve.
        let steps = if inter { 20 } else { 12 };
        let points = (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                let inv = 1.0 - t;
                let x = inv * inv * sx + 2.0 * inv * t * ((sx + tx) * 0.5) + t * t * tx;
                let z = inv * inv * sz + 2.0 * inv * t * ((sz + tz) * 0.5) + t * t * tz;
                let y = inv * inv * (src.height + 1.0)
                    + 2.0 * inv * t * lift
                    + t * t * (tgt.height + 1.0);
                (x, y, z)
            })
            .collect();

        links.push(Link {
            points,
            width: (0.45 + (weight as f64).ln_1p() * 0.5).min(3.0),
            in_cycle,
            source: id_of(src).to_string(),
            target: id_of(tgt).to_string(),
        });
    }
    links
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
                name: name.to_string(),
                typology: "core".into(),
                ..Default::default()
            };
            for i in 0..*n {
                d.buildings.push(Building {
                    id: format!("{name}/f{}.c::T{i}", i % 4),
                    name: format!("T{i}"),
                    kind: "type".into(),
                    dir: format!("{name}/sub{}", i % 3),
                    num_fields: (i % 17) as u32,
                    num_methods: (i % 11) as u32,
                    lines_of_code: 10 + i as u32,
                    ..Default::default()
                });
            }
            c.districts.push(d);
        }
        c
    }

    /// The core invariant: no two buildings overlap, whatever the relative
    /// district sizes.
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
                let overlap = (a.center_x - b.center_x).abs() < (a.width + b.width) * 0.5 - 1e-6
                    && (a.center_z - b.center_z).abs() < (a.depth + b.depth) * 0.5 - 1e-6;
                assert!(!overlap, "{} overlaps {}", a.building_idx, b.building_idx);
            }
        }
    }

    #[test]
    fn every_building_sits_inside_its_district() {
        let c = city(&[("a", 30), ("b", 12), ("c", 80)]);
        let r = compute_layout(&c);
        for b in &r.buildings {
            let g = r.districts[b.district_idx];
            assert!(
                b.center_x - b.width * 0.5 >= g.x
                    && b.center_x + b.width * 0.5 <= g.x + g.w
                    && b.center_z - b.depth * 0.5 >= g.z
                    && b.center_z + b.depth * 0.5 <= g.z + g.d,
                "building escaped district {}",
                b.district_idx
            );
        }
    }

    /// Ground area must track district size.
    #[test]
    fn district_area_scales_with_content() {
        let c = city(&[("big", 150), ("small", 6)]);
        let r = compute_layout(&c);
        let big = r.districts[0].area();
        let small = r.districts[1].area();
        assert!(big > small * 4.0, "big={big} small={small}");
    }

    #[test]
    fn handles_degenerate_input() {
        let empty = CityMap::default();
        assert!(compute_layout(&empty).buildings.is_empty());
        let one = city(&[("solo", 1)]);
        assert_eq!(compute_layout(&one).buildings.len(), 1);
    }
}

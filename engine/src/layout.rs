use crate::data::CityMap;
use crate::treemap::{squarify, Item, Rect};
use std::collections::HashMap;
use voronoice::{BoundingBox, Point, VoronoiBuilder};

const MIN_FOOTPRINT: f64 = 4.0;
const MIN_HEIGHT: f64 = 3.0;
/// Gap left around each building lot; becomes the alley between buildings.
const LOT_MARGIN: f64 = 1.1;
/// Gap left around each directory block; becomes a street.
const STREET_WIDTH: f64 = 4.5;
/// Gap between a district's outer boundary and its built-up area.
const DISTRICT_PADDING: f64 = 7.0;

pub struct PlacedBuilding {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// World-space centre of the footprint.
    pub center_x: f64,
    pub center_z: f64,
    /// Footprint centre in district-local (unrotated) space, used for picking.
    pub local_x: f64,
    pub local_z: f64,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub district_idx: usize,
    pub churn_rank: f64,
    pub age_days: u32,
}

pub struct Street {
    pub local_x: f64,
    pub local_z: f64,
    pub width: f64,
    pub depth: f64,
    pub district_idx: usize,
}

pub struct PlacedDistrict {
    pub name: String,
    pub typology: String,
    /// Convex Voronoi cell, world space.
    pub polygon: Vec<(f64, f64)>,
    /// Rotation of the built-up area about `origin`.
    pub rot: f64,
    pub origin_x: f64,
    pub origin_z: f64,
    pub label_x: f64,
    pub label_z: f64,
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
    pub building_count: usize,
}

/// A dependency drawn as an elevated arc rather than a ground road.
pub struct Link {
    pub points: Vec<(f64, f64, f64)>,
    pub width: f64,
    pub weight: u32,
    /// Weight each way. Reciprocal edges share geometry but keep direction,
    /// so the inspector can still tell callers from callees.
    pub fwd: u32,
    pub rev: u32,
    pub kind: String,
    /// Both ends sit in the same reported dependency cycle.
    pub in_cycle: bool,
    pub source: String,
    pub target: String,
    pub source_name: String,
    pub target_name: String,
    pub inter_district: bool,
}

pub struct LayoutResult {
    pub districts: Vec<PlacedDistrict>,
    pub buildings: Vec<PlacedBuilding>,
    pub streets: Vec<Street>,
    pub links: Vec<Link>,
    pub radius: f64,
    pub center_x: f64,
    pub center_z: f64,
}

struct Sized {
    idx_in_city: usize,
    district_idx: usize,
    width: f64,
    depth: f64,
    height: f64,
    dir: String,
}

fn hash01(name: &str, seed: u64) -> f64 {
    let mut h: u64 = seed;
    for b in name.bytes() {
        h = h.wrapping_mul(1099511628211).wrapping_add(b as u64);
    }
    ((h >> 11) % 100_000) as f64 / 100_000.0
}

pub fn compute_layout(city: &CityMap) -> LayoutResult {
    let sized = compute_dimensions(city);
    if sized.is_empty() {
        return LayoutResult {
            districts: Vec::new(),
            buildings: Vec::new(),
            streets: Vec::new(),
            links: Vec::new(),
            radius: 60.0,
            center_x: 0.0,
            center_z: 0.0,
        };
    }

    // Area each district must accommodate, including its streets and padding.
    let n_districts = city.districts.len();
    let mut needed = vec![0.0_f64; n_districts];
    for s in &sized {
        needed[s.district_idx] += (s.width + LOT_MARGIN * 2.0) * (s.depth + LOT_MARGIN * 2.0);
    }
    for (i, a) in needed.iter_mut().enumerate() {
        let count = city.districts[i].buildings.len().max(1) as f64;
        // Streets and gutters are real area: budget for them, or the cell is
        // sized for the footprints alone.
        *a = (*a * 1.45 + count * STREET_WIDTH * STREET_WIDTH).max(400.0);
    }

    let seeds = place_seeds(city, &needed);
    let mut cells = voronoi_cells(&seeds, &needed);
    fit_cells(&mut cells, &needed);

    let mut districts = Vec::with_capacity(n_districts);
    let mut buildings = Vec::new();
    let mut streets = Vec::new();

    for (i, district) in city.districts.iter().enumerate() {
        let polygon = cells[i].clone();
        let (cx, cz) = polygon_centroid(&polygon);
        let rot = principal_angle(&polygon, cx, cz);

        // The largest rectangle fitting the cell at its own principal
        // orientation. Rotating each district independently keeps the result
        // organic rather than a grid.
        let raw_plot = inscribed_rect(&polygon, cx, cz, rot);
        // Scaled: a fixed inset ate a one-building district's whole plot.
        let pad = (raw_plot.w.min(raw_plot.d) * 0.07).clamp(0.5, DISTRICT_PADDING);
        let plot = raw_plot.inset(pad);

        let mine: Vec<&Sized> = sized.iter().filter(|s| s.district_idx == i).collect();
        pack_district(&mine, &plot, rot, cx, cz, i, city, &mut buildings, &mut streets);

        let (min_x, min_z, max_x, max_z) = polygon_bounds(&polygon);
        districts.push(PlacedDistrict {
            name: district.name.clone(),
            typology: district.typology.clone(),
            polygon,
            rot,
            origin_x: cx,
            origin_z: cz,
            label_x: cx,
            label_z: cz,
            min_x,
            min_z,
            max_x,
            max_z,
            building_count: district.buildings.len(),
        });
    }

    let links = build_links(city, &buildings, &districts);

    let (min_x, min_z, max_x, max_z) = districts.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(a, b, c, d), p| (a.min(p.min_x), b.min(p.min_z), c.max(p.max_x), d.max(p.max_z)),
    );
    let center_x = (min_x + max_x) * 0.5;
    let center_z = (min_z + max_z) * 0.5;
    let radius = ((max_x - min_x).max(max_z - min_z) * 0.5).max(50.0);

    LayoutResult {
        districts,
        buildings,
        streets,
        links,
        radius,
        center_x,
        center_z,
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
                idx_in_city: b_idx,
                district_idx: d_idx,
                width: final_side * aspect,
                depth: final_side / aspect,
                height,
                dir: if b.dir.is_empty() { "root".to_string() } else { b.dir.clone() },
            });
        }
    }
    out
}

/// Places district seeds by dependency attraction, then enforces the spacing a
/// Voronoi diagram actually needs.
fn place_seeds(city: &CityMap, needed: &[f64]) -> Vec<(f64, f64)> {
    let n = city.districts.len();
    let radii: Vec<f64> = needed
        .iter()
        .map(|a| (a / std::f64::consts::PI).sqrt())
        .collect();

    // Inter-district coupling, summed from the building-level call graph.
    let mut owner: HashMap<&str, usize> = HashMap::new();
    for (i, d) in city.districts.iter().enumerate() {
        for b in &d.buildings {
            owner.insert(b.id.as_str(), i);
        }
    }
    let mut coupling = vec![0.0_f64; n * n];
    for e in &city.dependencies {
        if let (Some(&a), Some(&b)) = (owner.get(e.source.as_str()), owner.get(e.target.as_str())) {
            if a != b {
                coupling[a * n + b] += e.weight as f64;
                coupling[b * n + a] += e.weight as f64;
            }
        }
    }
    let max_coupling = coupling.iter().cloned().fold(1.0_f64, f64::max);

    // Deterministic start: a golden-angle spiral scaled by district size.
    let avg_r = radii.iter().sum::<f64>() / (n.max(1) as f64);
    let mut seeds: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            if i == 0 {
                (0.0, 0.0)
            } else {
                let h = hash01(&city.districts[i].id, 42);
                let angle = i as f64 * 2.399963229728653 + h * 0.4;
                let dist = (i as f64).sqrt() * avg_r * 2.1;
                (dist * angle.cos(), dist * angle.sin())
            }
        })
        .collect();

    if n <= 1 {
        return seeds;
    }

    for pass in 0..140 {
        // Districts that call each other pull together, so proximity in the
        // finished city means coupling rather than alphabetical order.
        let cool = 1.0 - (pass as f64 / 140.0);
        for i in 0..n {
            for j in (i + 1)..n {
                let c = coupling[i * n + j];
                if c <= 0.0 {
                    continue;
                }
                let dx = seeds[j].0 - seeds[i].0;
                let dz = seeds[j].1 - seeds[i].1;
                let dist = (dx * dx + dz * dz).sqrt().max(1e-3);
                let pull = (c / max_coupling) * cool * dist * 0.02;
                let (nx, nz) = (dx / dist, dz / dist);
                seeds[i].0 += nx * pull;
                seeds[i].1 += nz * pull;
                seeds[j].0 -= nx * pull;
                seeds[j].1 -= nz * pull;
            }
        }
        separate(&mut seeds, &radii);
    }

    seeds
}

/// Shrinks each cell about its centroid to the area its district needs.
///
/// Seed placement alone cannot do this: a Voronoi boundary is equidistant from
/// both seeds, so giving a large cell its radius hands its small neighbour the
/// same one. The separation rule makes every cell big ENOUGH; this removes the
/// surplus, leaving ground area proportional to the code and open land between
/// districts.
fn fit_cells(cells: &mut [Vec<(f64, f64)>], needed: &[f64]) {
    for (i, poly) in cells.iter_mut().enumerate() {
        let area = polygon_area(poly);
        if area <= 1e-6 {
            continue;
        }
        // A floor: a two-building district must stay findable and clickable.
        let scale = (needed[i] / area).sqrt().clamp(0.28, 1.0);
        if scale >= 0.999 {
            continue;
        }
        let (cx, cz) = polygon_centroid(poly);
        for v in poly.iter_mut() {
            v.0 = cx + (v.0 - cx) * scale;
            v.1 = cz + (v.1 - cz) * scale;
        }
    }
}


fn separate(seeds: &mut [(f64, f64)], radii: &[f64]) {
    let n = seeds.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = seeds[j].0 - seeds[i].0;
            let dz = seeds[j].1 - seeds[i].1;
            let dist = (dx * dx + dz * dz).sqrt().max(1e-3);
            // The separation a bisector needs to give the larger cell its
            // radius; 0.93 tolerates non-circular cells.
            let min_dist = 2.0 * radii[i].max(radii[j]) * 1.02;
            if dist < min_dist {
                let push = (min_dist - dist) * 0.5;
                let (nx, nz) = (dx / dist, dz / dist);
                seeds[i].0 -= nx * push;
                seeds[i].1 -= nz * push;
                seeds[j].0 += nx * push;
                seeds[j].1 += nz * push;
            }
        }
    }
}

fn voronoi_cells(seeds: &[(f64, f64)], needed: &[f64]) -> Vec<Vec<(f64, f64)>> {
    let n = seeds.len();
    let fallback = |i: usize| -> Vec<(f64, f64)> {
        let r = (needed[i] / std::f64::consts::PI).sqrt().max(12.0);
        (0..10)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 10.0;
                (seeds[i].0 + r * a.cos(), seeds[i].1 + r * a.sin())
            })
            .collect()
    };

    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![fallback(0)];
    }

    let max_r = needed
        .iter()
        .map(|a| (a / std::f64::consts::PI).sqrt())
        .fold(0.0_f64, f64::max);
    let extent = seeds
        .iter()
        .map(|(x, z)| x.hypot(*z))
        .fold(0.0_f64, f64::max)
        + max_r;

    // The box must comfortably contain every site: voronoice's default
    // ClipBehavior drops sites outside it, which silently loses anchors and
    // skews the outer cells.
    let box_size = (extent * 3.2).max(200.0);
    let anchor_radius = extent * 1.28 + max_r * 0.5;

    let mut sites: Vec<Point> = seeds.iter().map(|&(x, z)| Point { x, y: z }).collect();
    let anchors = 16;
    for k in 0..anchors {
        let a = k as f64 * std::f64::consts::TAU / anchors as f64;
        sites.push(Point {
            x: anchor_radius * a.cos(),
            y: anchor_radius * a.sin(),
        });
    }

    let built = VoronoiBuilder::default()
        .set_sites(sites)
        .set_bounding_box(BoundingBox::new_centered(box_size, box_size))
        // No Lloyd relaxation: it moves every site to its centroid, so the
        // seeds this code reasons about stop being the cells' sites, and it
        // equalises areas - the opposite of size-proportional districts.
        .build();

    (0..n)
        .map(|i| match built {
            Some(ref v) => {
                let mut poly: Vec<(f64, f64)> =
                    v.cell(i).iter_vertices().map(|p| (p.x, p.y)).collect();
                if poly.len() < 3 {
                    return fallback(i);
                }
                if polygon_signed_area(&poly) < 0.0 {
                    poly.reverse();
                }
                poly
            }
            None => fallback(i),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn pack_district(
    mine: &[&Sized],
    plot: &Rect,
    rot: f64,
    cx: f64,
    cz: f64,
    district_idx: usize,
    city: &CityMap,
    buildings: &mut Vec<PlacedBuilding>,
    streets: &mut Vec<Street>,
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
            area: by_dir[d]
                .iter()
                .map(|s| (s.width + LOT_MARGIN * 2.0) * (s.depth + LOT_MARGIN * 2.0))
                .sum::<f64>()
                .max(1.0),
        })
        .collect();

    let (sin_r, cos_r) = rot.sin_cos();
    let to_world = |lx: f64, lz: f64| -> (f64, f64) {
        let dx = lx - cx;
        let dz = lz - cz;
        (cx + dx * cos_r - dz * sin_r, cz + dx * sin_r + dz * cos_r)
    };

    for (dir_key, block) in squarify(&dir_items, *plot) {
        let dir = dir_keys[dir_key];
        // The street between blocks, proportional so a block never disappears
        // into its own gutter.
        let street = (block.w.min(block.d) * 0.09).clamp(0.25, STREET_WIDTH * 0.5);
        let inner = block.inset(street);
        if inner.area() <= 1e-6 {
            continue;
        }
        let (bcx, bcz) = block.center();
        streets.push(Street {
            local_x: bcx,
            local_z: bcz,
            width: block.w,
            depth: block.d,
            district_idx,
        });

        // One lot per building inside its directory's block.
        let items: Vec<Item> = by_dir[dir]
            .iter()
            .enumerate()
            .map(|(i, s)| Item {
                key: i,
                area: (s.width + LOT_MARGIN * 2.0) * (s.depth + LOT_MARGIN * 2.0),
            })
            .collect();

        for (k, lot) in squarify(&items, inner) {
            let s = by_dir[dir][k];
            let b = &city.districts[district_idx].buildings[s.idx_in_city];
            let margin = (lot.w.min(lot.d) * 0.12).clamp(0.05, LOT_MARGIN);
            let cell = lot.inset(margin);
            if cell.w <= 1e-4 || cell.d <= 1e-4 {
                continue;
            }

            // Shrink to fit when the lot is smaller. Either way the building
            // stays inside its lot, so no two can touch.
            let w = s.width.min(cell.w).max(0.4);
            let d = s.depth.min(cell.d).max(0.4);
            let (lx, lz) = cell.center();
            let (wx, wz) = to_world(lx, lz);

            buildings.push(PlacedBuilding {
                id: b.id.clone(),
                name: b.name.clone(),
                kind: b.kind.clone(),
                center_x: wx,
                center_z: wz,
                local_x: lx,
                local_z: lz,
                width: w,
                height: s.height,
                depth: d,
                district_idx,
                churn_rank: b.churn_rank,
                age_days: b.age_days,
            });
        }
    }
}

/// Dependency links, drawn as arcs above the rooftops. A ground road runs
/// centre to centre, so it starts under its source building, ends under its
/// target and crosses everything between; an arc cannot intersect the city and
/// leaves the ground plane to the street grid.
fn build_links(city: &CityMap, buildings: &[PlacedBuilding], districts: &[PlacedDistrict]) -> Vec<Link> {
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, b) in buildings.iter().enumerate() {
        index.insert(b.id.as_str(), i);
    }

    // Collapse reciprocal edges; A->B and B->A would overlap exactly.
    struct Merged {
        fwd: u32,
        rev: u32,
        kind: String,
        in_cycle: bool,
    }
    let mut merged: HashMap<(usize, usize), Merged> = HashMap::new();
    for e in &city.dependencies {
        let (Some(&a), Some(&b)) = (index.get(e.source.as_str()), index.get(e.target.as_str())) else {
            continue;
        };
        if a == b {
            continue;
        }
        let forward = a < b;
        let key = if forward { (a, b) } else { (b, a) };
        let slot = merged.entry(key).or_insert(Merged {
            fwd: 0,
            rev: 0,
            kind: e.kind.clone(),
            in_cycle: false,
        });
        slot.in_cycle |= e.in_cycle;
        if forward {
            slot.fwd += e.weight.max(1);
        } else {
            slot.rev += e.weight.max(1);
        }
    }

    let mut keys: Vec<(&(usize, usize), &Merged)> = merged.iter().collect();
    keys.sort_by(|x, y| {
        (y.1.fwd + y.1.rev)
            .cmp(&(x.1.fwd + x.1.rev))
            .then(x.0.cmp(y.0))
    });

    let mut links = Vec::with_capacity(keys.len());
    for (&(a, b), m) in keys {
        let weight = &(m.fwd + m.rev);
        let src = &buildings[a];
        let tgt = &buildings[b];
        let inter = src.district_idx != tgt.district_idx;

        let sx = src.center_x;
        let sz = src.center_z;
        let tx = tgt.center_x;
        let tz = tgt.center_z;
        let span = (tx - sx).hypot(tz - sz);

        let base = src.height.max(tgt.height) + 4.0;
        let lift = base + span * if inter { 0.18 } else { 0.28 };

        // Quadratic arc, sampled just densely enough to read as a curve.
        let steps = if inter { 20 } else { 12 };
        let mut points = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let inv = 1.0 - t;
            let x = inv * inv * sx + 2.0 * inv * t * ((sx + tx) * 0.5) + t * t * tx;
            let z = inv * inv * sz + 2.0 * inv * t * ((sz + tz) * 0.5) + t * t * tz;
            let y = inv * inv * (src.height + 1.0)
                + 2.0 * inv * t * lift
                + t * t * (tgt.height + 1.0);
            points.push((x, y, z));
        }

        let d_src = districts.get(src.district_idx);
        let d_tgt = districts.get(tgt.district_idx);
        links.push(Link {
            points,
            width: (0.45 + (*weight as f64).ln_1p() * 0.5).min(3.0),
            weight: *weight,
            fwd: m.fwd,
            rev: m.rev,
            kind: m.kind.clone(),
            in_cycle: m.in_cycle,
            source: src.id.clone(),
            target: tgt.id.clone(),
            source_name: src.name.clone(),
            target_name: tgt.name.clone(),
            inter_district: inter
                && d_src.map(|d| d.building_count).unwrap_or(0) > 0
                && d_tgt.map(|d| d.building_count).unwrap_or(0) > 0,
        });
    }

    links
}

// ---- polygon helpers ----

pub fn polygon_signed_area(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        let (x1, z1) = poly[i];
        let (x2, z2) = poly[(i + 1) % n];
        a += x1 * z2 - x2 * z1;
    }
    a * 0.5
}

pub fn polygon_area(poly: &[(f64, f64)]) -> f64 {
    polygon_signed_area(poly).abs()
}

pub fn polygon_centroid(poly: &[(f64, f64)]) -> (f64, f64) {
    let n = poly.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let a = polygon_signed_area(poly);
    if a.abs() < 1e-9 {
        let sx: f64 = poly.iter().map(|p| p.0).sum();
        let sz: f64 = poly.iter().map(|p| p.1).sum();
        return (sx / n as f64, sz / n as f64);
    }
    let (mut cx, mut cz) = (0.0, 0.0);
    for i in 0..n {
        let (x1, z1) = poly[i];
        let (x2, z2) = poly[(i + 1) % n];
        let cross = x1 * z2 - x2 * z1;
        cx += (x1 + x2) * cross;
        cz += (z1 + z2) * cross;
    }
    (cx / (6.0 * a), cz / (6.0 * a))
}

pub fn polygon_bounds(poly: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    poly.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(a, b, c, d), &(x, z)| (a.min(x), b.min(z), c.max(x), d.max(z)),
    )
}

/// Orientation of the cell's dominant axis, from the covariance of its
/// vertices. Building each district along its own axis is what stops the city
/// looking like a circuit board.
fn principal_angle(poly: &[(f64, f64)], cx: f64, cz: f64) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let (mut sxx, mut szz, mut sxz) = (0.0, 0.0, 0.0);
    for &(x, z) in poly {
        let dx = x - cx;
        let dz = z - cz;
        sxx += dx * dx;
        szz += dz * dz;
        sxz += dx * dz;
    }
    0.5 * (2.0 * sxz).atan2(sxx - szz)
}

pub fn point_in_convex(px: f64, pz: f64, poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        // A degenerate cell contains nothing; returning true lets every
        // candidate rectangle "fit" and scatters the buildings.
        return false;
    }
    for i in 0..n {
        let (x1, z1) = poly[i];
        let (x2, z2) = poly[(i + 1) % n];
        if (x2 - x1) * (pz - z1) - (z2 - z1) * (px - x1) < -1e-6 {
            return false;
        }
    }
    true
}

/// Largest axis-aligned-at-angle-`rot` rectangle centred on the cell centroid
/// that fits inside the polygon, found by bisection on scale.
fn inscribed_rect(poly: &[(f64, f64)], cx: f64, cz: f64, rot: f64) -> Rect {
    let (sin_r, cos_r) = rot.sin_cos();
    // Work in the rotated frame.
    let local: Vec<(f64, f64)> = poly
        .iter()
        .map(|&(x, z)| {
            let dx = x - cx;
            let dz = z - cz;
            (dx * cos_r + dz * sin_r, -dx * sin_r + dz * cos_r)
        })
        .collect();

    let (min_x, min_z, max_x, max_z) = polygon_bounds(&local);
    let half_w = ((max_x - min_x) * 0.5).max(1.0);
    let half_d = ((max_z - min_z) * 0.5).max(1.0);

    let fits = |s: f64| -> bool {
        let hw = half_w * s;
        let hd = half_d * s;
        point_in_convex(-hw, -hd, &local)
            && point_in_convex(hw, -hd, &local)
            && point_in_convex(hw, hd, &local)
            && point_in_convex(-hw, hd, &local)
    };

    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    if fits(hi) {
        lo = hi;
    } else {
        for _ in 0..28 {
            let mid = (lo + hi) * 0.5;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }

    let w = half_w * lo * 2.0;
    let d = half_d * lo * 2.0;
    // Returned in the rotated frame, centred on the centroid.
    Rect::new(cx - w * 0.5, cz - d * 0.5, w, d)
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
        let c = city(&[("huge", 221), ("mid", 40), ("tiny", 2), ("small", 7), ("m2", 60)]);
        let r = compute_layout(&c);
        assert_eq!(r.buildings.len(), c.building_count());

        let mut per_district: HashMap<usize, Vec<&PlacedBuilding>> = HashMap::new();
        for b in &r.buildings {
            per_district.entry(b.district_idx).or_default().push(b);
        }
        // District-local space, where footprints are axis aligned.
        for (_, list) in per_district {
            for i in 0..list.len() {
                for j in (i + 1)..list.len() {
                    let (a, b) = (list[i], list[j]);
                    let dx = (a.local_x - b.local_x).abs();
                    let dz = (a.local_z - b.local_z).abs();
                    let overlap = dx < (a.width + b.width) * 0.5 - 1e-6
                        && dz < (a.depth + b.depth) * 0.5 - 1e-6;
                    assert!(!overlap, "{} overlaps {}", a.name, b.name);
                }
            }
        }
    }

    #[test]
    fn every_building_sits_inside_its_district_cell() {
        let c = city(&[("a", 30), ("b", 12), ("c", 80)]);
        let r = compute_layout(&c);
        for b in &r.buildings {
            let d = &r.districts[b.district_idx];
            assert!(
                point_in_convex(b.center_x, b.center_z, &d.polygon),
                "{} escaped district {}",
                b.name,
                d.name
            );
        }
    }

    /// Cell area must track district size.
    #[test]
    fn district_cell_area_scales_with_content() {
        let c = city(&[("big", 150), ("small", 6)]);
        let r = compute_layout(&c);
        let big = polygon_area(&r.districts[0].polygon);
        let small = polygon_area(&r.districts[1].polygon);
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

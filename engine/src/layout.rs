use crate::data::CityMap;
use std::collections::HashMap;
use voronoice::{BoundingBox, Point, VoronoiBuilder};

const MIN_FOOTPRINT: f64 = 4.0;
const MIN_HEIGHT: f64 = 4.0;
const LOT_MARGIN: f64 = 1.5;
const LOCAL_STREET_WIDTH: f64 = 8.0;
const MAX_ROAD_WIDTH: f64 = 8.0;
const MIN_ROAD_WIDTH: f64 = 1.0;

pub struct PlacedBuilding {
    pub name: String,
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub num_fields: u32,
    pub num_methods: u32,
}

pub struct PlacedDistrict {
    pub name: String,
    pub typology: String,
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
    pub polygon: Vec<(f64, f64)>,
    pub buildings: Vec<PlacedBuilding>,
}

pub struct RoadSegment {
    pub start_x: f64,
    pub start_z: f64,
    pub end_x: f64,
    pub end_z: f64,
    pub points: Vec<(f64, f64)>,
    pub width: f64,
    pub road_type: String,
}

pub struct Platform {
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
    pub polygon: Vec<(f64, f64)>,
    pub typology: String,
}

pub struct LayoutResult {
    pub districts: Vec<PlacedDistrict>,
    pub roads: Vec<RoadSegment>,
    pub platforms: Vec<Platform>,
}

struct RawBuilding {
    name: String,
    width: f64,
    height: f64,
    depth: f64,
    num_fields: u32,
    num_methods: u32,
    district_idx: usize,
    pos_x: f64,
    pos_z: f64,
}

pub struct DistrictLayoutInfo {
    pub idx: usize,
    pub seed_x: f64,
    pub seed_z: f64,
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
    pub polygon: Vec<(f64, f64)>,
}

struct DepEdge {
    source_idx: usize,
    target_idx: usize,
    weight: f64,
}

fn hash_str(name: &str, seed: u64) -> f64 {
    let mut hash: u64 = seed;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    (hash % 1000) as f64 / 1000.0
}

pub fn compute_layout(city: &CityMap) -> LayoutResult {
    let mut raw_buildings = compute_dimensions(city);

    if raw_buildings.is_empty() {
        return LayoutResult {
            districts: city.districts.iter().map(|district| PlacedDistrict {
                name: district.name.clone(),
                typology: district.typology.clone(),
                pos_x: 0.0,
                pos_z: 0.0,
                width: 0.0,
                depth: 0.0,
                polygon: Vec::new(),
                buildings: Vec::new(),
            }).collect(),
            roads: Vec::new(),
            platforms: Vec::new(),
        };
    }

    let district_infos = layout_districts_voronoi(&mut raw_buildings, city);
    let edges = resolve_dependencies(city, &raw_buildings);
    let roads = route_dependencies_agent(&raw_buildings, &edges, &district_infos);
    let platforms = generate_platforms(&raw_buildings, city, &district_infos);

    package_result(city, &raw_buildings, &district_infos, roads, platforms)
}

fn compute_dimensions(city: &CityMap) -> Vec<RawBuilding> {
    let mut buildings = Vec::new();

    for (district_idx, district) in city.districts.iter().enumerate() {
        for building in &district.buildings {
            let raw_side = MIN_FOOTPRINT + (building.num_fields as f64).powf(0.55) * 2.2;
            let side = if raw_side > 24.0 { 24.0 + (raw_side - 24.0).powf(0.5) } else { raw_side };

            let raw_height = if building.num_methods > 0 {
                MIN_HEIGHT + (building.num_methods as f64).powf(0.72) * 3.6
            } else if building.lines_of_code > 0 {
                MIN_HEIGHT + (building.lines_of_code as f64).powf(0.55) * 0.3
            } else {
                MIN_HEIGHT
            };
            let height = if raw_height > 55.0 { 55.0 + (raw_height - 55.0).powf(0.55) } else { raw_height };

            let min_side = height * 0.22 + 2.5;
            let final_side = if side < min_side { min_side } else { side };

            let hash = hash_str(&building.name, 10);
            let aspect = 0.7 + (hash * 0.6);
            let width = final_side * aspect;
            let depth = final_side / aspect;

            buildings.push(RawBuilding {
                name: building.name.clone(),
                width,
                height,
                depth,
                num_fields: building.num_fields,
                num_methods: building.num_methods,
                district_idx,
                pos_x: 0.0,
                pos_z: 0.0,
            });
        }
    }
    buildings
}

/// Computes organic seeds, generates Voronoi cells, and places building footprints
/// organically inside the Voronoi polygon of each district.
fn layout_districts_voronoi(buildings: &mut [RawBuilding], city: &CityMap) -> Vec<DistrictLayoutInfo> {
    let num_districts = city.districts.len();
    let mut district_to_buildings: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, building) in buildings.iter().enumerate() {
        district_to_buildings.entry(building.district_idx).or_default().push(idx);
    }

    // 1. Calculate estimated radius for each district based on building footprints
    let mut district_radii = Vec::with_capacity(num_districts);
    for district_idx in 0..num_districts {
        let building_indices = district_to_buildings.get(&district_idx);
        let total_area: f64 = match building_indices {
            Some(indices) if !indices.is_empty() => {
                indices.iter().map(|&idx| {
                    let b = &buildings[idx];
                    (b.width + LOCAL_STREET_WIDTH) * (b.depth + LOCAL_STREET_WIDTH)
                }).sum()
            }
            _ => 400.0,
        };
        let radius = (total_area / std::f64::consts::PI).sqrt() * 1.5 + 24.0;
        district_radii.push(radius);
    }

    // 2. Generate organic seed points using golden spiral with relaxation
    let mut seeds: Vec<(f64, f64)> = Vec::with_capacity(num_districts);
    let avg_radius = district_radii.iter().sum::<f64>() / (num_districts.max(1) as f64);
    for i in 0..num_districts {
        if i == 0 {
            seeds.push((0.0, 0.0));
        } else {
            let hash = hash_str(&city.districts[i].name, 42);
            let angle = (i as f64) * 2.399963229728653 + hash * 0.35;
            let dist = (i as f64).sqrt() * (avg_radius * 1.8) + hash * 6.0;
            seeds.push((dist * angle.cos(), dist * angle.sin()));
        }
    }

    // 3. Relaxation pass: prevent seed crowding while preserving organic European clustering
    if num_districts > 1 {
        for _ in 0..30 {
            for i in 0..num_districts {
                for j in (i + 1)..num_districts {
                    let dx = seeds[j].0 - seeds[i].0;
                    let dz = seeds[j].1 - seeds[i].1;
                    let dist = (dx * dx + dz * dz).sqrt().max(0.1);
                    let min_dist = (district_radii[i] + district_radii[j]) * 1.05;
                    if dist < min_dist {
                        let overlap = min_dist - dist;
                        let nx = dx / dist;
                        let nz = dz / dist;
                        seeds[i].0 -= nx * overlap * 0.45;
                        seeds[i].1 -= nz * overlap * 0.45;
                        seeds[j].0 += nx * overlap * 0.45;
                        seeds[j].1 += nz * overlap * 0.45;
                    }
                }
            }
        }
    }

    // 4. Compute Voronoi cells using voronoice
    let max_extent = seeds.iter().enumerate()
        .map(|(i, (x, z))| (x * x + z * z).sqrt() + district_radii[i])
        .fold(0.0_f64, f64::max);
    let box_size = (max_extent * 2.6).max(350.0);

    // Anchor sites outside the district cluster guarantee closed cells for all districts
    let mut sites: Vec<Point> = seeds.iter()
        .map(|&(x, z)| Point { x, y: z })
        .collect();

    let anchor_count = 8;
    let anchor_radius = box_size * 0.48;
    for k in 0..anchor_count {
        let a = (k as f64) * 2.0 * std::f64::consts::PI / (anchor_count as f64);
        sites.push(Point {
            x: anchor_radius * a.cos(),
            y: anchor_radius * a.sin(),
        });
    }

    let voronoi = VoronoiBuilder::default()
        .set_sites(sites)
        .set_bounding_box(BoundingBox::new_centered(box_size, box_size))
        .set_lloyd_relaxation_iterations(1)
        .build();

    let mut district_infos = Vec::with_capacity(num_districts);

    for i in 0..num_districts {
        let (sx, sz) = seeds[i];
        let mut poly: Vec<(f64, f64)> = if let Some(ref v) = voronoi {
            v.cell(i).iter_vertices().map(|p| (p.x, p.y)).collect()
        } else {
            let r = district_radii[i];
            (0..8).map(|k| {
                let a = (k as f64) * 2.0 * std::f64::consts::PI / 8.0;
                (sx + r * a.cos(), sz + r * a.sin())
            }).collect()
        };

        // Ensure CCW orientation around seed
        poly.sort_by(|a, b| {
            let angle_a = (a.1 - sz).atan2(a.0 - sx);
            let angle_b = (b.1 - sz).atan2(b.0 - sx);
            angle_a.partial_cmp(&angle_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut min_x = f64::MAX;
        let mut min_z = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_z = f64::MIN;
        for &(vx, vz) in &poly {
            if vx < min_x { min_x = vx; }
            if vz < min_z { min_z = vz; }
            if vx > max_x { max_x = vx; }
            if vz > max_z { max_z = vz; }
        }

        // 5. Organic Polygon Lot Packing
        // Place building footprints inside the Voronoi polygon along concentric arcs
        if let Some(building_indices) = district_to_buildings.get(&i) {
            let mut sorted_b = building_indices.clone();
            sorted_b.sort_by(|&a, &b| {
                let score_a = (buildings[a].num_methods * 3 + buildings[a].num_fields) as f64 + buildings[a].height;
                let score_b = (buildings[b].num_methods * 3 + buildings[b].num_fields) as f64 + buildings[b].height;
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut placed_lots: Vec<(f64, f64, f64, f64)> = Vec::new();

            for &b_idx in &sorted_b {
                let bw = buildings[b_idx].width;
                let bd = buildings[b_idx].depth;
                let lot_w = bw + LOT_MARGIN * 2.0;
                let lot_d = bd + LOT_MARGIN * 2.0;

                let mut placed = false;

                // Center spot for the primary building
                if placed_lots.is_empty() && is_rect_in_polygon(sx, sz, lot_w, lot_d, &poly) {
                    buildings[b_idx].pos_x = sx - bw / 2.0;
                    buildings[b_idx].pos_z = sz - bd / 2.0;
                    placed_lots.push((sx, sz, lot_w, lot_d));
                    placed = true;
                }

                if !placed {
                    let r_step = (bw.max(bd) + LOCAL_STREET_WIDTH) * 0.95;
                    let mut ring_r = 12.0;
                    let max_r = district_radii[i] * 1.5;

                    while ring_r <= max_r && !placed {
                        let circumference = 2.0 * std::f64::consts::PI * ring_r;
                        let count = ((circumference / (bw.max(bd) + LOCAL_STREET_WIDTH)).floor() as usize).max(6);
                        let ring_idx = (ring_r / r_step).floor() as usize;
                        let phase = (ring_idx as f64) * 0.61803398875 * 2.0 * std::f64::consts::PI;

                        for step in 0..count {
                            let theta = phase + (step as f64) * 2.0 * std::f64::consts::PI / (count as f64);
                            let cx = sx + ring_r * theta.cos();
                            let cz = sz + ring_r * theta.sin();

                            if !is_rect_in_polygon(cx, cz, lot_w, lot_d, &poly) {
                                continue;
                            }

                            let mut collides = false;
                            for &(px, pz, pw, pd) in &placed_lots {
                                if (cx - px).abs() < (lot_w + pw) * 0.5 && (cz - pz).abs() < (lot_d + pd) * 0.5 {
                                    collides = true;
                                    break;
                                }
                            }

                            if !collides {
                                buildings[b_idx].pos_x = cx - bw / 2.0;
                                buildings[b_idx].pos_z = cz - bd / 2.0;
                                placed_lots.push((cx, cz, lot_w, lot_d));
                                placed = true;
                                break;
                            }
                        }
                        ring_r += r_step;
                    }
                }

                // Fallback placement if cell is dense: place with reduced radius near seed
                if !placed {
                    let hash = hash_str(&buildings[b_idx].name, 88);
                    let angle = hash * 2.0 * std::f64::consts::PI;
                    let dist = 8.0 + (hash * 0.5) * district_radii[i];
                    let cx = sx + dist * angle.cos();
                    let cz = sz + dist * angle.sin();
                    buildings[b_idx].pos_x = cx - bw / 2.0;
                    buildings[b_idx].pos_z = cz - bd / 2.0;
                    placed_lots.push((cx, cz, lot_w, lot_d));
                }
            }
        }

        district_infos.push(DistrictLayoutInfo {
            idx: i,
            seed_x: sx,
            seed_z: sz,
            min_x,
            min_z,
            max_x,
            max_z,
            polygon: poly,
        });
    }

    district_infos
}

fn is_point_in_convex_poly(px: f64, pz: f64, poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return true;
    }
    for i in 0..n {
        let (x1, z1) = poly[i];
        let (x2, z2) = poly[(i + 1) % n];
        let cross = (x2 - x1) * (pz - z1) - (z2 - z1) * (px - x1);
        if cross < -0.01 {
            return false;
        }
    }
    true
}

fn is_rect_in_polygon(cx: f64, cz: f64, w: f64, d: f64, poly: &[(f64, f64)]) -> bool {
    let hw = w * 0.5;
    let hd = d * 0.5;
    is_point_in_convex_poly(cx - hw, cz - hd, poly)
        && is_point_in_convex_poly(cx + hw, cz - hd, poly)
        && is_point_in_convex_poly(cx + hw, cz + hd, poly)
        && is_point_in_convex_poly(cx - hw, cz + hd, poly)
}

fn resolve_dependencies(city: &CityMap, _buildings: &[RawBuilding]) -> Vec<DepEdge> {
    let mut name_to_idx = HashMap::new();
    let mut file_to_idx = HashMap::new();
    let mut flat_idx = 0;

    for district in &city.districts {
        for building in &district.buildings {
            name_to_idx.insert(&building.name, flat_idx);
            if !building.source_file.is_empty() {
                file_to_idx.insert(&building.source_file, flat_idx);
            }
            flat_idx += 1;
        }
    }

    let mut edges = Vec::new();
    for dep in &city.dependencies {
        let src_idx = name_to_idx.get(&dep.source).or_else(|| file_to_idx.get(&dep.source));
        let tgt_idx = name_to_idx.get(&dep.target).or_else(|| file_to_idx.get(&dep.target));

        if let (Some(&source_idx), Some(&target_idx)) = (src_idx, tgt_idx) {
            if source_idx != target_idx {
                edges.push(DepEdge {
                    source_idx,
                    target_idx,
                    weight: (dep.weight as f64).max(1.0),
                });
            }
        }
    }
    edges
}

/// Agent-based pathfinding: dependency traffic navigates between Voronoi boundaries
/// and within organic districts, forming continuous curved splines.
fn route_dependencies_agent(
    buildings: &[RawBuilding],
    edges: &[DepEdge],
    district_infos: &[DistrictLayoutInfo],
) -> Vec<RoadSegment> {
    let mut roads = Vec::with_capacity(edges.len() + district_infos.len());

    // 1. Dependency-driven roads
    for edge in edges {
        let src = &buildings[edge.source_idx];
        let tgt = &buildings[edge.target_idx];

        let p_src = (src.pos_x + src.width * 0.5, src.pos_z + src.depth * 0.5);
        let p_tgt = (tgt.pos_x + tgt.width * 0.5, tgt.pos_z + tgt.depth * 0.5);

        let d_src = src.district_idx;
        let d_tgt = tgt.district_idx;

        let mut waypoints: Vec<(f64, f64)> = Vec::new();
        waypoints.push(p_src);

        if d_src == d_tgt && d_src < district_infos.len() {
            // Intra-district curve around the central district seed
            let seed = (district_infos[d_src].seed_x, district_infos[d_src].seed_z);
            let mid_x = (p_src.0 + p_tgt.0) * 0.5;
            let mid_z = (p_src.1 + p_tgt.1) * 0.5;
            let vx = mid_x - seed.0;
            let vz = mid_z - seed.1;
            let len = (vx * vx + vz * vz).sqrt().max(1.0);
            let curved_mid = (mid_x + (vx / len) * 6.0, mid_z + (vz / len) * 6.0);
            waypoints.push(curved_mid);
        } else if d_src < district_infos.len() && d_tgt < district_infos.len() {
            // Inter-district navigation along the Voronoi boundary corridor
            let s_src = (district_infos[d_src].seed_x, district_infos[d_src].seed_z);
            let s_tgt = (district_infos[d_tgt].seed_x, district_infos[d_tgt].seed_z);

            // Boundary corridor midpoint between the two district seeds
            let b_mid = ((s_src.0 + s_tgt.0) * 0.5, (s_src.1 + s_tgt.1) * 0.5);

            // Intermediate feeder waypoints heading out to and coming in from the boundary
            let w_src = (p_src.0 * 0.45 + b_mid.0 * 0.55, p_src.1 * 0.45 + b_mid.1 * 0.55);
            let w_tgt = (b_mid.0 * 0.55 + p_tgt.0 * 0.45, b_mid.1 * 0.55 + p_tgt.1 * 0.45);

            waypoints.push(w_src);
            waypoints.push(b_mid);
            waypoints.push(w_tgt);
        } else {
            let mid = ((p_src.0 + p_tgt.0) * 0.5, (p_src.1 + p_tgt.1) * 0.5);
            waypoints.push(mid);
        }

        waypoints.push(p_tgt);

        // Smooth waypoints into a continuous organic curved spline using Chaikin's algorithm
        let smooth_curve = smooth_path(&waypoints, 3);
        let width = (MIN_ROAD_WIDTH + edge.weight.ln().max(0.0) * 1.5).min(MAX_ROAD_WIDTH);

        roads.push(RoadSegment {
            start_x: p_src.0,
            start_z: p_src.1,
            end_x: p_tgt.0,
            end_z: p_tgt.1,
            points: smooth_curve,
            width,
            road_type: "dependency".to_string(),
        });
    }

    // 2. Shared Voronoi boundary boulevards (arterial streets outlining the European districts)
    for i in 0..district_infos.len() {
        for j in (i + 1)..district_infos.len() {
            let poly_i = &district_infos[i].polygon;
            let poly_j = &district_infos[j].polygon;

            for e_i in 0..poly_i.len() {
                let p1 = poly_i[e_i];
                let p2 = poly_i[(e_i + 1) % poly_i.len()];

                for e_j in 0..poly_j.len() {
                    let q1 = poly_j[e_j];
                    let q2 = poly_j[(e_j + 1) % poly_j.len()];

                    let d1 = (p1.0 - q2.0).hypot(p1.1 - q2.1) + (p2.0 - q1.0).hypot(p2.1 - q1.1);
                    let d2 = (p1.0 - q1.0).hypot(p1.1 - q1.1) + (p2.0 - q2.0).hypot(p2.1 - q2.1);

                    if d1 < 2.0 || d2 < 2.0 {
                        roads.push(RoadSegment {
                            start_x: p1.0,
                            start_z: p1.1,
                            end_x: p2.0,
                            end_z: p2.1,
                            points: vec![p1, p2],
                            width: 3.5,
                            road_type: "arterial".to_string(),
                        });
                    }
                }
            }
        }
    }

    roads
}

/// Chaikin's corner-cutting subdivision algorithm for organic curved roads
fn smooth_path(points: &[(f64, f64)], iterations: usize) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut current = points.to_vec();
    for _ in 0..iterations {
        let mut next = Vec::with_capacity(current.len() * 2);
        next.push(current[0]);
        for i in 0..(current.len() - 1) {
            let p0 = current[i];
            let p1 = current[i + 1];
            let q = (0.75 * p0.0 + 0.25 * p1.0, 0.75 * p0.1 + 0.25 * p1.1);
            let r = (0.25 * p0.0 + 0.75 * p1.0, 0.25 * p0.1 + 0.75 * p1.1);
            next.push(q);
            next.push(r);
        }
        next.push(*current.last().unwrap());
        current = next;
    }
    current
}

fn generate_platforms(
    buildings: &[RawBuilding],
    city: &CityMap,
    district_infos: &[DistrictLayoutInfo],
) -> Vec<Platform> {
    let mut platforms: Vec<Platform> = buildings.iter().map(|building| {
        Platform {
            pos_x: building.pos_x - LOT_MARGIN,
            pos_z: building.pos_z - LOT_MARGIN,
            width: building.width + LOT_MARGIN * 2.0,
            depth: building.depth + LOT_MARGIN * 2.0,
            polygon: Vec::new(),
            typology: city.districts[building.district_idx].typology.clone(),
        }
    }).collect();

    for info in district_infos {
        if !info.polygon.is_empty() {
            platforms.push(Platform {
                pos_x: info.min_x,
                pos_z: info.min_z,
                width: info.max_x - info.min_x,
                depth: info.max_z - info.min_z,
                polygon: info.polygon.clone(),
                typology: "district_base".to_string(),
            });
        }
    }

    platforms
}

fn package_result(
    city: &CityMap,
    buildings: &[RawBuilding],
    district_infos: &[DistrictLayoutInfo],
    roads: Vec<RoadSegment>,
    platforms: Vec<Platform>,
) -> LayoutResult {
    let mut district_buildings: Vec<Vec<PlacedBuilding>> = city.districts.iter().map(|_| Vec::new()).collect();

    for building in buildings {
        district_buildings[building.district_idx].push(PlacedBuilding {
            name: building.name.clone(),
            pos_x: building.pos_x,
            pos_z: building.pos_z,
            width: building.width,
            height: building.height,
            depth: building.depth,
            num_fields: building.num_fields,
            num_methods: building.num_methods,
        });
    }

    let districts = city.districts.iter().enumerate().map(|(index, district)| {
        let info = &district_infos[index];
        PlacedDistrict {
            name: district.name.clone(),
            typology: district.typology.clone(),
            pos_x: info.min_x,
            pos_z: info.min_z,
            width: info.max_x - info.min_x,
            depth: info.max_z - info.min_z,
            polygon: info.polygon.clone(),
            buildings: std::mem::take(&mut district_buildings[index]),
        }
    }).collect();

    LayoutResult { districts, roads, platforms }
}

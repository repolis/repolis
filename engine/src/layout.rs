use crate::data::CityMap;
use std::collections::HashMap;

const MIN_FOOTPRINT: f64 = 4.0;
const MIN_HEIGHT: f64 = 4.0;
const LOT_MARGIN: f64 = 1.5;
const LOCAL_STREET_WIDTH: f64 = 8.0;
const ARTERIAL_STREET_WIDTH: f64 = 16.0;
const MAX_ROAD_WIDTH: f64 = 8.0;
const MIN_ROAD_WIDTH: f64 = 1.0;
const GRID_STEP: f64 = 4.0; 

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
    pub buildings: Vec<PlacedBuilding>,
}

pub struct RoadSegment {
    pub start_x: f64,
    pub start_z: f64,
    pub end_x: f64,
    pub end_z: f64,
    pub width: f64,
    pub road_type: String,
}

pub struct Platform {
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
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

struct BBox {
    x: f64,
    z: f64,
    width: f64,
    depth: f64,
}

struct DepEdge {
    source_idx: usize,
    target_idx: usize,
    weight: f64,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct Seg(i64, i64, i64, i64);

fn hash_str(name: &str, seed: u64) -> f64 {
    let mut hash: u64 = seed;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    (hash % 1000) as f64 / 1000.0
}

fn round_to_grid(val: f64, step: f64) -> f64 {
    (val / step).round() * step
}

fn normalize_seg(x1: f64, z1: f64, x2: f64, z2: f64) -> Seg {
    let pts = [
        (x1 * 100.0).round() as i64, 
        (z1 * 100.0).round() as i64, 
        (x2 * 100.0).round() as i64, 
        (z2 * 100.0).round() as i64
    ];
    if pts[0] > pts[2] || (pts[0] == pts[2] && pts[1] > pts[3]) {
        Seg(pts[2], pts[3], pts[0], pts[1])
    } else {
        Seg(pts[0], pts[1], pts[2], pts[3])
    }
}

fn add_segment(traffic: &mut HashMap<Seg, f64>, x1: f64, z1: f64, x2: f64, z2: f64, weight: f64) {
    if (x1 - x2).abs() < 0.01 && (z1 - z2).abs() < 0.01 {
        return;
    }
    let seg = normalize_seg(x1, z1, x2, z2);
    *traffic.entry(seg).or_insert(0.0) += weight;
}

pub fn compute_layout(city: &CityMap) -> LayoutResult {
    let mut raw_buildings = compute_dimensions(city);

    if raw_buildings.is_empty() {
        return LayoutResult {
            districts: city.districts.iter().map(|district| PlacedDistrict {
                name: district.name.clone(), 
                typology: district.typology.clone(),
                pos_x: 0.0, pos_z: 0.0, width: 0.0, depth: 0.0,
                buildings: Vec::new(),
            }).collect(),
            roads: Vec::new(), platforms: Vec::new(),
        };
    }

    layout_districts(&mut raw_buildings, city);
    let district_boxes = layout_city(&mut raw_buildings, city);
    let edges = resolve_dependencies(city, &raw_buildings);
    let roads = route_dependencies(&raw_buildings, &edges);
    let platforms = generate_platforms(&raw_buildings, city, &district_boxes);

    package_result(city, &raw_buildings, &district_boxes, roads, platforms)
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
            } else { MIN_HEIGHT };
            let height = if raw_height > 55.0 { 55.0 + (raw_height - 55.0).powf(0.55) } else { raw_height };

            let min_side = height * 0.22 + 2.5;
            let final_side = if side < min_side { min_side } else { side };

            let hash = hash_str(&building.name, 10);
            let aspect = 0.7 + (hash * 0.6);
            let width = final_side * aspect;
            let depth = final_side / aspect;

            buildings.push(RawBuilding {
                name: building.name.clone(),
                width, height, depth,
                num_fields: building.num_fields,
                num_methods: building.num_methods,
                district_idx,
                pos_x: 0.0, pos_z: 0.0,
            });
        }
    }
    buildings
}

fn layout_districts(buildings: &mut [RawBuilding], city: &CityMap) {
    let mut district_to_buildings: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, building) in buildings.iter().enumerate() {
        district_to_buildings.entry(building.district_idx).or_default().push(idx);
    }

    for district_idx in 0..city.districts.len() {
        if let Some(building_indices) = district_to_buildings.get(&district_idx) {
            let total_area: f64 = building_indices.iter().map(|&idx| {
                let building = &buildings[idx];
                (building.width + LOCAL_STREET_WIDTH) * (building.depth + LOCAL_STREET_WIDTH)
            }).sum();

            let target_width = total_area.sqrt() * 1.5;

            let mut sorted_buildings = building_indices.clone();
            sorted_buildings.sort_by(|&i, &j| {
                let hash_i = hash_str(&buildings[i].name, 1);
                let hash_j = hash_str(&buildings[j].name, 1);
                hash_i.partial_cmp(&hash_j).unwrap()
            });

            let mut rows: Vec<Vec<usize>> = Vec::new();
            let mut current_row = Vec::new();
            let mut current_x = 0.0;

            for &idx in &sorted_buildings {
                let building = &buildings[idx];
                let extra_x = hash_str(&building.name, 2) * 6.0;

                if current_x + building.width + extra_x > target_width && !current_row.is_empty() {
                    rows.push(current_row);
                    current_row = Vec::new();
                    current_x = 0.0;
                }
                current_row.push(idx);
                current_x += building.width + LOCAL_STREET_WIDTH + extra_x;
            }
            if !current_row.is_empty() { rows.push(current_row); }

            let mut current_z = 0.0;
            for row in rows {
                let row_depth = row.iter().map(|&idx| buildings[idx].depth).fold(0.0, f64::max);
                let mut current_x = 0.0;
                for &idx in &row {
                    let building = &mut buildings[idx];
                    let extra_x = hash_str(&building.name, 2) * 6.0;
                    current_x += extra_x;

                    let slack_z = row_depth - building.depth;
                    let z_offset = slack_z * hash_str(&building.name, 3);

                    building.pos_x = current_x;
                    building.pos_z = current_z + z_offset;
                    current_x += building.width + LOCAL_STREET_WIDTH;
                }
                current_z += row_depth + LOCAL_STREET_WIDTH;
            }
        }
    }
}

fn layout_city(buildings: &mut [RawBuilding], city: &CityMap) -> Vec<BBox> {
    let mut district_boxes: Vec<BBox> = (0..city.districts.len())
        .map(|_| BBox { x: 0.0, z: 0.0, width: 0.0, depth: 0.0 })
        .collect();
    
    let mut total_city_area = 0.0;
    let mut district_to_buildings: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, building) in buildings.iter().enumerate() {
        district_to_buildings.entry(building.district_idx).or_default().push(idx);
    }
    
    for district_idx in 0..city.districts.len() {
        if let Some(building_indices) = district_to_buildings.get(&district_idx) {
            let mut min_x = f64::MAX; 
            let mut min_z = f64::MAX;
            let mut max_x = 0.0; 
            let mut max_z = 0.0;
            
            for &idx in building_indices {
                let building = &buildings[idx];
                if building.pos_x < min_x { min_x = building.pos_x; }
                if building.pos_z < min_z { min_z = building.pos_z; }
                if building.pos_x + building.width > max_x { max_x = building.pos_x + building.width; }
                if building.pos_z + building.depth > max_z { max_z = building.pos_z + building.depth; }
            }
            
            district_boxes[district_idx].width = max_x - min_x;
            district_boxes[district_idx].depth = max_z - min_z;
            
            for &idx in building_indices {
                buildings[idx].pos_x -= min_x;
                buildings[idx].pos_z -= min_z;
            }
            total_city_area += (district_boxes[district_idx].width + ARTERIAL_STREET_WIDTH) * (district_boxes[district_idx].depth + ARTERIAL_STREET_WIDTH);
        }
    }

    let target_city_width = total_city_area.sqrt() * 1.3;
    
    let mut district_indices: Vec<usize> = (0..city.districts.len()).collect();
    district_indices.sort_by(|&i, &j| {
        hash_str(&city.districts[i].name, 4).partial_cmp(&hash_str(&city.districts[j].name, 4)).unwrap()
    });

    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut current_row = Vec::new();
    let mut current_x = 0.0;
    
    for &district_idx in &district_indices {
        let bbox = &district_boxes[district_idx];
        let extra_x = hash_str(&city.districts[district_idx].name, 5) * 10.0;

        if current_x + bbox.width + extra_x > target_city_width && !current_row.is_empty() {
            rows.push(current_row);
            current_row = Vec::new();
            current_x = 0.0;
        }
        current_row.push(district_idx);
        current_x += bbox.width + ARTERIAL_STREET_WIDTH + extra_x;
    }
    if !current_row.is_empty() { rows.push(current_row); }

    let mut current_z = 0.0;
    for row in rows {
        let row_depth = row.iter().map(|&idx| district_boxes[idx].depth).fold(0.0, f64::max);
        let mut current_x = 0.0;
        for &district_idx in &row {
            let bbox = &mut district_boxes[district_idx];
            let extra_x = hash_str(&city.districts[district_idx].name, 5) * 10.0;
            current_x += extra_x;

            let slack_z = row_depth - bbox.depth;
            let z_offset = slack_z * hash_str(&city.districts[district_idx].name, 6);

            bbox.x = current_x;
            bbox.z = current_z + z_offset;

            if let Some(building_indices) = district_to_buildings.get(&district_idx) {
                for &idx in building_indices {
                    buildings[idx].pos_x += bbox.x;
                    buildings[idx].pos_z += bbox.z;
                }
            }
            current_x += bbox.width + ARTERIAL_STREET_WIDTH;
        }
        current_z += row_depth + ARTERIAL_STREET_WIDTH;
    }
    
    district_boxes
}

fn resolve_dependencies(city: &CityMap, buildings: &[RawBuilding]) -> Vec<DepEdge> {
    let mut name_to_idx = HashMap::new();
    let mut file_to_idx = HashMap::new();
    let mut flat_idx = 0;

    for district in &city.districts {
        for building in &district.buildings {
            name_to_idx.insert(&building.name, flat_idx);
            if !building.source_file.is_empty() { file_to_idx.insert(&building.source_file, flat_idx); }
            flat_idx += 1;
        }
    }

    let mut edges = Vec::new();
    for dep in &city.dependencies {
        let src_idx = name_to_idx.get(&dep.source).or_else(|| file_to_idx.get(&dep.source));
        let tgt_idx = name_to_idx.get(&dep.target).or_else(|| file_to_idx.get(&dep.target));

        if let (Some(&source_idx), Some(&target_idx)) = (src_idx, tgt_idx) {
            if source_idx != target_idx { 
                edges.push(DepEdge { source_idx, target_idx, weight: (dep.weight as f64).max(1.0) }); 
            }
        }
    }
    edges
}

fn route_dependencies(buildings: &[RawBuilding], edges: &[DepEdge]) -> Vec<RoadSegment> {
    let mut traffic = HashMap::new();
    
    for (index, edge) in edges.iter().enumerate() {
        let source = &buildings[edge.source_idx];
        let target = &buildings[edge.target_idx];
        
        let source_x = source.pos_x + source.width / 2.0;
        let source_z = source.pos_z + source.depth / 2.0;
        
        let target_x = target.pos_x + target.width / 2.0;
        let target_z = target.pos_z + target.depth / 2.0;
        
        let grid_source_x = round_to_grid(source_x, GRID_STEP);
        let grid_source_z = round_to_grid(source_z, GRID_STEP);
        let grid_target_x = round_to_grid(target_x, GRID_STEP);
        let grid_target_z = round_to_grid(target_z, GRID_STEP);
        
        let weight = edge.weight;
        
        add_segment(&mut traffic, source_x, source_z, grid_source_x, source_z, weight);
        add_segment(&mut traffic, grid_source_x, source_z, grid_source_x, grid_source_z, weight);
        
        if index % 2 == 0 {
            let mut current_x = grid_source_x;
            while (current_x - grid_target_x).abs() > 0.01 {
                let next_x = if current_x < grid_target_x { (current_x + GRID_STEP).min(grid_target_x) } else { (current_x - GRID_STEP).max(grid_target_x) };
                add_segment(&mut traffic, current_x, grid_source_z, next_x, grid_source_z, weight);
                current_x = next_x;
            }
            let mut current_z = grid_source_z;
            while (current_z - grid_target_z).abs() > 0.01 {
                let next_z = if current_z < grid_target_z { (current_z + GRID_STEP).min(grid_target_z) } else { (current_z - GRID_STEP).max(grid_target_z) };
                add_segment(&mut traffic, grid_target_x, current_z, grid_target_x, next_z, weight);
                current_z = next_z;
            }
        } else {
            let mut current_z = grid_source_z;
            while (current_z - grid_target_z).abs() > 0.01 {
                let next_z = if current_z < grid_target_z { (current_z + GRID_STEP).min(grid_target_z) } else { (current_z - GRID_STEP).max(grid_target_z) };
                add_segment(&mut traffic, grid_source_x, current_z, grid_source_x, next_z, weight);
                current_z = next_z;
            }
            let mut current_x = grid_source_x;
            while (current_x - grid_target_x).abs() > 0.01 {
                let next_x = if current_x < grid_target_x { (current_x + GRID_STEP).min(grid_target_x) } else { (current_x - GRID_STEP).max(grid_target_x) };
                add_segment(&mut traffic, current_x, grid_target_z, next_x, grid_target_z, weight);
                current_x = next_x;
            }
        }
        
        add_segment(&mut traffic, grid_target_x, grid_target_z, target_x, grid_target_z, weight);
        add_segment(&mut traffic, target_x, grid_target_z, target_x, target_z, weight);
    }
    
    traffic.into_iter().map(|(Seg(x1, z1, x2, z2), weight)| {
        let width = (MIN_ROAD_WIDTH + weight.ln().max(0.0) * 1.5).min(MAX_ROAD_WIDTH);
        RoadSegment {
            start_x: x1 as f64 / 100.0, start_z: z1 as f64 / 100.0,
            end_x: x2 as f64 / 100.0, end_z: z2 as f64 / 100.0,
            width, road_type: "dependency".to_string(),
        }
    }).collect()
}

fn generate_platforms(buildings: &[RawBuilding], city: &CityMap, district_boxes: &[BBox]) -> Vec<Platform> {
    let mut platforms: Vec<Platform> = buildings.iter().map(|building| {
        Platform {
            pos_x: building.pos_x - LOT_MARGIN, 
            pos_z: building.pos_z - LOT_MARGIN,
            width: building.width + LOT_MARGIN * 2.0, 
            depth: building.depth + LOT_MARGIN * 2.0,
            typology: city.districts[building.district_idx].typology.clone(),
        }
    }).collect();

    for bbox in district_boxes {
        let pad = LOT_MARGIN * 2.0;
        if bbox.width > 0.0 && bbox.depth > 0.0 {
            platforms.push(Platform {
                pos_x: bbox.x - pad,
                pos_z: bbox.z - pad,
                width: bbox.width + pad * 2.0,
                depth: bbox.depth + pad * 2.0,
                typology: "district_base".to_string(),
            });
        }
    }

    platforms
}

fn package_result(
    city: &CityMap, buildings: &[RawBuilding], district_boxes: &[BBox],
    roads: Vec<RoadSegment>, platforms: Vec<Platform>,
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
        PlacedDistrict {
            name: district.name.clone(), 
            typology: district.typology.clone(),
            pos_x: district_boxes[index].x, 
            pos_z: district_boxes[index].z,
            width: district_boxes[index].width, 
            depth: district_boxes[index].depth,
            buildings: std::mem::take(&mut district_buildings[index]),
        }
    }).collect();

    LayoutResult { districts, roads, platforms }
}

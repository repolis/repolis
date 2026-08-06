// ═══════════════════════════════════════════════════════════════════
// Layout Engine — Exact-Fit Urban Block Subdivision & Street Grids
// ═══════════════════════════════════════════════════════════════════
//
// Combines an organic radial arterial network between neighborhoods with
// an orthogonal Manhattan-style urban street grid within each neighborhood.
//
// Algorithm overview:
//   1. Compute architecturally proportioned building dimensions.
//   2. Build a folder tree from buildings' source_file paths.
//   3. For each neighborhood (folder node):
//      - Group buildings into Urban Blocks (up to 6 buildings per block).
//      - Perform exact lot packing inside each block with sidewalk margins.
//      - Arrange blocks in an orthogonal grid separated by urban streets.
//   4. Compute subtree weights and radial positions for neighborhoods.
//   5. Emit raised Sidewalk Platforms under each urban block.
//   6. Generate internal street grids and connecting arterial highways.
//
// References:
//   - Wettel & Lanza (2007), "Visualizing Software Systems as Cities"
//   - Reingold & Tilford (1981), "Tidier Drawings of Trees"

use crate::data::CityMap;
use std::collections::BTreeMap;

// ─── Tuning constants ────────────────────────────────────────────

const MIN_FOOTPRINT: f64 = 3.0;       // minimum building base side
const MIN_HEIGHT: f64 = 3.0;          // minimum building height
const BUILDING_GAP: f64 = 1.5;        // alley gap between buildings in a block
const SIDEWALK_MARGIN: f64 = 2.0;     // curb margin around block perimeter
const STREET_WIDTH: f64 = 7.0;        // width of urban streets between blocks
const BLOCK_CAPACITY: usize = 6;      // maximum buildings per city block
const BASE_ROAD_LENGTH: f64 = 15.0;   // open highway gap between neighborhoods
const CHILD_CONE_HALF: f64 = 1.0472;  // ±60° spread for non-root children (π/3)

// ─── Output structures ──────────────────────────────────────────

/// A building with computed world-space position and dimensions.
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

/// A district with computed world-space bounding box.
pub struct PlacedDistrict {
    pub name: String,
    pub typology: String,
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
    pub buildings: Vec<PlacedBuilding>,
}

/// A road segment (street or highway) in world space.
pub struct RoadSegment {
    pub start_x: f64,
    pub start_z: f64,
    pub end_x: f64,
    pub end_z: f64,
    pub width: f64,
}

/// A raised sidewalk platform beneath an urban block.
pub struct Platform {
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
    pub typology: String,
}

/// The full layout result consumed by the Bevy renderer.
pub struct LayoutResult {
    pub districts: Vec<PlacedDistrict>,
    pub roads: Vec<RoadSegment>,
    pub platforms: Vec<Platform>,
}

// ─── Internal structures ─────────────────────────────────────────

struct RawBuilding {
    name: String,
    width: f64,
    height: f64,
    depth: f64,
    num_fields: u32,
    num_methods: u32,
    district_idx: usize,
}

#[derive(Clone)]
struct UrbanBlock {
    building_indices: Vec<usize>,
    width: f64,
    depth: f64,
    local_b_pos: Vec<(f64, f64)>, // local (x, z) relative to block corner
    typology: String,
}

struct FolderNode {
    #[allow(dead_code)]
    path: String,
    children: Vec<usize>,
    building_indices: Vec<usize>,
    parent: Option<usize>,

    weight: f64,
    count: usize,
    grid_radius: f64,
    x: f64,
    z: f64,

    // Neighborhood grid structure
    blocks: Vec<UrbanBlock>,
    nh_width: f64,
    nh_depth: f64,
    nh_cols: usize,
    nh_rows: usize,
    col_widths: Vec<f64>,
    row_depths: Vec<f64>,
}

// ─── Public API ──────────────────────────────────────────────────

pub fn compute_layout(city: &CityMap) -> LayoutResult {
    // ── Phase 1: Compute building dimensions ─────────────────
    let mut raw_buildings: Vec<RawBuilding> = Vec::new();

    for (di, district) in city.districts.iter().enumerate() {
        for b in &district.buildings {
            let raw_side = MIN_FOOTPRINT + (b.num_fields as f64).powf(0.55) * 2.2;
            let side = if raw_side > 24.0 {
                24.0 + (raw_side - 24.0).powf(0.5)
            } else {
                raw_side
            };

            let raw_height = if b.num_methods > 0 {
                MIN_HEIGHT + (b.num_methods as f64).powf(0.72) * 3.6
            } else if b.lines_of_code > 0 {
                MIN_HEIGHT + (b.lines_of_code as f64).powf(0.55) * 0.3
            } else {
                MIN_HEIGHT
            };
            let height = if raw_height > 55.0 {
                55.0 + (raw_height - 55.0).powf(0.55)
            } else {
                raw_height
            };

            // Enforce stable architectural base for towers
            let min_side = height * 0.22 + 2.5;
            let final_side = if side < min_side { min_side } else { side };

            raw_buildings.push(RawBuilding {
                name: b.name.clone(),
                width: final_side,
                height,
                depth: final_side,
                num_fields: b.num_fields,
                num_methods: b.num_methods,
                district_idx: di,
            });
        }
    }

    if raw_buildings.is_empty() {
        return LayoutResult {
            districts: city
                .districts
                .iter()
                .map(|d| PlacedDistrict {
                    name: d.name.clone(),
                    typology: d.typology.clone(),
                    pos_x: 0.0,
                    pos_z: 0.0,
                    width: 0.0,
                    depth: 0.0,
                    buildings: Vec::new(),
                })
                .collect(),
            roads: Vec::new(),
            platforms: Vec::new(),
        };
    }

    // ── Phase 2: Build folder tree ───────────────────────────
    let mut nodes = build_folder_tree(&raw_buildings, city);

    // ── Phase 3: Compute neighborhood urban block layouts ────
    compute_neighborhood_layouts(&mut nodes, &raw_buildings, city);

    // ── Phase 4: Compute subtree weights ─────────────────────
    compute_weights(&mut nodes, &raw_buildings, 0);

    // ── Phase 5: Radial arterial layout ──────────────────────
    nodes[0].x = 0.0;
    nodes[0].z = 0.0;
    radial_layout(&mut nodes, 0, 0.0, std::f64::consts::TAU);

    // ── Phase 6: Place blocks, sidewalks, and streets ────────
    let mut placed_positions: Vec<(f64, f64)> = vec![(0.0, 0.0); raw_buildings.len()];
    let mut platforms: Vec<Platform> = Vec::new();
    let mut roads: Vec<RoadSegment> = Vec::new();
    place_neighborhood_elements(&nodes, &mut placed_positions, &mut platforms, &mut roads);

    // ── Phase 7: Generate arterial highways ──────────────────
    let mut arterial_roads = generate_arterial_roads(&nodes);
    roads.append(&mut arterial_roads);

    // ── Phase 8: Package into districts ──────────────────────
    let mut district_buildings: Vec<Vec<PlacedBuilding>> =
        city.districts.iter().map(|_| Vec::new()).collect();

    for (bi, raw) in raw_buildings.iter().enumerate() {
        let (px, pz) = placed_positions[bi];
        district_buildings[raw.district_idx].push(PlacedBuilding {
            name: raw.name.clone(),
            pos_x: px,
            pos_z: pz,
            width: raw.width,
            height: raw.height,
            depth: raw.depth,
            num_fields: raw.num_fields,
            num_methods: raw.num_methods,
        });
    }

    let districts: Vec<PlacedDistrict> = city
        .districts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let buildings = &district_buildings[i];
            let (min_x, min_z, max_x, max_z) = if buildings.is_empty() {
                (0.0, 0.0, 0.0, 0.0)
            } else {
                let mut mnx = f64::MAX;
                let mut mnz = f64::MAX;
                let mut mxx = f64::MIN;
                let mut mxz = f64::MIN;
                for b in buildings {
                    if b.pos_x < mnx { mnx = b.pos_x; }
                    if b.pos_z < mnz { mnz = b.pos_z; }
                    if b.pos_x + b.width > mxx { mxx = b.pos_x + b.width; }
                    if b.pos_z + b.depth > mxz { mxz = b.pos_z + b.depth; }
                }
                (mnx, mnz, mxx, mxz)
            };

            PlacedDistrict {
                name: d.name.clone(),
                typology: d.typology.clone(),
                pos_x: min_x,
                pos_z: min_z,
                width: max_x - min_x,
                depth: max_z - min_z,
                buildings: std::mem::take(&mut district_buildings[i]),
            }
        })
        .collect();

    LayoutResult {
        districts,
        roads,
        platforms,
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 2: Folder Tree Construction
// ═══════════════════════════════════════════════════════════════════

fn extract_folder(source_file: &str) -> String {
    let s = source_file.trim_start_matches("./").trim_start_matches('/');
    if let Some(pos) = s.rfind('/') {
        s[..pos].to_string()
    } else {
        String::new()
    }
}

fn build_folder_tree(buildings: &[RawBuilding], city: &CityMap) -> Vec<FolderNode> {
    let mut folder_set: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    let mut flat_idx = 0;
    for district in &city.districts {
        for b in &district.buildings {
            let folder = extract_folder(&b.source_file);
            folder_set.entry(folder).or_default().push(flat_idx);
            flat_idx += 1;
        }
    }

    let mut nodes: Vec<FolderNode> = Vec::new();
    nodes.push(FolderNode {
        path: String::new(),
        children: Vec::new(),
        building_indices: Vec::new(),
        parent: None,
        weight: 0.0,
        count: 0,
        grid_radius: 0.0,
        x: 0.0,
        z: 0.0,
        blocks: Vec::new(),
        nh_width: 0.0,
        nh_depth: 0.0,
        nh_cols: 0,
        nh_rows: 0,
        col_widths: Vec::new(),
        row_depths: Vec::new(),
    });

    let mut path_to_node: BTreeMap<String, usize> = BTreeMap::new();
    path_to_node.insert(String::new(), 0);

    let folder_paths: Vec<String> = folder_set.keys().cloned().collect();
    for folder_path in &folder_paths {
        ensure_path(&mut nodes, &mut path_to_node, folder_path);
    }

    for (folder_path, b_indices) in &folder_set {
        if let Some(&node_idx) = path_to_node.get(folder_path) {
            for &bi in b_indices {
                nodes[node_idx].building_indices.push(bi);
            }
        }
    }

    nodes
}

fn ensure_path(
    nodes: &mut Vec<FolderNode>,
    path_to_node: &mut BTreeMap<String, usize>,
    path: &str,
) -> usize {
    if let Some(&idx) = path_to_node.get(path) {
        return idx;
    }

    let parent_path = if let Some(pos) = path.rfind('/') {
        &path[..pos]
    } else {
        ""
    };

    let parent_idx = ensure_path(nodes, path_to_node, parent_path);

    let new_idx = nodes.len();
    nodes.push(FolderNode {
        path: path.to_string(),
        children: Vec::new(),
        building_indices: Vec::new(),
        parent: Some(parent_idx),
        weight: 0.0,
        count: 0,
        grid_radius: 0.0,
        x: 0.0,
        z: 0.0,
        blocks: Vec::new(),
        nh_width: 0.0,
        nh_depth: 0.0,
        nh_cols: 0,
        nh_rows: 0,
        col_widths: Vec::new(),
        row_depths: Vec::new(),
    });
    nodes[parent_idx].children.push(new_idx);
    path_to_node.insert(path.to_string(), new_idx);

    new_idx
}

// ═══════════════════════════════════════════════════════════════════
// Phase 3: Neighborhood Urban Block Layouts & Lot Packing
// ═══════════════════════════════════════════════════════════════════

fn compute_neighborhood_layouts(
    nodes: &mut [FolderNode],
    buildings: &[RawBuilding],
    city: &CityMap,
) {
    for node in nodes {
        if node.building_indices.is_empty() {
            continue;
        }

        // Sort buildings by district index then volume descending for cohesive downtown skyscrapers
        let mut sorted: Vec<usize> = node.building_indices.clone();
        sorted.sort_by(|&a, &b| {
            let raw_a = &buildings[a];
            let raw_b = &buildings[b];
            raw_a.district_idx.cmp(&raw_b.district_idx).then_with(|| {
                let vol_a = (raw_a.width * raw_a.depth * raw_a.height) as i64;
                let vol_b = (raw_b.width * raw_b.depth * raw_b.height) as i64;
                vol_b.cmp(&vol_a)
            })
        });

        // 1. Pack buildings into discrete Urban Blocks
        let mut blocks = Vec::new();
        for chunk in sorted.chunks(BLOCK_CAPACITY) {
            let n = chunk.len();
            let cols = match n {
                1 => 1,
                2 => 2,
                4 => 2,
                _ => 3.min(n),
            };
            let rows = (n + cols - 1) / cols;

            let mut c_w: Vec<f64> = vec![0.0; cols];
            let mut c_d: Vec<f64> = vec![0.0; rows];

            for (i, &bi) in chunk.iter().enumerate() {
                let c = i % cols;
                let r = i / cols;
                c_w[c] = c_w[c].max(buildings[bi].width);
                c_d[r] = c_d[r].max(buildings[bi].depth);
            }

            let block_w = SIDEWALK_MARGIN * 2.0
                + c_w.iter().sum::<f64>()
                + (cols.saturating_sub(1) as f64) * BUILDING_GAP;
            let block_d = SIDEWALK_MARGIN * 2.0
                + c_d.iter().sum::<f64>()
                + (rows.saturating_sub(1) as f64) * BUILDING_GAP;

            let mut local_b_pos = Vec::with_capacity(n);
            for (i, &bi) in chunk.iter().enumerate() {
                let c = i % cols;
                let r = i / cols;
                let start_x = SIDEWALK_MARGIN
                    + c_w[0..c].iter().sum::<f64>()
                    + c as f64 * BUILDING_GAP;
                let start_z = SIDEWALK_MARGIN
                    + c_d[0..r].iter().sum::<f64>()
                    + r as f64 * BUILDING_GAP;

                let offset_x = (c_w[c] - buildings[bi].width) / 2.0;
                let offset_z = (c_d[r] - buildings[bi].depth) / 2.0;

                local_b_pos.push((start_x + offset_x, start_z + offset_z));
            }

            let first_district_idx = buildings[chunk[0]].district_idx;
            let typology = city.districts[first_district_idx].typology.clone();

            blocks.push(UrbanBlock {
                building_indices: chunk.to_vec(),
                width: block_w,
                depth: block_d,
                local_b_pos,
                typology,
            });
        }

        // 2. Arrange blocks into an orthogonal neighborhood grid
        let num_blocks = blocks.len();
        let nh_cols = ((num_blocks as f64).sqrt().ceil() as usize).max(1);
        let nh_rows = (num_blocks + nh_cols - 1) / nh_cols;

        let mut nh_col_w: Vec<f64> = vec![0.0; nh_cols];
        let mut nh_row_d: Vec<f64> = vec![0.0; nh_rows];

        for (i, blk) in blocks.iter().enumerate() {
            let c = i % nh_cols;
            let r = i / nh_cols;
            nh_col_w[c] = nh_col_w[c].max(blk.width);
            nh_row_d[r] = nh_row_d[r].max(blk.depth);
        }

        let nh_w = nh_col_w.iter().sum::<f64>() + (nh_cols + 1) as f64 * STREET_WIDTH;
        let nh_d = nh_row_d.iter().sum::<f64>() + (nh_rows + 1) as f64 * STREET_WIDTH;

        node.blocks = blocks;
        node.nh_width = nh_w;
        node.nh_depth = nh_d;
        node.nh_cols = nh_cols;
        node.nh_rows = nh_rows;
        node.col_widths = nh_col_w;
        node.row_depths = nh_row_d;
        node.grid_radius = (nh_w * nh_w + nh_d * nh_d).sqrt() / 2.0;
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 4: Subtree Weight Computation
// ═══════════════════════════════════════════════════════════════════

fn compute_weights(nodes: &mut Vec<FolderNode>, buildings: &[RawBuilding], idx: usize) {
    let children: Vec<usize> = nodes[idx].children.clone();

    let mut weight: f64 = 0.0;
    let mut count: usize = 0;

    for &bi in &nodes[idx].building_indices {
        weight += buildings[bi].width * buildings[bi].depth;
        count += 1;
    }

    for child_idx in children {
        compute_weights(nodes, buildings, child_idx);
        weight += nodes[child_idx].weight;
        count += nodes[child_idx].count;
    }

    nodes[idx].weight = weight;
    nodes[idx].count = count;
}

// ═══════════════════════════════════════════════════════════════════
// Phase 5: Recursive Radial Layout
// ═══════════════════════════════════════════════════════════════════

fn radial_layout(
    nodes: &mut Vec<FolderNode>,
    idx: usize,
    start_angle: f64,
    sweep: f64,
) {
    let children: Vec<usize> = nodes[idx].children.clone();
    if children.is_empty() {
        return;
    }

    let parent_x = nodes[idx].x;
    let parent_z = nodes[idx].z;
    let parent_radius = nodes[idx].grid_radius;

    let total_weight: f64 = children
        .iter()
        .map(|&ci| nodes[ci].weight.max(1.0))
        .sum();

    let mut current_angle = start_angle;

    for &child_idx in &children {
        let child_weight = nodes[child_idx].weight.max(1.0);
        let fraction = child_weight / total_weight;
        let child_sweep = sweep * fraction;
        let mid_angle = current_angle + child_sweep / 2.0;

        let child_radius = nodes[child_idx].grid_radius;
        let dist = BASE_ROAD_LENGTH + parent_radius + child_radius * 1.15;

        nodes[child_idx].x = parent_x + mid_angle.cos() * dist;
        nodes[child_idx].z = parent_z + mid_angle.sin() * dist;

        let child_start = mid_angle - CHILD_CONE_HALF;
        let child_sweep_inner = CHILD_CONE_HALF * 2.0;
        radial_layout(nodes, child_idx, child_start, child_sweep_inner);

        current_angle += child_sweep;
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 6: Place Sidewalks, Internal Street Grids, and Buildings
// ═══════════════════════════════════════════════════════════════════

fn place_neighborhood_elements(
    nodes: &[FolderNode],
    positions: &mut [(f64, f64)],
    platforms: &mut Vec<Platform>,
    roads: &mut Vec<RoadSegment>,
) {
    for node in nodes {
        if node.blocks.is_empty() {
            continue;
        }

        let left_x = -node.nh_width / 2.0;
        let bottom_z = -node.nh_depth / 2.0;

        // 1. Generate orthogonal internal street grid (Avenues and Streets)
        // Vertical street avenues along depth
        let mut curr_x = left_x;
        for c in 0..=node.nh_cols {
            let center_x = node.x + curr_x + STREET_WIDTH / 2.0;
            roads.push(RoadSegment {
                start_x: center_x,
                start_z: node.z - node.nh_depth / 2.0,
                end_x: center_x,
                end_z: node.z + node.nh_depth / 2.0,
                width: STREET_WIDTH,
            });
            if c < node.nh_cols {
                curr_x += STREET_WIDTH + node.col_widths[c];
            }
        }

        // Horizontal street avenues along width
        let mut curr_z = bottom_z;
        for r in 0..=node.nh_rows {
            let center_z = node.z + curr_z + STREET_WIDTH / 2.0;
            roads.push(RoadSegment {
                start_x: node.x - node.nh_width / 2.0,
                start_z: center_z,
                end_x: node.x + node.nh_width / 2.0,
                end_z: center_z,
                width: STREET_WIDTH,
            });
            if r < node.nh_rows {
                curr_z += STREET_WIDTH + node.row_depths[r];
            }
        }

        // 2. Place urban blocks, raised sidewalk platforms, and buildings
        for (i, blk) in node.blocks.iter().enumerate() {
            let c = i % node.nh_cols;
            let r = i / node.nh_cols;

            let lot_start_x = left_x
                + STREET_WIDTH * (c + 1) as f64
                + node.col_widths[0..c].iter().sum::<f64>();
            let lot_start_z = bottom_z
                + STREET_WIDTH * (r + 1) as f64
                + node.row_depths[0..r].iter().sum::<f64>();

            let offset_x = (node.col_widths[c] - blk.width) / 2.0;
            let offset_z = (node.row_depths[r] - blk.depth) / 2.0;

            let blk_world_x = node.x + lot_start_x + offset_x;
            let blk_world_z = node.z + lot_start_z + offset_z;

            platforms.push(Platform {
                pos_x: blk_world_x,
                pos_z: blk_world_z,
                width: blk.width,
                depth: blk.depth,
                typology: blk.typology.clone(),
            });

            for (bi_idx, &bi) in blk.building_indices.iter().enumerate() {
                let (lbx, lbz) = blk.local_b_pos[bi_idx];
                positions[bi] = (blk_world_x + lbx, blk_world_z + lbz);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 7: Arterial Spine Highways
// ═══════════════════════════════════════════════════════════════════

fn generate_arterial_roads(nodes: &[FolderNode]) -> Vec<RoadSegment> {
    let mut roads = Vec::new();

    for node in nodes {
        if let Some(parent_idx) = node.parent {
            let parent = &nodes[parent_idx];
            // Prominent arterial highway sizing
            let width = 8.0 + (node.count as f64 + 1.0).log2() * 2.0;
            roads.push(RoadSegment {
                start_x: parent.x,
                start_z: parent.z,
                end_x: node.x,
                end_z: node.z,
                width,
            });
        }
    }

    roads
}

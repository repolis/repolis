// ═══════════════════════════════════════════════════════════════════
// Layout Engine
// ═══════════════════════════════════════════════════════════════════
//
// Two-tier spatial layout for CodeCity visualization:
//
//   Micro-layout (Skyline Bin-Packing):
//     Packs buildings within each district using the Skyline
//     Bottom-Left algorithm. Maintains a stepped contour of placed
//     rectangles and greedily fills the lowest available position.
//
//   Macro-layout (Manhattan Grid):
//     Arranges districts in a regular grid separated by fixed-width
//     road gaps, like city blocks on a street grid.
//
// References:
//   - Wettel & Lanza (2007), "Visualizing Software Systems as Cities"
//   - Jylänki (2010), "A Thousand Ways to Pack the Bin"

use crate::data::CityMap;

// ─── Tuning constants ────────────────────────────────────────────

const MIN_FOOTPRINT: f64 = 2.0;   // minimum building base side
const FIELD_SCALE: f64 = 1.5;     // each field adds this to the base
const MIN_HEIGHT: f64 = 1.0;       // minimum building height
const METHOD_SCALE: f64 = 2.0;     // each method adds this to height
const LOC_HEIGHT_SCALE: f64 = 0.05; // LOC fallback for orphan buildings

const BUILDING_GAP: f64 = 1.5;    // padding between buildings
const DISTRICT_MARGIN: f64 = 3.0; // padding inside district edges
const ROAD_WIDTH: f64 = 10.0;     // gap between districts

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

/// A district with computed world-space position and dimensions.
pub struct PlacedDistrict {
    pub name: String,
    pub typology: String,
    pub pos_x: f64,
    pub pos_z: f64,
    pub width: f64,
    pub depth: f64,
    pub buildings: Vec<PlacedBuilding>,
}

/// The full layout result consumed by the renderer.
pub struct LayoutResult {
    pub districts: Vec<PlacedDistrict>,
}

// ─── Public API ──────────────────────────────────────────────────

/// Runs the full layout pipeline on a CityMap.
///
/// 1. Compute building dimensions from code metrics
/// 2. Pack buildings inside each district (Skyline)
/// 3. Arrange districts on a city-wide grid (Manhattan)
pub fn compute_layout(city: &CityMap) -> LayoutResult {
    let mut districts: Vec<PlacedDistrict> = city
        .districts
        .iter()
        .map(|d| {
            let buildings: Vec<PlacedBuilding> = d
                .buildings
                .iter()
                .map(|b| {
                    let raw_side = MIN_FOOTPRINT + (b.num_fields as f64) * FIELD_SCALE;
                    let side = if raw_side > 30.0 {
                        30.0 + (raw_side - 30.0).powf(0.6)
                    } else {
                        raw_side
                    };

                    let raw_height = if b.num_methods > 0 {
                        MIN_HEIGHT + (b.num_methods as f64) * METHOD_SCALE
                    } else if b.lines_of_code > 0 {
                        MIN_HEIGHT + (b.lines_of_code as f64) * LOC_HEIGHT_SCALE
                    } else {
                        MIN_HEIGHT
                    };
                    let height = if raw_height > 60.0 {
                        60.0 + (raw_height - 60.0).powf(0.65)
                    } else {
                        raw_height
                    };

                    PlacedBuilding {
                        name: b.name.clone(),
                        pos_x: 0.0,
                        pos_z: 0.0,
                        width: side,
                        height,
                        depth: side,
                        num_fields: b.num_fields,
                        num_methods: b.num_methods,
                    }
                })
                .collect();

            PlacedDistrict {
                name: d.name.clone(),
                typology: d.typology.clone(),
                pos_x: 0.0,
                pos_z: 0.0,
                width: 0.0,
                depth: 0.0,
                buildings,
            }
        })
        .collect();

    // Phase 2: Skyline packing within each district
    for district in &mut districts {
        skyline_pack(district);
    }

    // Phase 3: Manhattan grid
    manhattan_layout(&mut districts);

    LayoutResult { districts }
}

// ═══════════════════════════════════════════════════════════════════
// Skyline Bin-Packing Algorithm
// ═══════════════════════════════════════════════════════════════════
//
// The skyline is a piecewise-constant contour representing the top
// edge of all placed rectangles:
//
//   Z (depth)
//   ↑
//   │  ┌──────┐
//   │  │  B3  │  ┌─────────┐
//   │  │      │  │   B4    │
//   │  ├──────┤  ├─────────┤  ┌─────┐
//   │  │  B1  │  │   B2    │  │ B5  │  ← skyline
//   │  │      │  │         │  │     │
//   └──┴──────┴──┴─────────┴──┴─────┴──→ X
//
// Each segment: (x_start, z_height, width)
//
// Placement (Bottom-Left heuristic):
//   For each skyline segment as a potential left anchor, compute
//   the max z across all segments the rectangle would cover.
//   Pick the anchor with the smallest such max z.

/// One horizontal piece of the skyline contour.
struct Segment {
    x: f64,
    z: f64,
    w: f64,
}

fn skyline_pack(district: &mut PlacedDistrict) {
    let n = district.buildings.len();
    if n == 0 {
        district.width = DISTRICT_MARGIN * 2.0;
        district.depth = DISTRICT_MARGIN * 2.0;
        return;
    }

    // Sort by footprint area descending — large items first
    district.buildings.sort_by(|a, b| {
        let area_a = a.width * a.depth;
        let area_b = b.width * b.depth;
        area_b.partial_cmp(&area_a).unwrap()
    });

    // Compute bin width: target a square-ish district
    let total_area: f64 = district
        .buildings
        .iter()
        .map(|b| (b.width + BUILDING_GAP) * (b.depth + BUILDING_GAP))
        .sum();
    let mut bin_width = (total_area).sqrt() * 1.2;

    // Bin must be at least as wide as the widest building
    for b in &district.buildings {
        let needed = b.width + BUILDING_GAP;
        if needed > bin_width {
            bin_width = needed;
        }
    }

    // Initialize skyline: one flat segment at z=0
    let mut skyline = vec![Segment {
        x: 0.0,
        z: 0.0,
        w: bin_width,
    }];

    let mut max_z: f64 = 0.0;

    for building in &mut district.buildings {
        let bw = building.width + BUILDING_GAP;
        let bd = building.depth + BUILDING_GAP;

        // Find the best (lowest z) position
        let (best_x, best_z) = find_best_position(&skyline, bw, bin_width);

        // Assign local position within district (offset by margin)
        building.pos_x = best_x + DISTRICT_MARGIN + BUILDING_GAP / 2.0;
        building.pos_z = best_z + DISTRICT_MARGIN + BUILDING_GAP / 2.0;

        // Update skyline
        let new_z = best_z + bd;
        skyline = update_skyline(skyline, best_x, bw, new_z);

        if new_z > max_z {
            max_z = new_z;
        }
    }

    district.width = bin_width + DISTRICT_MARGIN * 2.0;
    district.depth = max_z + DISTRICT_MARGIN * 2.0;
}

/// Scan the skyline for the position that places a rectangle of
/// width `w` at the lowest possible z.
fn find_best_position(skyline: &[Segment], w: f64, bin_width: f64) -> (f64, f64) {
    let mut best_x: f64 = 0.0;
    let mut best_z: f64 = f64::MAX;

    for (i, seg) in skyline.iter().enumerate() {
        // Would the rectangle overflow the bin?
        if seg.x + w > bin_width + 0.001 {
            continue;
        }

        // Max z across all covered segments
        let mut cover_z: f64 = 0.0;
        let mut remaining = w;
        let mut j = i;
        while j < skyline.len() && remaining > 0.001 {
            if skyline[j].z > cover_z {
                cover_z = skyline[j].z;
            }
            remaining -= skyline[j].w;
            j += 1;
        }

        // Did we cover enough width?
        if remaining > 0.001 {
            continue;
        }

        if cover_z < best_z {
            best_z = cover_z;
            best_x = seg.x;
        }
    }

    (best_x, best_z)
}

/// Update the skyline after placing a rectangle at [x, x+w] with
/// height new_z. Splits overlapping segments and merges adjacent
/// ones at the same height.
fn update_skyline(skyline: Vec<Segment>, x: f64, w: f64, new_z: f64) -> Vec<Segment> {
    let mut result: Vec<Segment> = Vec::new();
    let x_end = x + w;
    let mut placed = false;

    for seg in &skyline {
        let seg_end = seg.x + seg.w;

        // Segment entirely outside the rectangle
        if seg_end <= x + 0.001 || seg.x >= x_end - 0.001 {
            result.push(Segment {
                x: seg.x,
                z: seg.z,
                w: seg.w,
            });
            continue;
        }

        // Left remnant
        if seg.x < x - 0.001 {
            result.push(Segment {
                x: seg.x,
                z: seg.z,
                w: x - seg.x,
            });
        }

        // New segment (once)
        if !placed {
            result.push(Segment {
                x,
                z: new_z,
                w,
            });
            placed = true;
        }

        // Right remnant
        if seg_end > x_end + 0.001 {
            result.push(Segment {
                x: x_end,
                z: seg.z,
                w: seg_end - x_end,
            });
        }
    }

    merge_skyline(result)
}

/// Merge adjacent segments at the same z to keep the list compact.
fn merge_skyline(skyline: Vec<Segment>) -> Vec<Segment> {
    if skyline.len() <= 1 {
        return skyline;
    }
    let mut merged = vec![Segment {
        x: skyline[0].x,
        z: skyline[0].z,
        w: skyline[0].w,
    }];
    for seg in skyline.iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if (last.z - seg.z).abs() < 0.001 {
            last.w += seg.w;
        } else {
            merged.push(Segment {
                x: seg.x,
                z: seg.z,
                w: seg.w,
            });
        }
    }
    merged
}

// ═══════════════════════════════════════════════════════════════════
// Manhattan Grid Layout
// ═══════════════════════════════════════════════════════════════════
//
// Districts are placed in a row-major grid. Each row's height
// adapts to the tallest district in that row. Gaps between
// districts form the city's road network.
//
// ┌───────────┐          ┌─────────┐
// │ District 0│ roadGap  │District1│
// │ (largest) │◄────────►│         │
// └───────────┘          └─────────┘
//       ▲ roadGap
// ┌───────────┐          ┌─────────┐
// │ District 2│          │District3│
// └───────────┘          └─────────┘

fn manhattan_layout(districts: &mut [PlacedDistrict]) {
    let n = districts.len();
    if n == 0 {
        return;
    }

    // Sort by area descending — largest anchors top-left
    districts.sort_by(|a, b| {
        let area_a = a.width * a.depth;
        let area_b = b.width * b.depth;
        area_b.partial_cmp(&area_a).unwrap()
    });

    let cols = (n as f64).sqrt().ceil() as usize;

    let mut cur_x: f64 = 0.0;
    let mut cur_z: f64 = 0.0;
    let mut row_max_depth: f64 = 0.0;
    let mut col = 0;

    for district in districts.iter_mut() {
        district.pos_x = cur_x;
        district.pos_z = cur_z;

        // Offset buildings from district-local to world coordinates
        for building in &mut district.buildings {
            building.pos_x += cur_x;
            building.pos_z += cur_z;
        }

        cur_x += district.width + ROAD_WIDTH;
        if district.depth > row_max_depth {
            row_max_depth = district.depth;
        }

        col += 1;
        if col >= cols {
            col = 0;
            cur_x = 0.0;
            cur_z += row_max_depth + ROAD_WIDTH;
            row_max_depth = 0.0;
        }
    }
}

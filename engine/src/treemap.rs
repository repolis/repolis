//! Squarified treemap packing (Bruls, Huizing & van Wijk, 2000).
//!
//! A treemap cannot produce an overlap: it recursively subdivides a rectangle,
//! so disjointness comes from the construction rather than from a collision
//! test afterwards. It is also what the original CodeCity work used, is
//! O(n log n), and is stable - similar inputs give similar pictures.
//!
//! Rectangles live on the ground plane: `x` is world x, `y` is world z.

use bevy::math::{Rect, Vec2};

pub fn area(r: Rect) -> f32 {
    r.width() * r.height()
}

/// Shrinks a rectangle by `m` on every side, never past its centre; the band
/// removed becomes a street or an alley.
pub fn inset(r: Rect, m: f32) -> Rect {
    Rect::from_center_half_size(r.center(), (r.half_size() - m).max(Vec2::ZERO))
}

/// Packs `areas` into `bounds`, one rectangle per area, in input order. Areas
/// are scaled to fill `bounds` exactly, so only their relative size matters.
pub fn squarify(areas: &[f32], bounds: Rect) -> Vec<Rect> {
    let mut out = vec![Rect::default(); areas.len()];
    if areas.is_empty() || area(bounds) <= 0.0 {
        return out;
    }

    // Largest first; the index breaks ties so the result is deterministic.
    let mut order: Vec<usize> = (0..areas.len()).collect();
    order.sort_by(|&a, &b| areas[b].total_cmp(&areas[a]).then(a.cmp(&b)));
    let total: f32 = areas.iter().map(|a| a.max(1e-9)).sum();
    let sizes: Vec<f32> = order
        .iter()
        .map(|&i| areas[i].max(1e-9) * area(bounds) / total)
        .collect();

    let mut free = bounds;
    let mut start = 0;
    while start < order.len() {
        let side = free.width().min(free.height());
        if side <= 1e-6 {
            // Degenerate strip: zero-area rects rather than lost items.
            for &i in &order[start..] {
                out[i] = Rect::from_corners(free.min, free.min);
            }
            break;
        }

        // Grow the row while its worst aspect ratio keeps improving.
        let mut end = start + 1;
        while end < order.len()
            && worst_ratio(&sizes[start..=end], side) <= worst_ratio(&sizes[start..end], side)
        {
            end += 1;
        }

        // Lay the row as a strip across the shorter side, then shrink the
        // free space past it.
        let row_area: f32 = sizes[start..end].iter().sum();
        let vertical = free.width() >= free.height();
        let thickness = row_area
            / if vertical {
                free.height()
            } else {
                free.width()
            };
        let mut cursor = free.min;
        for k in start..end {
            let len = sizes[k] / thickness;
            let size = if vertical {
                Vec2::new(thickness, len)
            } else {
                Vec2::new(len, thickness)
            };
            out[order[k]] = Rect::from_corners(cursor, cursor + size);
            if vertical {
                cursor.y += len;
            } else {
                cursor.x += len;
            }
        }
        if vertical {
            free.min.x += thickness;
        } else {
            free.min.y += thickness;
        }
        start = end;
    }
    out
}

/// Worst (largest) aspect ratio in a row laid along `side`. The row is sorted
/// largest first.
fn worst_ratio(row: &[f32], side: f32) -> f32 {
    let sum: f32 = row.iter().sum();
    let (largest, smallest) = (row[0], row[row.len() - 1]);
    let (s2, sum2) = (side * side, sum * sum);
    (s2 * largest / sum2).max(sum2 / (s2 * smallest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: Rect, b: Rect) -> bool {
        let i = a.intersect(b);
        i.width() > 1e-3 && i.height() > 1e-3
    }

    #[test]
    fn packs_without_overlap_and_stays_inside() {
        let bounds = Rect::new(-40.0, 15.0, 80.0, 95.0);
        let areas: Vec<f32> = (0..97).map(|i| 1.0 + ((i * 37) % 53) as f32).collect();

        let placed = squarify(&areas, bounds);
        assert_eq!(placed.len(), areas.len());

        for r in &placed {
            assert!(r.min.cmpge(bounds.min - 1e-3).all() && r.max.cmple(bounds.max + 1e-3).all());
        }
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                assert!(!overlaps(placed[i], placed[j]), "rects {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn preserves_relative_area() {
        let placed = squarify(&[75.0, 25.0], Rect::new(0.0, 0.0, 100.0, 100.0));
        let ratio = area(placed[0]) / area(placed[1]);
        assert!((ratio - 3.0).abs() < 0.05, "ratio was {ratio}");
    }

    #[test]
    fn handles_empty_and_single() {
        assert!(squarify(&[], Rect::new(0.0, 0.0, 10.0, 10.0)).is_empty());
        let one = squarify(&[1.0], Rect::new(0.0, 0.0, 10.0, 20.0));
        assert!((area(one[0]) - 200.0).abs() < 1e-3);
    }
}

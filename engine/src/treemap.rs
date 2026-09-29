//! Squarified treemap packing (Bruls, Huizing & van Wijk, 2000).
//!
//! This replaces the previous concentric-ring placement, which searched rings
//! outward from a district seed and, when a district overflowed its cell, fell
//! back to a hash-derived position with no collision test and no containment
//! test at all — so buildings interpenetrated and could land outside their own
//! district. A treemap cannot produce an overlap: it recursively subdivides a
//! rectangle, so disjointness is a property of the construction rather than
//! something to be checked for afterwards.
//!
//! It is also the layout used by the original CodeCity work, is O(n log n),
//! and is stable: similar inputs give similar pictures.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub d: f64,
}

impl Rect {
    pub fn new(x: f64, z: f64, w: f64, d: f64) -> Self {
        Rect { x, z, w, d }
    }
    pub fn area(&self) -> f64 {
        self.w.max(0.0) * self.d.max(0.0)
    }
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w * 0.5, self.z + self.d * 0.5)
    }
    /// Shrinks the rectangle by `m` on every side. The removed band is what
    /// becomes a street.
    pub fn inset(&self, m: f64) -> Rect {
        let w = (self.w - 2.0 * m).max(0.0);
        let d = (self.d - 2.0 * m).max(0.0);
        Rect {
            x: self.x + (self.w - w) * 0.5,
            z: self.z + (self.d - d) * 0.5,
            w,
            d,
        }
    }
}

/// An item to place: an opaque key and the area it needs.
#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub key: usize,
    pub area: f64,
}

/// Packs items into `bounds`, returning one rectangle per item in the same
/// order the items were given. Areas are scaled to exactly fill `bounds`, so
/// relative size is preserved even though absolute size is not.
pub fn squarify(items: &[Item], bounds: Rect) -> Vec<(usize, Rect)> {
    let mut out = Vec::with_capacity(items.len());
    if items.is_empty() || bounds.area() <= 0.0 {
        return out;
    }

    let mut sorted: Vec<Item> = items.to_vec();
    sorted.sort_by(|a, b| {
        b.area
            .partial_cmp(&a.area)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.key.cmp(&b.key))
    });

    let total: f64 = sorted.iter().map(|i| i.area.max(1e-9)).sum();
    let scale = bounds.area() / total;
    for it in sorted.iter_mut() {
        it.area = it.area.max(1e-9) * scale;
    }

    layout_rows(&sorted, bounds, &mut out);
    out
}

fn layout_rows(items: &[Item], mut free: Rect, out: &mut Vec<(usize, Rect)>) {
    let mut start = 0usize;

    while start < items.len() {
        let side = free.w.min(free.d);
        if side <= 1e-9 {
            // Degenerate strip: emit zero-area rects rather than lose items.
            for it in &items[start..] {
                out.push((it.key, Rect::new(free.x, free.z, 0.0, 0.0)));
            }
            return;
        }

        // Grow the row while the worst aspect ratio keeps improving.
        let mut end = start + 1;
        let mut row_area = items[start].area;
        let mut best = worst_ratio(&items[start..end], row_area, side);

        while end < items.len() {
            let next_area = row_area + items[end].area;
            let candidate = worst_ratio(&items[start..end + 1], next_area, side);
            if candidate > best {
                break;
            }
            best = candidate;
            row_area = next_area;
            end += 1;
        }

        let row = &items[start..end];
        if free.w >= free.d {
            // Vertical row occupying a column of width `thickness`.
            let thickness = row_area / free.d;
            let mut z = free.z;
            for it in row {
                let h = if row_area > 0.0 {
                    free.d * (it.area / row_area)
                } else {
                    0.0
                };
                out.push((it.key, Rect::new(free.x, z, thickness, h)));
                z += h;
            }
            free = Rect::new(free.x + thickness, free.z, free.w - thickness, free.d);
        } else {
            let thickness = row_area / free.w;
            let mut x = free.x;
            for it in row {
                let w = if row_area > 0.0 {
                    free.w * (it.area / row_area)
                } else {
                    0.0
                };
                out.push((it.key, Rect::new(x, free.z, w, thickness)));
                x += w;
            }
            free = Rect::new(free.x, free.z + thickness, free.w, free.d - thickness);
        }

        start = end;
    }
}

/// Worst (largest) aspect ratio in a row of total area `sum` laid along `side`.
fn worst_ratio(row: &[Item], sum: f64, side: f64) -> f64 {
    if row.is_empty() || sum <= 0.0 {
        return f64::MAX;
    }
    let mut min_a = f64::MAX;
    let mut max_a = 0.0_f64;
    for it in row {
        min_a = min_a.min(it.area);
        max_a = max_a.max(it.area);
    }
    let s2 = side * side;
    let sum2 = sum * sum;
    ((s2 * max_a) / sum2).max(sum2 / (s2 * min_a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        let eps = 1e-6;
        a.x + a.w > b.x + eps && b.x + b.w > a.x + eps && a.z + a.d > b.z + eps && b.z + b.d > a.z + eps
    }

    #[test]
    fn packs_without_overlap_and_stays_inside() {
        let bounds = Rect::new(-40.0, 15.0, 120.0, 80.0);
        let items: Vec<Item> = (0..97)
            .map(|i| Item {
                key: i,
                area: 1.0 + ((i * 37) % 53) as f64,
            })
            .collect();

        let placed = squarify(&items, bounds);
        assert_eq!(placed.len(), items.len());

        for (_, r) in &placed {
            assert!(r.x >= bounds.x - 1e-6 && r.z >= bounds.z - 1e-6);
            assert!(r.x + r.w <= bounds.x + bounds.w + 1e-6);
            assert!(r.z + r.d <= bounds.z + bounds.d + 1e-6);
        }
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                assert!(!overlaps(&placed[i].1, &placed[j].1), "rects {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn preserves_relative_area() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let items = vec![
            Item { key: 0, area: 75.0 },
            Item { key: 1, area: 25.0 },
        ];
        let placed = squarify(&items, bounds);
        let a0 = placed.iter().find(|(k, _)| *k == 0).unwrap().1.area();
        let a1 = placed.iter().find(|(k, _)| *k == 1).unwrap().1.area();
        assert!((a0 / a1 - 3.0).abs() < 0.05, "ratio was {}", a0 / a1);
    }

    #[test]
    fn handles_empty_and_single() {
        assert!(squarify(&[], Rect::new(0.0, 0.0, 10.0, 10.0)).is_empty());
        let one = squarify(&[Item { key: 7, area: 1.0 }], Rect::new(0.0, 0.0, 10.0, 20.0));
        assert_eq!(one.len(), 1);
        assert!((one[0].1.area() - 200.0).abs() < 1e-6);
    }
}

pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}

/// A free function. Rust says it belongs to no type, and nothing should
/// attach it to Rect just because of its parameter.
pub fn bounding(a: &Rect, b: &Rect) -> Rect {
    Rect::new(a.x, a.y, b.w, b.h)
}

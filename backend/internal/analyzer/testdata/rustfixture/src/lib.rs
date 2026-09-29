pub mod shapes;
mod util;

use crate::shapes::rect::Rect;
use crate::shapes::circle::{Circle, area as circle_area};
use util::clamp;

pub struct Canvas {
    width: u32,
    height: u32,
    shapes: Vec<Rect>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas { width, height, shapes: Vec::new() }
    }

    pub fn add(&mut self, r: Rect) {
        self.shapes.push(r);
    }

    pub fn total_area(&self) -> f64 {
        let mut sum = 0.0;
        for s in &self.shapes {
            sum += Rect::area(s);
        }
        sum + circle_area(1.0)
    }

    pub fn fit(&self, v: u32) -> u32 {
        clamp(v, 0, self.width)
    }
}

pub fn make_canvas() -> Canvas {
    Canvas::new(10, 10)
}

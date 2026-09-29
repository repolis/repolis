pub struct Circle {
    pub r: f64,
}

impl Circle {
    pub fn new(r: f64) -> Circle {
        Circle { r }
    }
}

pub fn area(r: f64) -> f64 {
    3.14159 * r * r
}

pub fn clamp(v: u32, lo: u32, hi: u32) -> u32 {
    if v < lo { lo } else if v > hi { hi } else { v }
}

/// Deliberately shares a name with shapes::circle::area, to prove that
/// resolution uses imports rather than bare names.
pub fn area(x: f64) -> f64 {
    x
}

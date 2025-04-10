const PI_RADIANS: f64 = (1 << 31) as f64;
const CONVERSION_FACTOR: f64 = 180.0 / PI_RADIANS;

/// Converts semicircles to degrees. It returns f64 invalid when value is invalid.
pub fn to_degrees(semicircles: i32) -> f64 {
    if semicircles == i32::MAX {
        return f64::from_bits(u64::MAX);
    }
    semicircles as f64 * CONVERSION_FACTOR
}

/// Converts degrees to semicircles. It returns i32 invalid when value is invalid.
pub fn to_semicircles(degrees: f64) -> i32 {
    if degrees.is_nan() || degrees.is_infinite() {
        return i32::MAX;
    }
    (degrees / CONVERSION_FACTOR) as i32
}

const PI_RADIANS: f64 = (1u32 << 31) as f64;
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

#[cfg(test)]
mod tests {
    use crate::semconv;

    #[test]
    fn test_to_degrees() {
        let lat: i32 = 424480360;
        assert_eq!(semconv::to_degrees(lat), 35.579532757401466);

        let long: i32 = -940295581;
        assert_eq!(semconv::to_degrees(long), -78.81466512568295);
    }

    #[test]
    fn test_to_semicircles() {
        let lat: f64 = 35.579532757401466;
        assert_eq!(semconv::to_semicircles(lat), 424480360);

        let long: f64 = -78.81466512568295;
        assert_eq!(semconv::to_semicircles(long), -940295581);
    }
}

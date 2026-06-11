const PI_RADIANS: f64 = (1u32 << 31) as f64;
const CONVERSION_FACTOR: f64 = 180.0 / PI_RADIANS;

/// Converts semicircles to degrees. It returns `None` when value is invalid.
pub fn to_degrees(semicircles: i32) -> Option<f64> {
    if semicircles == i32::MAX {
        return None;
    }
    Some(semicircles as f64 * CONVERSION_FACTOR)
}

/// Converts degrees to semicircles. It returns `None` when value is invalid.
pub fn to_semicircles(degrees: f64) -> Option<i32> {
    if degrees.is_nan() || degrees.is_infinite() {
        return None;
    }
    Some((degrees / CONVERSION_FACTOR) as i32)
}

#[cfg(test)]
mod tests {
    use std::f64;

    use crate::semconv;

    #[test]
    fn test_to_degrees() {
        let lat: i32 = 424480360;
        assert_eq!(semconv::to_degrees(lat), Some(35.579532757401466));

        let long: i32 = -940295581;
        assert_eq!(semconv::to_degrees(long), Some(-78.81466512568295));

        let invalid = i32::MAX;
        assert_eq!(semconv::to_degrees(invalid), None);
    }

    #[test]
    fn test_to_semicircles() {
        let lat: f64 = 35.579532757401466;
        assert_eq!(semconv::to_semicircles(lat), Some(424480360));

        let long: f64 = -78.81466512568295;
        assert_eq!(semconv::to_semicircles(long), Some(-940295581));

        let invalid = f64::NAN;
        assert_eq!(semconv::to_semicircles(invalid), None);
    }
}

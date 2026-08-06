use serde_json::Value;

/// The largest whole number that survives a round trip through a JSON number in
/// every consumer of a package, including JavaScript in the editor.
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Reports whether a value satisfies the `integer` type.
///
/// The rule lives here so that the manifest validator, Script Settings and the
/// runtime cannot disagree about which numbers are integers. A value outside
/// the safe range is rejected rather than silently truncated, and a float is
/// never an integer because the two types are disjoint.
pub fn is_safe_integer(value: &Value) -> bool {
    let Some(number) = value.as_number() else {
        return false;
    };
    number
        .as_i64()
        .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
        .is_some_and(|whole| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&whole))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_whole_numbers_inside_the_safe_range_only() {
        for value in [json!(0), json!(42), json!(-42), json!(MAX_SAFE_INTEGER)] {
            assert!(is_safe_integer(&value), "{value} should be an integer");
        }
        for value in [
            json!(3.7),
            json!(MAX_SAFE_INTEGER + 1),
            json!(-MAX_SAFE_INTEGER - 1),
            json!(i64::MIN),
            json!(u64::MAX),
            json!("42"),
            json!(null),
        ] {
            assert!(!is_safe_integer(&value), "{value} should not be an integer");
        }
    }
}

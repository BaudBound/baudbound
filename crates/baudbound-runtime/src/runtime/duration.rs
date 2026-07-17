use std::time::Duration;

const MINIMUM_DURATION: Duration = Duration::from_millis(1);

pub(crate) fn duration_from_amount(amount: f64, unit: &str) -> Result<Duration, String> {
    if !amount.is_finite() || amount <= 0.0 {
        return Err("duration amount must be a finite number greater than zero".to_owned());
    }

    let seconds_per_unit = match unit.trim().to_ascii_lowercase().as_str() {
        "millisecond" | "milliseconds" | "ms" => 0.001,
        "second" | "seconds" | "s" => 1.0,
        "minute" | "minutes" | "min" => 60.0,
        "hour" | "hours" | "h" => 60.0 * 60.0,
        "day" | "days" | "d" => 24.0 * 60.0 * 60.0,
        _ => return Err(format!("unsupported duration unit {unit:?}")),
    };
    let seconds = amount * seconds_per_unit;
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| "duration must fit the supported range".to_owned())?;
    if duration < MINIMUM_DURATION {
        return Err("duration must be at least one millisecond".to_owned());
    }

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_every_supported_unit_without_fallbacks() {
        for (amount, unit, expected) in [
            (1.0, "milliseconds", Duration::from_millis(1)),
            (1.0, "seconds", Duration::from_secs(1)),
            (1.0, "minutes", Duration::from_secs(60)),
            (1.0, "hours", Duration::from_secs(3_600)),
            (1.0, "days", Duration::from_secs(86_400)),
        ] {
            assert_eq!(duration_from_amount(amount, unit), Ok(expected), "{unit}");
        }
    }

    #[test]
    fn accepts_common_singular_and_short_aliases() {
        for unit in ["millisecond", "ms"] {
            assert_eq!(
                duration_from_amount(2.0, unit),
                Ok(Duration::from_millis(2))
            );
        }
        assert_eq!(duration_from_amount(2.0, "s"), Ok(Duration::from_secs(2)));
        assert_eq!(
            duration_from_amount(2.0, "min"),
            Ok(Duration::from_secs(120))
        );
        assert_eq!(
            duration_from_amount(2.0, "h"),
            Ok(Duration::from_secs(7_200))
        );
        assert_eq!(
            duration_from_amount(2.0, "d"),
            Ok(Duration::from_secs(172_800))
        );
    }

    #[test]
    fn rejects_unknown_non_finite_non_positive_and_sub_millisecond_values() {
        for (amount, unit) in [
            (1.0, "fortnights"),
            (f64::NAN, "seconds"),
            (f64::INFINITY, "seconds"),
            (0.0, "seconds"),
            (-1.0, "seconds"),
            (0.999, "milliseconds"),
        ] {
            assert!(
                duration_from_amount(amount, unit).is_err(),
                "{amount} {unit}"
            );
        }
    }

    #[test]
    fn rejects_values_larger_than_rust_duration() {
        assert!(duration_from_amount(f64::MAX, "days").is_err());
    }

    #[test]
    fn accepts_largest_representable_f64_duration_below_rust_limit() {
        let rust_limit = 2_f64.powi(64);
        let largest_supported = f64::from_bits(rust_limit.to_bits() - 1);

        assert!(duration_from_amount(largest_supported, "seconds").is_ok());
        assert!(duration_from_amount(rust_limit, "seconds").is_err());
    }
}

use serde_json::Value;

const MAX_TOTAL_RGB_DISTANCE: f64 = 441.672_955_930_063_7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorMatchEvaluation {
    pub blue_difference: u8,
    pub difference_percent: f64,
    pub green_difference: u8,
    pub matches: bool,
    pub red_difference: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorComparisonMode {
    PerChannel,
    TotalDistance,
}

impl ColorComparisonMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "per_channel" => Ok(Self::PerChannel),
            "total_distance" => Ok(Self::TotalDistance),
            other => Err(format!("comparison mode {other:?} is unsupported")),
        }
    }
}

pub fn evaluate_color_match(
    actual_value: &Value,
    expected_value: &Value,
    mode: ColorComparisonMode,
    tolerance_percent: f64,
) -> Result<ColorMatchEvaluation, String> {
    if !tolerance_percent.is_finite() || !(0.0..=100.0).contains(&tolerance_percent) {
        return Err("tolerance must be a finite percentage from 0 through 100".to_owned());
    }

    let actual = parse_rgb_color(actual_value, "actual color")?;
    let expected = parse_rgb_color(expected_value, "expected color")?;
    let red_difference = actual.red.abs_diff(expected.red);
    let green_difference = actual.green.abs_diff(expected.green);
    let blue_difference = actual.blue.abs_diff(expected.blue);
    let difference_percent = match mode {
        ColorComparisonMode::PerChannel => {
            f64::from(red_difference.max(green_difference).max(blue_difference)) / 255.0 * 100.0
        }
        ColorComparisonMode::TotalDistance => {
            let squared_distance = u32::from(red_difference).pow(2)
                + u32::from(green_difference).pow(2)
                + u32::from(blue_difference).pow(2);
            f64::from(squared_distance).sqrt() / MAX_TOTAL_RGB_DISTANCE * 100.0
        }
    };

    Ok(ColorMatchEvaluation {
        blue_difference,
        difference_percent,
        green_difference,
        matches: difference_percent <= tolerance_percent,
        red_difference,
    })
}

pub fn parse_rgb_color(value: &Value, label: &str) -> Result<RgbColor, String> {
    match value {
        Value::String(value) => parse_rgb_string(value, label),
        Value::Object(channels) => {
            if channels.len() != 3
                || !channels.contains_key("r")
                || !channels.contains_key("g")
                || !channels.contains_key("b")
            {
                return Err(format!(
                    "{label} RGB object must contain exactly r, g, and b channels"
                ));
            }
            Ok(RgbColor {
                red: parse_channel(&channels["r"], label)?,
                green: parse_channel(&channels["g"], label)?,
                blue: parse_channel(&channels["b"], label)?,
            })
        }
        _ => Err(format!(
            "{label} must be #RRGGBB, rgb(r, g, b), or an RGB object with r, g, and b channels"
        )),
    }
}

fn parse_rgb_string(value: &str, label: &str) -> Result<RgbColor, String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#')
        && hex.len() == 6
        && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Ok(RgbColor {
            red: parse_hex_channel(&hex[0..2], label)?,
            green: parse_hex_channel(&hex[2..4], label)?,
            blue: parse_hex_channel(&hex[4..6], label)?,
        });
    }

    if value.len() >= 5
        && value
            .get(..3)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rgb"))
        && let Some(channels) = value.get(3..).and_then(|value| {
            value
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))
        })
    {
        let channels = channels.split(',').map(str::trim).collect::<Vec<_>>();
        if channels.len() == 3 {
            return Ok(RgbColor {
                red: parse_decimal_channel(channels[0], label)?,
                green: parse_decimal_channel(channels[1], label)?,
                blue: parse_decimal_channel(channels[2], label)?,
            });
        }
    }

    Err(format!(
        "{label} must be #RRGGBB, rgb(r, g, b), or an RGB object with r, g, and b channels"
    ))
}

fn parse_channel(value: &Value, label: &str) -> Result<u8, String> {
    value
        .as_u64()
        .and_then(|channel| u8::try_from(channel).ok())
        .ok_or_else(|| format!("{label} channels must be integers from 0 through 255"))
}

fn parse_decimal_channel(value: &str, label: &str) -> Result<u8, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "{label} channels must be integers from 0 through 255"
        ));
    }
    value
        .parse::<u8>()
        .map_err(|_| format!("{label} channels must be integers from 0 through 255"))
}

fn parse_hex_channel(value: &str, label: &str) -> Result<u8, String> {
    u8::from_str_radix(value, 16).map_err(|_| format!("{label} contains an invalid hex channel"))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_every_supported_color_representation() {
        let expected = RgbColor {
            red: 255,
            green: 128,
            blue: 0,
        };
        for value in [
            json!("#FF8000"),
            json!("rgb(255, 128, 0)"),
            json!({"r": 255, "g": 128, "b": 0}),
        ] {
            assert_eq!(parse_rgb_color(&value, "color"), Ok(expected));
        }
    }

    #[test]
    fn rejects_ambiguous_or_out_of_range_colors() {
        for value in [
            json!("#fff"),
            json!("rgba(1, 2, 3, 1)"),
            json!("rgb(256, 0, 0)"),
            json!({"r": 1.5, "g": 0, "b": 0}),
            json!({"r": 0, "g": 0, "b": 0, "a": 255}),
        ] {
            assert!(parse_rgb_color(&value, "color").is_err(), "{value}");
        }
    }

    #[test]
    fn evaluates_both_percentage_modes_at_their_boundaries() {
        let black = json!("#000000");
        let red = json!("#FF0000");
        let per_channel =
            evaluate_color_match(&black, &red, ColorComparisonMode::PerChannel, 100.0)
                .expect("valid comparison");
        assert_eq!(per_channel.difference_percent, 100.0);
        assert!(per_channel.matches);

        let total = evaluate_color_match(
            &black,
            &red,
            ColorComparisonMode::TotalDistance,
            100.0 / 3.0_f64.sqrt(),
        )
        .expect("valid comparison");
        assert!((total.difference_percent - 100.0 / 3.0_f64.sqrt()).abs() < 1e-12);
        assert!(total.matches);

        let exact = evaluate_color_match(&black, &black, ColorComparisonMode::TotalDistance, 0.0)
            .expect("valid exact comparison");
        assert!(exact.matches);
        assert_eq!(exact.difference_percent, 0.0);
    }

    #[test]
    fn matches_the_cross_language_color_contract_cases() {
        #[derive(Deserialize)]
        struct Case {
            actual: Value,
            difference_percent: f64,
            expected: Value,
            matches: bool,
            mode: String,
            name: String,
            tolerance_percent: f64,
        }

        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../contracts/runner/color-match-cases.json"
        ))
        .expect("color match cases should be valid JSON");
        for case in cases {
            let mode =
                ColorComparisonMode::parse(&case.mode).expect("fixture mode should be valid");
            let result =
                evaluate_color_match(&case.actual, &case.expected, mode, case.tolerance_percent)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", case.name));
            assert_eq!(result.matches, case.matches, "{}", case.name);
            assert!(
                (result.difference_percent - case.difference_percent).abs() < 1e-12,
                "{} returned {} instead of {}",
                case.name,
                result.difference_percent,
                case.difference_percent
            );
        }
    }
}

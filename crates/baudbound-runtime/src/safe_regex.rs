use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

const REGEX_POLICY_JSON: &str = include_str!("../../../contracts/regex-policy.json");
const REGEX_POLICY_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct RegexPolicy {
    max_pattern_utf8_bytes: usize,
    max_simulation_input_utf8_bytes: usize,
    version: u32,
}

pub fn compile_safe_regex(pattern: &str) -> Result<Regex, String> {
    let maximum = regex_policy().max_pattern_utf8_bytes;
    if pattern.len() > maximum {
        return Err(format!("regex pattern exceeds {maximum} UTF-8 bytes"));
    }

    Regex::new(pattern).map_err(|source| format!("invalid regex pattern: {source}"))
}

#[must_use]
pub fn max_simulation_regex_input_bytes() -> usize {
    regex_policy().max_simulation_input_utf8_bytes
}

fn regex_policy() -> &'static RegexPolicy {
    static POLICY: OnceLock<RegexPolicy> = OnceLock::new();
    POLICY.get_or_init(|| {
        let policy = serde_json::from_str::<RegexPolicy>(REGEX_POLICY_JSON)
            .expect("embedded regex policy must be valid JSON");
        assert_eq!(
            policy.version, REGEX_POLICY_VERSION,
            "embedded regex policy version must be supported"
        );
        assert!(
            policy.max_pattern_utf8_bytes > 0 && policy.max_simulation_input_utf8_bytes > 0,
            "embedded regex policy limits must be positive"
        );
        policy
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{compile_safe_regex, regex_policy};

    #[derive(Deserialize)]
    struct RegexFixtures {
        condition_cases: Vec<ConditionFixture>,
        invalid_patterns: Vec<String>,
        version: u32,
    }

    #[derive(Deserialize)]
    struct ConditionFixture {
        input: String,
        matched: bool,
        name: String,
        pattern: String,
    }

    #[test]
    fn pattern_limit_is_measured_in_utf8_bytes() {
        let maximum = regex_policy().max_pattern_utf8_bytes;
        let pattern = "é".repeat(maximum / 2 + 1);
        let error = compile_safe_regex(&pattern).expect_err("UTF-8 byte overflow must be rejected");
        assert!(error.contains("UTF-8 bytes"), "{error}");
    }

    #[test]
    fn shared_regex_condition_fixtures_conform() {
        let fixtures = serde_json::from_str::<RegexFixtures>(include_str!(
            "../../../contracts/regex-conformance.json"
        ))
        .expect("shared regex fixtures must be valid JSON");
        assert_eq!(fixtures.version, 1);

        for fixture in fixtures.condition_cases {
            let regex = compile_safe_regex(&fixture.pattern)
                .unwrap_or_else(|error| panic!("{} did not compile: {error}", fixture.name));
            assert_eq!(
                regex.is_match(&fixture.input),
                fixture.matched,
                "{}",
                fixture.name
            );
        }
        for pattern in fixtures.invalid_patterns {
            assert!(
                compile_safe_regex(&pattern).is_err(),
                "invalid shared pattern compiled: {pattern}"
            );
        }
    }
}

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

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

/// Largest number of compiled patterns kept in memory.
///
/// This bounds cache memory, not what a script may do. Passing the limit evicts
/// an entry and recompiles later, so a script using an unbounded variety of
/// patterns keeps working at the original cost.
const MAX_CACHED_PATTERNS: usize = 256;

pub fn compile_safe_regex(pattern: &str) -> Result<Regex, String> {
    compile_cached_regex(pattern).map(|regex| regex.as_ref().clone())
}

/// Compiles a pattern, reusing a previous compilation where possible.
///
/// Condition evaluation compiles on every call, so a pattern inside a loop was
/// recompiled once per iteration. The regex engine is a finite automaton and
/// matching is linear, so compilation was the expensive part.
pub fn compile_cached_regex(pattern: &str) -> Result<Arc<Regex>, String> {
    let maximum = regex_policy().max_pattern_utf8_bytes;
    if pattern.len() > maximum {
        return Err(format!("regex pattern exceeds {maximum} UTF-8 bytes"));
    }

    let cache = regex_cache();
    let mut entries = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(regex) = entries.get(pattern) {
        return Ok(Arc::clone(regex));
    }

    let regex =
        Arc::new(Regex::new(pattern).map_err(|source| format!("invalid regex pattern: {source}"))?);
    if entries.len() >= MAX_CACHED_PATTERNS {
        entries.clear();
    }
    entries.insert(pattern.to_owned(), Arc::clone(&regex));
    Ok(regex)
}

fn regex_cache() -> &'static Mutex<HashMap<String, Arc<Regex>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Regex>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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

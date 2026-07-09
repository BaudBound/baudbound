use std::cmp::Ordering;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VersionCompatibilityError {
    #[error("package minimum_runner_version {value:?} is invalid; expected MAJOR.MINOR.PATCH")]
    InvalidMinimumRunnerVersion { value: String },
    #[error("runner version {value:?} is invalid; expected MAJOR.MINOR.PATCH")]
    InvalidRunnerVersion { value: String },
    #[error("package requires runner version {required}, but this runner is {actual}")]
    RunnerTooOld { actual: String, required: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Ord for SemanticVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn validate_minimum_runner_version(
    minimum_runner_version: &str,
    runner_version: &str,
) -> Result<(), VersionCompatibilityError> {
    let required = parse_package_version(minimum_runner_version)?;
    let actual = parse_runner_version(runner_version)?;

    if actual >= required {
        return Ok(());
    }

    Err(VersionCompatibilityError::RunnerTooOld {
        actual: runner_version.trim().to_owned(),
        required: minimum_runner_version.trim().to_owned(),
    })
}

fn parse_package_version(value: &str) -> Result<SemanticVersion, VersionCompatibilityError> {
    parse_version(value).ok_or_else(|| VersionCompatibilityError::InvalidMinimumRunnerVersion {
        value: value.to_owned(),
    })
}

fn parse_runner_version(value: &str) -> Result<SemanticVersion, VersionCompatibilityError> {
    parse_version(value).ok_or_else(|| VersionCompatibilityError::InvalidRunnerVersion {
        value: value.to_owned(),
    })
}

fn parse_version(value: &str) -> Option<SemanticVersion> {
    let value = value.trim().strip_prefix('v').unwrap_or(value.trim());
    let mut parts = value.split('.');
    let major = parse_part(parts.next()?)?;
    let minor = parse_part(parts.next()?)?;
    let patch = parse_part(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }

    Some(SemanticVersion {
        major,
        minor,
        patch,
    })
}

fn parse_part(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_equal_and_older_minimum_runner_versions() {
        validate_minimum_runner_version("1.2.3", "1.2.3").expect("equal version should pass");
        validate_minimum_runner_version("1.2.2", "1.2.3").expect("older patch should pass");
        validate_minimum_runner_version("1.1.9", "1.2.0").expect("older minor should pass");
        validate_minimum_runner_version("0.9.9", "1.0.0").expect("older major should pass");
    }

    #[test]
    fn accepts_leading_v_prefix_for_release_tag_compatibility() {
        validate_minimum_runner_version("v1.2.3", "1.2.3").expect("package v prefix should pass");
        validate_minimum_runner_version("1.2.3", "v1.2.3").expect("runner v prefix should pass");
    }

    #[test]
    fn rejects_newer_minimum_runner_version() {
        let error = validate_minimum_runner_version("1.2.4", "1.2.3")
            .expect_err("newer package requirement should fail");

        assert!(matches!(
            error,
            VersionCompatibilityError::RunnerTooOld { .. }
        ));
        assert_eq!(
            error.to_string(),
            "package requires runner version 1.2.4, but this runner is 1.2.3"
        );
    }

    #[test]
    fn rejects_invalid_package_version() {
        let error = validate_minimum_runner_version("1.2", "1.2.3")
            .expect_err("package version must have three numeric parts");

        assert!(matches!(
            error,
            VersionCompatibilityError::InvalidMinimumRunnerVersion { .. }
        ));
    }

    #[test]
    fn rejects_invalid_runner_version() {
        let error = validate_minimum_runner_version("1.2.3", "dev")
            .expect_err("runner version must have three numeric parts");

        assert!(matches!(
            error,
            VersionCompatibilityError::InvalidRunnerVersion { .. }
        ));
    }
}

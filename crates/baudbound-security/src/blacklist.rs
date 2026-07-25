use std::{collections::BTreeSet, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BlacklistSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl BlacklistSeverity {
    #[must_use]
    pub const fn blocks_distribution(self) -> bool {
        matches!(self, Self::Medium | Self::High | Self::Critical)
    }

    #[must_use]
    pub const fn blocks_execution(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }

    #[must_use]
    pub const fn requests_cancellation(self) -> bool {
        matches!(self, Self::Critical)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BlacklistScope {
    Repository,
    Publisher,
    Domain,
    Script,
    Package,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlacklistEntry {
    pub advisory_url: String,
    pub id: String,
    pub published_at: String,
    pub reason: String,
    pub scope: BlacklistScope,
    pub severity: BlacklistSeverity,
    #[serde(default)]
    pub subdomains: bool,
    pub target: String,
    pub title: String,
    pub updated: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlacklistMatchSubject {
    pub package_hash: Option<String>,
    pub script_id: Option<String>,
    pub trusted_urls: Vec<String>,
}

impl BlacklistMatchSubject {
    #[must_use]
    pub fn installed(script_id: impl Into<String>, package_hash: impl Into<String>) -> Self {
        Self {
            package_hash: Some(package_hash.into()),
            script_id: Some(script_id.into()),
            trusted_urls: Vec::new(),
        }
    }

    #[must_use]
    pub fn url(url: impl Into<String>) -> Self {
        Self {
            trusted_urls: vec![url.into()],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlacklistDecision {
    pub entries: Vec<BlacklistEntry>,
    pub severity: Option<BlacklistSeverity>,
}

impl BlacklistDecision {
    #[must_use]
    pub fn from_entries(mut entries: Vec<BlacklistEntry>) -> Self {
        entries.sort_by(|left, right| {
            right
                .severity
                .cmp(&left.severity)
                .then_with(|| left.id.cmp(&right.id))
        });
        entries.dedup_by(|left, right| left.id == right.id);
        let severity = entries.iter().map(|entry| entry.severity).max();
        Self { entries, severity }
    }

    #[must_use]
    pub fn blocks_distribution(&self) -> bool {
        self.severity
            .is_some_and(BlacklistSeverity::blocks_distribution)
    }

    #[must_use]
    pub fn blocks_execution(&self) -> bool {
        self.severity
            .is_some_and(BlacklistSeverity::blocks_execution)
    }

    #[must_use]
    pub fn blocks_update_source(&self) -> bool {
        self.entries.iter().any(|entry| {
            entry.scope != BlacklistScope::Package && entry.severity.blocks_distribution()
        })
    }

    #[must_use]
    pub fn requests_cancellation(&self) -> bool {
        self.severity
            .is_some_and(BlacklistSeverity::requests_cancellation)
    }
}

pub trait BlacklistPolicy: Send + Sync {
    fn decide(&self, subject: &BlacklistMatchSubject) -> BlacklistDecision;
}

#[derive(Debug, Default)]
pub struct PermissiveBlacklistPolicy;

impl BlacklistPolicy for PermissiveBlacklistPolicy {
    fn decide(&self, _subject: &BlacklistMatchSubject) -> BlacklistDecision {
        BlacklistDecision::default()
    }
}

impl<T> BlacklistPolicy for Arc<T>
where
    T: BlacklistPolicy + ?Sized,
{
    fn decide(&self, subject: &BlacklistMatchSubject) -> BlacklistDecision {
        self.as_ref().decide(subject)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlacklistValidationError {
    #[error("blacklist entry {0:?} is invalid")]
    InvalidField(&'static str),
    #[error("blacklist entry subdomains can only be enabled for domain scope")]
    InvalidSubdomains,
}

pub fn normalize_blacklist_entry(
    mut entry: BlacklistEntry,
) -> Result<BlacklistEntry, BlacklistValidationError> {
    for (name, value) in [
        ("id", entry.id.as_str()),
        ("title", entry.title.as_str()),
        ("reason", entry.reason.as_str()),
        ("published_at", entry.published_at.as_str()),
        ("updated", entry.updated.as_str()),
    ] {
        if value.trim().is_empty() || value.len() > 16 * 1024 {
            return Err(BlacklistValidationError::InvalidField(name));
        }
    }
    for (name, value) in [
        ("published_at", entry.published_at.as_str()),
        ("updated", entry.updated.as_str()),
    ] {
        if chrono::DateTime::parse_from_rfc3339(value).is_err()
            && chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.fZ").is_err()
        {
            return Err(BlacklistValidationError::InvalidField(name));
        }
    }
    let advisory = Url::parse(entry.advisory_url.trim())
        .map_err(|_| BlacklistValidationError::InvalidField("advisory_url"))?;
    if advisory.scheme() != "https"
        || advisory.host_str().is_none()
        || !advisory.username().is_empty()
        || advisory.password().is_some()
    {
        return Err(BlacklistValidationError::InvalidField("advisory_url"));
    }
    entry.advisory_url = advisory.to_string();
    entry.target = match entry.scope {
        BlacklistScope::Repository => normalize_repository_url(&entry.target)
            .map_err(|_| BlacklistValidationError::InvalidField("target"))?,
        BlacklistScope::Publisher => normalize_publisher(&entry.target)
            .ok_or(BlacklistValidationError::InvalidField("target"))?,
        BlacklistScope::Domain => normalize_domain(&entry.target)
            .ok_or(BlacklistValidationError::InvalidField("target"))?,
        BlacklistScope::Script => normalize_script_id(&entry.target)
            .ok_or(BlacklistValidationError::InvalidField("target"))?,
        BlacklistScope::Package => normalize_package_hash(&entry.target)
            .ok_or(BlacklistValidationError::InvalidField("target"))?,
    };
    if entry.scope != BlacklistScope::Domain && entry.subdomains {
        return Err(BlacklistValidationError::InvalidSubdomains);
    }
    Ok(entry)
}

#[must_use]
pub fn matching_entries(
    entries: &[BlacklistEntry],
    subject: &BlacklistMatchSubject,
) -> Vec<BlacklistEntry> {
    let script_id = subject.script_id.as_deref();
    let package_hash = subject.package_hash.as_deref().map(str::to_ascii_lowercase);
    let parsed_urls = subject
        .trusted_urls
        .iter()
        .filter_map(|value| Url::parse(value).ok())
        .collect::<Vec<_>>();
    let repository_urls = parsed_urls
        .iter()
        .filter_map(|url| normalize_repository_url(url.as_str()).ok())
        .collect::<BTreeSet<_>>();
    let hosts = parsed_urls
        .iter()
        .filter_map(normalized_url_host)
        .collect::<BTreeSet<_>>();
    let publishers = parsed_urls
        .iter()
        .filter_map(github_publisher_for_url)
        .collect::<BTreeSet<_>>();

    entries
        .iter()
        .filter(|entry| match entry.scope {
            BlacklistScope::Repository => repository_urls.contains(&entry.target),
            BlacklistScope::Publisher => publishers.contains(&entry.target),
            BlacklistScope::Domain => hosts.iter().any(|host| {
                host == &entry.target
                    || (entry.subdomains
                        && host
                            .strip_suffix(&entry.target)
                            .is_some_and(|prefix| prefix.ends_with('.')))
            }),
            BlacklistScope::Script => script_id.is_some_and(|value| value == entry.target),
            BlacklistScope::Package => package_hash
                .as_deref()
                .is_some_and(|value| value == entry.target),
        })
        .cloned()
        .collect()
}

pub fn normalize_repository_url(value: &str) -> Result<String, url::ParseError> {
    let mut url = Url::parse(value.trim())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(url::ParseError::RelativeUrlWithoutBase);
    }
    let host = url
        .host_str()
        .map(|host| host.trim_end_matches('.').to_owned());
    url.set_host(host.as_deref())?;
    if url.port() == Some(443) {
        let _ = url.set_port(None);
    }
    Ok(url.to_string())
}

#[must_use]
pub fn github_publisher_for_url(url: &Url) -> Option<String> {
    let host = normalized_url_host(url)?;
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let owner = match host.as_str() {
        "github.com"
        | "raw.githubusercontent.com"
        | "gist.github.com"
        | "gist.githubusercontent.com" => segments.first().copied(),
        "api.github.com" if segments.first() == Some(&"repos") => segments.get(1).copied(),
        host if host.ends_with(".github.io") => host.strip_suffix(".github.io"),
        _ => None,
    }?;
    normalize_github_owner(owner).map(|owner| format!("github:{owner}"))
}

fn normalize_publisher(value: &str) -> Option<String> {
    let (provider, account) = value.trim().split_once(':')?;
    if !provider.eq_ignore_ascii_case("github") {
        return None;
    }
    normalize_github_owner(account).map(|owner| format!("github:{owner}"))
}

fn normalize_github_owner(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 39
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return None;
    }
    Some(value)
}

fn normalize_domain(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('.');
    if value.is_empty() || value.contains(['/', ':', '?', '#', '@']) || value.len() > 253 {
        return None;
    }
    let url = Url::parse(&format!("https://{value}/")).ok()?;
    let host = normalized_url_host(&url)?;
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    Some(host)
}

fn normalized_url_host(url: &Url) -> Option<String> {
    url.host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
        .filter(|host| !host.is_empty())
}

fn normalize_script_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return None;
    }
    Some(value.to_owned())
}

fn normalize_package_hash(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: BlacklistScope, target: &str, subdomains: bool) -> BlacklistEntry {
        normalize_blacklist_entry(BlacklistEntry {
            advisory_url: "https://baudbound.app/advisories/test".to_owned(),
            id: format!("{scope:?}-{target}"),
            published_at: "2026-07-25 12:00:00.000Z".to_owned(),
            reason: "Test advisory".to_owned(),
            scope,
            severity: BlacklistSeverity::High,
            subdomains,
            target: target.to_owned(),
            title: "Test".to_owned(),
            updated: "2026-07-25 12:00:00.000Z".to_owned(),
        })
        .expect("entry should normalize")
    }

    #[test]
    fn exact_domain_does_not_match_descendants_or_deceptive_suffixes() {
        let entries = vec![entry(BlacklistScope::Domain, "example.com", false)];
        assert_eq!(
            matching_entries(
                &entries,
                &BlacklistMatchSubject::url("https://example.com/a")
            )
            .len(),
            1
        );
        assert!(
            matching_entries(
                &entries,
                &BlacklistMatchSubject::url("https://files.example.com/a")
            )
            .is_empty()
        );
        assert!(
            matching_entries(
                &entries,
                &BlacklistMatchSubject::url("https://example.com.attacker.test/a")
            )
            .is_empty()
        );
    }

    #[test]
    fn subdomain_matching_uses_hostname_boundaries() {
        let entries = vec![entry(BlacklistScope::Domain, "example.com", true)];
        assert_eq!(
            matching_entries(
                &entries,
                &BlacklistMatchSubject::url("https://deep.files.example.com/a")
            )
            .len(),
            1
        );
        assert!(
            matching_entries(
                &entries,
                &BlacklistMatchSubject::url("https://notexample.com/a")
            )
            .is_empty()
        );
    }

    #[test]
    fn recognizes_supported_github_publisher_urls() {
        for value in [
            "https://github.com/Some-Owner/repo",
            "https://raw.githubusercontent.com/some-owner/repo/master/file",
            "https://api.github.com/repos/some-owner/repo",
            "https://some-owner.github.io/repo",
            "https://gist.github.com/some-owner/abc",
            "https://gist.githubusercontent.com/some-owner/abc/raw/file",
        ] {
            let url = Url::parse(value).expect("URL should parse");
            assert_eq!(
                github_publisher_for_url(&url).as_deref(),
                Some("github:some-owner")
            );
        }
        assert!(
            github_publisher_for_url(
                &Url::parse("https://api.github.com/gists/abc").expect("URL should parse")
            )
            .is_none()
        );
    }

    #[test]
    fn highest_matching_severity_controls_the_decision() {
        let mut low = entry(BlacklistScope::Script, "script-1", false);
        low.severity = BlacklistSeverity::Low;
        let mut critical = entry(BlacklistScope::Package, &"a".repeat(64), false);
        critical.severity = BlacklistSeverity::Critical;
        let matches = matching_entries(
            &[low, critical],
            &BlacklistMatchSubject::installed("script-1", "a".repeat(64)),
        );
        let decision = BlacklistDecision::from_entries(matches);
        assert_eq!(decision.severity, Some(BlacklistSeverity::Critical));
        assert!(decision.requests_cancellation());
    }

    #[test]
    fn package_only_restriction_allows_a_safe_replacement_to_be_discovered() {
        let mut package = entry(BlacklistScope::Package, &"a".repeat(64), false);
        package.severity = BlacklistSeverity::Critical;
        let decision = BlacklistDecision::from_entries(vec![package]);
        assert!(decision.blocks_distribution());
        assert!(!decision.blocks_update_source());

        let mut repository = entry(
            BlacklistScope::Repository,
            "https://example.com/repository.json",
            false,
        );
        repository.severity = BlacklistSeverity::Medium;
        let decision = BlacklistDecision::from_entries(vec![repository]);
        assert!(decision.blocks_update_source());
    }
}

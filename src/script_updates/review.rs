use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use super::PreparedRemotePackage;

const REVIEW_LIFETIME: Duration = Duration::from_secs(10 * 60);
const MAX_PENDING_REVIEWS: usize = 16;

struct PendingReview {
    expires_at: Instant,
    package: PreparedRemotePackage,
}

#[derive(Default)]
pub(crate) struct RemotePackageReviews {
    pending: Mutex<HashMap<String, PendingReview>>,
}

impl RemotePackageReviews {
    pub(crate) fn insert(&self, package: PreparedRemotePackage) -> Result<String, String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "remote package review lock is poisoned".to_owned())?;
        pending.retain(|_, review| review.expires_at > Instant::now());
        if pending.len() >= MAX_PENDING_REVIEWS {
            return Err("too many remote packages are awaiting review".to_owned());
        }
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|error| format!("failed to create package review token: {error}"))?;
        let id = URL_SAFE_NO_PAD.encode(bytes);
        pending.insert(
            id.clone(),
            PendingReview {
                expires_at: Instant::now() + REVIEW_LIFETIME,
                package,
            },
        );
        Ok(id)
    }

    pub(crate) fn take(
        &self,
        review_id: &str,
        expected_sha256: &str,
    ) -> Result<PreparedRemotePackage, String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "remote package review lock is poisoned".to_owned())?;
        pending.retain(|_, review| review.expires_at > Instant::now());
        let review = pending
            .remove(review_id)
            .ok_or_else(|| "the remote package review expired or was already used".to_owned())?;
        if review.package.review.sha256 != expected_sha256 {
            return Err("the reviewed remote package hash changed".to_owned());
        }
        Ok(review.package)
    }

    pub(crate) fn discard(&self, review_id: &str) -> Result<bool, String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "remote package review lock is poisoned".to_owned())?;
        pending.retain(|_, review| review.expires_at > Instant::now());
        Ok(pending.remove(review_id).is_some())
    }
}

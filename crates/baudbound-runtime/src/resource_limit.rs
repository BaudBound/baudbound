use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// An operator-owned resource boundary. `Unlimited` means BaudBound applies no
/// policy limit; operating-system and allocation failures still surface normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    Limited(u64),
    Unlimited,
}

impl ResourceLimit {
    #[must_use]
    pub const fn limited(value: u64) -> Self {
        Self::Limited(value)
    }

    #[must_use]
    pub const fn value(self) -> Option<u64> {
        match self {
            Self::Limited(value) => Some(value),
            Self::Unlimited => None,
        }
    }

    #[must_use]
    pub const fn value_or_max(self) -> u64 {
        match self {
            Self::Limited(value) => value,
            Self::Unlimited => u64::MAX,
        }
    }

    #[must_use]
    pub const fn is_exceeded_by(self, value: u64) -> bool {
        matches!(self, Self::Limited(limit) if value > limit)
    }

    #[must_use]
    pub const fn permits(self, value: u64) -> bool {
        !self.is_exceeded_by(value)
    }
}

impl fmt::Display for ResourceLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limited(value) => value.fmt(formatter),
            Self::Unlimited => formatter.write_str("unlimited"),
        }
    }
}

impl Serialize for ResourceLimit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Limited(value) => serializer.serialize_u64(*value),
            Self::Unlimited => serializer.serialize_str("unlimited"),
        }
    }
}

impl<'de> Deserialize<'de> for ResourceLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ResourceLimitVisitor;

        impl<'de> de::Visitor<'de> for ResourceLimitVisitor {
            type Value = ResourceLimit;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-negative integer or the string \"unlimited\"")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(ResourceLimit::Limited(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u64::try_from(value)
                    .map(ResourceLimit::Limited)
                    .map_err(|_| E::custom("resource limits cannot be negative"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value == "unlimited" {
                    Ok(ResourceLimit::Unlimited)
                } else {
                    Err(E::custom(
                        "resource limit strings must be exactly \"unlimited\"",
                    ))
                }
            }
        }

        deserializer.deserialize_any(ResourceLimitVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_finite_and_unlimited_values_without_ambiguity() {
        assert_eq!(
            serde_json::to_string(&ResourceLimit::limited(7)).unwrap(),
            "7"
        );
        assert_eq!(
            serde_json::to_string(&ResourceLimit::Unlimited).unwrap(),
            "\"unlimited\""
        );
        assert_eq!(
            serde_json::from_str::<ResourceLimit>("7").unwrap(),
            ResourceLimit::limited(7)
        );
        assert_eq!(
            serde_json::from_str::<ResourceLimit>("\"unlimited\"").unwrap(),
            ResourceLimit::Unlimited
        );
    }

    #[test]
    fn rejects_negative_and_unknown_string_values() {
        assert!(serde_json::from_str::<ResourceLimit>("-1").is_err());
        assert!(serde_json::from_str::<ResourceLimit>("\"none\"").is_err());
    }
}

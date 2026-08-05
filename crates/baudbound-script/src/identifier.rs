/// Returns whether a user-controlled identifier contains only the portable
/// characters accepted by editor, package, and runner contracts.
pub fn is_user_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_shared_identifier_character_set() {
        for value in ["A", "z", "0", "Release-Channel_2", "-", "_"] {
            assert!(is_user_identifier(value), "expected {value:?} to be valid");
        }
    }

    #[test]
    fn rejects_empty_or_non_portable_identifiers() {
        for value in ["", "has space", "has.dot", "slash/name", "unicode-\u{e4}"] {
            assert!(
                !is_user_identifier(value),
                "expected {value:?} to be invalid"
            );
        }
    }
}

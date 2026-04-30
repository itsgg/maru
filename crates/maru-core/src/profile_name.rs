//! [`ProfileName`] — validated profile identifier.
//!
//! Names are constrained by the regex `[A-Za-z0-9][A-Za-z0-9_-]{0,63}` per
//! GENESIS §6 line 162. The newtype wrapper guarantees that any value of
//! type `ProfileName` is valid — there is no way to construct an invalid one
//! outside this module.

use std::fmt;

/// A validated profile identifier.
///
/// Construct via [`ProfileName::new`]. The wrapped string is guaranteed to
/// satisfy `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`.
///
/// # Examples
///
/// ```
/// use maru_core::ProfileName;
///
/// let work = ProfileName::new("work").unwrap();
/// assert_eq!(work.as_str(), "work");
///
/// // Empty names, names starting with `-`, and names containing invalid
/// // characters are rejected.
/// assert!(ProfileName::new("").is_err());
/// assert!(ProfileName::new("-bad").is_err());
/// assert!(ProfileName::new("has space").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProfileName(String);

impl ProfileName {
    /// The maximum length of a profile name in bytes (and characters, since
    /// the validation only accepts ASCII).
    pub const MAX_LEN: usize = 64;

    /// Construct a [`ProfileName`] from any string-ish input.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidName`] if the input is empty, longer than
    /// [`Self::MAX_LEN`], or contains characters outside `[A-Za-z0-9_-]`, or
    /// if the first character is not in `[A-Za-z0-9]`.
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidName> {
        let s = s.into();
        validate(&s)?;
        Ok(Self(s))
    }

    /// Borrow the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProfileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ProfileName> for String {
    fn from(name: ProfileName) -> Self {
        name.0
    }
}

impl TryFrom<String> for ProfileName {
    type Error = InvalidName;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

/// Reasons a candidate string is not a valid [`ProfileName`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidName {
    /// The candidate string was empty.
    #[error("profile name must be non-empty")]
    Empty,

    /// The candidate string exceeded [`ProfileName::MAX_LEN`] bytes.
    #[error("profile name exceeds {max} bytes (got {actual})")]
    TooLong {
        /// Actual length of the candidate.
        actual: usize,
        /// Maximum permitted length ([`ProfileName::MAX_LEN`]).
        max: usize,
    },

    /// The first character was not in `[A-Za-z0-9]`.
    #[error("profile name must start with [A-Za-z0-9], got {first:?}")]
    BadFirstChar {
        /// The offending first character.
        first: char,
    },

    /// A character other than the first was not in `[A-Za-z0-9_-]`.
    #[error("profile name contains illegal character {ch:?} at byte {index}")]
    IllegalChar {
        /// The offending character.
        ch: char,
        /// Byte index of the offending character.
        index: usize,
    },
}

fn validate(s: &str) -> Result<(), InvalidName> {
    if s.is_empty() {
        return Err(InvalidName::Empty);
    }
    if s.len() > ProfileName::MAX_LEN {
        return Err(InvalidName::TooLong {
            actual: s.len(),
            max: ProfileName::MAX_LEN,
        });
    }

    let mut chars = s.char_indices();
    let Some((_, first)) = chars.next() else {
        // Unreachable: empty case rejected above.
        return Err(InvalidName::Empty);
    };
    if !first.is_ascii_alphanumeric() {
        return Err(InvalidName::BadFirstChar { first });
    }

    for (index, ch) in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
            return Err(InvalidName::IllegalChar { ch, index });
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::as_conversions,
    reason = "tests assert correctness; convenience over discipline"
)]
mod tests {
    use super::{InvalidName, ProfileName};

    #[test]
    fn accepts_simple_names() {
        for s in [
            "a",
            "A",
            "0",
            "work",
            "WORK",
            "p1",
            "client-1",
            "personal_dev",
            "x".repeat(64).as_str(),
        ] {
            assert!(ProfileName::new(s).is_ok(), "{s:?} should be accepted");
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(ProfileName::new("").unwrap_err(), InvalidName::Empty);
    }

    #[test]
    fn rejects_too_long() {
        let s = "x".repeat(65);
        match ProfileName::new(&s).unwrap_err() {
            InvalidName::TooLong { actual, max } => {
                assert_eq!(actual, 65);
                assert_eq!(max, 64);
            }
            other => panic!("expected TooLong, got {other:?}"),
        }
    }

    #[test]
    fn rejects_bad_first_char() {
        for s in ["-bad", "_bad", " bad", ".bad"] {
            match ProfileName::new(s).unwrap_err() {
                InvalidName::BadFirstChar { .. } => {}
                other => panic!("{s:?} expected BadFirstChar, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_illegal_chars() {
        for s in ["a b", "a/b", "a.b", "a:b", "a@b"] {
            match ProfileName::new(s).unwrap_err() {
                InvalidName::IllegalChar { .. } => {}
                other => panic!("{s:?} expected IllegalChar, got {other:?}"),
            }
        }
    }

    #[test]
    fn round_trips_through_serde() {
        let name = ProfileName::new("work-1").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"work-1\"");
        let back: ProfileName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, name);
    }

    #[test]
    fn rejects_invalid_via_serde() {
        let result: Result<ProfileName, _> = serde_json::from_str("\"-bad\"");
        assert!(result.is_err());
    }

    #[test]
    fn display_and_as_ref() {
        let name = ProfileName::new("client").unwrap();
        assert_eq!(name.to_string(), "client");
        let r: &str = name.as_ref();
        assert_eq!(r, "client");
        assert_eq!(name.as_str(), "client");
    }
}

//! Secret token validation and redacted formatting.

use std::fmt;

use serde::{Deserialize, Deserializer};
use thiserror::Error;

const REDACTED: &str = "<redacted-token>";

/// Validated bearer token whose `Debug` and `Display` forms are always redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretToken(String);

impl SecretToken {
    /// Creates a token after rejecting empty values and header-breaking controls.
    pub fn new(value: impl Into<String>) -> Result<Self, SecretTokenError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SecretTokenError::Empty);
        }
        if value.chars().any(char::is_control) {
            return Err(SecretTokenError::ContainsControlCharacter);
        }
        Ok(Self(value))
    }

    pub(crate) fn expose(&self) -> &str {
        // Raw access stays crate-private so only the transport boundary can
        // place a credential into the Authorization header.
        &self.0
    }

    pub(crate) fn redact(&self, value: &str) -> String {
        value.replace(&self.0, REDACTED)
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretToken(<redacted>)")
    }
}

impl fmt::Display for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

impl<'de> Deserialize<'de> for SecretToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validation failures for secret bearer-token input.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretTokenError {
    /// The supplied token is empty or whitespace-only.
    #[error("token must not be empty")]
    Empty,
    /// The token contains a control character unsafe for an HTTP header.
    #[error("token must not contain control characters")]
    ContainsControlCharacter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_debug_and_display_are_redacted() {
        let token = SecretToken::new("secret-value").expect("valid token");

        assert!(!format!("{token:?}").contains("secret-value"));
        assert!(!token.to_string().contains("secret-value"));
        assert_eq!(token.redact("error secret-value"), "error <redacted-token>");
    }
}

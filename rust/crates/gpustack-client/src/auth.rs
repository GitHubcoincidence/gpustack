//! Secret token validation and redacted formatting.

use std::{fmt, io};

use serde::{Deserialize, Deserializer};
use thiserror::Error;

const REDACTED: &str = "<redacted-token>";

#[derive(Clone, PartialEq, Eq)]
/// Bearer token whose debug and display representations never reveal its value.
pub struct SecretToken(String);

impl SecretToken {
    /// Rejects values that are empty or unsafe to place in an HTTP header.
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

    /// Writes the credential only to a caller-selected secret sink.
    pub fn write_secret(&self, writer: &mut impl io::Write) -> io::Result<()> {
        // Deliberate write-only exposure lets the Worker persist an issued
        // credential without making raw token access generally available.
        writer.write_all(self.0.as_bytes())
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretTokenError {
    #[error("token must not be empty")]
    Empty,
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

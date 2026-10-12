use std::convert::TryFrom;
use std::io::Write;

use serde::Serialize;

use crate::statics::EMAIL_REGEX;

#[derive(Serialize)]
pub struct Email(String);

impl Email {
    pub fn from_stdin() -> Self {
        loop {
            eprint!("Please enter your email: ");
            std::io::stdout()
                .flush()
                .inspect_err(|error| tracing::error!(?error, "stdout flush"))
                .ok();
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .inspect_err(|error| tracing::error!(?error, "stdin readline"))
                .ok();
            let email = input.trim();
            if EMAIL_REGEX.is_match(email) {
                return Self(email.to_string());
            } else {
                tracing::error!(?email, "invalid email");
            }
        }
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for Email {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        if EMAIL_REGEX.is_match(value) {
            Ok(Self(value.to_string()))
        } else {
            anyhow::bail!("invalid email {}", value)
        }
    }
}

impl From<Email> for String {
    fn from(val: Email) -> Self {
        val.0.clone()
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_emails() {
        for case in [
            "user@example.com",
            "first.last@sub.domain.org",
            "a@b.co",
            "name+tag@example.io",
        ] {
            let email = Email::try_from(case).expect("should accept valid email");
            assert_eq!(email.as_ref(), case);
        }
    }

    #[test]
    fn rejects_invalid_emails() {
        for case in [
            "",
            "plainaddress",
            "@missing-local.org",
            "missing-at-sign.org",
            "no-tld@domain",
            "space in@local.com",
            "user@domain@",
            "user @domain.com",
        ] {
            assert!(
                Email::try_from(case).is_err(),
                "should reject invalid email: {:?}",
                case
            );
        }
    }

    #[test]
    fn display_and_into_string_round_trip() {
        let email = Email::try_from("user@example.com").unwrap();
        assert_eq!(email.to_string(), "user@example.com");
        let as_string: String = email.into();
        assert_eq!(as_string, "user@example.com");
    }
}

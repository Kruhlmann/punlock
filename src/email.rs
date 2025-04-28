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

    pub fn as_ref(&self) -> &str {
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

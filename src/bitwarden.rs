use std::process::Stdio;

use tokio::process::Command;

use crate::{config::PunlockConfigurationEntry, email::Email};

pub struct Bitwarden<S> {
    pub email: Email,
    pub session: S,
}

impl Bitwarden<()> {
    pub fn new(email: Email) -> Self {
        Self { email, session: () }
    }

    pub async fn authenticate(self, domain: Option<String>) -> anyhow::Result<Bitwarden<String>> {
        Command::new("bw")
            .args(["logout"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .inspect(|_| tracing::debug!("spawn logout"))
            .inspect_err(|error| tracing::error!(?error, "spawn logout"))
            .ok();
        if let Some(ref d) = domain {
            Command::new("bw")
                .args(["config", "server", d])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .inspect(|_| tracing::debug!(?domain, "spawn config server"))
                .inspect_err(|error| tracing::error!(?error, ?domain, "spawn config server"))
                .ok();
        }
        loop {
            let prompt = match domain {
                Some(ref d) => format!("Enter password for bitwarden[{d}] user {}: ", self.email),
                None => format!("Enter password for bitwarden user {}: ", self.email),
            };
            let password = rpassword::prompt_password(prompt)
                .inspect_err(|error| tracing::error!(?error, "read password"))
                .unwrap_or("".to_string());
            if password.trim().is_empty() {
                continue;
            }

            let out = Command::new("bw")
                .args(["login", self.email.as_ref(), &password, "--raw"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .inspect(|_| tracing::debug!("spawn login"))
                .inspect_err(|error| tracing::error!(?error, "spawn login"))?;

            if !out.status.success() {
                let error = String::from_utf8_lossy(&out.stderr);
                tracing::error!(?error, "bitwarden error");
                continue;
            }

            let session = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if session.is_empty() {
                continue;
            }
            return Ok(Bitwarden::<String> {
                email: self.email,
                session,
            });
        }
    }
}

impl Bitwarden<String> {
    pub async fn fetch(&self, entry: &PunlockConfigurationEntry) -> anyhow::Result<String> {
        let bw = Command::new("bw")
            .args(["get", "item", entry.id.as_ref(), "--session", &self.session])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .inspect_err(|error| tracing::error!(?error, "spawn get"))?;

        let output = bw
            .wait_with_output()
            .await
            .inspect_err(|error| tracing::error!(?error, "spawn get"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("`bw get item` failed: {}", err.trim());
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .inspect_err(|error| tracing::error!(?error, json = ?output.stdout, "invalid json"))?;

        extract_secret(&data, &entry.query)
    }

    pub async fn logout(&self) -> anyhow::Result<()> {
        Command::new("bw")
            .args(["logout", "--session", &self.session])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .inspect(|_| tracing::debug!("spawn logout"))
            .inspect_err(|error| tracing::error!(?error, "spawn logout"))?;
        Ok(())
    }
}

pub(crate) fn extract_secret(data: &serde_json::Value, query: &str) -> anyhow::Result<String> {
    let expr = jmespath::compile(query)
        .inspect_err(|error| tracing::error!(?error, ?query, "input/expression mismatch"))?;
    let result = expr
        .search(data)
        .inspect_err(|error| tracing::error!(?error, ?expr, "query failed to apply"))?;

    match result.as_string() {
        Some(s) => Ok(s.to_owned()),
        None => {
            tracing::error!(?data, ?expr, "find secret");
            anyhow::bail!("invalid item")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_a_string_value() {
        let item = json!({ "login": { "username": "u", "password": "s3cr3t" } });
        let secret = extract_secret(&item, "login.password").expect("should extract password");
        assert_eq!(secret, "s3cr3t");
    }

    #[test]
    fn extracts_custom_field_via_filter_and_index() {
        let item = json!({
            "fields": [
                { "name": "other", "value": "ignored" },
                { "name": "ssh_private_key", "value": "-----BEGIN KEY-----" }
            ]
        });
        let query = "fields[?name=='ssh_private_key'].value | [0]";
        let secret = extract_secret(&item, query).expect("should extract custom field");
        assert_eq!(secret, "-----BEGIN KEY-----");
    }

    #[test]
    fn rejects_query_that_yields_a_non_string() {
        let item = json!({ "fields": [{ "name": "ssh_private_key", "value": "k" }] });
        // a filter projection yields an array, not a string
        assert!(extract_secret(&item, "fields[?name=='ssh_private_key'].value").is_err());
    }

    #[test]
    fn rejects_missing_key() {
        let item = json!({ "login": {} });
        assert!(extract_secret(&item, "login.password").is_err());
    }

    #[test]
    fn rejects_invalid_jmespath_expression() {
        let item = json!({});
        assert!(extract_secret(&item, "fields[?name==").is_err());
    }
}

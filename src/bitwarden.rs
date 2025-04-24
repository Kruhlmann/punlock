use std::process::Stdio;

use tokio::process::Command;

use crate::{config::PunlockConfigurationEntry, email::Email};

pub struct Bitwarden<S> {
    email: Email,
    session: S,
}

impl Bitwarden<()> {
    pub fn new(email: Email) -> Self {
        Self { email, session: () }
    }

    pub async fn authenticate(self) -> anyhow::Result<Bitwarden<String>> {
        Command::new("bw")
            .args(&["logout"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .ok();
        loop {
            let password = rpassword::prompt_password(&format!(
                "Enter bitwarden password for {}: ",
                self.email
            ))
            .inspect_err(|error| tracing::error!(?error, "read password"))
            .unwrap_or("".to_string());
            if password.trim().is_empty() {
                continue;
            }

            let out = Command::new("bw")
                .args(&["login", self.email.as_ref(), &password, "--raw"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .inspect_err(|error| tracing::error!(?error, "bw login"))?;

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
            .args(&["get", "item", &entry.id, "--session", &self.session])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .inspect_err(|error| tracing::error!(?error, "bw get"))?;

        let output = bw
            .wait_with_output()
            .await
            .inspect_err(|error| tracing::error!(?error, "bw output"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("`bw get item` failed: {}", err.trim());
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .inspect_err(|error| tracing::error!(?error, json = ?output.stdout, "invalid json"))?;

        let expr = jmespath::compile(&entry.query)
            .inspect_err(|error| tracing::error!(?error, ?entry, "input/expression mismatch"))?;
        let result = expr
            .search(&data)
            .inspect_err(|error| tracing::error!(?error, ?expr, "query failed to apply"))?;

        let secret = match result.as_string() {
            Some(s) => s.to_owned(),
            None => anyhow::bail!("invalid item"),
        };

        Ok(secret)
    }

    pub async fn logout(&self) -> anyhow::Result<()> {
        Command::new("bw")
            .args(&["logout", "--session", &self.session])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .inspect_err(|error| tracing::error!(?error, "bw logout"))?;
        Ok(())
    }
}

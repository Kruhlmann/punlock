use std::process::Stdio;

use tokio::process::Command;

use super::{bitwarden::EncryptedBitwardenItem, Bitwarden};
use crate::data::{Email, Password};

enum BitwardenCliCommand<'a> {
    SetDomain(&'a str),
    GetItem(&'a str, &'a str),
    Login(&'a str, &'a str),
}

impl BitwardenCliCommand<'_> {
    fn create_process(self) -> tokio::process::Command {
        match self {
            BitwardenCliCommand::GetItem(id, session) => {
                let mut cmd = Command::new("bw");
                cmd.args(["get", "item", &id, "--session", &session])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                cmd
            }
            BitwardenCliCommand::SetDomain(domain) => {
                let mut cmd = Command::new("bw");
                cmd.args(["config", "server", &domain])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                cmd
            }
            BitwardenCliCommand::Login(email, password) => {
                let mut cmd = Command::new("bw");
                cmd.args(["login", &email, &password, "--raw"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                cmd
            }
        }
    }
}

pub struct BitwardenCli<S> {
    email: Email,
    session: S,
}

impl BitwardenCli<()> {
    pub fn new(email: Email) -> Self {
        Self { email, session: () }
    }

    pub async fn authenticate(
        self,
        domain: Option<String>,
    ) -> anyhow::Result<BitwardenCli<String>> {
        if let Some(ref d) = domain {
            BitwardenCliCommand::SetDomain(d)
                .create_process()
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
            let Password(password) = Password::from_user_input(&prompt, "Enter Password");
            let output = BitwardenCliCommand::Login(&self.email.to_string(), &password)
                .create_process()
                .output()
                .await
                .inspect(|_| tracing::debug!("spawn login"))
                .inspect_err(|error| tracing::error!(?error, "spawn login"))?;

            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                if !error.starts_with("You are already logged in as") {
                    tracing::error!(?error, "login");
                    continue;
                }
                tracing::info!(?error, "errror");
            }

            let session = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if session.is_empty() {
                continue;
            }
            return Ok(BitwardenCli::<String> {
                email: self.email,
                session,
            });
        }
    }
}

#[async_trait::async_trait]
impl Bitwarden for BitwardenCli<String> {
    async fn get_item(&self, id: &str, query: &str) -> anyhow::Result<EncryptedBitwardenItem> {
        let output = BitwardenCliCommand::GetItem(&id, &self.session)
            .create_process()
            .spawn()
            .inspect_err(|error| tracing::error!(?error, "spawn get"))?
            .wait_with_output()
            .await
            .inspect_err(|error| tracing::error!(?error, "spawn get"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("`bw get item` failed: {}", err.trim());
        }
        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .inspect_err(|error| tracing::error!(?error, json = ?output.stdout, "invalid json"))?;
        let secret: EncryptedBitwardenItem = jmespath::compile(&query)
            .inspect_err(|error| tracing::error!(?error, ?query, "input/expression mismatch"))?
            .search(&data)
            .inspect_err(|error| tracing::error!(?error, ?query, "query failed to apply"))?
            .as_string()
            .map(|s| s.to_owned())
            .ok_or(anyhow::anyhow!("invalid item"))?
            .into();

        Ok(secret)
    }
}

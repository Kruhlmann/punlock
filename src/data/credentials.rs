use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use super::Password;

#[derive(Deserialize, Serialize)]
pub struct BitwardenClientCredentials {
    pub id: String,
    pub secret: String,
}

impl BitwardenClientCredentials {
    pub async fn new(path: impl AsRef<Path>) -> Self {
        match tokio::fs::read(&path).await {
            Ok(content_vec_u8) => {
                match String::from_utf8_lossy(&content_vec_u8).parse::<String>() {
                    Ok(content) => match toml::from_str::<BitwardenClientCredentials>(&content) {
                        Ok(config) => return config,
                        Err(error) => {
                            tracing::error!(?error, "invalid credentials file; deleting");
                            tokio::fs::remove_file(&path).await.inspect_err(|e| tracing::error!(error = ?e, "unable to remove credentials file")).ok();
                        }
                    },
                    Err(error) => tracing::warn!(?error, "parse credentials file content"),
                }
            }
            Err(error) => tracing::warn!(?error, "read credentials file"),
        };
        return BitwardenClientCredentials::from_user_prompt();
    }

    pub async fn write_to_disk(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let contents = toml::to_string_pretty(self)?;
        let mut file = tokio::fs::File::create(path).await.inspect_err(|error| {
            tracing::error!(?path, ?error, "unable to create credentials file")
        })?;
        file.write_all(contents.as_bytes())
            .await
            .inspect_err(|error| {
                tracing::error!(?path, ?error, "unable to write credentials to file")
            })?;
        tracing::debug!(?path, "wrote credentials");
        Ok(())
    }

    fn from_user_prompt() -> Self {
        let Password(id) = Password::from_user_input("Enter client id", "Enter client id");
        let Password(secret) =
            Password::from_user_input("Enter client secret", "Enter client secret");
        Self { id, secret }
    }
}

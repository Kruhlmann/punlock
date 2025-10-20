use std::convert::{TryFrom, TryInto};
use std::path::Path;

use tokio::{fs::File, io::AsyncWriteExt};

use crate::statics::{
    DEFAULT_BITWARDEN_DOMAIN, LATEST_CONFIGURATION_VERSION, SYSTEM_CONFIG_PATH_CANDIDATES,
};

use super::Domain;
use super::Email;

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct PunlockConfigurationEntry {
    pub id: String,
    pub query: String,
    pub path: String,
    pub links: Option<Vec<String>>,
    #[serde(default)]
    pub public: bool,
}

#[derive(serde::Deserialize)]
pub struct PartialPunlockConfiguration {
    pub domain: Option<String>,
    pub version: Option<String>,
    pub email: Option<String>,
    pub entries: Option<Vec<PunlockConfigurationEntry>>,
    pub cache_token: Option<bool>,
}

impl TryFrom<&Path> for PartialPunlockConfiguration {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> anyhow::Result<Self> {
        tracing::debug!(?path, "loading configuration");
        let content = std::fs::read_to_string(path).inspect_err(|error| {
            tracing::error!(?error, ?path, "unable to read configuration file");
        })?;
        let config: PartialPunlockConfiguration =
            toml::from_str(&content).inspect_err(|error| {
                tracing::error!(?error, ?path, "configuration is invalid");
            })?;
        Ok(config)
    }
}

impl PartialPunlockConfiguration {
    pub fn try_from_default_path() -> anyhow::Result<Self> {
        for path in SYSTEM_CONFIG_PATH_CANDIDATES.iter() {
            tracing::debug!(?path, "inspecting configuration candidate");
            if path.exists() {
                tracing::debug!(?path, "configuration file exists");
                let cfg: PartialPunlockConfiguration =
                    path.as_path().try_into().inspect_err(|error| {
                        tracing::error!(?error, ?path, "unable to load configuration")
                    })?;
                return Ok(cfg);
            }
        }
        anyhow::bail!("no default configuration found");
    }
}

#[derive(serde::Serialize)]
pub struct PunlockConfiguration {
    pub cache_token: bool,
    pub version: String,
    pub email: Email,
    pub domain: Domain,
    pub entries: Vec<PunlockConfigurationEntry>,
}

impl TryFrom<PartialPunlockConfiguration> for PunlockConfiguration {
    type Error = anyhow::Error;

    fn try_from(value: PartialPunlockConfiguration) -> anyhow::Result<Self> {
        Ok(Self {
            cache_token: value.cache_token.unwrap_or(false),
            domain: value
                .domain
                .unwrap_or(DEFAULT_BITWARDEN_DOMAIN.to_string())
                .into(),
            version: value
                .version
                .unwrap_or(LATEST_CONFIGURATION_VERSION.to_string()),
            email: if let Some(e) = value.email {
                e.as_str().try_into().inspect_err(
                    |error| tracing::error!(?error, email = ?e, "invalid email in configuration"),
                )?
            } else {
                Email::from_stdin()
            },
            entries: value.entries.unwrap_or_default(),
        })
    }
}

impl PunlockConfiguration {
    pub async fn write_to_disk(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let contents = toml::to_string_pretty(self)?;
        let mut file = File::create(path).await.inspect_err(|error| {
            tracing::error!(?path, ?error, "unable to create configuration file")
        })?;
        file.write_all(contents.as_bytes())
            .await
            .inspect_err(|error| {
                tracing::error!(?path, ?error, "unable to write configuration to file")
            })?;
        tracing::debug!(?path, "wrote configuration");
        Ok(())
    }
}

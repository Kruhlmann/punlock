use std::convert::{TryFrom, TryInto};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncWriteExt};

use crate::{
    email::Email,
    item_id::UuidV4,
    statics::{LATEST_CONFIGURATION_VERSION, SYSTEM_CONFIG_PATH_CANDIDATES},
};

#[derive(Deserialize, Serialize, Debug)]
pub struct PunlockConfigurationEntry {
    pub id: UuidV4,
    pub query: String,
    pub path: String,
    pub links: Option<Vec<String>>,
    #[serde(default)]
    pub public: bool,
}

#[derive(Deserialize)]
pub struct PartialPunlockConfiguration {
    pub domain: Option<String>,
    pub version: Option<String>,
    pub email: Option<String>,
    pub entries: Option<Vec<PunlockConfigurationEntry>>,
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

#[derive(Serialize)]
pub struct PunlockConfiguration {
    pub version: String,
    pub email: Email,
    pub domain: Option<String>,
    pub entries: Vec<PunlockConfigurationEntry>,
}

impl TryFrom<PartialPunlockConfiguration> for PunlockConfiguration {
    type Error = anyhow::Error;

    fn try_from(value: PartialPunlockConfiguration) -> anyhow::Result<Self> {
        Ok(Self {
            domain: value.domain,
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

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_ID: &str = "11111111-2222-3333-4444-555555555555";

    #[test]
    fn partial_config_deserializes_full_document() {
        let toml = r#"
domain = "https://bw.example.com"
version = "0.1.0"
email = "user@example.com"

[[entries]]
id = "11111111-2222-3333-4444-555555555555"
query = "details.fields[?@.name=='ssh_private_key'].value"
path = "ssh/id_rsa"
public = true
links = ["~/.ssh/id_rsa"]
"#;
        let cfg: PartialPunlockConfiguration =
            toml::from_str(toml).expect("valid configuration should deserialize");

        assert_eq!(cfg.domain.as_deref(), Some("https://bw.example.com"));
        assert_eq!(cfg.version.as_deref(), Some("0.1.0"));
        assert_eq!(cfg.email.as_deref(), Some("user@example.com"));

        let entries = cfg.entries.expect("entries should be present");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id.as_ref(), VALID_ID);
        assert_eq!(entries[0].path, "ssh/id_rsa");
        assert!(entries[0].public);
        assert_eq!(entries[0].links, Some(vec!["~/.ssh/id_rsa".to_string()]));
    }

    #[test]
    fn partial_config_applies_field_defaults() {
        let toml = r#"
[[entries]]
id = "11111111-2222-3333-4444-555555555555"
query = "details.login.password"
path = "api/token"
"#;
        let cfg: PartialPunlockConfiguration =
            toml::from_str(toml).expect("minimal configuration should deserialize");

        assert!(cfg.domain.is_none());
        assert!(cfg.version.is_none());
        assert!(cfg.email.is_none());

        let entries = cfg.entries.expect("entries should be present");
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].public, "public should default to false");
        assert!(entries[0].links.is_none(), "links should default to none");
    }

    #[test]
    fn partial_config_rejects_invalid_toml() {
        assert!(toml::from_str::<PartialPunlockConfiguration>("this is not = = toml").is_err());
    }

    #[test]
    fn conversion_defaults_version_and_entries() {
        let partial = PartialPunlockConfiguration {
            domain: None,
            version: None,
            email: Some("user@example.com".to_string()),
            entries: None,
        };
        let cfg: PunlockConfiguration = partial.try_into().expect("valid email should convert");

        assert_eq!(cfg.version, *LATEST_CONFIGURATION_VERSION);
        assert_eq!(cfg.email.as_ref(), "user@example.com");
        assert!(cfg.domain.is_none());
        assert!(cfg.entries.is_empty());
    }

    #[test]
    fn conversion_preserves_provided_version_and_entries() {
        let entry = PunlockConfigurationEntry {
            id: UuidV4::try_from(VALID_ID).unwrap(),
            query: "details.login.password".to_string(),
            path: "api/token".to_string(),
            links: None,
            public: false,
        };
        let partial = PartialPunlockConfiguration {
            domain: Some("https://bw.example.com".to_string()),
            version: Some("0.1.0".to_string()),
            email: Some("user@example.com".to_string()),
            entries: Some(vec![entry]),
        };
        let cfg: PunlockConfiguration = partial.try_into().expect("valid config should convert");

        assert_eq!(cfg.version, "0.1.0");
        assert_eq!(cfg.domain.as_deref(), Some("https://bw.example.com"));
        assert_eq!(cfg.entries.len(), 1);
        assert_eq!(cfg.entries[0].id.as_ref(), VALID_ID);
    }

    #[test]
    fn partial_config_rejects_invalid_item_id() {
        let toml = r#"
[[entries]]
id = "not-a-uuid"
query = "details.login.password"
path = "api/token"
"#;
        assert!(toml::from_str::<PartialPunlockConfiguration>(toml).is_err());
    }

    #[test]
    fn conversion_rejects_invalid_email() {
        let partial = PartialPunlockConfiguration {
            domain: None,
            version: Some("0.1.0".to_string()),
            email: Some("not-an-email".to_string()),
            entries: Some(vec![]),
        };
        assert!(PunlockConfiguration::try_from(partial).is_err());
    }
}

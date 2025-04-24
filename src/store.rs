use std::{path::PathBuf, process::Stdio};

use futures::{StreamExt, stream::FuturesUnordered};
use tokio::{fs::File, io::AsyncWriteExt, process::Command};

use crate::{bitwarden::Bitwarden, config::PunlockConfigurationEntry, statics::PROJECT_DIRS};

pub struct UnmountedSecretStore {
    bitwarden: Bitwarden<String>,
    root_path: PathBuf,
}

impl UnmountedSecretStore {
    pub fn new(bitwarden: Bitwarden<String>) -> Self {
        Self {
            bitwarden,
            root_path: PROJECT_DIRS.cache_dir().to_owned(),
        }
    }
}

impl UnmountedSecretStore {
    pub async fn into_platform_store(self) -> anyhow::Result<UnixSecretStore> {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                let store = UnixSecretStore::new(self.bitwarden, self.root_path).unmount().await?.mount().await?;
                Ok(store)
            } else if #[cfg(target_os = "macos")] {
                // mount_ramdisk_macos(mount_point)?;
                panic!("todo");
            } else {
                panic!("todo");
                // debug!("On Windows or unsupported OS: using plain dir at {}", mount_point.display());
            }
        }
    }
}

pub struct UnixSecretStore {
    bitwarden: Bitwarden<String>,
    root_path: PathBuf,
}

impl UnixSecretStore {
    pub fn new(bitwarden: Bitwarden<String>, root_path: PathBuf) -> Self {
        Self {
            bitwarden,
            root_path,
        }
    }

    pub async fn write_secrets(
        &self,
        entries: &Vec<PunlockConfigurationEntry>,
    ) -> anyhow::Result<()> {
        let mut tasks = FuturesUnordered::new();
        for entry in entries.iter() {
            tasks.push(async move {
                let secret = self.bitwarden.fetch(&entry).await?;
                let path = self.root_path.join(&entry.path);
                tokio::fs::create_dir_all(path.parent().unwrap_or(&path)).await.inspect_err(|error| tracing::error!(?error, "create secret directory"))?;
                let mut file = File::create(path).await.inspect_err(
                    |error| tracing::error!(?error, id = ?entry.id, path = ?entry.path, "create secret file"),
                )?;
                file.write_all(secret.as_bytes()).await.inspect_err(
                    |error| tracing::error!(?error, id = ?entry.id, path = ?entry.path, "write secret"),
                )?;
                Ok::<(String, String), anyhow::Error>((entry.id.clone(), entry.path.clone()))
            })
        }
        while let Some(result) = tasks.next().await {
            match result {
                Ok((id, path)) => tracing::info!(?id, ?path, "load secret"),
                Err(error) => tracing::error!(?error, "load secret"),
            }
        }
        Ok(())
    }

    async fn unmount(self) -> anyhow::Result<Self> {
        if self.root_path.exists() {
            Command::new("sudo")
                .args(&["umount", self.root_path.to_str().unwrap()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .ok();
            tokio::fs::remove_dir_all(&self.root_path).await.ok();
        }
        Ok(self)
    }

    async fn mount(self) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(&self.root_path)
            .await
            .inspect_err(
                |error| tracing::error!(?error, path = ?self.root_path, "unable to create secret path"),
            )?;
        let status = Command::new("sudo")
            .args(&["mount", "-t", "tmpfs", "-o", "size=50M", "tmpfs"])
            .arg(&self.root_path)
            .status()
            .await
            .inspect_err(|error| tracing::error!(?error, "mount failed"))?;
        if !status.success() {
            anyhow::bail!("mount command failed with {}", status);
        }
        tracing::debug!(path = ?self.root_path, "tmpfs mounted");

        let uid = users::get_current_uid();
        let gid = users::get_current_gid();
        Command::new("sudo")
            .args(&["chown", &format!("{}:{}", uid, gid)])
            .arg(&self.root_path)
            .status()
            .await
            .inspect_err(|error| tracing::error!(?error, "chown failed"))?;
        Ok(self)
    }
}

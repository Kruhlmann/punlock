use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};

use futures::{StreamExt, stream::FuturesUnordered};
use tokio::io::AsyncWriteExt;

use crate::{
    bitwarden::Bitwarden,
    config::PunlockConfigurationEntry,
    statics::{self, HOME_DIRECTORY},
};

pub struct UnmountedSecretStore {
    bitwarden: Bitwarden<String>,
}

impl UnmountedSecretStore {
    pub fn new(bitwarden: Bitwarden<String>) -> Self {
        Self { bitwarden }
    }
}

impl UnmountedSecretStore {
    pub async fn into_platform_store(self) -> anyhow::Result<UnixSecretStore> {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                let root_path = statics::RUNTIME_DIRECTORY.join("punlock");
                let store = UnixSecretStore::new(self.bitwarden, root_path).teardown().await?.setup().await?;
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
    bitwarden: Arc<Bitwarden<String>>,
    root_path: Arc<PathBuf>,
}

impl UnixSecretStore {
    pub fn new(bitwarden: Bitwarden<String>, root_path: PathBuf) -> Self {
        Self {
            bitwarden: Arc::new(bitwarden),
            root_path: Arc::new(root_path),
        }
    }

    pub async fn write_secrets(&self, entries: &[PunlockConfigurationEntry]) -> anyhow::Result<()> {
        let mut tasks = FuturesUnordered::new();

        for entry in entries.iter() {
            let root = self.root_path.clone();
            let bw = self.bitwarden.clone();

            tasks.push(
                async move {
                    let secret = bw.fetch(entry).await.inspect_err(|error| tracing::error!(?error, ?entry, "item not found"))?;
                    let path = root.join(&entry.path);

                    tokio::fs::create_dir_all(path.parent().unwrap_or(&path)).await?;
                    {
                        let mut file = tokio::fs::File::create(&path).await?;
                        file.write_all(secret.as_bytes()).await?;
                        if !secret.ends_with('\n') {
                            file.write_all(b"\n").await?;
                        }
                        file.flush().await?;
                    }

                    let mut perms = tokio::fs::metadata(&path).await?.permissions();
                    perms.set_readonly(true);

                    if !entry.public {
                        #[cfg(unix)]
                        perms.set_mode(0o400);
                    }
                    tokio::fs::set_permissions(&path, perms)
                        .await
                        .inspect(|_| tracing::debug!(?path, "set readonly"))
                        .inspect_err(|error| tracing::error!(?error, ?path, "remove runtime dir"))?;

                    if let Some(ref links) = entry.links {
                        for link in links {
                            let link_path: PathBuf = if PathBuf::from(link).is_absolute() {
                                PathBuf::from(link)
                            } else {
                                HOME_DIRECTORY.join(link)
                            };
                            tokio::fs::create_dir_all(link_path.parent().unwrap_or(&link_path))
                                .await?;
                            match tokio::fs::symlink_metadata(&link_path).await {
                                Ok(meta) if meta.file_type().is_symlink() => {
                                    let current = tokio::fs::read_link(&link_path).await?;
                                    if current == path {
                                        tracing::debug!(?link_path, "skipping existing symlink");
                                        continue;
                                    }
                                    tokio::fs::remove_file(&link_path).await?;
                                }
                                Ok(_) => tokio::fs::remove_file(&link_path).await?,
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                                Err(e) => return Err(e.into()),
                            }

                            let src = path.clone();
                            let dst = link_path.clone();
                            tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                                #[cfg(unix)]
                                std::os::unix::fs::symlink(&src, &dst)?;
                                #[cfg(windows)]
                                std::os::windows::fs::symlink_file(&src, &dst)?;
                                Ok(())
                            })
                            .await
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))??;

                            tracing::info!(src =? path, destination = ?link_path, "created/updated symlink");
                        }
                    }

                    Ok::<_, anyhow::Error>((entry.id.clone(), entry.path.clone()))
                }
            );
        }

        let mut count = 0;
        let mut success = 0;
        while let Some(res) = tasks.next().await {
            count += 1;
            match res {
                Ok((id, path)) => {
                    success += 1;
                    tracing::info!(?id, ?path, "secret written")
                }
                Err(error) => tracing::error!(?error, "failed to write secret"),
            }
        }

        tracing::info!("wrote {success}/{count} secrets");

        Ok(())
    }

    async fn teardown(self) -> anyhow::Result<Self> {
        if self.root_path.exists() {
            tokio::fs::remove_dir_all(&*self.root_path)
                .await
                .inspect_err(
                    |error| tracing::error!(?error, path = ?self.root_path, "remove runtime dir"),
                )
                .ok();
        }
        Ok(self)
    }

    async fn setup(self) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(&*self.root_path)
            .await
            .inspect_err(
                |error| tracing::error!(?error, path = ?self.root_path, "create runtime dir"),
            )?;
        Ok(self)
    }
}

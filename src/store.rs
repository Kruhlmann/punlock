use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use aes_gcm::{aead::Aead, aes::Aes256, Aes256Gcm, KeyInit, Nonce};
use anyhow::Context;
use argon2::{Argon2, Params};
use base64::{engine::general_purpose, Engine};
use futures::{stream::FuturesUnordered, StreamExt};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use tokio::io::AsyncWriteExt;

use crate::{
    bw::{bitwarden::EncryptedBitwardenItem, BitwardenApi, BitwardenToken},
    data::PunlockConfigurationEntry,
    statics::{HOME_DIRECTORY, RUNTIME_DIRECTORY},
};

pub struct SecretStore {
    path: Arc<Path>,
    bitwarden: Arc<Box<dyn BitwardenApi>>,
}

// struct DecryptedString(String);

// impl DecryptedString {
//     pub fn from_encrypted(encrypted: &str, credentials: &BitwardenToken) -> anyhow::Result<Self> {
//         let cipher = Aes256Gcm::new(&credentials.key);
//         let blob = base64::engine::general_purpose::STANDARD
//             .decode(encrypted)
//             .context("failed to Base64‐decode encrypted data")?;
//         if blob.len() < 12 + 16 {
//             anyhow::bail!("encrypted data too short");
//         }
//         let (nonce_bytes, ciphertext_and_tag) = blob.split_at(12);
//         let nonce = Nonce::from_slice(nonce_bytes);
//         let plaintext = cipher
//             .decrypt(nonce, ciphertext_and_tag)
//             .map_err(|error| anyhow::anyhow!("decryption failed: {error}"))?;
//         let s = String::from_utf8(plaintext).context("decrypted bytes not valid utf-8")?;

//         Ok(Self(s))
//     }
// }

impl SecretStore {
    pub async fn new(bitwarden: impl BitwardenApi + 'static) -> anyhow::Result<SecretStore> {
        let this = Self {
            path: RUNTIME_DIRECTORY.join(env!("CARGO_PKG_NAME")).into(),
            bitwarden: Arc::new(Box::new(bitwarden)),
        }
        .teardown()
        .await?
        .setup()
        .await?;
        Ok(this)
    }

    async fn teardown(self) -> anyhow::Result<Self> {
        if self.path.exists() {
            tokio::fs::remove_dir_all(&*self.path)
                .await
                .inspect_err(
                    |error| tracing::error!(?error, path = ?self.path, "remove runtime dir"),
                )
                .ok();
        }
        Ok(self)
    }

    async fn setup(self) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(&*self.path).await.inspect_err(
            |error| tracing::error!(?error, path = ?self.path, "create runtime dir"),
        )?;
        Ok(self)
    }

    pub async fn write_secrets(&self, entries: &[PunlockConfigurationEntry]) -> anyhow::Result<()> {
        // let mut tasks = FuturesUnordered::new();
        // let token = self.bitwarden.get_token();

        // for entry in entries {
        //     let root = self.path.clone();

        //     tasks.push(async move {
        //         let item = self.bitwarden.get_item(&entry.id).await.inspect_err(|error| tracing::error!(id = ?entry.id, ?error, "get item"))?;
        //         let EncryptedBitwardenItem(cipher) = jmespath::compile(&entry.query)
        //             .inspect_err(|error| {
        //                 tracing::error!(?error, query = ?entry.query, "input/expression mismatch")
        //             })?
        //             .search(&item)
        //             .inspect_err(|error| tracing::error!(?error, query = ?entry.query, "query failed to apply"))?
        //             .as_string()
        //             .map(|s| s.to_owned())
        //             .ok_or(anyhow::anyhow!("invalid item"))?
        //             .into();
        //         anyhow::bail!("hi");
        // let DecryptedString(cipher) = DecryptedString::from_encrypted(&cipher, token)?;
        // let source = root.join(&entry.path);
        // tokio::fs::create_dir_all(source.parent().unwrap_or(&source)).await?;
        // {
        //     let mut file = tokio::fs::File::create(&source).await?;
        //     file.write_all(cipher.as_bytes()).await?;
        //     if !cipher.ends_with('\n') {
        //         file.write_all(b"\n").await?;
        //     }
        //     file.flush().await?;
        // }

        // let mut perms = tokio::fs::metadata(&source).await?.permissions();
        // perms.set_readonly(true);
        // #[cfg(unix)]
        // if !entry.public {
        //     std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o400);
        // }
        // tokio::fs::set_permissions(&source, perms)
        //     .await
        //     .inspect(|_| tracing::debug!(?source, "set perms"))
        //     .inspect_err(|error| tracing::error!(?error, ?source, "set perms"))?;

        // if let Some(links) = &entry.links {
        //     for link in links {
        //         let destination: PathBuf = if PathBuf::from(link).is_absolute() {
        //             PathBuf::from(link)
        //         } else {
        //             HOME_DIRECTORY.join(link)
        //         };

        //         tokio::fs::create_dir_all(destination.parent().unwrap_or(&destination))
        //             .await?;

        //         match tokio::fs::symlink_metadata(&destination).await {
        //             Ok(md) if md.file_type().is_symlink() => {
        //                 let cur = tokio::fs::read_link(&destination).await?;
        //                 if cur == source {
        //                     tracing::debug!(?source, ?destination, "skip symlink");
        //                     continue;
        //                 }
        //                 tokio::fs::remove_file(&destination).await?;
        //             }
        //             Ok(_) => tokio::fs::remove_file(&destination).await?,
        //             Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        //             Err(e) => return Err(e.into()),
        //         }

        //         #[cfg(unix)]
        //         std::os::unix::fs::symlink(&source, &destination)?;
        //         #[cfg(windows)]
        //         if std::fs::metadata(&source)?.is_dir() {
        //             std::os::windows::fs::symlink_dir(&source, &destination)?;
        //         } else {
        //             std::os::windows::fs::symlink_file(&source, &destination)?;
        //         }

        //         tracing::info!(?source, ?destination, "created/updated symlink");
        //     }
        // }

        // Ok::<_, anyhow::Error>((entry.id.clone(), entry.path.clone()))
        // });
        // }

        // tracing::error!(len = tasks.len(), "tasks");

        // let mut total = 0;
        // let mut ok = 0;
        // while let Some(r) = tasks.next().await {
        //     total += 1;
        //     match r {
        //         Ok((id, p)) => {
        //             ok += 1;
        //             tracing::info!(?id, path = ?p, "write");
        //         }
        //         Err(error) => tracing::error!(?error, "write"),
        //     }
        // }
        // tracing::info!(ok, total, "secrets successfully written");
        Ok(())
    }
}

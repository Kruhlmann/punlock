use aes_gcm::aes::cipher::BlockDecrypt;
use aes_gcm::aes::Aes256;
use aes_gcm::KeyInit;
use anyhow::Context;
use base64::Engine;
use hkdf::Hkdf;
use hmac::Hmac;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::convert::{TryFrom, TryInto};
use std::path::{Path, PathBuf};

use clap::Parser;
use hmac::Mac;
use punlock::bw::{BitwardenApi, BitwardenHttpClient};
use punlock::data::config::PartialPunlockConfiguration;
use punlock::data::{BitwardenClientCredentials, Password, PunlockConfiguration};
use punlock::statics::USER_CREDENTIALS_FILE_PATH;
use punlock::{statics::USER_CONFIG_FILE_PATH, store::SecretStore};
use tracing_subscriber::{
    fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[derive(Parser, Debug)]
#[command(name = "punlock", about = "PasswordUNLOCKer")]
struct Cli {
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug)]
struct Cipher {
    pub iv: Vec<u8>,
    pub ct: Vec<u8>,
    pub mac: Vec<u8>,
}

impl TryFrom<&str> for Cipher {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let rest = value.splitn(2, '.').nth(1).expect("invalid format, no dot");
        let parts: Vec<&str> = rest.split('|').collect();
        assert_eq!(parts.len(), 3, "expected three parts");
        let iv = base64::engine::general_purpose::STANDARD.decode(parts[0])?;
        let ct = base64::engine::general_purpose::STANDARD.decode(parts[1])?;
        let mac = base64::engine::general_purpose::STANDARD.decode(parts[2])?;

        Ok(Self { iv, ct, mac })
    }
}
fn decrypt_aes_cbc_pkcs7(key: &[u8; 32], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let iv: &[u8; 16] = iv.try_into()?;
    if ct.len() % 16 != 0 {
        anyhow::bail!("ciphertext length must be a multiple of 16");
    }
    let cipher = Aes256::new(key.into());
    let mut prev = *iv;
    let mut plaintext = Vec::with_capacity(ct.len());
    for chunk in ct.chunks(16) {
        let mut block = <[u8; 16]>::try_from(chunk).expect("chunk is 16 bytes");
        cipher.decrypt_block(&mut block.into());
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        plaintext.extend_from_slice(&block);
        prev = <[u8; 16]>::try_from(chunk).unwrap();
    }

    // 4) Strip & verify PKCS#7 padding
    let pad_len = *plaintext.last().context("decrypted data is empty")? as usize;
    if pad_len == 0 || pad_len > 16 {
        anyhow::bail!("invalid padding length");
    }
    let len = plaintext.len();
    let pad_start = len - pad_len;
    if !plaintext[pad_start..]
        .iter()
        .all(|&b| b as usize == pad_len)
    {
        anyhow::bail!("invalid PKCS#7 padding");
    }
    plaintext.truncate(pad_start);

    Ok(plaintext)
}

fn testdecrypt(
    config: &PunlockConfiguration,
    bitwarden: &BitwardenHttpClient,
) -> anyhow::Result<()> {
    let Password(master_password) = Password::from_user_input("Enter master password", "");
    let str = "2.lt/eAKnlHsPcUCR5bGf/Kg==|8xFsF52BQx14Pb6MuW7ByYLE3ptmbTER+FxDhwGBj10=|6CDbFNKPyjXpOYblz64XFV88ofgDUKpM0YVaGLnWVt0=";
    let mut master_key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        master_password.as_bytes(),
        config.email.to_string().as_bytes(),
        bitwarden.get_token().kdf_iterations,
        &mut master_key,
    );
    tracing::info!(master_key = hex::encode(&master_key), "master key");

    // --- HKDF expand ---

    let (_prk_bytes, hk) = Hkdf::<Sha256>::extract(None, &master_key);
    let mut stretch_enc = [0u8; 32];
    let mut stretch_mac = [0u8; 32];
    println!("stretch_enc = {:x?}", stretch_enc);
    println!("stretch_mac = {:x?}", stretch_mac);
    hk.expand(b"enc", &mut stretch_enc)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("HKDF expand(enc) failed")?;
    hk.expand(b"mac", &mut stretch_mac)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("HKDF expand(mac) failed")?;

    // --- Parse blob ---
    let cipher: Cipher = str.try_into()?;

    // --- Verify HMAC ---
    type HmacSha256 = Hmac<Sha256>;
    let mut h = <HmacSha256 as Mac>::new_from_slice(&stretch_mac)?;
    h.update(&cipher.iv);
    h.update(&cipher.ct);
    let computed = h.clone().finalize().into_bytes();
    println!("computed MAC = {}", hex::encode(&computed));
    println!("expected MAC = {}", hex::encode(&cipher.mac));
    h.verify_slice(&cipher.mac)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("HMAC verification failed")?;

    // --- AES-CBC decrypt (64 bytes) ---
    let clear = decrypt_aes_cbc_pkcs7(&stretch_enc, &cipher.iv, &cipher.ct)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("AES-CBC decrypt failed")?;

    // --- Split into real vault keys ---
    let vault_enc_key: [u8; 32] = clear[0..32].try_into().unwrap();
    let vault_mac_key: [u8; 32] = clear[32..64].try_into().unwrap();

    println!("Vault ENC key: {:x?}", vault_enc_key);
    println!("Vault MAC key: {:x?}", vault_mac_key);

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let formattter = tracing_subscriber::fmt::Layer::new()
        .with_thread_names(true)
        .with_span_events(FmtSpan::FULL);
    let filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(formattter)
        .with(filter)
        .try_init()
        .inspect_err(|error| eprintln!("error configuring tracing subscriber: {error}"))?;
    tracing::debug!("tracing initialized");

    let cli = Cli::parse();
    let config: PartialPunlockConfiguration = if let Some(path) = cli.config {
        <&Path as TryInto<PartialPunlockConfiguration>>::try_into(path.as_path())?
    } else {
        PartialPunlockConfiguration::try_from_default_path()?
    };

    let config: PunlockConfiguration = config.try_into()?;
    config
        .write_to_disk(USER_CONFIG_FILE_PATH.as_path())
        .await?;

    let credentials = BitwardenClientCredentials::new(USER_CREDENTIALS_FILE_PATH.as_path()).await;
    credentials
        .write_to_disk(USER_CREDENTIALS_FILE_PATH.as_path())
        .await?;

    let bitwarden =
        BitwardenHttpClient::new(&config.email, &credentials, config.domain.clone()).await?;

    testdecrypt(&config, &bitwarden)?;

    // let store = SecretStore::new(bitwarden).await?;
    // store.write_secrets(&config.entries).await?;
    Ok(())
}

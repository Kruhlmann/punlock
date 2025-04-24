use std::path::{Path, PathBuf};

use clap::Parser;
use punlock::{
    bitwarden::Bitwarden,
    config::{PartialPunlockConfiguration, PunlockConfiguration},
    statics::USER_CONFIG_FILE_PATH,
    store::UnmountedSecretStore,
};
use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

#[derive(Parser, Debug)]
#[command(name = "punlock", about = "PasswordUNLOCKer")]
struct Cli {
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let formattter = tracing_subscriber::fmt::Layer::new()
        .with_thread_names(true)
        .with_span_events(FmtSpan::FULL);
    let filter = EnvFilter::try_from_default_env()?;
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

    let bitwarden = Bitwarden::new(config.email).authenticate().await?;
    let store = UnmountedSecretStore::new(bitwarden)
        .into_platform_store()
        .await?;
    store.write_secrets(&config.entries).await?;
    Ok(())
}

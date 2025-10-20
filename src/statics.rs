use std::path::PathBuf;

use directories::ProjectDirs;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    pub static ref LATEST_CONFIGURATION_VERSION: &'static str = "1.0.0";
    pub static ref CONFIG_FILE_NAME: &'static str = "config.toml";
    pub static ref CREDENTIALS_FILE_NAME: &'static str = "credentials.toml";
    pub static ref PROJECT_DIRS: ProjectDirs =
        ProjectDirs::from("dev", "kruhlmann", "punlock").unwrap();
    pub static ref USER_CONFIG_FILE_PATH: PathBuf =
        PROJECT_DIRS.config_dir().join(CONFIG_FILE_NAME.to_string());
    pub static ref USER_CREDENTIALS_FILE_PATH: PathBuf = PROJECT_DIRS
        .config_dir()
        .join(CREDENTIALS_FILE_NAME.to_string());
    pub static ref HOME_DIRECTORY: PathBuf = dirs::home_dir().unwrap();
    pub static ref SYSTEM_CONFIG_PATH_CANDIDATES: Vec<PathBuf> = [
        PathBuf::from(CONFIG_FILE_NAME.to_string()),
        USER_CONFIG_FILE_PATH.to_path_buf(),
        PathBuf::from("/etc/punlock/").join("CONFIG_FILE_NAME")
    ]
    .to_vec();
    pub static ref EMAIL_REGEX: Regex = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    pub static ref DEFAULT_BITWARDEN_DOMAIN: &'static str = "vault.bitwarden.com";
}

#[cfg(unix)]
lazy_static! {
    pub static ref RUNTIME_DIRECTORY: PathBuf = dirs::runtime_dir().unwrap();
}

#[cfg(not(unix))]
lazy_static! {
    pub static ref RUNTIME_DIRECTORY: PathBuf = std::env::temp_dir();
}

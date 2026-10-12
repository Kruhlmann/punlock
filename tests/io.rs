use std::{
    convert::TryFrom,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    sync::{Mutex, MutexGuard},
};

use punlock::{
    bitwarden::Bitwarden, config::PunlockConfigurationEntry, email::Email, item_id::UuidV4,
    store::UnixSecretStore,
};

static PATH_LOCK: Mutex<()> = Mutex::new(());

struct FakeBw {
    _dir: tempfile::TempDir,
    _lock: MutexGuard<'static, ()>,
    old_path: OsString,
}

impl FakeBw {
    fn new(script: &str) -> Self {
        let lock = PATH_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempfile::tempdir().expect("tempdir");
        let bw = dir.path().join("bw");
        fs::write(&bw, format!("#!/usr/bin/env bash\n{script}\n")).expect("write fake bw");
        fs::set_permissions(&bw, fs::Permissions::from_mode(0o755)).expect("chmod fake bw");

        let old_path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            format!("{}:{}", dir.path().display(), old_path.to_string_lossy()),
        );

        Self {
            _dir: dir,
            _lock: lock,
            old_path,
        }
    }
}

impl Drop for FakeBw {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.old_path);
    }
}

fn bitwarden() -> Bitwarden<String> {
    Bitwarden::<String> {
        email: Email::try_from("user@example.com").expect("valid email"),
        session: "fake-session".to_string(),
    }
}

fn entry(id: &str, path: &str, public: bool) -> PunlockConfigurationEntry {
    PunlockConfigurationEntry {
        id: UuidV4::try_from(id).expect("valid uuid"),
        query: "login.password".to_string(),
        path: path.to_string(),
        links: None,
        public,
    }
}

#[tokio::test]
async fn fetch_returns_secret_from_fake_bw() {
    let _fake = FakeBw::new(
        r#"case "$1" in
  get) printf '%s' '{"login":{"password":"s3cr3t"}}' ;;
esac"#,
    );

    let secret = bitwarden()
        .fetch(&entry(
            "11111111-1111-4111-8111-111111111111",
            "x/token",
            false,
        ))
        .await
        .expect("fetch should succeed");

    assert_eq!(secret, "s3cr3t");
}

#[tokio::test]
async fn fetch_errors_when_bw_fails() {
    let _fake = FakeBw::new(
        r#"case "$1" in
  get) echo "item not found" >&2; exit 1 ;;
esac"#,
    );

    let result = bitwarden()
        .fetch(&entry(
            "11111111-1111-4111-8111-111111111111",
            "x/token",
            false,
        ))
        .await;

    assert!(result.is_err(), "fetch should fail when bw exits non-zero");
}

#[tokio::test]
async fn logout_succeeds_against_fake_bw() {
    let _fake = FakeBw::new("exit 0");
    bitwarden().logout().await.expect("logout should succeed");
}

#[tokio::test]
async fn write_secrets_writes_files_and_skips_failed_items() {
    let _fake = FakeBw::new(
        r#"case "$1" in
  sync) exit 0 ;;
  get)
    case "$3" in
      11111111-1111-4111-8111-111111111111) printf '%s' '{"login":{"password":"alpha"}}' ;;
      22222222-2222-4222-8222-222222222222) printf '%s' '{"login":{"password":"beta"}}' ;;
      33333333-3333-4333-8333-333333333333) echo "missing item" >&2; exit 1 ;;
      *) printf '%s' '{}' ;;
    esac ;;
esac"#,
    );

    let root = tempfile::tempdir().expect("tempdir");
    let store = UnixSecretStore::new(bitwarden(), root.path().to_path_buf());

    let entries = vec![
        entry("11111111-1111-4111-8111-111111111111", "a/token", false),
        entry("22222222-2222-4222-8222-222222222222", "b/token", true),
        entry("33333333-3333-4333-8333-333333333333", "c/token", false),
    ];

    store
        .write_secrets(&entries)
        .await
        .expect("write_secrets should not fail on a single bad item");

    let private = root.path().join("a/token");
    let public = root.path().join("b/token");
    let missing = root.path().join("c/token");

    assert_eq!(
        fs::read_to_string(&private).expect("private written"),
        "alpha\n"
    );
    assert_eq!(
        fs::read_to_string(&public).expect("public written"),
        "beta\n"
    );
    assert!(!missing.exists(), "failed item should not be written");

    let private_mode = fs::metadata(&private).expect("meta").permissions().mode();
    assert_eq!(private_mode & 0o777, 0o400, "private secret should be 0400");

    let public_mode = fs::metadata(&public).expect("meta").permissions().mode();
    assert_eq!(
        public_mode & 0o222,
        0,
        "public secret should still be read-only (no write bits)"
    );
}

use directories::ProjectDirs;
use rust_embed::RustEmbed;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Return the canonical assets directory path.
///
/// **Pure**: no filesystem side effects. Use [`validate_or_init_assets`]
/// at startup to ensure the directory exists before any caller reads from
/// it. Splitting these responsibilities removes the per-call
/// `create_dir_all` syscall that previously fired for every config /
/// credentials / signing-key access (CONCERNS Tech Debt #4).
///
/// Paths:
/// - macOS → ~/Library/Application Support/MyApp
/// - Linux → ~/.local/share/myapp (respects XDG_DATA_HOME)
/// - Windows → %APPDATA%\Example\MyApp
pub fn asset_dir() -> std::path::PathBuf {
    if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../../dev_assets")
    } else {
        prod_asset_dir_path()
    }
}

/// WARNING 5 fix: path-injectable inner helper. Lets tests drive the
/// side-effecting init flow against a tempdir without depending on
/// `PROJECT_ROOT` or the OS data dir.
pub(crate) fn validate_or_init_assets_at(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        tracing::info!(
            "Assets directory does not exist; creating at {}",
            path.display()
        );
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Validate (and lazily create) the assets directory. Must be called once
/// at startup before any code reads from [`asset_dir`].
///
/// Debug builds auto-create so a fresh clone + `cargo run` works without
/// manual setup; release builds auto-create on first launch. Returns
/// `io::Error` (mapped to `DeploymentError` at the call site in
/// `initialize_deployment`) instead of panicking — VKSTART-08 / VKSTART-11.
pub fn validate_or_init_assets() -> std::io::Result<()> {
    validate_or_init_assets_at(&asset_dir())
}

pub fn prod_asset_dir_path() -> std::path::PathBuf {
    ProjectDirs::from("ai", "bloop", "vibe-kanban")
        .expect("OS didn't give us a home directory")
        .data_dir()
        .to_path_buf()
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

pub fn profiles_path() -> std::path::PathBuf {
    asset_dir().join("profiles.json")
}

pub fn credentials_path() -> std::path::PathBuf {
    asset_dir().join("credentials.json")
}

pub fn trusted_keys_path() -> std::path::PathBuf {
    asset_dir().join("trusted_ed25519_public_keys.json")
}

pub fn server_signing_key_path() -> std::path::PathBuf {
    asset_dir().join("server_ed25519_signing_key")
}

pub fn relay_host_credentials_path() -> std::path::PathBuf {
    asset_dir().join("relay_host_credentials.json")
}

#[derive(RustEmbed)]
#[folder = "../../assets/sounds"]
pub struct SoundAssets;

#[derive(RustEmbed)]
#[folder = "../../assets/scripts"]
pub struct ScriptAssets;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_dir_is_pure_no_side_effects() {
        let first = asset_dir();
        let second = asset_dir();
        assert_eq!(first, second, "asset_dir() must be deterministic");

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let before = std::fs::metadata(tmp.path()).expect("metadata before");
        for _ in 0..10 {
            let _ = asset_dir();
        }
        let after = std::fs::metadata(tmp.path()).expect("metadata after");
        assert_eq!(
            before.modified().ok(),
            after.modified().ok(),
            "tempdir mtime changed after asset_dir() calls — side effect detected"
        );
    }

    #[test]
    fn validate_or_init_creates_when_missing() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let target = tmp.path().join("new-subdir");
        assert!(!target.exists(), "precondition: target must not exist");
        validate_or_init_assets_at(&target).expect("validate_or_init_assets_at should create");
        assert!(
            target.exists(),
            "target should exist after validate_or_init_assets_at"
        );
        assert!(target.is_dir(), "target should be a directory");
    }

    #[test]
    fn validate_or_init_idempotent_when_present() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let target = tmp.path().join("already-there");
        std::fs::create_dir_all(&target).expect("setup");
        assert!(target.exists());
        validate_or_init_assets_at(&target).expect("first call");
        validate_or_init_assets_at(&target).expect("second call (idempotent)");
        assert!(target.exists());
    }

    #[test]
    fn validate_or_init_errors_on_unwritable_parent() {
        let unwritable = std::path::PathBuf::from("/nonexistent-root-xyz123/sub/path");
        let result = validate_or_init_assets_at(&unwritable);
        assert!(
            result.is_err(),
            "expected validate_or_init_assets_at to Err on unwritable parent"
        );
    }

    #[test]
    fn validate_or_init_assets_returns_ok_on_real_dir() {
        validate_or_init_assets().expect(
            "validate_or_init_assets should not error in debug builds — dev_assets must be \
             creatable from PROJECT_ROOT",
        );
    }
}

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::remote_cli::error::RemoteCliError;

/// Parsed `.env.remote` contents.
#[derive(Debug)]
pub struct ParsedEnv {
    pub path: PathBuf,
    pub values: BTreeMap<String, String>,
}

const REQUIRED_KEYS: &[&str] = &["VIBEKANBAN_REMOTE_JWT_SECRET"];

/// Optional env vars: missing means the named feature is disabled, not a
/// hard failure. Order is stable so callers (and tests) see a deterministic
/// warn-stream.
const WARN_IF_MISSING: &[(&str, &str)] = &[
    ("GITHUB_OAUTH_CLIENT_ID", "GitHub OAuth login disabled"),
    ("GOOGLE_OAUTH_CLIENT_ID", "Google OAuth login disabled"),
    (
        "SELF_HOST_LOCAL_AUTH_EMAIL",
        "Self-host local auth disabled",
    ),
    ("LOOPS_EMAIL_API_KEY", "Email notifications disabled"),
    ("R2_ACCESS_KEY_ID", "R2 attachment storage disabled"),
    ("GITHUB_APP_ID", "GitHub App integration disabled"),
    ("STRIPE_SECRET_KEY", "Stripe billing disabled"),
    (
        "AZURE_STORAGE_CONNECTION_STRING",
        "Azure storage attachments disabled",
    ),
];

/// Source of truth for the env-var names compose actually consumes. Extracted
/// from `crates/remote/docker-compose.yml` at planning time. The
/// `compose_env_keys_are_known` test reads compose.yml at test time and
/// asserts every `${VAR}` expansion is a member of this list — so when
/// compose adds a new env-var, the test fails loudly and forces the init
/// template (plan 05) to be updated to match.
#[cfg(test)]
const COMPOSE_CONSUMED_KEYS: &[&str] = &[
    "DIGEST_ENABLED",
    "ELECTRIC_ROLE_PASSWORD",
    "FEATURES",
    "GITHUB_APP_ID",
    "GITHUB_APP_PRIVATE_KEY",
    "GITHUB_APP_SLUG",
    "GITHUB_APP_WEBHOOK_SECRET",
    "GITHUB_OAUTH_CLIENT_ID",
    "GITHUB_OAUTH_CLIENT_SECRET",
    "GOOGLE_OAUTH_CLIENT_ID",
    "GOOGLE_OAUTH_CLIENT_SECRET",
    "LOOPS_EMAIL_API_KEY",
    "LOOPS_INVITE_TEMPLATE_ID",
    "LOOPS_REVIEW_FAILED_TEMPLATE_ID",
    "LOOPS_REVIEW_READY_TEMPLATE_ID",
    "POSTHOG_API_ENDPOINT",
    "POSTHOG_API_KEY",
    "PUBLIC_BASE_URL",
    "R2_ACCESS_KEY_ID",
    "R2_REVIEW_BUCKET",
    "R2_REVIEW_ENDPOINT",
    "R2_SECRET_ACCESS_KEY",
    "REMOTE_SERVER_PORTS",
    "REVIEW_WORKER_BASE_URL",
    "SELF_HOST_LOCAL_AUTH_EMAIL",
    "SELF_HOST_LOCAL_AUTH_PASSWORD",
    "SENTRY_DSN_REMOTE",
    "STRIPE_FREE_SEAT_LIMIT",
    "STRIPE_SECRET_KEY",
    "STRIPE_TEAM_SEAT_PRICE_ID",
    "STRIPE_WEBHOOK_SECRET",
    "VIBEKANBAN_REMOTE_JWT_SECRET",
    "VITE_PUBLIC_REACT_VIRTUOSO_LICENSE_KEY",
    "VITE_RELAY_API_BASE_URL",
];

/// Abstracts process env lookup so the search-order logic is testable
/// without touching shared `std::env` state.
trait EnvSource {
    fn var(&self, key: &str) -> Option<String>;
}

struct ProcessEnv;
impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// Resolve the path to `.env.remote` per D-10:
///   1. `--env-file <path>` (explicit override; not existence-checked here)
///   2. `${VIBE_KANBAN_REPO}/.env.remote` (when env var is set and file exists)
///   3. `~/workspace/vibe-kanban/.env.remote` (default; when file exists)
///   4. `EnvFileNotFound` error listing every path searched.
pub fn resolve_env_file_path(explicit: Option<&Path>) -> Result<PathBuf, RemoteCliError> {
    resolve_env_file_path_with(&ProcessEnv, explicit)
}

fn resolve_env_file_path_with(
    env: &dyn EnvSource,
    explicit: Option<&Path>,
) -> Result<PathBuf, RemoteCliError> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }

    let mut searched: Vec<PathBuf> = Vec::new();

    if let Some(repo) = env.var("VIBE_KANBAN_REPO") {
        let candidate = PathBuf::from(repo).join(".env.remote");
        if candidate.exists() {
            return Ok(candidate);
        }
        searched.push(candidate);
    }

    let home = env.var("HOME").ok_or(RemoteCliError::HomeUnset)?;
    let candidate = PathBuf::from(home).join("workspace/vibe-kanban/.env.remote");
    if candidate.exists() {
        return Ok(candidate);
    }
    searched.push(candidate);

    Err(RemoteCliError::EnvFileNotFound { searched })
}

/// Resolve the repo root that anchors compose's `--project-directory` and
/// other relative paths. Same precedence as the env-file search order
/// (D-10) but applies to the repo *directory* rather than the file inside.
pub fn resolve_repo_root() -> Result<PathBuf, RemoteCliError> {
    resolve_repo_root_with(&ProcessEnv)
}

fn resolve_repo_root_with(env: &dyn EnvSource) -> Result<PathBuf, RemoteCliError> {
    if let Some(repo) = env.var("VIBE_KANBAN_REPO") {
        return Ok(PathBuf::from(repo));
    }
    let home = env.var("HOME").ok_or(RemoteCliError::HomeUnset)?;
    Ok(PathBuf::from(home).join("workspace/vibe-kanban"))
}

/// Hand-rolled KEY=VALUE parser. Skips comments + blank lines, strips
/// matched outer quotes (single OR double), splits on the first `=` only,
/// and emits a `Usage` error naming the line number for malformed lines.
///
/// We never call `dotenvy` because we don't want its `std::env::set_var`
/// side effects (compose substitutes from the file directly).
pub fn parse_env_file(path: &Path) -> Result<ParsedEnv, RemoteCliError> {
    let contents = std::fs::read_to_string(path).map_err(RemoteCliError::Io)?;
    let mut values = BTreeMap::new();
    for (lineno, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(RemoteCliError::Usage(format!(
                "Malformed env line at {}:{}: expected KEY=VALUE",
                path.display(),
                lineno + 1
            )));
        };
        let key = k.trim().to_string();
        let mut val = v.trim().to_string();
        // Strip matched outer quotes (no escape interpretation, shell-style).
        if val.len() >= 2
            && ((val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\'')))
        {
            val = val[1..val.len() - 1].to_string();
        }
        values.insert(key, val);
    }
    Ok(ParsedEnv {
        path: path.to_path_buf(),
        values,
    })
}

/// Hard-fail on missing/empty required keys (D-11). Optional keys produce a
/// `tracing::warn!` naming the disabled feature — but NEVER the value.
pub fn validate(env: &ParsedEnv) -> Result<(), RemoteCliError> {
    for &key in REQUIRED_KEYS {
        let present = env.values.get(key).map(|v| !v.is_empty()).unwrap_or(false);
        if !present {
            return Err(RemoteCliError::MissingRequiredEnv {
                key: key.to_string(),
                hint: "Generate with: openssl rand -base64 48".to_string(),
                env_file: env.path.clone(),
            });
        }
    }
    for &(key, feature) in WARN_IF_MISSING {
        let missing = env.values.get(key).map(|v| v.is_empty()).unwrap_or(true);
        if missing {
            tracing::warn!("{} missing — {} disabled", key, feature);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, io::Write};

    use tempfile::TempDir;

    use super::*;

    struct MockEnv(HashMap<String, String>);
    impl EnvSource for MockEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }
    impl MockEnv {
        fn new() -> Self {
            Self(HashMap::new())
        }
        fn with(mut self, key: &str, value: &str) -> Self {
            self.0.insert(key.to_string(), value.to_string());
            self
        }
    }

    fn write_env(dir: &TempDir, contents: &str) -> PathBuf {
        let path = dir.path().join(".env.remote");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    // ----- parser -----

    #[test]
    fn parser_handles_basic() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "KEY=value\nOTHER=v2\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.get("KEY"), Some(&"value".to_string()));
        assert_eq!(parsed.values.get("OTHER"), Some(&"v2".to_string()));
    }

    #[test]
    fn parser_skips_comments_and_blank_lines() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "# header comment\nKEY=value\n\n# another\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.len(), 1);
        assert_eq!(parsed.values.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn parser_strips_double_quotes() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "KEY=\"quoted value\"\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.get("KEY"), Some(&"quoted value".to_string()));
    }

    #[test]
    fn parser_strips_single_quotes() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "KEY='quoted'\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.get("KEY"), Some(&"quoted".to_string()));
    }

    #[test]
    fn parser_preserves_equals_in_value() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "KEY=a=b=c\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.get("KEY"), Some(&"a=b=c".to_string()));
    }

    #[test]
    fn parser_rejects_malformed_line() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "VALID=ok\nnot_a_kv_line_no_equals\n");
        let err = parse_env_file(&path).unwrap_err();
        match err {
            RemoteCliError::Usage(msg) => {
                assert!(msg.contains(":2"), "msg should name line 2; got: {msg}");
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parser_leaves_mismatched_quotes_intact() {
        let dir = TempDir::new().unwrap();
        let path = write_env(&dir, "KEY=\"unmatched\n");
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(parsed.values.get("KEY"), Some(&"\"unmatched".to_string()));
    }

    // ----- resolve_env_file_path -----

    #[test]
    fn resolve_explicit_path_returns_verbatim() {
        let explicit = Path::new("/totally/made/up/path/.env.remote");
        let env = MockEnv::new();
        let result = resolve_env_file_path_with(&env, Some(explicit)).unwrap();
        assert_eq!(result, explicit);
    }

    #[test]
    fn resolve_via_repo_env_var_when_file_exists() {
        let dir = TempDir::new().unwrap();
        write_env(&dir, "VIBEKANBAN_REMOTE_JWT_SECRET=x\n");
        let env = MockEnv::new().with("VIBE_KANBAN_REPO", dir.path().to_str().unwrap());
        let result = resolve_env_file_path_with(&env, None).unwrap();
        assert_eq!(result, dir.path().join(".env.remote"));
    }

    #[test]
    fn resolve_falls_through_when_repo_set_but_file_missing() {
        // VIBE_KANBAN_REPO is set, but .env.remote doesn't exist there.
        // HOME is set, with a default-path file. The default should win.
        let dir = TempDir::new().unwrap();
        let bogus = dir.path().join("bogus");
        fs::create_dir_all(&bogus).unwrap();
        let home = TempDir::new().unwrap();
        let default_path = home.path().join("workspace/vibe-kanban/.env.remote");
        fs::create_dir_all(default_path.parent().unwrap()).unwrap();
        fs::write(&default_path, "X=Y\n").unwrap();
        let env = MockEnv::new()
            .with("VIBE_KANBAN_REPO", bogus.to_str().unwrap())
            .with("HOME", home.path().to_str().unwrap());
        let result = resolve_env_file_path_with(&env, None).unwrap();
        assert_eq!(result, default_path);
    }

    #[test]
    fn resolve_default_under_home_when_no_repo() {
        let home = TempDir::new().unwrap();
        let default_path = home.path().join("workspace/vibe-kanban/.env.remote");
        fs::create_dir_all(default_path.parent().unwrap()).unwrap();
        fs::write(&default_path, "X=Y\n").unwrap();
        let env = MockEnv::new().with("HOME", home.path().to_str().unwrap());
        let result = resolve_env_file_path_with(&env, None).unwrap();
        assert_eq!(result, default_path);
    }

    #[test]
    fn resolve_returns_env_file_not_found_with_searched_paths() {
        let home = TempDir::new().unwrap();
        let env = MockEnv::new().with("HOME", home.path().to_str().unwrap());
        let err = resolve_env_file_path_with(&env, None).unwrap_err();
        match err {
            RemoteCliError::EnvFileNotFound { searched } => {
                assert_eq!(searched.len(), 1);
                let expected = home.path().join("workspace/vibe-kanban/.env.remote");
                assert_eq!(searched[0], expected);
            }
            other => panic!("expected EnvFileNotFound, got {other:?}"),
        }
    }

    #[test]
    fn resolve_returns_home_unset_when_no_env_vars() {
        let env = MockEnv::new();
        let err = resolve_env_file_path_with(&env, None).unwrap_err();
        assert!(
            matches!(err, RemoteCliError::HomeUnset),
            "expected HomeUnset, got {err:?}"
        );
    }

    // ----- resolve_repo_root -----

    #[test]
    fn repo_root_from_env_var() {
        let env = MockEnv::new().with("VIBE_KANBAN_REPO", "/custom/repo/path");
        let result = resolve_repo_root_with(&env).unwrap();
        assert_eq!(result, PathBuf::from("/custom/repo/path"));
    }

    #[test]
    fn repo_root_default_under_home() {
        let env = MockEnv::new().with("HOME", "/home/test");
        let result = resolve_repo_root_with(&env).unwrap();
        assert_eq!(result, PathBuf::from("/home/test/workspace/vibe-kanban"));
    }

    #[test]
    fn repo_root_home_unset_errors() {
        let env = MockEnv::new();
        let err = resolve_repo_root_with(&env).unwrap_err();
        assert!(
            matches!(err, RemoteCliError::HomeUnset),
            "expected HomeUnset, got {err:?}"
        );
    }

    // ----- validate -----

    fn parsed_with(entries: &[(&str, &str)]) -> ParsedEnv {
        let mut values = BTreeMap::new();
        for (k, v) in entries {
            values.insert((*k).to_string(), (*v).to_string());
        }
        ParsedEnv {
            path: PathBuf::from("/test/.env.remote"),
            values,
        }
    }

    #[test]
    fn validate_required_present_passes() {
        let env = parsed_with(&[("VIBEKANBAN_REMOTE_JWT_SECRET", "anything-non-empty")]);
        assert!(validate(&env).is_ok());
    }

    #[test]
    fn validate_required_missing_returns_missing_required_env() {
        let env = parsed_with(&[]);
        let err = validate(&env).unwrap_err();
        match err {
            RemoteCliError::MissingRequiredEnv {
                key,
                hint,
                env_file,
            } => {
                assert_eq!(key, "VIBEKANBAN_REMOTE_JWT_SECRET");
                assert!(
                    hint.contains("openssl rand -base64 48"),
                    "hint should suggest openssl; got: {hint}"
                );
                assert_eq!(env_file, PathBuf::from("/test/.env.remote"));
            }
            other => panic!("expected MissingRequiredEnv, got {other:?}"),
        }
    }

    #[test]
    fn validate_required_empty_returns_missing_required_env() {
        let env = parsed_with(&[("VIBEKANBAN_REMOTE_JWT_SECRET", "")]);
        let err = validate(&env).unwrap_err();
        assert!(
            matches!(err, RemoteCliError::MissingRequiredEnv { .. }),
            "empty value should be treated as missing; got {err:?}"
        );
    }

    #[test]
    fn validate_optional_missing_still_passes() {
        // Required is set; every optional is absent. validate should still Ok(()).
        let env = parsed_with(&[("VIBEKANBAN_REMOTE_JWT_SECRET", "x")]);
        assert!(validate(&env).is_ok());
    }

    // ----- compose-keys guard -----

    /// Hand-rolled `${VAR}` / `${VAR:?...}` / `${VAR:-...}` extractor —
    /// kept inside the test module so the production `env.rs` doesn't need
    /// a `regex` dep just for this guard test. Returns lower-case `name`
    /// strings; empty name segments are skipped.
    fn extract_compose_env_keys(body: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let bytes = body.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'$' && bytes[i + 1] == b'{' {
                let mut j = i + 2;
                let start = j;
                while j < bytes.len() {
                    let c = bytes[j];
                    if c == b':' || c == b'}' || c == b'-' || c == b'?' {
                        break;
                    }
                    if !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
                        break;
                    }
                    j += 1;
                }
                if j > start
                    && let Ok(name) = std::str::from_utf8(&bytes[start..j])
                {
                    out.insert(name.to_string());
                }
                i = j;
                continue;
            }
            i += 1;
        }
        out
    }

    #[test]
    fn compose_env_keys_are_known() {
        // Find the compose file. Cargo runs tests with CWD = the crate dir.
        let candidates = [
            PathBuf::from("../remote/docker-compose.yml"),
            PathBuf::from("crates/remote/docker-compose.yml"),
        ];
        let compose = candidates
            .iter()
            .find(|p| p.exists())
            .expect("docker-compose.yml not found");
        let body = fs::read_to_string(compose).unwrap();
        let known: std::collections::HashSet<&str> =
            COMPOSE_CONSUMED_KEYS.iter().copied().collect();
        let extracted = extract_compose_env_keys(&body);
        let missing: Vec<String> = extracted
            .into_iter()
            .filter(|name| !known.contains(name.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "docker-compose.yml uses env keys not in COMPOSE_CONSUMED_KEYS: {missing:?}. \
             Add them to the COMPOSE_CONSUMED_KEYS array in env.rs AND mirror the same name \
             in plan 05's init.rs template."
        );
    }
}

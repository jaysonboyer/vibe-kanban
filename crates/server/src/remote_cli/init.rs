use std::{
    io::{IsTerminal, Write},
    path::Path,
};

use base64::engine::{Engine, general_purpose::STANDARD};
use dialoguer::{Input, Password, Select, theme::ColorfulTheme};
use rand::RngCore;

use crate::remote_cli::{
    args::{AuthMethod, InitArgs},
    env::resolve_repo_root,
    error::RemoteCliError,
};

/// Env-var names consumed by `crates/remote/docker-compose.yml`.
/// Source of truth for `render_env_file`. Updated whenever compose adds/removes
/// a `${VAR}` expansion — the env.rs guard test (plan 02) fails loudly if this
/// list drifts from the live compose file.
#[cfg(test)]
const COMPOSE_CONSUMED_KEYS: &[&str] = &[
    "VIBEKANBAN_REMOTE_JWT_SECRET",
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
    "VITE_PUBLIC_REACT_VIRTUOSO_LICENSE_KEY",
    "VITE_RELAY_API_BASE_URL",
];

#[derive(Debug)]
enum AuthChoice {
    SelfHost {
        email: String,
        password: String,
    },
    GithubOauth {
        client_id: String,
        client_secret: String,
    },
    GoogleOauth {
        client_id: String,
        client_secret: String,
    },
}

/// `vibe-kanban remote init` — bootstrap a `.env.remote` file (VKREMOTE-06).
///
/// - Interactive (default): one Select for auth method, then auth-specific
///   prompts. Skips every other optional category.
/// - Non-interactive (`--non-interactive` OR stdin is not a TTY): reads from
///   flags; fails fast on missing required flag combinations.
/// - `--force`: overwrite existing file; requires "yes" confirmation on TTY.
/// - `--merge`: preserves existing values; appends only missing keys.
pub async fn run(args: InitArgs) -> Result<(), RemoteCliError> {
    let target = match args.env_file.as_deref() {
        Some(p) => p.to_path_buf(),
        None => resolve_repo_root()?.join(".env.remote"),
    };

    let target_exists = target.exists();
    match (target_exists, args.force, args.merge) {
        (false, _, _) => {}
        (true, true, false) => confirm_force_or_exit(&args)?,
        (true, false, true) => {}
        (true, false, false) => {
            return Err(RemoteCliError::ExistingEnvFile {
                path: target.clone(),
            });
        }
        (true, true, true) => {
            return Err(RemoteCliError::Usage(
                "--force and --merge are mutually exclusive".to_string(),
            ));
        }
    }

    let is_tty = std::io::stdin().is_terminal();
    let effective_non_interactive = args.non_interactive || !is_tty;

    if effective_non_interactive {
        validate_non_interactive_args(&args)?;
    }

    let jwt = args.jwt_secret.clone().unwrap_or_else(generate_jwt_secret);

    let auth = if effective_non_interactive {
        collect_auth_non_interactive(&args)?
    } else {
        collect_auth_interactive()?
    };

    let new_rendered = render_env_file(&auth, &jwt);

    let final_contents = if args.merge && target_exists {
        let existing = std::fs::read_to_string(&target).map_err(RemoteCliError::Io)?;
        merge_render(&existing, &new_rendered)
    } else {
        new_rendered
    };

    atomic_write(&target, &final_contents)?;
    println!("Created {}. Next: vibe-kanban remote up", target.display());
    Ok(())
}

/// 48 cryptographically-secure random bytes encoded as standard base64
/// (matches `openssl rand -base64 48` output shape: 64 chars).
fn generate_jwt_secret() -> String {
    let mut bytes = [0u8; 48];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    STANDARD.encode(bytes)
}

fn confirm_force_or_exit(args: &InitArgs) -> Result<(), RemoteCliError> {
    if args.non_interactive || !std::io::stdin().is_terminal() {
        return Ok(());
    }
    eprintln!(
        "WARNING: --force will overwrite the existing .env.remote, destroying any credentials currently stored there."
    );
    let answer: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Type 'yes' to confirm overwrite")
        .interact_text()
        .map_err(|e| RemoteCliError::Usage(format!("Prompt failed: {e}")))?;
    if answer.trim() != "yes" {
        return Err(RemoteCliError::Usage("Overwrite cancelled".to_string()));
    }
    Ok(())
}

fn collect_auth_interactive() -> Result<AuthChoice, RemoteCliError> {
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Authentication method?")
        .items(&[
            "Self-host (email + password) — recommended for local dev",
            "GitHub OAuth",
            "Google OAuth",
        ])
        .default(0)
        .interact()
        .map_err(|e| RemoteCliError::Usage(format!("Prompt failed: {e}")))?;

    match selection {
        0 => {
            let email = prompt_text("Self-host admin email")?;
            let password = Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Self-host admin password")
                .with_confirmation("Confirm password", "Passwords don't match")
                .interact()
                .map_err(|e| RemoteCliError::Usage(format!("Input failed: {e}")))?;
            Ok(AuthChoice::SelfHost { email, password })
        }
        1 => {
            let client_id = prompt_text("GitHub OAuth client ID")?;
            let client_secret = prompt_password("GitHub OAuth client secret")?;
            Ok(AuthChoice::GithubOauth {
                client_id,
                client_secret,
            })
        }
        2 => {
            let client_id = prompt_text("Google OAuth client ID")?;
            let client_secret = prompt_password("Google OAuth client secret")?;
            Ok(AuthChoice::GoogleOauth {
                client_id,
                client_secret,
            })
        }
        _ => unreachable!(),
    }
}

fn prompt_text(prompt: &str) -> Result<String, RemoteCliError> {
    Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact_text()
        .map_err(|e| RemoteCliError::Usage(format!("Input failed: {e}")))
}

fn prompt_password(prompt: &str) -> Result<String, RemoteCliError> {
    Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()
        .map_err(|e| RemoteCliError::Usage(format!("Input failed: {e}")))
}

fn collect_auth_non_interactive(args: &InitArgs) -> Result<AuthChoice, RemoteCliError> {
    match args.auth_method.as_ref() {
        Some(AuthMethod::SelfHost) => Ok(AuthChoice::SelfHost {
            email: args.self_host_email.clone().expect("validated"),
            password: args.self_host_password.clone().expect("validated"),
        }),
        Some(AuthMethod::GithubOauth) => Ok(AuthChoice::GithubOauth {
            client_id: args.github_client_id.clone().expect("validated"),
            client_secret: args.github_client_secret.clone().expect("validated"),
        }),
        Some(AuthMethod::GoogleOauth) => Ok(AuthChoice::GoogleOauth {
            client_id: args.google_client_id.clone().expect("validated"),
            client_secret: args.google_client_secret.clone().expect("validated"),
        }),
        None => unreachable!("validated above"),
    }
}

fn validate_non_interactive_args(args: &InitArgs) -> Result<(), RemoteCliError> {
    let Some(method) = args.auth_method.as_ref() else {
        return Err(RemoteCliError::Usage(
            "--auth-method is required in non-interactive mode (or when stdin is not a TTY)"
                .to_string(),
        ));
    };
    let (method_name, missing): (&str, Vec<&str>) = match method {
        AuthMethod::SelfHost => {
            let mut m = Vec::new();
            if args.self_host_email.is_none() {
                m.push("--self-host-email");
            }
            if args.self_host_password.is_none() {
                m.push("--self-host-password");
            }
            ("self-host", m)
        }
        AuthMethod::GithubOauth => {
            let mut m = Vec::new();
            if args.github_client_id.is_none() {
                m.push("--github-client-id");
            }
            if args.github_client_secret.is_none() {
                m.push("--github-client-secret");
            }
            ("github-oauth", m)
        }
        AuthMethod::GoogleOauth => {
            let mut m = Vec::new();
            if args.google_client_id.is_none() {
                m.push("--google-client-id");
            }
            if args.google_client_secret.is_none() {
                m.push("--google-client-secret");
            }
            ("google-oauth", m)
        }
    };
    if !missing.is_empty() {
        return Err(RemoteCliError::Usage(format!(
            "Missing required flag(s) for --auth-method={}: {}",
            method_name,
            missing.join(", ")
        )));
    }
    Ok(())
}

fn render_env_file(auth: &AuthChoice, jwt: &str) -> String {
    let mut s = String::new();
    s.push_str("# Generated by `vibe-kanban remote init`\n");
    s.push_str("# DO NOT commit this file. File perms enforced at 0600 on unix.\n\n");

    s.push_str("# Required\n");
    s.push_str(&format!("VIBEKANBAN_REMOTE_JWT_SECRET={}\n\n", jwt));

    s.push_str("# Authentication method\n");
    match auth {
        AuthChoice::SelfHost { email, password } => {
            s.push_str(&format!("SELF_HOST_LOCAL_AUTH_EMAIL={}\n", email));
            s.push_str(&format!("SELF_HOST_LOCAL_AUTH_PASSWORD={}\n\n", password));
            s.push_str(
                "# Optional: GitHub OAuth login\n# GITHUB_OAUTH_CLIENT_ID=\n# GITHUB_OAUTH_CLIENT_SECRET=\n\n",
            );
            s.push_str(
                "# Optional: Google OAuth login\n# GOOGLE_OAUTH_CLIENT_ID=\n# GOOGLE_OAUTH_CLIENT_SECRET=\n\n",
            );
        }
        AuthChoice::GithubOauth {
            client_id,
            client_secret,
        } => {
            s.push_str(&format!("GITHUB_OAUTH_CLIENT_ID={}\n", client_id));
            s.push_str(&format!("GITHUB_OAUTH_CLIENT_SECRET={}\n\n", client_secret));
            s.push_str(
                "# Optional: Self-host email + password auth\n# SELF_HOST_LOCAL_AUTH_EMAIL=\n# SELF_HOST_LOCAL_AUTH_PASSWORD=\n\n",
            );
            s.push_str(
                "# Optional: Google OAuth login\n# GOOGLE_OAUTH_CLIENT_ID=\n# GOOGLE_OAUTH_CLIENT_SECRET=\n\n",
            );
        }
        AuthChoice::GoogleOauth {
            client_id,
            client_secret,
        } => {
            s.push_str(&format!("GOOGLE_OAUTH_CLIENT_ID={}\n", client_id));
            s.push_str(&format!("GOOGLE_OAUTH_CLIENT_SECRET={}\n\n", client_secret));
            s.push_str(
                "# Optional: Self-host email + password auth\n# SELF_HOST_LOCAL_AUTH_EMAIL=\n# SELF_HOST_LOCAL_AUTH_PASSWORD=\n\n",
            );
            s.push_str(
                "# Optional: GitHub OAuth login\n# GITHUB_OAUTH_CLIENT_ID=\n# GITHUB_OAUTH_CLIENT_SECRET=\n\n",
            );
        }
    }

    s.push_str("# Optional: Email notifications via Loops\n# LOOPS_EMAIL_API_KEY=\n\n");
    s.push_str(
        "# Optional: R2 attachment storage\n# R2_ACCESS_KEY_ID=\n# R2_SECRET_ACCESS_KEY=\n# R2_REVIEW_BUCKET=\n# R2_REVIEW_ENDPOINT=\n\n",
    );
    s.push_str(
        "# Optional: GitHub App integration\n# GITHUB_APP_ID=\n# GITHUB_APP_PRIVATE_KEY=\n# GITHUB_APP_SLUG=\n# GITHUB_APP_WEBHOOK_SECRET=\n\n",
    );
    s.push_str(
        "# Optional: Stripe billing\n# STRIPE_SECRET_KEY=\n# STRIPE_WEBHOOK_SECRET=\n# STRIPE_FREE_SEAT_LIMIT=\n# STRIPE_TEAM_SEAT_PRICE_ID=\n\n",
    );

    s
}

fn merge_render(existing: &str, new_rendered: &str) -> String {
    let existing_keys: std::collections::BTreeSet<String> = existing
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start_matches(|c: char| c == '#' || c.is_whitespace());
            trimmed.split_once('=').map(|(k, _)| k.trim().to_string())
        })
        .collect();

    let mut additions = String::new();
    for line in new_rendered.lines() {
        let trimmed = line.trim_start_matches(|c: char| c == '#' || c.is_whitespace());
        if let Some((k, _)) = trimmed.split_once('=') {
            let key = k.trim();
            if !existing_keys.contains(key) {
                additions.push_str(line);
                additions.push('\n');
            }
        }
    }

    if additions.is_empty() {
        return existing.to_string();
    }
    let mut out = existing.trim_end().to_string();
    out.push_str("\n\n# Added by `vibe-kanban remote init --merge`\n");
    out.push_str(&additions);
    out
}

fn atomic_write(target: &Path, contents: &str) -> Result<(), RemoteCliError> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(RemoteCliError::Io)?;
    let tmp = dir.join(format!(".env.remote.tmp.{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp).map_err(RemoteCliError::Io)?;
        f.write_all(contents.as_bytes())
            .map_err(RemoteCliError::Io)?;
        f.sync_all().ok();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(RemoteCliError::Io)?;
    }
    std::fs::rename(&tmp, target).map_err(RemoteCliError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use base64::Engine as _;
    use tempfile::TempDir;

    use super::*;
    use crate::remote_cli::env::parse_env_file;

    // ----- JWT secret -----

    #[test]
    fn generate_jwt_secret_decodes_to_48_bytes() {
        let s = generate_jwt_secret();
        // 48 bytes encoded in base64 = ceil(48/3)*4 = 64 chars (no padding needed)
        assert_eq!(s.len(), 64, "expected 64 chars, got {} ({s})", s.len());
        let decoded = STANDARD.decode(&s).expect("decodes");
        assert_eq!(decoded.len(), 48);
    }

    #[test]
    fn generate_jwt_secret_is_random() {
        let a = generate_jwt_secret();
        let b = generate_jwt_secret();
        assert_ne!(a, b, "two calls should produce different secrets");
    }

    // ----- render_env_file -----

    #[test]
    fn render_env_file_self_host_contains_expected_keys() {
        let auth = AuthChoice::SelfHost {
            email: "a@b.c".into(),
            password: "pw1".into(),
        };
        let out = render_env_file(&auth, "JWT-SECRET-VALUE");
        assert!(out.contains("VIBEKANBAN_REMOTE_JWT_SECRET=JWT-SECRET-VALUE"));
        assert!(out.contains("SELF_HOST_LOCAL_AUTH_EMAIL=a@b.c"));
        assert!(out.contains("SELF_HOST_LOCAL_AUTH_PASSWORD=pw1"));
        // Optionals are commented out
        assert!(out.contains("# GITHUB_OAUTH_CLIENT_ID="));
        assert!(out.contains("# GOOGLE_OAUTH_CLIENT_ID="));
        assert!(out.contains("# LOOPS_EMAIL_API_KEY="));
        assert!(out.contains("# R2_ACCESS_KEY_ID="));
        assert!(out.contains("# STRIPE_SECRET_KEY="));
    }

    #[test]
    fn render_env_file_github_oauth_uncommented() {
        let auth = AuthChoice::GithubOauth {
            client_id: "gh-id".into(),
            client_secret: "gh-secret".into(),
        };
        let out = render_env_file(&auth, "X");
        assert!(out.contains("GITHUB_OAUTH_CLIENT_ID=gh-id"));
        assert!(out.contains("GITHUB_OAUTH_CLIENT_SECRET=gh-secret"));
        // Self-host commented out
        assert!(out.contains("# SELF_HOST_LOCAL_AUTH_EMAIL="));
    }

    #[test]
    fn render_env_file_round_trip_with_parser() {
        let auth = AuthChoice::SelfHost {
            email: "user@example.com".into(),
            password: "p455w0rd".into(),
        };
        let rendered = render_env_file(&auth, "JWTABC");
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.remote");
        std::fs::write(&path, &rendered).unwrap();
        let parsed = parse_env_file(&path).unwrap();
        assert_eq!(
            parsed.values.get("VIBEKANBAN_REMOTE_JWT_SECRET"),
            Some(&"JWTABC".to_string())
        );
        assert_eq!(
            parsed.values.get("SELF_HOST_LOCAL_AUTH_EMAIL"),
            Some(&"user@example.com".to_string())
        );
        assert_eq!(
            parsed.values.get("SELF_HOST_LOCAL_AUTH_PASSWORD"),
            Some(&"p455w0rd".to_string())
        );
        // Commented lines must NOT show up in the parsed map.
        assert!(!parsed.values.contains_key("GITHUB_OAUTH_CLIENT_ID"));
    }

    #[test]
    fn render_env_file_keys_are_compose_consumed() {
        let auth = AuthChoice::SelfHost {
            email: "e".into(),
            password: "p".into(),
        };
        let out = render_env_file(&auth, "x");
        let known: HashSet<&str> = COMPOSE_CONSUMED_KEYS.iter().copied().collect();
        for line in out.lines() {
            let trimmed = line.trim_start_matches(|c: char| c == '#' || c.is_whitespace());
            if let Some((k, _)) = trimmed.split_once('=') {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                assert!(
                    known.contains(key),
                    "render_env_file emits key {key:?} not in COMPOSE_CONSUMED_KEYS"
                );
            }
        }
    }

    // ----- merge -----

    #[test]
    fn merge_preserves_existing_and_appends_missing() {
        let existing = "VIBEKANBAN_REMOTE_JWT_SECRET=OLD-VALUE\nRANDOM_USER_KEY=user-data\n";
        let new = "VIBEKANBAN_REMOTE_JWT_SECRET=NEW-VALUE\nSELF_HOST_LOCAL_AUTH_EMAIL=a@b\n# GITHUB_OAUTH_CLIENT_ID=\n";
        let merged = merge_render(existing, new);
        // Existing values unchanged
        assert!(merged.contains("VIBEKANBAN_REMOTE_JWT_SECRET=OLD-VALUE"));
        assert!(merged.contains("RANDOM_USER_KEY=user-data"));
        // Missing keys appended
        assert!(merged.contains("SELF_HOST_LOCAL_AUTH_EMAIL=a@b"));
        assert!(merged.contains("# GITHUB_OAUTH_CLIENT_ID="));
        // Old value did NOT get replaced by NEW-VALUE
        assert!(!merged.contains("VIBEKANBAN_REMOTE_JWT_SECRET=NEW-VALUE"));
    }

    #[test]
    fn merge_noop_when_all_keys_already_present() {
        let existing = "VIBEKANBAN_REMOTE_JWT_SECRET=X\nSELF_HOST_LOCAL_AUTH_EMAIL=Y\n";
        let new = "VIBEKANBAN_REMOTE_JWT_SECRET=ALT\nSELF_HOST_LOCAL_AUTH_EMAIL=ALT\n";
        let merged = merge_render(existing, new);
        assert_eq!(merged, existing);
    }

    // ----- atomic_write -----

    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let target = dir.path().join(".env.remote");
        atomic_write(&target, "x=1\n").unwrap();
        assert!(target.exists());
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "expected 0600, got {:o}", mode & 0o777);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "x=1\n");
    }

    // ----- non-interactive validation -----

    fn empty_init_args() -> InitArgs {
        InitArgs {
            non_interactive: false,
            auth_method: None,
            jwt_secret: None,
            self_host_email: None,
            self_host_password: None,
            github_client_id: None,
            github_client_secret: None,
            google_client_id: None,
            google_client_secret: None,
            force: false,
            merge: false,
            env_file: None,
        }
    }

    #[test]
    fn non_interactive_requires_auth_method() {
        let mut args = empty_init_args();
        args.non_interactive = true;
        let err = validate_non_interactive_args(&args).unwrap_err();
        match err {
            RemoteCliError::Usage(msg) => assert!(msg.contains("--auth-method"), "msg: {msg}"),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn non_interactive_self_host_requires_email_password() {
        let mut args = empty_init_args();
        args.non_interactive = true;
        args.auth_method = Some(AuthMethod::SelfHost);
        let err = validate_non_interactive_args(&args).unwrap_err();
        match err {
            RemoteCliError::Usage(msg) => {
                assert!(msg.contains("--self-host-email"), "msg: {msg}");
                assert!(msg.contains("--self-host-password"), "msg: {msg}");
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn non_interactive_github_requires_client_id_secret() {
        let mut args = empty_init_args();
        args.non_interactive = true;
        args.auth_method = Some(AuthMethod::GithubOauth);
        let err = validate_non_interactive_args(&args).unwrap_err();
        let RemoteCliError::Usage(msg) = err else {
            panic!("expected Usage");
        };
        assert!(msg.contains("--github-client-id"), "msg: {msg}");
        assert!(msg.contains("--github-client-secret"), "msg: {msg}");
    }

    #[test]
    fn non_interactive_all_flags_present_ok() {
        let mut args = empty_init_args();
        args.non_interactive = true;
        args.auth_method = Some(AuthMethod::SelfHost);
        args.self_host_email = Some("a@b.c".into());
        args.self_host_password = Some("pw".into());
        assert!(validate_non_interactive_args(&args).is_ok());
    }
}

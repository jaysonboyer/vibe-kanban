use serde::Deserialize;

use crate::remote_cli::{
    args::StatusArgs,
    compose::ComposeInvocation,
    docker,
    env::{resolve_env_file_path, resolve_repo_root},
    error::RemoteCliError,
};

/// One row from `docker compose ps --format json`.
///
/// Every field is `#[serde(default)]` so a compose-version drift that drops
/// or renames a field won't panic — missing values render as `"-"` in the
/// table (T-01.1-10).
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct ComposePsRow {
    #[serde(default)]
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    service: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    health: Option<String>,
    #[serde(default)]
    publishers: Option<Vec<Publisher>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct Publisher {
    #[serde(default)]
    published_port: Option<u16>,
    #[serde(default)]
    target_port: Option<u16>,
    #[serde(default)]
    protocol: Option<String>,
}

/// `vibe-kanban remote status` — runs `docker compose ps --format json --all`
/// and renders a fixed-width table (SERVICE / STATE / HEALTH / PORTS).
///
/// D-07: reports on every service in the project namespace regardless of
/// which profile started them — no `--profile` flag is applied here.
pub async fn run(args: StatusArgs) -> Result<(), RemoteCliError> {
    let env_path = resolve_env_file_path(args.env_file.as_deref())?;
    docker::detect_docker().await?;
    let invocation = ComposeInvocation {
        repo_root: resolve_repo_root()?,
        env_file: env_path,
        project_name: args
            .project_name
            .clone()
            .unwrap_or_else(|| "vibe-kanban-remote".to_string()),
        profiles: Vec::new(),
    };
    let output = invocation
        .ps_json()
        .output()
        .await
        .map_err(RemoteCliError::Io)?;
    if !output.status.success() {
        return Err(RemoteCliError::ComposeFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let rows = parse_ps_output(&output.stdout)?;
    print!("{}", render_table(&rows));
    Ok(())
}

/// Tolerates both array-form (newer compose) and NDJSON (older compose).
fn parse_ps_output(stdout: &[u8]) -> Result<Vec<ComposePsRow>, RemoteCliError> {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed).map_err(|e| {
            RemoteCliError::Usage(format!(
                "Failed to parse `compose ps --format json` array: {e}"
            ))
        });
    }
    let mut rows = Vec::new();
    for line in trimmed.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: ComposePsRow = serde_json::from_str(line).map_err(|e| {
            RemoteCliError::Usage(format!(
                "Failed to parse `compose ps --format json` line: {e}"
            ))
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn format_ports(pubs: Option<&[Publisher]>) -> String {
    let Some(list) = pubs else {
        return "-".to_string();
    };
    if list.is_empty() {
        return "-".to_string();
    }
    list.iter()
        .map(|p| {
            format!(
                "{}->{}/{}",
                p.published_port
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                p.target_port
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                p.protocol.as_deref().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_table(rows: &[ComposePsRow]) -> String {
    if rows.is_empty() {
        return "No services running\n".to_string();
    }
    let header = ["SERVICE", "STATE", "HEALTH", "PORTS"];
    let cells: Vec<[String; 4]> = rows
        .iter()
        .map(|r| {
            [
                r.service.clone(),
                r.state.clone(),
                r.health.clone().unwrap_or_else(|| "-".into()),
                format_ports(r.publishers.as_deref()),
            ]
        })
        .collect();
    let widths: [usize; 4] = std::array::from_fn(|i| {
        std::iter::once(header[i].len())
            .chain(cells.iter().map(|row| row[i].len()))
            .max()
            .unwrap_or(0)
    });
    let mut s = String::new();
    push_row(&mut s, &header.map(String::from), &widths);
    for row in &cells {
        push_row(&mut s, row, &widths);
    }
    s
}

fn push_row(s: &mut String, row: &[String; 4], widths: &[usize; 4]) {
    for i in 0..4 {
        if i > 0 {
            s.push_str("  ");
        }
        s.push_str(&format!("{:<width$}", row[i], width = widths[i]));
    }
    s.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_one_row() -> &'static str {
        r#"[{
          "Name": "vibe-kanban-remote-remote-db-1",
          "Service": "remote-db",
          "State": "running",
          "Health": "healthy",
          "Publishers": [{ "PublishedPort": 5433, "TargetPort": 5432, "Protocol": "tcp" }]
        }]"#
    }

    #[test]
    fn parse_ps_json_full_row() {
        let rows = parse_ps_output(fixture_one_row().as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.service, "remote-db");
        assert_eq!(row.state, "running");
        assert_eq!(row.health.as_deref(), Some("healthy"));
        let pubs = row.publishers.as_ref().unwrap();
        assert_eq!(pubs[0].published_port, Some(5433));
        assert_eq!(pubs[0].target_port, Some(5432));
        assert_eq!(pubs[0].protocol.as_deref(), Some("tcp"));
    }

    #[test]
    fn parse_ps_json_missing_health() {
        let json = r#"[{ "Service": "x", "State": "running" }]"#;
        let rows = parse_ps_output(json.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].health, None);
    }

    #[test]
    fn parse_ps_json_missing_publishers() {
        let json = r#"[{ "Service": "x", "State": "running", "Health": "healthy" }]"#;
        let rows = parse_ps_output(json.as_bytes()).unwrap();
        assert!(rows[0].publishers.is_none());
    }

    #[test]
    fn parse_ps_json_empty_array() {
        let rows = parse_ps_output(b"[]").unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn parse_ps_ndjson_fallback() {
        let ndjson =
            "{\"Service\":\"a\",\"State\":\"running\"}\n{\"Service\":\"b\",\"State\":\"exited\"}\n";
        let rows = parse_ps_output(ndjson.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].service, "a");
        assert_eq!(rows[1].state, "exited");
    }

    #[test]
    fn parse_ps_empty_stdout_yields_no_rows() {
        let rows = parse_ps_output(b"   \n").unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn format_ports_none_returns_dash() {
        assert_eq!(format_ports(None), "-");
    }

    #[test]
    fn format_ports_empty_returns_dash() {
        assert_eq!(format_ports(Some(&[])), "-");
    }

    #[test]
    fn format_ports_one_publisher() {
        let pubs = vec![Publisher {
            published_port: Some(5433),
            target_port: Some(5432),
            protocol: Some("tcp".into()),
        }];
        assert_eq!(format_ports(Some(&pubs)), "5433->5432/tcp");
    }

    #[test]
    fn format_ports_two_publishers_comma_separated() {
        let pubs = vec![
            Publisher {
                published_port: Some(5433),
                target_port: Some(5432),
                protocol: Some("tcp".into()),
            },
            Publisher {
                published_port: Some(3000),
                target_port: Some(8081),
                protocol: Some("tcp".into()),
            },
        ];
        assert_eq!(format_ports(Some(&pubs)), "5433->5432/tcp, 3000->8081/tcp");
    }

    #[test]
    fn render_table_empty_says_no_services() {
        assert!(render_table(&[]).contains("No services running"));
    }

    #[test]
    fn render_table_one_row_contains_headers_and_data() {
        let rows = parse_ps_output(fixture_one_row().as_bytes()).unwrap();
        let table = render_table(&rows);
        for needle in [
            "SERVICE",
            "STATE",
            "HEALTH",
            "PORTS",
            "remote-db",
            "running",
            "healthy",
            "5433->5432/tcp",
        ] {
            assert!(
                table.contains(needle),
                "table missing {needle:?}; got:\n{table}"
            );
        }
        // Each non-empty line (pre-trim) has the same total width: every
        // cell is left-padded to its column max so columns stay aligned.
        let widths: Vec<usize> = table
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.len())
            .collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "uneven column widths: {widths:?}\n{table}"
        );
    }
}

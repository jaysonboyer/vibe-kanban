use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::remote_cli::error::RemoteCliError;

/// Parsed `.env.remote` contents.
#[derive(Debug)]
pub struct ParsedEnv {
    pub path: PathBuf,
    pub values: BTreeMap<String, String>,
}

#[allow(dead_code)]
pub fn resolve_env_file_path(_explicit: Option<&Path>) -> Result<PathBuf, RemoteCliError> {
    todo!("plan 01.1-02")
}

#[allow(dead_code)]
pub fn parse_env_file(_path: &Path) -> Result<ParsedEnv, RemoteCliError> {
    todo!("plan 01.1-02")
}

#[allow(dead_code)]
pub fn validate(_env: &ParsedEnv) -> Result<(), RemoteCliError> {
    todo!("plan 01.1-02")
}

#[allow(dead_code)]
pub fn resolve_repo_root() -> Result<PathBuf, RemoteCliError> {
    todo!("plan 01.1-02")
}

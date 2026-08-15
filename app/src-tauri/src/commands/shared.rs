use std::path::PathBuf;

use dune_manager_core::models::CommandFailure;
use dune_manager_core::orchestration::{RusshRunner, RusshTarget};

/// Status/discovery calls are expected to be quick Kubernetes reads. Keeping
/// their individual SSH commands short prevents a reconciling battlegroup
/// from pinning a desktop refresh worker indefinitely.
pub const REMOTE_READ_COMMAND_TIMEOUT_SECONDS: u64 = 15;
/// Lifecycle wrapper and polling commands can take longer than status reads,
/// but should still fail predictably when the remote session stops making
/// meaningful progress.
pub const REMOTE_ACTION_COMMAND_TIMEOUT_SECONDS: u64 = 60;

pub fn remote_runner(
    host: String,
    user: String,
    key_path: String,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    let mut target = RusshTarget::new(PathBuf::from(key_path), user, host);
    if let Some(p) = port {
        target.port = p;
    }
    target.validate().map_err(|err| err.message)?;
    Ok(RusshRunner::new(target))
}

pub fn runner_for_remote_kind(
    _server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    let key_path = key_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SSH private key is required for remote Ubuntu servers.".to_string())?;
    remote_runner(host, user, key_path, port)
}

pub fn runner_for_remote_read(
    server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    runner_for_remote_kind(server_type, host, user, key_path, port)
        .map(|runner| runner.with_command_timeout_seconds(REMOTE_READ_COMMAND_TIMEOUT_SECONDS))
}

pub fn runner_for_remote_action(
    server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    runner_for_remote_kind(server_type, host, user, key_path, port)
        .map(|runner| runner.with_command_timeout_seconds(REMOTE_ACTION_COMMAND_TIMEOUT_SECONDS))
}

pub fn command_error_message(err: CommandFailure) -> String {
    let mut parts = vec![err.message];
    if !err.stderr.trim().is_empty() {
        parts.push(err.stderr);
    }
    if !err.stdout.trim().is_empty() {
        parts.push(err.stdout);
    }
    parts.join("\n")
}

pub fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

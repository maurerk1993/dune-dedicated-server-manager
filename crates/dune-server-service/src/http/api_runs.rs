use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::scheduler::Task;
use crate::store::TaskTrigger;

use super::{AppState, VERSION};

pub async fn root() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "dune-server-service",
        "version": VERSION,
    }))
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub version: &'static str,
    pub now: String,
}

pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        ok: true,
        version: VERSION,
        now: Utc::now().to_rfc3339(),
    })
}

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub limit: Option<u32>,
    pub task: Option<String>,
}

pub async fn list_runs(
    State(state): State<AppState>,
    Query(q): Query<RunsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let runs = state
        .store
        .list_runs(q.limit.unwrap_or(50), q.task.as_deref())?;
    Ok(Json(runs))
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<u32>,
    #[serde(rename = "runId")]
    pub run_id: Option<i64>,
}

pub async fn list_logs(
    State(state): State<AppState>,
    Query(q): Query<LogsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let logs = state.store.list_logs(q.limit.unwrap_or(200), q.run_id)?;
    Ok(Json(logs))
}

#[derive(Debug, Deserialize)]
pub struct TriggerRequest {
    pub task: String,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

pub async fn trigger(
    State(state): State<AppState>,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tasks: Vec<Arc<dyn Task>> = crate::tasks::build_all(state.env.clone());
    let task = tasks
        .into_iter()
        .find(|t| t.id() == req.task)
        .ok_or_else(|| ApiError::not_found(format!("unknown task: {}", req.task)))?;

    let runner = state.runner.clone();
    let options = req.options.clone();
    tokio::spawn(async move {
        if let Err(err) = runner.run(task, TaskTrigger::Manual, false, options).await {
            tracing::error!(error = %err, "manual trigger failed");
        }
    });

    Ok(Json(serde_json::json!({"ok": true, "task": req.task})))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub restart_hour: u32,
    pub restart_minute: u32,
    pub restart_warning_frequency_secs: u64,
    pub restart_warning_duration_secs: u64,
    pub restart_tz: String,
    /// Master switch for the daily restart.
    pub restart_enabled: bool,
    pub welcome_message_enabled: bool,
    pub welcome_package_enabled: bool,
    pub welcome_package_version: String,
    pub welcome_package_actions_json: String,
    pub welcome_package_items_json: String,
    pub welcome_whisper_source_player: String,
    pub welcome_message: String,
    /// True if any saved override differs from the active TaskEnv values —
    /// signals to the UI that a service restart is needed to pick them up.
    pub restart_required: bool,
}

pub async fn get_config(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let env = &state.env;
    let stored_hour = state
        .store
        .get_config_i64("restart_hour")?
        .map(|v| v as u32);
    let stored_minute = state
        .store
        .get_config_i64("restart_minute")?
        .map(|v| v as u32);
    let stored_freq = state
        .store
        .get_config_i64("restart_warning_frequency_secs")?
        .map(|v| v as u64);
    let stored_dur = state
        .store
        .get_config_i64("restart_warning_duration_secs")?
        .map(|v| v as u64);
    let stored_restart_enabled = state
        .store
        .get_config_i64("restart_enabled")?
        .map(|v| v != 0);
    let stored_tz = state.store.get_config("restart_tz")?;
    let stored_welcome_enabled = state
        .store
        .get_config_i64("welcome_package_enabled")?
        .map(|v| v != 0);
    let stored_welcome_message_enabled = state
        .store
        .get_config_i64("welcome_message_enabled")?
        .map(|v| v != 0);
    let stored_welcome_actions_json = state.store.get_config("welcome_package_actions_json")?;
    let stored_welcome_items_json = state.store.get_config("welcome_package_items_json")?;
    let stored_welcome_whisper_source = state.store.get_config("welcome_whisper_source_player")?;
    let stored_welcome_message = state.store.get_config("welcome_message")?;

    let restart_required = stored_hour.map(|v| v != env.restart_hour).unwrap_or(false)
        || stored_minute
            .map(|v| v != env.restart_minute)
            .unwrap_or(false)
        || stored_freq
            .map(|v| v != env.restart_warning_frequency_secs)
            .unwrap_or(false)
        || stored_dur
            .map(|v| v != env.restart_warning_duration_secs)
            .unwrap_or(false)
        || stored_restart_enabled
            .map(|v| v != env.restart_enabled)
            .unwrap_or(false)
        || stored_tz
            .as_deref()
            .map(|v| v != env.restart_tz.name())
            .unwrap_or(false)
        || stored_welcome_enabled
            .map(|v| v != env.welcome_package_enabled)
            .unwrap_or(false)
        || stored_welcome_message_enabled
            .map(|v| v != env.welcome_message_enabled)
            .unwrap_or(false)
        || stored_welcome_actions_json
            .as_deref()
            .map(|v| v != env.welcome_package_actions_json)
            .unwrap_or_else(|| {
                stored_welcome_items_json
                    .as_deref()
                    .map(|v| v != env.welcome_package_actions_json)
                    .unwrap_or(false)
            })
        || stored_welcome_whisper_source
            .as_deref()
            .map(|v| v != env.welcome_whisper_source_player)
            .unwrap_or(false)
        || stored_welcome_message
            .as_deref()
            .map(|v| v != env.welcome_message)
            .unwrap_or(false);

    Ok(Json(ConfigResponse {
        restart_hour: env.restart_hour,
        restart_minute: env.restart_minute,
        restart_warning_frequency_secs: env.restart_warning_frequency_secs,
        restart_warning_duration_secs: env.restart_warning_duration_secs,
        restart_tz: env.restart_tz.name().to_string(),
        restart_enabled: env.restart_enabled,
        welcome_message_enabled: env.welcome_message_enabled,
        welcome_package_enabled: env.welcome_package_enabled,
        welcome_package_version: env.welcome_package_version.clone(),
        welcome_package_actions_json: env.welcome_package_actions_json.clone(),
        welcome_package_items_json: env.welcome_package_actions_json.clone(),
        welcome_whisper_source_player: env.welcome_whisper_source_player.clone(),
        welcome_message: env.welcome_message.clone(),
        restart_required,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdate {
    pub restart_hour: Option<u32>,
    pub restart_minute: Option<u32>,
    pub restart_warning_frequency_secs: Option<u64>,
    pub restart_warning_duration_secs: Option<u64>,
    pub restart_tz: Option<String>,
    pub restart_enabled: Option<bool>,
    pub welcome_message_enabled: Option<bool>,
    pub welcome_package_enabled: Option<bool>,
    pub welcome_package_version: Option<String>,
    pub welcome_package_actions_json: Option<String>,
    pub welcome_package_items_json: Option<String>,
    pub welcome_whisper_source_player: Option<String>,
    pub welcome_message: Option<String>,
}

pub async fn set_config(
    State(state): State<AppState>,
    Json(req): Json<ConfigUpdate>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(h) = req.restart_hour {
        if h > 23 {
            return Err(ApiError::bad_request("restart_hour must be 0..=23"));
        }
        state.store.set_config("restart_hour", &h.to_string())?;
    }
    if let Some(m) = req.restart_minute {
        if m > 59 {
            return Err(ApiError::bad_request("restart_minute must be 0..=59"));
        }
        state.store.set_config("restart_minute", &m.to_string())?;
    }
    if let Some(s) = req.restart_warning_frequency_secs {
        if s == 0 {
            return Err(ApiError::bad_request(
                "restart_warning_frequency_secs must be greater than 0",
            ));
        }
        state
            .store
            .set_config("restart_warning_frequency_secs", &s.to_string())?;
    }
    if let Some(s) = req.restart_warning_duration_secs {
        if s == 0 {
            return Err(ApiError::bad_request(
                "restart_warning_duration_secs must be greater than 0",
            ));
        }
        state
            .store
            .set_config("restart_warning_duration_secs", &s.to_string())?;
    }
    if let Some(tz) = req.restart_tz.as_deref() {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(ApiError::bad_request(format!(
                "invalid IANA timezone: {tz}"
            )));
        }
        state.store.set_config("restart_tz", tz)?;
    }
    if let Some(enabled) = req.restart_enabled {
        state
            .store
            .set_config("restart_enabled", if enabled { "1" } else { "0" })?;
    }
    if let Some(enabled) = req.welcome_package_enabled {
        state
            .store
            .set_config("welcome_package_enabled", if enabled { "1" } else { "0" })?;
    }
    if let Some(enabled) = req.welcome_message_enabled {
        state
            .store
            .set_config("welcome_message_enabled", if enabled { "1" } else { "0" })?;
    }
    if let Some(version) = req.welcome_package_version.as_deref() {
        let trimmed = version.trim();
        if trimmed.is_empty() {
            return Err(ApiError::bad_request(
                "welcome_package_version must not be empty",
            ));
        }
        // Pinned to the daemon's current env value while the feature is
        // experimental. A no-op when the client echoes the same value back
        // (e.g. from a prior GET); a 400 on mismatch so silent drift is
        // visible instead of looking like a successful save.
        if trimmed != state.env.welcome_package_version {
            return Err(ApiError::bad_request(format!(
                "welcome_package_version is currently fixed to {} and cannot be changed",
                state.env.welcome_package_version
            )));
        }
    }
    if let Some(raw) = req.welcome_package_actions_json.as_deref() {
        let trimmed = raw.trim();
        crate::tasks::welcome_package::parse_welcome_actions(trimmed)
            .map_err(|err| ApiError::bad_request(err.to_string()))?;
        state
            .store
            .set_config("welcome_package_actions_json", trimmed)?;
    } else if let Some(raw) = req.welcome_package_items_json.as_deref() {
        // Legacy alias: only applied when actions_json is absent so a mixed
        // payload from a stale client can't clobber the canonical field.
        let trimmed = raw.trim();
        crate::tasks::welcome_package::parse_welcome_actions(trimmed)
            .map_err(|err| ApiError::bad_request(err.to_string()))?;
        state
            .store
            .set_config("welcome_package_actions_json", trimmed)?;
    }
    if let Some(source) = req.welcome_whisper_source_player.as_deref() {
        state
            .store
            .set_config("welcome_whisper_source_player", source.trim())?;
    }
    if let Some(message) = req.welcome_message.as_deref() {
        if message.len() > 1000 {
            return Err(ApiError::bad_request(
                "welcome_message must be <= 1000 characters",
            ));
        }
        state.store.set_config("welcome_message", message)?;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_timezones() -> impl IntoResponse {
    let names: Vec<&'static str> = chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name()).collect();
    Json(names)
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            message: msg.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
        Self::internal(scrubbed)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({"error": self.message})),
        )
            .into_response()
    }
}

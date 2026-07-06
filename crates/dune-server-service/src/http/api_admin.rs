use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use futures::FutureExt;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::admin::{commands, data, players, MqPublisher};
use crate::store::AdminHistoryFilter;

use super::api_runs::ApiError;
use super::AppState;

pub async fn list_commands() -> impl IntoResponse {
    Json(commands::SPECS)
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<u32>,
}

pub async fn search_items(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_items(&query, limit))
}

pub async fn search_vehicles(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(20);
    Json(data::search_vehicles(&query, limit))
}

pub async fn search_skill_modules(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_skill_modules(&query, limit))
}

pub async fn search_journey_nodes(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(80);
    Json(data::search_journey_nodes(&query, limit))
}

pub async fn search_xp_event_tags(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_xp_event_tags(&query, limit))
}

pub async fn search_players(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    let result = std::panic::AssertUnwindSafe(players::search_players(
        &state.env.pg,
        &state.env.cluster,
        &query,
        limit,
    ))
    .catch_unwind()
    .await;
    let rows = match result {
        Ok(Ok(rows)) => rows,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            tracing::error!("admin players route panicked");
            return Err(ApiError::internal("admin players route panicked"));
        }
    };
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct PlayerLocationQuery {
    #[serde(rename = "flsId")]
    pub fls_id: String,
}

pub async fn player_location(
    State(state): State<AppState>,
    Query(q): Query<PlayerLocationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    use crate::postgres::PositionProbe;
    let cluster = state.env.cluster.get().await?;
    let probe =
        crate::postgres::get_player_location(&state.env.pg, &cluster.namespace, &q.fls_id).await?;
    match probe {
        PositionProbe::Found(p) => Ok(Json(p).into_response()),
        PositionProbe::NoRow => Err(ApiError::not_found(format!(
            "no live pawn for fls_id {} — player may be offline",
            q.fls_id
        ))),
    }
}

#[derive(Debug, Deserialize)]
pub struct SpecializationQuery {
    #[serde(rename = "flsId")]
    pub fls_id: String,
}

pub async fn specialization(
    State(state): State<AppState>,
    Query(q): Query<SpecializationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let fls_id = q.fls_id.trim();
    if fls_id.is_empty() {
        return Err(ApiError::bad_request("flsId must not be empty"));
    }

    let cluster = state.env.cluster.get().await?;
    let Some(player) =
        crate::postgres::resolve_admin_player_by_fls(&state.env.pg, &cluster.namespace, fls_id)
            .await?
    else {
        return Err(ApiError::not_found(format!("player {fls_id} was not found")));
    };
    let response =
        crate::postgres::get_player_specialization(&state.env.pg, &cluster.namespace, player)
            .await?;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSpecializationLevelRequest {
    pub fls_id: String,
    pub track_type: String,
    pub level: i32,
}

pub async fn set_specialization_level(
    State(state): State<AppState>,
    Json(req): Json<SetSpecializationLevelRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let fls_id = req.fls_id.trim();
    if fls_id.is_empty() {
        return Err(ApiError::bad_request("flsId must not be empty"));
    }
    let Some(track) = crate::postgres::canonical_specialization_track(&req.track_type) else {
        return Err(ApiError::bad_request(format!(
            "unknown specialization track {}",
            req.track_type
        )));
    };

    let cluster = state.env.cluster.get().await?;
    let Some(player) =
        crate::postgres::resolve_admin_player_by_fls(&state.env.pg, &cluster.namespace, fls_id)
            .await?
    else {
        return Err(ApiError::not_found(format!("player {fls_id} was not found")));
    };
    if crate::postgres::is_player_online(&player.online) {
        return Err(ApiError::bad_request(format!(
            "{} is currently online. Specialization edits are offline-only; have the player fully log out first.",
            player.name
        )));
    }
    if player.controller_id <= 0 {
        return Err(ApiError::bad_request(format!(
            "{} does not have a valid controller id",
            player.name
        )));
    }

    let payload = json!({
        "flsId": fls_id,
        "characterName": player.name.clone(),
        "controllerId": player.controller_id,
        "trackType": track,
        "requestedLevel": req.level,
    });
    let result = crate::postgres::set_specialization_level(
        &state.env.pg,
        &cluster.namespace,
        player.controller_id,
        track,
        req.level,
    )
    .await;

    let (ok, output, error, inner) = match result {
        Ok(done) => {
            let inner = json!({
                "flsId": fls_id,
                "characterName": player.name.clone(),
                "controllerId": player.controller_id,
                "trackType": done.track_type,
                "level": done.level,
                "xp": done.xp,
            });
            (
                true,
                format!(
                    "Set {} specialization to level {} ({} xp). Player must fully relog before this appears in-game.",
                    done.track_type, done.level, done.xp
                ),
                None,
                inner,
            )
        }
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed), payload.clone())
        }
    };

    let _ = state.store.record_admin_command(
        "SpecializationLevelXp.Set",
        &inner,
        ok,
        error.as_deref(),
    );

    Ok(Json(json!({
        "ok": ok,
        "command": "SpecializationLevelXp.Set",
        "output": output,
        "error": error,
        "inner": inner,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantQualityItemRequest {
    pub fls_id: String,
    pub item_id: String,
    pub quantity: i64,
    pub quality: i64,
}

pub async fn grant_quality_item(
    State(state): State<AppState>,
    Json(req): Json<GrantQualityItemRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let fls_id = req.fls_id.trim();
    if fls_id.is_empty() {
        return Err(ApiError::bad_request("flsId must not be empty"));
    }
    let item_id = req.item_id.trim();
    if item_id.is_empty() {
        return Err(ApiError::bad_request("itemId must not be empty"));
    }
    if item_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::bad_request(
            "itemId must be a template class name, not a numeric id",
        ));
    }
    let quantity = req.quantity.clamp(1, 10_000);
    if !(1..=5).contains(&req.quality) {
        return Err(ApiError::bad_request(
            "quality must be 1..=5 for custom-tier item grants",
        ));
    }
    let Some(item) = data::find_item(item_id) else {
        return Err(ApiError::bad_request(format!(
            "item {item_id} was not found in the catalog"
        )));
    };
    if !item.gradeable {
        return Err(ApiError::bad_request(format!(
            "{} does not support item quality tiers",
            item.name
        )));
    }

    let cluster = state.env.cluster.get().await?;
    let Some(player) =
        crate::postgres::resolve_admin_player_by_fls(&state.env.pg, &cluster.namespace, fls_id)
            .await?
    else {
        return Err(ApiError::not_found(format!("player {fls_id} was not found")));
    };
    if crate::postgres::is_player_online(&player.online) {
        return Err(ApiError::bad_request(format!(
            "{} is currently online. Custom-tier item grants are offline-only; have the player fully log out first.",
            player.name
        )));
    }

    let Some(backpack) =
        crate::postgres::resolve_account_backpack(&state.env.pg, &cluster.namespace, player.account_id)
            .await?
    else {
        return Err(ApiError::not_found(format!(
            "no backpack inventory found for {}",
            player.name
        )));
    };

    let payload = json!({
        "flsId": fls_id,
        "characterName": player.name.clone(),
        "accountId": player.account_id,
        "inventoryId": backpack.inventory_id,
        "itemId": item.id.clone(),
        "itemName": item.name.clone(),
        "quantity": quantity,
        "quality": req.quality,
    });
    let grant = crate::postgres::BackpackGrantItem {
        template_id: item.id.clone(),
        quantity,
        stats_json: crate::postgres::grant_item_stats_json(item.stack_max).to_string(),
        quality_level: req.quality,
    };
    let result = crate::postgres::insert_items_to_backpack(
        &state.env.pg,
        &cluster.namespace,
        backpack.inventory_id,
        &[grant],
    )
    .await;

    let (ok, output, error, inner) = match result {
        Ok(ids) => {
            let inner = json!({
                "flsId": fls_id,
                "characterName": player.name.clone(),
                "accountId": player.account_id,
                "inventoryId": backpack.inventory_id,
                "itemId": item.id.clone(),
                "itemName": item.name.clone(),
                "quantity": quantity,
                "quality": req.quality,
                "itemIds": ids,
            });
            (
                true,
                format!(
                    "Gave {} x {} at quality {} to {}. Player must fully relog before this appears in-game.",
                    quantity, item.name, req.quality, player.name
                ),
                None,
                inner,
            )
        }
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed), payload)
        }
    };

    let _ = state.store.record_admin_command(
        "AddItemToInventory.Quality",
        &inner,
        ok,
        error.as_deref(),
    );

    Ok(Json(json!({
        "ok": ok,
        "command": "AddItemToInventory.Quality",
        "output": output,
        "error": error,
        "inner": inner,
    })))
}

pub async fn cluster(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let c = state.env.cluster.get().await?;
    Ok(Json(serde_json::json!({
        "namespace": c.namespace,
        "mqPod": c.mq_pod,
        "dbPod": c.db_pod,
        "serviceVersion": super::VERSION,
    })))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u32>,
}

pub async fn history(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let list = state
        .store
        .list_admin_commands(AdminHistoryFilter { limit: q.limit })?;
    Ok(Json(list))
}

pub async fn welcome_grants(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let list = state.store.list_welcome_grants(q.limit.unwrap_or(100))?;
    Ok(Json(list))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryWelcomeGrantRequest {
    pub player_id: String,
    pub package_version: String,
    pub account_id: i64,
}

/// Clears a failed welcome-grant ledger row so the next scan re-attempts it.
pub async fn retry_welcome_grant(
    State(state): State<AppState>,
    Json(req): Json<RetryWelcomeGrantRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let player_id = req.player_id.trim();
    if player_id.is_empty() {
        return Err(ApiError::bad_request("playerId must not be empty"));
    }
    let package_version = req.package_version.trim();
    if package_version.is_empty() {
        return Err(ApiError::bad_request("packageVersion must not be empty"));
    }
    let removed =
        state
            .store
            .delete_welcome_grant(player_id, package_version, req.account_id)?;
    Ok(Json(serde_json::json!({ "ok": removed > 0, "removed": removed })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeWhisperRequest {
    pub recipient_player_id: String,
    #[serde(default)]
    pub source_player_id: String,
    pub message: String,
}

pub async fn welcome_whisper(
    State(state): State<AppState>,
    Json(req): Json<WelcomeWhisperRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let recipient = req.recipient_player_id.trim();
    if recipient.is_empty() {
        return Err(ApiError::bad_request(
            "recipient_player_id must not be empty",
        ));
    }
    let message = req.message.trim();
    if message.is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }
    if message.len() > 1000 {
        return Err(ApiError::bad_request("message must be <= 1000 characters"));
    }

    let cluster = state.env.cluster.get().await?;
    let result = crate::tasks::welcome_package::send_welcome_whisper_now(
        &state.env,
        &cluster.namespace,
        req.source_player_id.trim(),
        recipient,
        message,
    )
    .await;

    let (ok, output, error) = match result {
        Ok(pr) => (pr.ok, pr.output, None),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let payload = serde_json::json!({
        "sourcePlayerId": req.source_player_id.trim(),
        "recipientPlayerId": recipient,
        "message": message,
    });
    let _ = state.store.record_admin_command(
        "WelcomePackage.SendWelcomeWhisper",
        &payload,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": "WelcomePackage.SendWelcomeWhisper",
        "output": output,
        "error": error,
        "inner": payload,
    })))
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub command: String,
    #[serde(default)]
    pub fields: Map<String, Value>,
}

pub async fn publish(
    State(state): State<AppState>,
    Json(req): Json<PublishRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let inner = commands::validate_and_build(&req.command, &req.fields)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    let publisher: &MqPublisher = &state.env.mq;
    let result = publisher.publish_inner(&inner, &req.command).await;

    let (ok, output, error) = match result {
        Ok(pr) => (pr.ok, pr.output, None),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let _ = state.store.record_admin_command(
        &req.command,
        &inner,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": req.command,
        "output": output,
        "error": error,
        "inner": inner,
    })))
}

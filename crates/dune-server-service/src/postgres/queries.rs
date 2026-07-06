use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use super::conn::PgClient;

#[derive(Debug, Clone, Serialize)]
pub struct PlayerLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(rename = "dimensionIndex")]
    pub dimension_index: Option<i32>,
    #[serde(rename = "partitionId")]
    pub partition_id: Option<i64>,
    /// Pawn actor class — useful sanity for the UI ("…DunePlayerCharacter_C").
    pub source: String,
}

// The live player position is on `dune.actors`, not `dune.player_state`. The
// pawn actor is referenced from `player_state.player_pawn_id`. Its `transform`
// is a composite `(location:(x,y,z), rotation:(x,y,z,w))`. Confirmed via
// schema probe 2026-05-26 against funcom-seabass-sh-* on the LAN test host.
const PLAYER_POSITION_SQL: &str = "
SELECT
    ((a.transform).location).x::float8 AS x,
    ((a.transform).location).y::float8 AS y,
    ((a.transform).location).z::float8 AS z,
    a.dimension_index,
    a.partition_id,
    a.class
FROM dune.player_state ps
JOIN dune.actors a       ON a.id = ps.player_pawn_id
JOIN dune.encrypted_accounts acct ON acct.id = ps.account_id
WHERE acct.\"user\"::text = $1
LIMIT 1
";

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    #[serde(rename = "flsId")]
    pub fls_id: String,
    pub name: String,
    pub online: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: String,
    pub level: Option<i32>,
    #[serde(rename = "partitionId")]
    pub partition_id: Option<i64>,
    #[serde(rename = "accountId")]
    pub account_id: i64,
    #[serde(rename = "pawnId")]
    pub pawn_id: i64,
    #[serde(rename = "controllerId")]
    pub controller_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlayerTarget {
    pub fls_id: String,
    pub name: String,
    pub online: String,
    pub account_id: i64,
    pub pawn_id: i64,
    pub controller_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecializationTrack {
    pub track_type: String,
    pub xp: i64,
    pub level: f64,
    pub xp_max: i32,
    pub level_max: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSpecialization {
    pub player: AdminPlayerTarget,
    pub tracks: Vec<SpecializationTrack>,
    pub keystones_total: i64,
    pub keystones_max: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSpecializationResult {
    pub track_type: String,
    pub level: i32,
    pub xp: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeAccount {
    pub account_id: i64,
    pub fls_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBackpack {
    pub inventory_id: i64,
    pub character_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlayer {
    pub account_id: i64,
    pub fls_id: String,
    pub funcom_id: String,
    pub character_name: String,
}

#[derive(Debug, Clone)]
pub struct BackpackGrantItem {
    pub template_id: String,
    pub quantity: i64,
    pub stats_json: String,
    pub quality_level: i64,
}

const PLAYER_STATE_COLUMN_SQL: &str = "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'dune'
  AND table_name = 'player_state'
  AND column_name = ANY($1)
";

const WELCOME_ACCOUNTS_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id
FROM dune.encrypted_accounts acct
WHERE COALESCE(acct.\"user\"::text, '') <> ''
ORDER BY acct.id ASC
";

const PLAYER_BACKPACK_INVENTORY_SQL: &str = "
SELECT
    inv.id::int8 AS inventory_id,
    NULLIF(ps.character_name, '') AS character_name
FROM dune.player_state ps
JOIN dune.actors pawn ON pawn.id = ps.player_pawn_id
JOIN dune.inventories inv ON inv.actor_id = ps.player_pawn_id
                         AND inv.inventory_type = 0
WHERE ps.account_id = $1::int8
  AND pawn.class = '/Game/Dune/Characters/Player/BP_DunePlayerCharacter.BP_DunePlayerCharacter_C'
ORDER BY ps.last_login_time DESC NULLS LAST, inv.id DESC
LIMIT 1
";

const PLAYER_BACKPACK_FREE_SLOTS_SQL: &str = "
SELECT gs::int8 AS position_index
FROM generate_series(0, 10000) AS gs
WHERE NOT EXISTS (
    SELECT 1
    FROM dune.items i
    WHERE i.inventory_id = $1::int8
      AND i.position_index = gs
)
ORDER BY gs
LIMIT $2
";

const PLAYER_BACKPACK_INSERT_ITEM_SQL: &str = "
INSERT INTO dune.items (
    inventory_id,
    stack_size,
    position_index,
    template_id,
    is_new,
    acquisition_time,
    stats,
    quality_level
)
VALUES (
    $1::int8,
    $2::int8,
    $3::int8,
    $4::text,
    TRUE,
    EXTRACT(EPOCH FROM now())::int8,
    $5::text::jsonb,
    $6::int8
)
RETURNING id::int8
";

const ADMIN_PLAYER_TARGET_SQL: &str = "
SELECT
    COALESCE(enc.\"user\"::text, '') AS fls_id,
    COALESCE(ps.character_name, '') AS character_name,
    COALESCE(ps.online_status::text, '') AS online_status,
    COALESCE(ps.account_id, 0)::int8 AS account_id,
    COALESCE(ps.player_pawn_id, 0)::int8 AS pawn_id,
    COALESCE(ps.player_controller_id, 0)::int8 AS controller_id
FROM dune.player_state ps
LEFT JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
WHERE lower(COALESCE(enc.\"user\"::text, '')) = lower($1)
ORDER BY ps.last_login_time DESC NULLS LAST
LIMIT 1
";

const SPECIALIZATION_TRACKS_SQL: &str = "
SELECT track_type::text AS track_type, xp_amount::int8 AS xp_amount, level::float8 AS level
FROM dune.specialization_tracks
WHERE player_id = $1::int8
ORDER BY track_type
";

const SPECIALIZATION_KEYSTONE_COUNT_SQL: &str = "
SELECT COUNT(*)::int8
FROM dune.purchased_specialization_keystones
WHERE player_id = $1::int8
";

const SET_SPECIALIZATION_LEVEL_SQL: &str = "
SELECT dune.set_specialization_xp_and_level(
    $1::int8,
    $2::dune.specializationtracktype,
    $3::int4,
    $4::real
)
";

const CHAT_PLAYER_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id,
    COALESCE(acct.funcom_id::text, '') AS funcom_id,
    COALESCE(ps.character_name, '') AS character_name
FROM dune.player_state ps
JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
LEFT JOIN dune.accounts acct ON acct.id = ps.account_id
WHERE lower(COALESCE(enc.\"user\"::text, '')) = lower($1)
   OR lower(COALESCE(acct.funcom_id::text, '')) = lower($1)
   OR lower(COALESCE(ps.character_name, '')) = lower($1)
ORDER BY
    CASE
        WHEN lower(COALESCE(enc.\"user\"::text, '')) = lower($1) THEN 0
        WHEN lower(COALESCE(acct.funcom_id::text, '')) = lower($1) THEN 1
        ELSE 2
    END,
    ps.last_login_time DESC NULLS LAST
LIMIT 1
";

const LEVEL_COLUMN_CANDIDATES: &[&str] = &[
    "level",
    "character_level",
    "player_level",
    "experience_level",
    "current_level",
    "total_level",
];

fn players_sql(level_expr: &str) -> String {
    format!(
        r#"
WITH matches AS (
    SELECT DISTINCT
        COALESCE(enc."user"::text, '') AS fls_id,
        COALESCE(ps.character_name, '')   AS character_name,
        COALESCE(ps.online_status::text, '') AS online_status,
        COALESCE(
            to_char(ps.last_avatar_activity AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),
            ''
        ) AS last_seen,
        {level_expr} AS player_level,
        a.partition_id,
        ps.account_id::int8 AS account_id,
        COALESCE(ps.player_pawn_id, 0)::int8 AS pawn_id,
        COALESCE(ps.player_controller_id, 0)::int8 AS controller_id
    FROM dune.player_state ps
    LEFT JOIN dune.accounts acct           ON acct.id = ps.account_id
    LEFT JOIN dune.encrypted_accounts enc  ON enc.id  = ps.account_id
    LEFT JOIN dune.actors a                ON a.id     = ps.player_pawn_id
    WHERE lower(ps.character_name) LIKE lower($1)
       OR lower(COALESCE(enc."user"::text, '')) LIKE lower($1)
       OR lower(COALESCE(acct.funcom_id::text, '')) LIKE lower($1)
)
SELECT fls_id, character_name, online_status, last_seen, player_level, partition_id,
       account_id, pawn_id, controller_id
FROM matches
WHERE fls_id <> ''
ORDER BY
    CASE WHEN lower(online_status) = 'online' THEN 0 ELSE 1 END,
    last_seen DESC,
    character_name ASC
LIMIT $2;
"#
    )
}

/// Outcome of a player-position probe.
pub enum PositionProbe {
    Found(PlayerLocation),
    /// No row matched — usually means the player is offline (no live pawn),
    /// or the fls_id doesn't exist on this server.
    NoRow,
}

/// Look up the live world position for a player. Joins `player_state` →
/// `actors` on `player_pawn_id` and deconstructs the composite
/// `actors.transform` (`(location:(x,y,z), rotation:(x,y,z,w))`).
pub async fn get_player_location(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
) -> Result<PositionProbe> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(PLAYER_POSITION_SQL, &[&fls_id])
        .await
        .context("querying player pawn position")?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(PositionProbe::NoRow);
    };
    Ok(PositionProbe::Found(PlayerLocation {
        x: row.get::<_, f64>(0),
        y: row.get::<_, f64>(1),
        z: row.get::<_, f64>(2),
        dimension_index: row.try_get::<_, i32>(3).ok(),
        partition_id: row.try_get::<_, i64>(4).ok(),
        source: row.try_get::<_, String>(5).unwrap_or_default(),
    }))
}

pub async fn search_players(
    pg: &PgClient,
    namespace: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<Player>> {
    let safe_limit = limit.clamp(1, 200) as i64;
    let pattern = format!("%{}%", query);

    let state = pg.client(namespace).await?;
    let level_column = player_level_column(state.client()).await?;
    let level_expr = level_column
        .as_deref()
        .map(|column| format!("ps.\"{column}\"::int"))
        .unwrap_or_else(|| "NULL::int".to_string());
    let sql = players_sql(&level_expr);
    let rows = state
        .client()
        .query(&sql, &[&pattern, &safe_limit])
        .await
        .context("running player search query")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(Player {
            fls_id: row.try_get::<_, String>(0).unwrap_or_default(),
            name: row.try_get::<_, String>(1).unwrap_or_default(),
            online: row.try_get::<_, String>(2).unwrap_or_default(),
            last_seen: row.try_get::<_, String>(3).unwrap_or_default(),
            level: row.try_get::<_, Option<i32>>(4).ok().flatten(),
            partition_id: row.try_get::<_, Option<i64>>(5).ok().flatten(),
            account_id: row.try_get::<_, i64>(6).unwrap_or_default(),
            pawn_id: row.try_get::<_, i64>(7).unwrap_or_default(),
            controller_id: row.try_get::<_, i64>(8).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub async fn resolve_admin_player_by_fls(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
) -> Result<Option<AdminPlayerTarget>> {
    let state = pg.client(namespace).await?;
    let row = state
        .client()
        .query_opt(ADMIN_PLAYER_TARGET_SQL, &[&fls_id.trim()])
        .await
        .with_context(|| format!("resolving admin player {}", fls_id.trim()))?;
    Ok(row.map(|row| AdminPlayerTarget {
        fls_id: row.try_get::<_, String>(0).unwrap_or_default(),
        name: row.try_get::<_, String>(1).unwrap_or_default(),
        online: row.try_get::<_, String>(2).unwrap_or_default(),
        account_id: row.try_get::<_, i64>(3).unwrap_or_default(),
        pawn_id: row.try_get::<_, i64>(4).unwrap_or_default(),
        controller_id: row.try_get::<_, i64>(5).unwrap_or_default(),
    }))
}

pub fn is_player_online(status: &str) -> bool {
    status.trim().eq_ignore_ascii_case("online")
}

pub const SPECIALIZATION_TRACK_ORDER: &[&str] =
    &["Combat", "Crafting", "Exploration", "Gathering", "Sabotage"];
pub const SPECIALIZATION_XP_MAX: i32 = 44_182;
pub const SPECIALIZATION_LEVEL_MAX: i32 = 100;
pub const SPECIALIZATION_KEYSTONE_MAX: i32 = 205;

pub fn canonical_specialization_track(track: &str) -> Option<&'static str> {
    SPECIALIZATION_TRACK_ORDER
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(track.trim()))
}

pub fn clamp_specialization_level(level: i32) -> i32 {
    level.clamp(0, SPECIALIZATION_LEVEL_MAX)
}

pub fn specialization_level_to_xp(level: i32) -> i32 {
    let level = clamp_specialization_level(level);
    if level >= SPECIALIZATION_LEVEL_MAX {
        return SPECIALIZATION_XP_MAX;
    }
    let estimate = (3.107 * f64::from(level * level) + 131.1 * f64::from(level)).round() as i32;
    estimate.clamp(0, SPECIALIZATION_XP_MAX)
}

pub async fn get_player_specialization(
    pg: &PgClient,
    namespace: &str,
    player: AdminPlayerTarget,
) -> Result<PlayerSpecialization> {
    if player.controller_id <= 0 {
        return Err(anyhow!(
            "player {} does not have a valid controller id",
            player.fls_id
        ));
    }

    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(SPECIALIZATION_TRACKS_SQL, &[&player.controller_id])
        .await
        .context("querying specialization tracks")?;
    let mut by_track = std::collections::HashMap::<String, (i64, f64)>::new();
    for row in rows {
        let track = row.try_get::<_, String>(0).unwrap_or_default();
        let xp = row.try_get::<_, i64>(1).unwrap_or_default();
        let level = row.try_get::<_, f64>(2).unwrap_or_default();
        by_track.insert(track.to_lowercase(), (xp, level));
    }

    let keystones_total = state
        .client()
        .query_opt(SPECIALIZATION_KEYSTONE_COUNT_SQL, &[&player.controller_id])
        .await
        .context("querying specialization keystone count")?
        .map(|row| row.try_get::<_, i64>(0).unwrap_or_default())
        .unwrap_or_default();

    let tracks = SPECIALIZATION_TRACK_ORDER
        .iter()
        .map(|track| {
            let (xp, level) = by_track
                .get(&track.to_lowercase())
                .copied()
                .unwrap_or((0, 0.0));
            SpecializationTrack {
                track_type: (*track).to_string(),
                xp,
                level,
                xp_max: SPECIALIZATION_XP_MAX,
                level_max: f64::from(SPECIALIZATION_LEVEL_MAX),
            }
        })
        .collect();

    Ok(PlayerSpecialization {
        player,
        tracks,
        keystones_total,
        keystones_max: SPECIALIZATION_KEYSTONE_MAX,
    })
}

pub async fn set_specialization_level(
    pg: &PgClient,
    namespace: &str,
    controller_id: i64,
    track_type: &str,
    level: i32,
) -> Result<SetSpecializationResult> {
    let track = canonical_specialization_track(track_type)
        .ok_or_else(|| anyhow!("unknown specialization track {track_type}"))?;
    let level = clamp_specialization_level(level);
    let xp = specialization_level_to_xp(level);
    let level_real = level as f32;

    let state = pg.client(namespace).await?;
    state
        .client()
        .query_one(
            SET_SPECIALIZATION_LEVEL_SQL,
            &[&controller_id, &track, &xp, &level_real],
        )
        .await
        .context("setting specialization level")?;

    Ok(SetSpecializationResult {
        track_type: track.to_string(),
        level,
        xp,
    })
}

pub async fn list_welcome_accounts(pg: &PgClient, namespace: &str) -> Result<Vec<WelcomeAccount>> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(WELCOME_ACCOUNTS_SQL, &[])
        .await
        .context("querying welcome package accounts")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WelcomeAccount {
            account_id: row.try_get::<_, i64>(0).unwrap_or_default(),
            fls_id: row.try_get::<_, String>(1).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub async fn resolve_account_backpack(
    pg: &PgClient,
    namespace: &str,
    account_id: i64,
) -> Result<Option<AccountBackpack>> {
    let state = pg.client(namespace).await?;
    let row = state
        .client()
        .query_opt(PLAYER_BACKPACK_INVENTORY_SQL, &[&account_id])
        .await
        .context("resolving account backpack inventory")?;
    Ok(row.map(|row| AccountBackpack {
        inventory_id: row.try_get::<_, i64>(0).unwrap_or_default(),
        character_name: row.try_get::<_, Option<String>>(1).ok().flatten(),
    }))
}

pub async fn insert_items_to_backpack(
    pg: &PgClient,
    namespace: &str,
    inventory_id: i64,
    items: &[BackpackGrantItem],
) -> Result<Vec<i64>> {
    let mut state = pg.dedicated_client(namespace).await?;
    let tx = state
        .client_mut()
        .transaction()
        .await
        .context("starting welcome item grant transaction")?;

    let slot_limit = items.len() as i64;
    let slot_rows = tx
        .query(
            PLAYER_BACKPACK_FREE_SLOTS_SQL,
            &[&inventory_id, &slot_limit],
        )
        .await
        .context("finding free backpack slots")?;
    if slot_rows.len() != items.len() {
        return Err(anyhow::anyhow!(
            "not enough free backpack slots: needed {}, found {}",
            items.len(),
            slot_rows.len()
        ));
    }

    let insert = tx
        .prepare(PLAYER_BACKPACK_INSERT_ITEM_SQL)
        .await
        .context("preparing backpack item insert")?;
    let mut inserted_ids = Vec::with_capacity(items.len());
    for (item, slot) in items.iter().zip(slot_rows.iter()) {
        let position_index = slot.try_get::<_, i64>(0).unwrap_or_default();
        let row = tx
            .query_one(
                &insert,
                &[
                    &inventory_id,
                    &item.quantity,
                    &position_index,
                    &item.template_id,
                    &item.stats_json,
                    &item.quality_level,
                ],
            )
            .await
            .with_context(|| format!("inserting backpack item {}", item.template_id))?;
        inserted_ids.push(row.try_get::<_, i64>(0).unwrap_or_default());
    }

    tx.commit()
        .await
        .context("committing backpack item grant transaction")?;
    Ok(inserted_ids)
}

pub fn grant_item_stats_json(stack_max: Option<u32>) -> &'static str {
    if stack_max.unwrap_or_default() > 1 {
        return r#"{"FItemStackAndDurabilityStats":[[],{"DecayedMaxDurability":0.0}]}"#;
    }
    r#"{"FCustomizationStats":[[],{}],"FItemStackAndDurabilityStats":[[],{}]}"#
}

pub async fn resolve_chat_player(
    pg: &PgClient,
    namespace: &str,
    lookup: &str,
) -> Result<Option<ChatPlayer>> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(CHAT_PLAYER_SQL, &[&lookup.trim()])
        .await
        .with_context(|| format!("resolving chat player {lookup}"))?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(ChatPlayer {
        account_id: row.try_get::<_, i64>(0).unwrap_or_default(),
        fls_id: row.try_get::<_, String>(1).unwrap_or_default(),
        funcom_id: row.try_get::<_, String>(2).unwrap_or_default(),
        character_name: row.try_get::<_, String>(3).unwrap_or_default(),
    }))
}

async fn player_level_column(client: &tokio_postgres::Client) -> Result<Option<String>> {
    let rows = client
        .query(PLAYER_STATE_COLUMN_SQL, &[&LEVEL_COLUMN_CANDIDATES])
        .await
        .context("checking player level column")?;
    let available = rows
        .into_iter()
        .filter_map(|row| row.try_get::<_, String>(0).ok())
        .collect::<std::collections::HashSet<_>>();
    Ok(LEVEL_COLUMN_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| available.contains(*candidate))
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_level_is_clamped_before_xp_estimate() {
        assert_eq!(clamp_specialization_level(-10), 0);
        assert_eq!(clamp_specialization_level(42), 42);
        assert_eq!(clamp_specialization_level(500), SPECIALIZATION_LEVEL_MAX);
    }

    #[test]
    fn specialization_xp_curve_matches_reference_bounds() {
        assert_eq!(specialization_level_to_xp(0), 0);
        assert_eq!(specialization_level_to_xp(100), SPECIALIZATION_XP_MAX);
        assert_eq!(specialization_level_to_xp(500), SPECIALIZATION_XP_MAX);
        assert_eq!(specialization_level_to_xp(45), 12_191);
        assert_eq!(specialization_level_to_xp(53), 15_676);
    }

    #[test]
    fn specialization_track_validation_is_case_insensitive() {
        assert_eq!(canonical_specialization_track("combat"), Some("Combat"));
        assert_eq!(canonical_specialization_track(" Sabotage "), Some("Sabotage"));
        assert_eq!(canonical_specialization_track("Mentat"), None);
    }

    #[test]
    fn grant_item_stats_follow_stack_shape() {
        assert!(grant_item_stats_json(Some(500)).contains("DecayedMaxDurability"));
        assert!(grant_item_stats_json(Some(1)).contains("FCustomizationStats"));
        assert!(grant_item_stats_json(None).contains("FCustomizationStats"));
    }

    #[test]
    fn online_status_check_is_strict() {
        assert!(is_player_online("Online"));
        assert!(is_player_online(" online "));
        assert!(!is_player_online("Offline"));
        assert!(!is_player_online(""));
    }
}

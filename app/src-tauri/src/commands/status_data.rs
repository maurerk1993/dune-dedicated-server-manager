use dune_manager_core::errors::failure;
use dune_manager_core::models::CommandResult;
use dune_manager_core::orchestration::{RemoteCommandRunner, RusshRunner};
use serde_json::Value;

use crate::commands::shared::sh_single_quoted;
use crate::commands::status_helpers::{pod_component, server_resource_components};
use crate::commands::status_naming::friendly_map_name;
use crate::dto::{
    RemoteBattlegroupServerStat, RemoteBattlegroupStatus, RemoteHostMetrics, RemoteServerComponent,
    RemoteServerPackageStatus, RemoteServerStatus,
};

pub fn read_remote_server_status(
    runner: &RusshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerStatus> {
    // The vendor wrapper's `status` text output is the source of truth in
    // older operator versions, but the format keeps shifting across Funcom
    // releases (newer wrappers show the partial world name in "Status",
    // "N/M" ratios under "Director", and semantic words like "Healthy"
    // under "Uptime" — none of which match the older
    // `Running/Running/Running/Running/1h2m` shape we used to parse).
    // Read the BattleGroup CR's `status` object directly so we stay
    // pinned to the stable Kubernetes schema instead of the rotating
    // text rendering.
    let bg = runner.run_json(
        &format!(
            "sudo kubectl get battlegroup -n {} {} -o json",
            sh_single_quoted(namespace),
            sh_single_quoted(battlegroup_name),
        ),
        "remote battlegroup",
    )?;
    // Per-partition live data (player count, gamePhase, ready) lives on a
    // separate ServerStats CRD published by the Funcom operator — the same
    // source `F:\Dune\Server\gt-server-status\gt_server_status.py` consumes.
    // Failing to fetch this is non-fatal; the UI still shows operator state
    // while runtime-only fields remain unavailable.
    let stats = runner
        .run_json(
            &format!(
                "sudo kubectl get serverstats -n {} -o json",
                sh_single_quoted(namespace),
            ),
            "remote serverstats",
        )
        .unwrap_or_else(|_| Value::Null);
    let battlegroup = battlegroup_status_from_json_with_stats(&bg, &stats).ok_or_else(|| {
        failure(format!(
            "BattleGroup `{battlegroup_name}` returned no status object yet (likely still initialising)"
        ))
    })?;
    let package = read_guest_package_status(runner, namespace, battlegroup_name)?;
    Ok(RemoteServerStatus {
        battlegroup,
        package,
        collected_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        host_metrics: read_host_metrics(runner).ok(),
    })
}

fn read_host_metrics(runner: &RusshRunner) -> CommandResult<RemoteHostMetrics> {
    let script = r#"
set +e
mem_total_kb=$(awk '$1 == "MemTotal:" { print $2; exit }' /proc/meminfo)
mem_available_kb=$(awk '$1 == "MemAvailable:" { print $2; exit }' /proc/meminfo)
if [ -z "$mem_available_kb" ]; then
  mem_available_kb=$(awk '
    $1 == "MemFree:" { free = $2 }
    $1 == "Buffers:" { buffers = $2 }
    $1 == "Cached:" { cached = $2 }
    END { print free + buffers + cached }
  ' /proc/meminfo)
fi
swap_total_kb=$(awk '$1 == "SwapTotal:" { print $2; exit }' /proc/meminfo)
swap_free_kb=$(awk '$1 == "SwapFree:" { print $2; exit }' /proc/meminfo)
uptime_seconds=$(awk '{ printf "%.0f", $1 }' /proc/uptime)
load_average_one=$(awk '{ print $1 }' /proc/loadavg)

disk_path=/
for candidate in /home/dune/.dune/download /home/dune/.dune /home/dune; do
  if [ -d "$candidate" ]; then
    disk_path="$candidate"
    break
  fi
done
disk_values=$(df -Pk "$disk_path" 2>/dev/null | awk 'NR == 2 { print $2, $3 }')
set -- $disk_values
disk_total_kb=${1:-0}
disk_used_kb=${2:-0}

cpu_sample() {
  awk '/^cpu / {
    total = 0
    for (i = 2; i <= NF; i++) total += $i
    idle = $5 + $6
    print total, idle
    exit
  }' /proc/stat
}
set -- $(cpu_sample)
cpu_total_one=${1:-0}
cpu_idle_one=${2:-0}
sleep 0.2
set -- $(cpu_sample)
cpu_total_two=${1:-0}
cpu_idle_two=${2:-0}
cpu_usage_percent=$(awk \
  -v total_one="$cpu_total_one" \
  -v idle_one="$cpu_idle_one" \
  -v total_two="$cpu_total_two" \
  -v idle_two="$cpu_idle_two" \
  'BEGIN {
    delta_total = total_two - total_one
    delta_idle = idle_two - idle_one
    if (delta_total > 0) printf "%.1f", 100 * (delta_total - delta_idle) / delta_total
  }')

printf 'memTotalKb=%s\n' "${mem_total_kb:-0}"
printf 'memAvailableKb=%s\n' "${mem_available_kb:-0}"
printf 'swapTotalKb=%s\n' "${swap_total_kb:-0}"
printf 'swapFreeKb=%s\n' "${swap_free_kb:-0}"
printf 'cpuUsagePercent=%s\n' "$cpu_usage_percent"
printf 'loadAverageOne=%s\n' "$load_average_one"
printf 'diskTotalKb=%s\n' "$disk_total_kb"
printf 'diskUsedKb=%s\n' "$disk_used_kb"
printf 'uptimeSeconds=%s\n' "$uptime_seconds"
"#;
    parse_host_metrics(&runner.run_script(script)?)
        .ok_or_else(|| failure("Remote host returned incomplete resource metrics".to_string()))
}

fn parse_host_metrics(output: &str) -> Option<RemoteHostMetrics> {
    let values: std::collections::HashMap<&str, &str> = output
        .lines()
        .filter_map(|line| line.trim().split_once('='))
        .collect();
    let integer = |key: &str| values.get(key)?.trim().parse::<u64>().ok();
    let decimal = |key: &str| values.get(key)?.trim().parse::<f64>().ok();
    let memory_total_kb = integer("memTotalKb")?;
    let memory_available_kb = integer("memAvailableKb")?;
    let swap_total_kb = integer("swapTotalKb").unwrap_or_default();
    let swap_free_kb = integer("swapFreeKb").unwrap_or_default();
    let disk_total_kb = integer("diskTotalKb")?;
    let disk_used_kb = integer("diskUsedKb")?;
    if memory_total_kb == 0 || disk_total_kb == 0 {
        return None;
    }
    Some(RemoteHostMetrics {
        memory_used_bytes: memory_total_kb
            .saturating_sub(memory_available_kb)
            .saturating_mul(1024),
        memory_total_bytes: memory_total_kb.saturating_mul(1024),
        swap_used_bytes: swap_total_kb
            .saturating_sub(swap_free_kb)
            .saturating_mul(1024),
        swap_total_bytes: swap_total_kb.saturating_mul(1024),
        cpu_usage_percent: decimal("cpuUsagePercent").map(|value| value.clamp(0.0, 100.0)),
        load_average_one: decimal("loadAverageOne"),
        disk_used_bytes: disk_used_kb.saturating_mul(1024),
        disk_total_bytes: disk_total_kb.saturating_mul(1024),
        uptime_seconds: integer("uptimeSeconds").unwrap_or_default(),
    })
}

/// Maps a raw `kubectl get battlegroup ... -o json` payload into the UI's
/// `RemoteBattlegroupStatus` and merges per-partition
/// live data (players, gamePhase, ready) from a `kubectl get serverstats`
/// JSON payload. Pass `Value::Null` when no stats are available.
pub(crate) fn battlegroup_status_from_json_with_stats(
    bg: &Value,
    serverstats: &Value,
) -> Option<RemoteBattlegroupStatus> {
    bg.get("metadata")?.get("name")?.as_str()?;
    let spec = bg.get("spec").cloned().unwrap_or(Value::Null);
    let status = bg.get("status").cloned().unwrap_or(Value::Null);

    let stop = spec
        .get("stop")
        .and_then(Value::as_bool)
        .or_else(|| status.get("stop").and_then(Value::as_bool))
        .unwrap_or(false);

    // Funcom's CR carries `status.startTimestamp` at the BG level (when the
    // BG first scheduled) but not per-server. We render it on every row as a
    // best-effort age — accurate when partitions all came up together, off
    // by however long a partition has restarted independently.
    let bg_age = status
        .get("startTimestamp")
        .and_then(Value::as_str)
        .map(format_age_since_iso)
        .unwrap_or_default();

    let stats_by_partition = index_serverstats_by_partition(serverstats);

    let server_stats = status
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| {
            servers
                .iter()
                .map(|s| server_stat_from_json(s, &bg_age, &stats_by_partition))
                .collect()
        })
        .unwrap_or_default();

    // Database/director phases are nested in the live CR, not top-level
    // fields. Fall back to top-level keys for older operator builds.
    let database_phase = status
        .get("database")
        .and_then(|d| d.get("phase"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| string_field(&status, "databasePhase"));
    let director_phase = status
        .get("utilities")
        .and_then(|u| u.get("director"))
        .and_then(|d| d.get("phase"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| string_field(&status, "directorPhase"));
    // Uptime: the CR doesn't expose a pre-formatted string anymore, so we
    // compute it from `status.startTimestamp` (the same field we use for
    // per-row age). Older operators that set a literal `uptime` string win.
    let uptime_literal = string_field(&status, "uptime");
    let uptime = if uptime_literal.is_empty() {
        bg_age.clone()
    } else {
        uptime_literal
    };

    Some(RemoteBattlegroupStatus {
        stop,
        phase: string_field(&status, "phase"),
        database_phase,
        server_group_phase: string_field(&status, "serverGroupPhase"),
        director_phase,
        uptime,
        server_stats,
    })
}

#[derive(Default, Clone)]
struct PartitionStats {
    players: Option<i64>,
    raw_map: String,
    sietch: String,
    dimension: Option<i64>,
    game_phase: String,
    runtime_ready: String,
    simulation_fps: Option<f64>,
    battlegroup_leader: Option<bool>,
    server_name: String,
}

/// Build a `partition_index -> PartitionStats` map from a `kubectl get
/// serverstats -n <ns> -o json` payload. The Funcom operator emits one
/// ServerStats CR per partition with `spec.area.partition` as the id and
/// `status.runtime` as the live game telemetry. Same source the
/// `gt_server_status.py` cron script consumes. Keep this parser tolerant of
/// absent fields because ServerStats is populated incrementally during boot.
fn index_serverstats_by_partition(stats: &Value) -> std::collections::HashMap<i64, PartitionStats> {
    let mut out = std::collections::HashMap::new();
    let Some(items) = stats.get("items").and_then(Value::as_array) else {
        return out;
    };
    for item in items {
        let partition = item
            .get("spec")
            .and_then(|s| s.get("area"))
            .and_then(|a| a.get("partition"))
            .and_then(Value::as_i64);
        let Some(partition) = partition else { continue };
        let area = item.get("spec").and_then(|s| s.get("area"));
        let status = item.get("status");
        let runtime = status.and_then(|s| s.get("runtime"));
        let players = runtime
            .and_then(|r| r.get("players"))
            .and_then(Value::as_i64);
        let simulation_fps = runtime.and_then(|r| r.get("sfps")).and_then(decimal_value);
        out.insert(
            partition,
            PartitionStats {
                players,
                raw_map: area
                    .and_then(|a| a.get("map"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sietch: area
                    .and_then(|a| a.get("sietch"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                dimension: area
                    .and_then(|a| a.get("dimension"))
                    .and_then(Value::as_i64),
                game_phase: runtime
                    .and_then(|r| r.get("gamePhase"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                runtime_ready: runtime
                    .and_then(|r| r.get("ready"))
                    .map(value_to_string)
                    .unwrap_or_default(),
                simulation_fps,
                battlegroup_leader: status
                    .and_then(|s| s.get("leadership"))
                    .and_then(|l| l.get("battlegroup"))
                    .and_then(Value::as_bool),
                server_name: item
                    .get("metadata")
                    .and_then(|m| m.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            },
        );
    }
    out
}

fn decimal_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn string_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn server_stat_from_json(
    server: &Value,
    bg_age: &str,
    stats_by_partition: &std::collections::HashMap<i64, PartitionStats>,
) -> RemoteBattlegroupServerStat {
    // The Funcom operator names this field `partitionMap` in the BattleGroup
    // CR's `status.servers[]` — confirmed against backed-up live CR YAML.
    // Older / alternate operators have used `map` or `name`, so we keep
    // those as fallbacks. With no map at all `friendly_map_name` returns
    // "Game Server" which is what we want to avoid here.
    let partition_index = server
        .get("partitionIndex")
        .and_then(Value::as_u64)
        .or_else(|| server.get("ordinalIndex").and_then(Value::as_u64));
    let partition_stats = partition_index.and_then(|index| stats_by_partition.get(&(index as i64)));
    let raw_map = server
        .get("partitionMap")
        .and_then(Value::as_str)
        .or_else(|| server.get("map").and_then(Value::as_str))
        .or_else(|| server.get("name").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .or_else(|| partition_stats.map(|stats| stats.raw_map.as_str()))
        .unwrap_or_default();
    let friendly = friendly_map_name(raw_map, raw_map);
    let labelled = match partition_index {
        Some(idx) => format!("{friendly} #{idx}"),
        None => friendly,
    };
    let ready_str = server.get("ready").map(value_to_string).unwrap_or_default();
    // The BG CR's status.servers[] entries don't carry a player count or
    // age; we inherit the BG-level age and merge the per-partition player
    // count from the matching ServerStats CR (keyed by partitionIndex).
    let age = if let Some(start) = server.get("startTimestamp").and_then(Value::as_str) {
        format_age_since_iso(start)
    } else {
        bg_age.to_string()
    };
    let players = partition_stats
        .and_then(|stats| stats.players)
        .map(|n| n.to_string())
        .unwrap_or_default();
    RemoteBattlegroupServerStat {
        map: labelled,
        raw_map: raw_map.to_string(),
        sietch: partition_stats
            .map(|stats| stats.sietch.clone())
            .unwrap_or_default(),
        partition_index,
        dimension: server
            .get("dimensionIndex")
            .and_then(Value::as_i64)
            .or_else(|| partition_stats.and_then(|stats| stats.dimension)),
        phase: string_field(server, "phase"),
        ready: ready_str,
        players,
        age,
        game_phase: partition_stats
            .map(|stats| stats.game_phase.clone())
            .unwrap_or_default(),
        runtime_ready: partition_stats
            .map(|stats| stats.runtime_ready.clone())
            .unwrap_or_default(),
        simulation_fps: partition_stats.and_then(|stats| stats.simulation_fps),
        battlegroup_leader: partition_stats.and_then(|stats| stats.battlegroup_leader),
        restarts: server.get("restarts").and_then(Value::as_u64),
        server_name: partition_stats
            .map(|stats| stats.server_name.clone())
            .unwrap_or_default(),
        game_port: server.get("gamePort").and_then(Value::as_u64),
        igw_port: server.get("igwPort").and_then(Value::as_u64),
    }
}

/// Format an RFC 3339 timestamp like `"2026-05-22T01:27:53Z"` as a compact
/// elapsed-time string (`5d 3h`, `2h 17m`, `45m`, `12s`). Returns empty
/// string when parsing fails — the UI just shows an empty cell.
fn format_age_since_iso(iso_ts: &str) -> String {
    let parsed = chrono::DateTime::parse_from_rfc3339(iso_ts.trim());
    let Ok(start) = parsed else {
        return String::new();
    };
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(start.with_timezone(&chrono::Utc));
    let secs = diff.num_seconds().max(0);
    if secs < 60 {
        return format!("{secs}s");
    }
    let minutes = secs / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let mins_rem = minutes % 60;
    if hours < 24 {
        return format!("{hours}h {mins_rem}m");
    }
    let days = hours / 24;
    let hours_rem = hours % 24;
    format!("{days}d {hours_rem}h")
}

fn read_guest_package_status(
    runner: &RusshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerPackageStatus> {
    let script = r#"
set -u
download=/home/dune/.dune/download
manifest="$download/steamapps/appmanifest_4754530.acf"
ns=__NAMESPACE__
bg=__BATTLEGROUP__
read_vdf_value() {
  key="$1"
  file="$2"
  [ -f "$file" ] || return 0
  awk -F '"' -v wanted="$key" '$2 == wanted { print $4; exit }' "$file" 2>/dev/null || true
}
read_file() {
  file="$1"
  [ -f "$file" ] || return 0
  head -n 1 "$file" 2>/dev/null | tr -d '\r\n'
}
printf 'installedBuildId=%s\n' "$(read_vdf_value buildid "$manifest")"
printf 'battlegroupVersion=%s\n' "$(read_file "$download/images/battlegroup/version.txt")"
printf 'operatorVersion=%s\n' "$(read_file "$download/images/operators/version.txt")"
live_image=$(sudo kubectl get battlegroup "$bg" -n "$ns" -o jsonpath='{..image}' 2>/dev/null | tr ' ' '\n' | awk -F: '/self-hosting\/(igw-server|seabass-server):/ { print $NF; exit }' || true)
printf 'liveBattlegroupVersion=%s\n' "$live_image"
"#
    .replace("__NAMESPACE__", &sh_single_quoted(namespace))
    .replace("__BATTLEGROUP__", &sh_single_quoted(battlegroup_name));
    let output = runner.run_script(&script)?;
    let value = |key: &str| {
        output.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
    };
    Ok(RemoteServerPackageStatus {
        installed_build_id: value("installedBuildId"),
        battlegroup_version: value("battlegroupVersion"),
        live_battlegroup_version: value("liveBattlegroupVersion"),
        operator_version: value("operatorVersion"),
    })
}

fn is_current_database_utility_pod(role: &str, phase: &str) -> bool {
    role.contains("database-monitor")
        || role.contains("database-pghero")
        || (role.contains("database-utility") && !matches!(phase, "Succeeded" | "Failed"))
}

pub fn read_remote_server_components(
    runner: &RusshRunner,
    namespace: &str,
) -> CommandResult<Vec<RemoteServerComponent>> {
    let pods = runner.run_json(
        &format!(
            "sudo kubectl get pods -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server pods",
    )?;
    let resources = runner.run_json(
        &format!(
            "sudo kubectl get servergroups,servergateways,serversets -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server resources",
    )?;
    let pod_metrics = runner
        .run_json(
            &format!(
                "sudo kubectl get --raw {}",
                sh_single_quoted(&format!(
                    "/apis/metrics.k8s.io/v1beta1/namespaces/{namespace}/pods"
                ))
            ),
            "remote pod resource metrics",
        )
        .unwrap_or_else(|_| Value::Null);

    let mut components = vec![
        pod_component(
            "Database",
            "database",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("database") && !name.contains("-util-"),
        ),
        pod_component(
            "Database utilities",
            "database-utilities",
            &pods,
            &pod_metrics,
            |role, _, phase| is_current_database_utility_pod(role, phase),
        ),
        pod_component(
            "Message Queue",
            "message-queue",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("message-queue") || name.contains("-mq-"),
        ),
        pod_component(
            "Director",
            "director",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("battlegroup-director") || name.contains("-bgd-"),
        ),
        pod_component(
            "Gateway",
            "gateway",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("server-gateway") || name.contains("-sgw-"),
        ),
        pod_component(
            "Text Router",
            "text-router",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("text-router") || name.contains("-tr-"),
        ),
        pod_component(
            "File Browser",
            "file-browser",
            &pods,
            &pod_metrics,
            |role, name, _| role.contains("filebrowser") || name.contains("-fb-"),
        ),
    ];
    components.extend(server_resource_components(&resources));
    Ok(components
        .into_iter()
        .filter(|component| component.state != "Not present")
        .collect())
}

pub fn remote_records_from_battlegroups(
    request: &crate::dto::RemoteConnectionRequest,
    value: &Value,
) -> Vec<crate::dto::RemoteServerRecord> {
    value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| remote_record_from_battlegroup(request, item))
        .collect()
}

fn remote_record_from_battlegroup(
    request: &crate::dto::RemoteConnectionRequest,
    item: &Value,
) -> Option<crate::dto::RemoteServerRecord> {
    let namespace = item
        .get("metadata")?
        .get("namespace")?
        .as_str()?
        .to_string();
    let battlegroup_name = item.get("metadata")?.get("name")?.as_str()?.to_string();
    let title = item
        .get("spec")
        .and_then(|spec| spec.get("title"))
        .and_then(Value::as_str)
        .unwrap_or(&battlegroup_name)
        .to_string();
    let phase = item
        .get("status")
        .and_then(|status| status.get("phase"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string();
    let server_type = request
        .server_type
        .as_deref()
        .unwrap_or("ubuntu")
        .trim()
        .to_string();
    let user = request
        .user
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    Some(crate::dto::RemoteServerRecord {
        id: remote_record_id(&server_type, &request.host, request.key_path.as_deref()),
        name: title,
        host: request.host.clone(),
        user,
        key_path: request.key_path.clone().unwrap_or_default(),
        port: request.port,
        server_type,
        namespace,
        battlegroup_name: battlegroup_name.clone(),
        world_unique_name: battlegroup_name,
        phase,
    })
}

fn remote_record_id(_server_type: &str, host: &str, key_path: Option<&str>) -> String {
    format!(
        "ubuntu:{}:{}",
        host.trim().to_lowercase(),
        key_path.unwrap_or_default().trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bg(spec: Value, status: Value) -> Value {
        json!({
            "metadata": {"name": "sh-test-bg", "namespace": "funcom-seabass-sh-test"},
            "spec": spec,
            "status": status,
        })
    }

    fn bg_status(bg: &Value) -> Option<RemoteBattlegroupStatus> {
        battlegroup_status_from_json_with_stats(bg, &Value::Null)
    }

    #[test]
    fn maps_reconciling_bg_with_null_director_phase() {
        // Mirrors the user-reported payload: phase Reconciling, gateway
        // Running, director not yet populated. Prior text-parse path was
        // confusing the UI into greying the Director tunnel; under direct
        // kubectl read the director_phase is just "" which the UI treats
        // as "ready enough".
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Reconciling",
                "serverGroupPhase": "Running",
                "directorPhase": Value::Null,
                "stop": Value::Null,
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        assert!(!dto.stop);
        assert_eq!(dto.phase, "Reconciling");
        assert_eq!(dto.server_group_phase, "Running");
        assert_eq!(dto.director_phase, "");
        assert_eq!(dto.uptime, "");
    }

    #[test]
    fn falls_back_to_status_stop_when_spec_missing() {
        let value = bg(json!({}), json!({"phase": "Stopped", "stop": true}));
        let dto = bg_status(&value).expect("status maps");
        assert!(dto.stop);
        assert_eq!(dto.phase, "Stopped");
    }

    #[test]
    fn server_stats_pulled_from_status_servers_array() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Running",
                "servers": [
                    {"map": "Survival_1", "phase": "Running", "ready": true},
                    {"name": "DeepDesert_1", "phase": "Stopped", "ready": false},
                ]
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        assert_eq!(dto.server_stats.len(), 2);
        assert_eq!(
            dto.server_stats[0].map,
            friendly_map_name("Survival_1", "Survival_1")
        );
        assert_eq!(dto.server_stats[0].phase, "Running");
        assert_eq!(dto.server_stats[0].ready, "true");
        // Players empty when no ServerStats CR is supplied — that data lives
        // on a separate CRD and is merged via `_with_stats`.
        assert_eq!(dto.server_stats[0].players, "");
        assert_eq!(
            dto.server_stats[1].map,
            friendly_map_name("DeepDesert_1", "DeepDesert_1")
        );
        assert_eq!(dto.server_stats[1].ready, "false");
        assert_eq!(dto.server_stats[1].age, "");
    }

    #[test]
    fn server_stats_merge_player_count_from_serverstats_crd() {
        // Mirrors the data shape gt_server_status.py reads: each ServerStats
        // CR has spec.area.partition matching the BG's partitionIndex, and
        // status.runtime.players is the live count.
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Healthy",
                "servers": [
                    {"partitionMap": "Survival_1", "partitionIndex": 1, "phase": "Running", "ready": true},
                    {"partitionMap": "Survival_1", "partitionIndex": 31, "phase": "Running", "ready": true},
                    {"partitionMap": "Overmap", "partitionIndex": 2, "phase": "Running", "ready": true},
                ],
            }),
        );
        let stats = json!({
            "items": [
                {"spec": {"area": {"partition": 1, "map": "Survival_1"}}, "status": {"runtime": {"players": 7}}},
                {"spec": {"area": {"partition": 31, "map": "Survival_1"}}, "status": {"runtime": {"players": 0}}},
                {"spec": {"area": {"partition": 2, "map": "Overmap"}}, "status": {"runtime": {"players": 3}}},
            ],
        });
        let dto = battlegroup_status_from_json_with_stats(&value, &stats).expect("status maps");
        assert_eq!(dto.server_stats[0].players, "7");
        assert_eq!(dto.server_stats[1].players, "0");
        assert_eq!(dto.server_stats[2].players, "3");
    }

    #[test]
    fn server_stats_merge_rich_runtime_telemetry_from_live_cr_shape() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Reconciling",
                "servers": [{
                    "partitionMap": "Survival_1",
                    "partitionIndex": 1,
                    "dimensionIndex": 0,
                    "phase": "Initializing",
                    "ready": false,
                    "restarts": 2,
                    "gamePort": 7778,
                    "igwPort": 7889,
                }],
            }),
        );
        let stats = json!({
            "items": [{
                "metadata": {"name": "sh-test-sg-survival-1-pod-1"},
                "spec": {"area": {
                    "partition": 1,
                    "dimension": 0,
                    "map": "Survival_1",
                    "sietch": "Abbir"
                }},
                "status": {
                    "leadership": {"battlegroup": true},
                    "runtime": {
                        "gamePhase": "PostLandscapePhysics",
                        "players": 4,
                        "ready": false,
                        "sfps": "19.48"
                    }
                }
            }]
        });

        let dto = battlegroup_status_from_json_with_stats(&value, &stats).expect("status maps");
        let map = &dto.server_stats[0];
        assert_eq!(map.map, "Hagga Basin #1");
        assert_eq!(map.raw_map, "Survival_1");
        assert_eq!(map.sietch, "Abbir");
        assert_eq!(map.partition_index, Some(1));
        assert_eq!(map.dimension, Some(0));
        assert_eq!(map.phase, "Initializing");
        assert_eq!(map.ready, "false");
        assert_eq!(map.players, "4");
        assert_eq!(map.game_phase, "PostLandscapePhysics");
        assert_eq!(map.runtime_ready, "false");
        assert_eq!(map.simulation_fps, Some(19.48));
        assert_eq!(map.battlegroup_leader, Some(true));
        assert_eq!(map.restarts, Some(2));
        assert_eq!(map.server_name, "sh-test-sg-survival-1-pod-1");
        assert_eq!(map.game_port, Some(7778));
        assert_eq!(map.igw_port, Some(7889));
    }

    #[test]
    fn server_stats_ignore_malformed_optional_runtime_values() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "servers": [{
                    "partitionMap": "Overmap",
                    "partitionIndex": 2,
                    "phase": "Running",
                    "ready": true
                }],
            }),
        );
        let stats = json!({
            "items": [{
                "spec": {"area": {"partition": 2, "map": "Overmap"}},
                "status": {"runtime": {"players": "unknown", "sfps": "not-a-number"}}
            }]
        });

        let dto = battlegroup_status_from_json_with_stats(&value, &stats).expect("status maps");
        assert_eq!(dto.server_stats[0].players, "");
        assert_eq!(dto.server_stats[0].simulation_fps, None);
        assert_eq!(dto.server_stats[0].runtime_ready, "");
    }

    #[test]
    fn server_stats_player_count_blank_when_partition_missing_from_stats() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "servers": [
                    {"partitionMap": "Survival_1", "partitionIndex": 1, "phase": "Running", "ready": true},
                ],
            }),
        );
        let stats = json!({"items": []});
        let dto = battlegroup_status_from_json_with_stats(&value, &stats).expect("status maps");
        assert_eq!(dto.server_stats[0].players, "");
    }

    #[test]
    fn server_stats_use_partition_map_and_index_from_real_cr() {
        // Mirrors the actual Funcom operator status.servers[] shape captured
        // from a live BattleGroup CR backup. Pre-fix the map column showed
        // "Game Server" for every row because we were reading `map`/`name`
        // instead of `partitionMap`.
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Healthy",
                "servers": [
                    {
                        "partitionMap": "Survival_1",
                        "partitionIndex": 1,
                        "phase": "Running",
                        "ready": true,
                    },
                    {
                        "partitionMap": "Survival_1",
                        "partitionIndex": 31,
                        "phase": "Running",
                        "ready": true,
                    },
                    {
                        "partitionMap": "Overmap",
                        "partitionIndex": 2,
                        "phase": "Running",
                        "ready": true,
                    },
                ]
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        assert_eq!(dto.server_stats.len(), 3);
        assert_eq!(dto.server_stats[0].map, "Hagga Basin #1");
        assert_eq!(dto.server_stats[1].map, "Hagga Basin #31");
        assert_eq!(dto.server_stats[2].map, "Overmap #2");
        assert!(dto.server_stats.iter().all(|s| s.phase == "Running"));
        assert!(dto.server_stats.iter().all(|s| s.ready == "true"));
    }

    #[test]
    fn returns_none_when_not_a_battlegroup_resource() {
        let value = json!({"kind": "Pod", "spec": {}, "status": {}});
        assert!(bg_status(&value).is_none());
    }

    #[test]
    fn bg_start_timestamp_propagates_to_every_server_row_when_per_server_missing() {
        // status.startTimestamp from the live CR backup is one minute in the
        // past for this test.
        let one_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Running",
                "startTimestamp": one_min_ago,
                "servers": [
                    {"partitionMap": "Survival_1", "partitionIndex": 1, "phase": "Running", "ready": true},
                    {"partitionMap": "Overmap", "partitionIndex": 2, "phase": "Running", "ready": true},
                ],
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        // All rows pick up the same BG-level age.
        assert_eq!(dto.server_stats.len(), 2);
        for row in &dto.server_stats {
            assert!(
                row.age == "1m" || row.age == "60s",
                "row age was {:?}",
                row.age
            );
        }
    }

    #[test]
    fn database_director_phases_pulled_from_nested_status() {
        // Live CR shape: status.database.phase + status.utilities.director.phase,
        // not top-level databasePhase/directorPhase.
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Healthy",
                "serverGroupPhase": "Running",
                "database": {"phase": "Ready", "address": "1.2.3.4:15432"},
                "utilities": {
                    "director": {"phase": "Healthy", "address": "1.2.3.4:30393"},
                },
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        assert_eq!(dto.database_phase, "Ready");
        assert_eq!(dto.director_phase, "Healthy");
    }

    #[test]
    fn terminal_database_setup_pods_do_not_degrade_utility_health() {
        let pods = json!({
            "items": [
                {
                    "metadata": {"name": "db-util-completed", "labels": {"role": "igw-database-utility"}},
                    "status": {
                        "phase": "Succeeded",
                        "containerStatuses": [{
                            "ready": false,
                            "restartCount": 0,
                            "state": {"terminated": {"reason": "Completed"}}
                        }]
                    }
                },
                {
                    "metadata": {"name": "db-util-failed", "labels": {"role": "igw-database-utility"}},
                    "status": {
                        "phase": "Failed",
                        "containerStatuses": [{
                            "ready": false,
                            "restartCount": 0,
                            "state": {"terminated": {"reason": "Error"}}
                        }]
                    }
                },
                {
                    "metadata": {"name": "db-monitor", "labels": {"role": "igw-database-monitor"}},
                    "status": {
                        "phase": "Running",
                        "containerStatuses": [{
                            "ready": true,
                            "restartCount": 0,
                            "state": {"running": {}}
                        }]
                    }
                },
                {
                    "metadata": {"name": "db-pghero", "labels": {"role": "igw-database-pghero"}},
                    "status": {
                        "phase": "Running",
                        "containerStatuses": [{
                            "ready": true,
                            "restartCount": 0,
                            "state": {"running": {}}
                        }]
                    }
                }
            ]
        });

        let component = pod_component(
            "Database utilities",
            "database-utilities",
            &pods,
            &Value::Null,
            |role, _, phase| is_current_database_utility_pod(role, phase),
        );

        assert_eq!(component.state, "Ready");
        assert_eq!(component.ready_pods, Some(2));
        assert_eq!(component.total_pods, Some(2));
        assert_eq!(component.details, vec!["2/2 pods ready"]);
        assert!(is_current_database_utility_pod(
            "igw-database-utility",
            "Pending"
        ));
    }

    #[test]
    fn uptime_derived_from_start_timestamp_when_no_literal() {
        let one_hr_ago =
            (chrono::Utc::now() - chrono::Duration::hours(1) - chrono::Duration::minutes(2))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let value = bg(
            json!({"stop": false}),
            json!({"phase": "Healthy", "startTimestamp": one_hr_ago}),
        );
        let dto = bg_status(&value).expect("status maps");
        assert_eq!(dto.uptime, "1h 2m");
    }

    #[test]
    fn uptime_prefers_literal_string_when_older_operator_set_it() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Healthy",
                "uptime": "1h2m",
                "startTimestamp": "2026-05-22T01:27:53Z",
            }),
        );
        let dto = bg_status(&value).expect("status maps");
        assert_eq!(dto.uptime, "1h2m");
    }

    #[test]
    fn format_age_since_iso_handles_common_shapes() {
        assert_eq!(format_age_since_iso(""), "");
        assert_eq!(format_age_since_iso("not a timestamp"), "");
        let recent = (chrono::Utc::now() - chrono::Duration::seconds(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        assert!(format_age_since_iso(&recent).ends_with('s'));
        let hours =
            (chrono::Utc::now() - chrono::Duration::hours(3) - chrono::Duration::minutes(15))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        assert_eq!(format_age_since_iso(&hours), "3h 15m");
        let days = (chrono::Utc::now() - chrono::Duration::days(5) - chrono::Duration::hours(7))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        assert_eq!(format_age_since_iso(&days), "5d 7h");
    }

    #[test]
    fn parses_complete_linux_host_metrics() {
        let metrics = parse_host_metrics(
            "memTotalKb=16777216\n\
             memAvailableKb=6291456\n\
             swapTotalKb=2097152\n\
             swapFreeKb=1572864\n\
             cpuUsagePercent=87.5\n\
             loadAverageOne=2.75\n\
             diskTotalKb=524288000\n\
             diskUsedKb=314572800\n\
             uptimeSeconds=86461\n",
        )
        .expect("metrics parse");
        assert_eq!(metrics.memory_total_bytes, 16 * 1024 * 1024 * 1024);
        assert_eq!(metrics.memory_used_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(metrics.swap_used_bytes, 512 * 1024 * 1024);
        assert_eq!(metrics.cpu_usage_percent, Some(87.5));
        assert_eq!(metrics.load_average_one, Some(2.75));
        assert_eq!(metrics.disk_total_bytes, 500 * 1024 * 1024 * 1024);
        assert_eq!(metrics.disk_used_bytes, 300 * 1024 * 1024 * 1024);
        assert_eq!(metrics.uptime_seconds, 86461);
    }

    #[test]
    fn host_metrics_allow_missing_optional_values() {
        let metrics = parse_host_metrics(
            "memTotalKb=1024\n\
             memAvailableKb=256\n\
             diskTotalKb=4096\n\
             diskUsedKb=1024\n",
        )
        .expect("partial metrics parse");
        assert_eq!(metrics.memory_used_bytes, 768 * 1024);
        assert_eq!(metrics.swap_total_bytes, 0);
        assert_eq!(metrics.cpu_usage_percent, None);
        assert_eq!(metrics.load_average_one, None);
        assert_eq!(metrics.uptime_seconds, 0);
    }

    #[test]
    fn host_metrics_reject_missing_or_malformed_required_values() {
        assert!(parse_host_metrics("memTotalKb=bad\ndiskTotalKb=10\n").is_none());
        assert!(parse_host_metrics(
            "memTotalKb=1024\nmemAvailableKb=512\ndiskTotalKb=0\ndiskUsedKb=0\n"
        )
        .is_none());
    }
}

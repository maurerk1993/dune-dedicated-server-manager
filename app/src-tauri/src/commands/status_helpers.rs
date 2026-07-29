use std::collections::HashMap;

use serde_json::Value;

use crate::commands::status_naming::{friendly_map_name, serverset_log_key};
use crate::dto::RemoteServerComponent;

pub fn pod_component(
    label: &str,
    log_key: &str,
    pods: &Value,
    metrics: &Value,
    matches: impl Fn(&str, &str) -> bool,
) -> RemoteServerComponent {
    let usage_by_pod = pod_resource_usage(metrics);
    let mut total = 0u64;
    let mut ready = 0u64;
    let mut restarts = 0u64;
    let mut cpu_millicores = 0.0;
    let mut memory_bytes = 0u64;
    let mut has_resource_usage = false;
    let mut reasons = Vec::new();
    let mut phases = Vec::new();
    for item in pods["items"].as_array().cloned().unwrap_or_default() {
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        let role = item["metadata"]["labels"]["role"]
            .as_str()
            .unwrap_or_default();
        if !matches(role, name) {
            continue;
        }
        total += 1;
        if let Some(usage) = usage_by_pod.get(name) {
            has_resource_usage = true;
            cpu_millicores += usage.cpu_millicores;
            memory_bytes = memory_bytes.saturating_add(usage.memory_bytes);
        }
        let phase = item["status"]["phase"].as_str().unwrap_or_default();
        if !phase.is_empty() {
            phases.push(phase.to_string());
        }
        let statuses = item["status"]["containerStatuses"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let pod_ready = !statuses.is_empty()
            && statuses
                .iter()
                .all(|status| status["ready"].as_bool().unwrap_or(false));
        if pod_ready || phase == "Succeeded" {
            ready += 1;
        }
        for status in statuses {
            restarts += status["restartCount"].as_u64().unwrap_or_default();
            if let Some(reason) = status["state"]["waiting"]["reason"].as_str() {
                reasons.push(reason.to_string());
            }
            if let Some(reason) = status["state"]["terminated"]["reason"].as_str() {
                if reason != "Completed" {
                    reasons.push(reason.to_string());
                }
            }
        }
    }

    if total == 0 {
        return with_pod_status(
            component(
                label,
                log_key,
                "system",
                "Not present",
                "gray",
                "No matching runtime component was found.",
                vec![],
            ),
            0,
            0,
            0,
            None,
            None,
        );
    }
    let details = compact_details(vec![
        format!("{ready}/{total} pods ready"),
        if restarts > 0 {
            format!("{restarts} container restarts")
        } else {
            String::new()
        },
        if reasons.is_empty() {
            String::new()
        } else {
            format!("Reason: {}", reasons.join(", "))
        },
    ]);
    let result = if ready == total && reasons.is_empty() {
        component(
            label,
            log_key,
            "system",
            "Ready",
            "green",
            "All pods are ready.",
            details,
        )
    } else if reasons.iter().any(|reason| is_bad_reason(reason))
        || phases.iter().any(|phase| phase == "Failed")
    {
        component(
            label,
            log_key,
            "system",
            "Problem",
            "red",
            "One or more pods are failing.",
            details,
        )
    } else {
        component(
            label,
            log_key,
            "system",
            "Starting",
            "amber",
            "Waiting for pods to become ready.",
            details,
        )
    };
    with_pod_status(
        result,
        ready,
        total,
        restarts,
        has_resource_usage.then_some(cpu_millicores),
        has_resource_usage.then_some(memory_bytes),
    )
}

#[derive(Debug, Default)]
struct PodResourceUsage {
    cpu_millicores: f64,
    memory_bytes: u64,
}

fn pod_resource_usage(metrics: &Value) -> HashMap<String, PodResourceUsage> {
    let mut output = HashMap::new();
    let Some(items) = metrics.get("items").and_then(Value::as_array) else {
        return output;
    };
    for item in items {
        let Some(name) = item
            .get("metadata")
            .and_then(|metadata| metadata.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let mut usage = PodResourceUsage::default();
        let mut observed = false;
        for container in item
            .get("containers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let values = container.get("usage").cloned().unwrap_or(Value::Null);
            if let Some(cpu) = values
                .get("cpu")
                .and_then(Value::as_str)
                .and_then(parse_cpu_quantity)
            {
                usage.cpu_millicores += cpu;
                observed = true;
            }
            if let Some(memory) = values
                .get("memory")
                .and_then(Value::as_str)
                .and_then(parse_memory_quantity)
            {
                usage.memory_bytes = usage.memory_bytes.saturating_add(memory);
                observed = true;
            }
        }
        if observed {
            output.insert(name.to_string(), usage);
        }
    }
    output
}

fn parse_cpu_quantity(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(raw) = value.strip_suffix('n') {
        return raw.parse::<f64>().ok().map(|number| number / 1_000_000.0);
    }
    if let Some(raw) = value.strip_suffix('u') {
        return raw.parse::<f64>().ok().map(|number| number / 1_000.0);
    }
    if let Some(raw) = value.strip_suffix('m') {
        return raw.parse::<f64>().ok();
    }
    value.parse::<f64>().ok().map(|number| number * 1_000.0)
}

fn parse_memory_quantity(value: &str) -> Option<u64> {
    let value = value.trim();
    const UNITS: [(&str, f64); 8] = [
        ("Ki", 1024.0),
        ("Mi", 1024.0 * 1024.0),
        ("Gi", 1024.0 * 1024.0 * 1024.0),
        ("Ti", 1024.0 * 1024.0 * 1024.0 * 1024.0),
        ("K", 1_000.0),
        ("M", 1_000_000.0),
        ("G", 1_000_000_000.0),
        ("T", 1_000_000_000_000.0),
    ];
    for (suffix, multiplier) in UNITS {
        if let Some(raw) = value.strip_suffix(suffix) {
            return raw
                .parse::<f64>()
                .ok()
                .map(|number| (number * multiplier).max(0.0) as u64);
        }
    }
    value.parse::<u64>().ok()
}

fn with_pod_status(
    mut component: RemoteServerComponent,
    ready_pods: u64,
    total_pods: u64,
    restart_count: u64,
    cpu_millicores: Option<f64>,
    memory_bytes: Option<u64>,
) -> RemoteServerComponent {
    component.component_kind = "pod-group".to_string();
    component.ready_pods = Some(ready_pods);
    component.total_pods = Some(total_pods);
    component.restart_count = restart_count;
    component.cpu_millicores = cpu_millicores;
    component.memory_bytes = memory_bytes;
    component
}

pub fn server_resource_components(resources: &Value) -> Vec<RemoteServerComponent> {
    let mut items = resources["items"].as_array().cloned().unwrap_or_default();
    items.sort_by(|left, right| {
        left["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["metadata"]["name"].as_str().unwrap_or_default())
    });
    let mut output = Vec::new();
    for item in items {
        let kind = item["kind"].as_str().unwrap_or_default();
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        match kind {
            "ServerGroup" => output.push(server_group_component(&item)),
            "ServerGateway" => output.push(resource_phase_component("Gateway Resource", &item)),
            "ServerSet" => {
                if should_show_serverset(&item) {
                    output.push(serverset_component(name, &item));
                }
            }
            _ => {}
        }
    }
    output
}

fn server_group_component(item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        "Server Group",
        "server-group",
        "system",
        phase,
        format!("Server Group reports {phase}."),
        vec![],
    )
}

fn resource_phase_component(label: &str, item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        label,
        "gateway-resource",
        "system",
        phase,
        format!("{label} reports {phase}."),
        vec![],
    )
}

fn serverset_component(name: &str, item: &Value) -> RemoteServerComponent {
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    let label = friendly_map_name(map, name);
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let ready = item["status"]["readyReplicas"].as_u64().unwrap_or_default();
    let completed = item["status"]["completedReplicas"]
        .as_u64()
        .unwrap_or_default();
    let pods = item["status"]["pods"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let game_ready = pods
        .iter()
        .filter(|pod| pod["ready"].as_bool().unwrap_or(false))
        .count();
    let details = compact_details(vec![
        format!("{ready}/{target} Kubernetes-ready replicas"),
        format!("{completed}/{target} completed game replicas"),
        format!("{game_ready}/{target} game-ready servers"),
    ]);
    let summary =
        if phase == "Initializing" && ready >= target && target > 0 && game_ready < target as usize
        {
            "Game process is running, but game readiness has not completed.".to_string()
        } else {
            format!("{label} reports {phase}.")
        };
    phase_component(
        &label,
        &serverset_log_key(name, map),
        "map",
        phase,
        summary,
        details,
    )
}

fn should_show_serverset(item: &Value) -> bool {
    let phase = item["status"]["phase"].as_str().unwrap_or_default();
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    phase != "Stopped" || target > 0 || matches!(map, "Survival_1" | "Overmap" | "DeepDesert_1")
}

fn phase_component(
    label: &str,
    log_key: &str,
    category: &str,
    phase: &str,
    summary: String,
    details: Vec<String>,
) -> RemoteServerComponent {
    let normalized = phase.to_ascii_lowercase();
    let (state, tone) = match normalized.as_str() {
        "healthy" | "running" | "ready" | "available" => ("Ready", "green"),
        "stopped" | "suspended" => ("Stopped", "gray"),
        "initializing" | "reconciling" | "pending" | "starting" => ("Starting", "amber"),
        "failed" | "error" | "degraded" => ("Problem", "red"),
        _ => ("Unknown", "amber"),
    };
    component(label, log_key, category, state, tone, summary, details)
}

fn component(
    name: &str,
    log_key: &str,
    category: &str,
    state: &str,
    tone: &str,
    summary: impl Into<String>,
    details: Vec<String>,
) -> RemoteServerComponent {
    RemoteServerComponent {
        name: name.to_string(),
        log_key: log_key.to_string(),
        category: category.to_string(),
        component_kind: "operator-resource".to_string(),
        state: state.to_string(),
        tone: tone.to_string(),
        summary: summary.into(),
        details,
        ready_pods: None,
        total_pods: None,
        restart_count: 0,
        cpu_millicores: None,
        memory_bytes: None,
    }
}

fn compact_details(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn is_bad_reason(reason: &str) -> bool {
    matches!(
        reason,
        "CrashLoopBackOff"
            | "ImagePullBackOff"
            | "ErrImagePull"
            | "CreateContainerConfigError"
            | "CreateContainerError"
            | "RunContainerError"
            | "OOMKilled"
            | "Error"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_kubernetes_resource_quantities() {
        assert_eq!(parse_cpu_quantity("250m"), Some(250.0));
        assert_eq!(parse_cpu_quantity("1500000n"), Some(1.5));
        assert_eq!(parse_cpu_quantity("1200u"), Some(1.2));
        assert_eq!(parse_cpu_quantity("2"), Some(2000.0));
        assert_eq!(parse_memory_quantity("512Ki"), Some(512 * 1024));
        assert_eq!(
            parse_memory_quantity("1.5Gi"),
            Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(parse_memory_quantity("100M"), Some(100_000_000));
        assert_eq!(parse_memory_quantity("4096"), Some(4096));
    }

    #[test]
    fn pod_component_aggregates_readiness_restarts_and_usage() {
        let pods = json!({
            "items": [
                {
                    "metadata": {"name": "bg-db-0", "labels": {"role": "database"}},
                    "status": {
                        "phase": "Running",
                        "containerStatuses": [
                            {"ready": true, "restartCount": 1, "state": {"running": {}}}
                        ]
                    }
                },
                {
                    "metadata": {"name": "bg-db-1", "labels": {"role": "database"}},
                    "status": {
                        "phase": "Pending",
                        "containerStatuses": [
                            {"ready": false, "restartCount": 2, "state": {"waiting": {"reason": "ContainerCreating"}}}
                        ]
                    }
                }
            ]
        });
        let metrics = json!({
            "items": [
                {
                    "metadata": {"name": "bg-db-0"},
                    "containers": [{"usage": {"cpu": "250m", "memory": "512Mi"}}]
                },
                {
                    "metadata": {"name": "bg-db-1"},
                    "containers": [{"usage": {"cpu": "100m", "memory": "256Mi"}}]
                }
            ]
        });
        let component = pod_component("Database", "database", &pods, &metrics, |role, _| {
            role == "database"
        });
        assert_eq!(component.component_kind, "pod-group");
        assert_eq!(component.ready_pods, Some(1));
        assert_eq!(component.total_pods, Some(2));
        assert_eq!(component.restart_count, 3);
        assert_eq!(component.cpu_millicores, Some(350.0));
        assert_eq!(component.memory_bytes, Some(768 * 1024 * 1024));
        assert_eq!(component.state, "Starting");
    }

    #[test]
    fn unavailable_metrics_do_not_hide_pod_health() {
        let pods = json!({
            "items": [{
                "metadata": {"name": "bg-director-0", "labels": {"role": "battlegroup-director"}},
                "status": {
                    "phase": "Running",
                    "containerStatuses": [
                        {"ready": true, "restartCount": 0, "state": {"running": {}}}
                    ]
                }
            }]
        });
        let component = pod_component("Director", "director", &pods, &Value::Null, |role, _| {
            role == "battlegroup-director"
        });
        assert_eq!(component.state, "Ready");
        assert_eq!(component.ready_pods, Some(1));
        assert_eq!(component.total_pods, Some(1));
        assert_eq!(component.cpu_millicores, None);
        assert_eq!(component.memory_bytes, None);
    }
}

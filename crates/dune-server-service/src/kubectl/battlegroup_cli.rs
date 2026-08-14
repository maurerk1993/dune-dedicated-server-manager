use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::time::{sleep, Instant};

use super::{battlegroup, run_process, KubectlClient};

/// Wraps the vendor `battlegroup` helper at `${DUNE_BIN_DIR}/battlegroup` plus
/// the readiness-polling utilities used by the daily-restart workflow.
#[derive(Clone)]
pub struct BattlegroupCli {
    bin: PathBuf,
}

impl BattlegroupCli {
    pub fn new(bin_dir: &std::path::Path) -> Self {
        Self {
            bin: bin_dir.join("battlegroup"),
        }
    }

    fn bin_str(&self) -> String {
        self.bin.to_string_lossy().into_owned()
    }

    pub async fn stop(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["stop"], None, 120).await?;
        result.require_ok("battlegroup stop")
    }

    pub async fn start(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["start"], None, 120).await?;
        result.require_ok("battlegroup start")
    }

    pub async fn restart(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["restart"], None, 1200).await?;
        result.require_ok("battlegroup restart")
    }

    pub async fn backup(&self, backup_name: &str) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["backup", backup_name], None, 600).await?;
        result.require_ok(&format!("battlegroup backup {backup_name}"))
    }
}

/// Wait for the battlegroup to reach a fully stopped state: `spec.stop=true`
/// AND no server pods (matching `-sg-...-pod-`) present.
pub async fn wait_until_stopped(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
    timeout: Duration,
) -> Result<()> {
    let start = Instant::now();
    let interval = Duration::from_secs(10);
    while start.elapsed() < timeout {
        let stop_value = battlegroup::bg_field(kubectl, namespace, bg_name, "{.spec.stop}")
            .await
            .unwrap_or_default();
        let pod_count = count_server_pods(kubectl, namespace).await.unwrap_or(0);
        tracing::info!(
            stop = %stop_value,
            pods = pod_count,
            elapsed_s = start.elapsed().as_secs(),
            "waiting for battlegroup stop"
        );
        if stop_value == "true" && pod_count == 0 {
            return Ok(());
        }
        sleep(interval).await;
    }
    Err(anyhow!(
        "timeout waiting for battlegroup {bg_name} to stop after {}s",
        timeout.as_secs()
    ))
}

/// Wait for the battlegroup to reach a fully-running state: serverGroupPhase
/// is "Running" AND all servers report ready=true with phase=Running.
pub async fn wait_until_running(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
    timeout: Duration,
) -> Result<ReadySummary> {
    let start = Instant::now();
    let interval = Duration::from_secs(10);
    while start.elapsed() < timeout {
        if let Ok(summary) = ready_summary(kubectl, namespace, bg_name).await {
            tracing::info!(
                phase = %summary.phase,
                server_group_phase = %summary.server_group_phase,
                ready = %format!("{}/{}", summary.ready, summary.size),
                elapsed_s = start.elapsed().as_secs(),
                "waiting for battlegroup run"
            );
            if summary.is_running() {
                return Ok(summary);
            }
        }
        sleep(interval).await;
    }
    Err(anyhow!(
        "timeout waiting for battlegroup {bg_name} to become ready after {}s",
        timeout.as_secs()
    ))
}

pub async fn count_server_pods(kubectl: &KubectlClient, namespace: &str) -> Result<usize> {
    let result = kubectl
        .run(&[
            "get",
            "pods",
            "-n",
            namespace,
            "--no-headers",
            "-o",
            "custom-columns=NAME:.metadata.name,DEL:.metadata.deletionTimestamp",
        ])
        .await?;
    result.require_ok(&format!("kubectl get pods -n {namespace}"))?;
    let mut count = 0;
    for line in result.stdout.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let name = parts.next().unwrap_or("");
        let deletion = parts.next().unwrap_or("");
        if name.contains("-sg-")
            && name.contains("-pod-")
            && (deletion.is_empty() || deletion == "<none>")
        {
            count += 1;
        }
    }
    Ok(count)
}

#[derive(Debug, Clone)]
pub struct ReadySummary {
    pub phase: String,
    pub server_group_phase: String,
    pub ready: u32,
    pub size: u32,
}

impl ReadySummary {
    /// A battlegroup counts as back up once both the overall phase and the
    /// server-group phase report a started-ish state. This mirrors the desktop
    /// UI classifier (`is_started_phase` in dune-manager-core) which treats
    /// `Reconciling` as up: a BG can linger at `phase=Reconciling` with
    /// `serverGroupPhase=Running` while every map server is already reachable.
    /// Holding out for `serverGroupPhase=="Running"` + all per-server `ready`
    /// flags produced false restart-timeout failures (issue #20), so we gate on
    /// the phases only and keep `ready`/`size` purely for logging.
    pub fn is_running(&self) -> bool {
        is_started_phase(&self.phase) && is_started_phase(&self.server_group_phase)
    }
}

/// Phases that mean "the battlegroup is started / serving", matching the
/// desktop UI's `is_started_phase`. `Reconciling` is included on purpose: the
/// servers are reachable in that state even though the controller has not
/// settled back to `Running`.
fn is_started_phase(phase: &str) -> bool {
    matches!(
        phase.trim().to_ascii_lowercase().as_str(),
        "running" | "ready" | "healthy" | "available" | "reconciling"
    )
}

pub async fn ready_summary(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
) -> Result<ReadySummary> {
    let bg = battlegroup::bg_json(kubectl, namespace, bg_name).await?;
    let status = bg
        .get("status")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let phase = status
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let server_group_phase = status
        .get("serverGroupPhase")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let servers = status
        .get("servers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ready = servers
        .iter()
        .filter(|s| {
            s.get("ready").and_then(|v| v.as_bool()).unwrap_or(false)
                && s.get("phase").and_then(|v| v.as_str()) == Some("Running")
        })
        .count() as u32;
    let size = status
        .get("size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or_else(|| servers.len() as u32);
    Ok(ReadySummary {
        phase,
        server_group_phase,
        ready,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(phase: &str, sgp: &str, ready: u32, size: u32) -> ReadySummary {
        ReadySummary {
            phase: phase.to_string(),
            server_group_phase: sgp.to_string(),
            ready,
            size,
        }
    }

    #[test]
    fn reconciling_bg_is_running_even_with_lagging_ready_flags() {
        // Issue #20: real payload was phase=Reconciling, serverGroupPhase=Running
        // with per-server ready flags not yet flipped. Servers were reachable, so
        // the gate must accept it instead of timing out at 1200s.
        assert!(summary("Reconciling", "Running", 0, 3).is_running());
        assert!(summary("Reconciling", "Reconciling", 1, 3).is_running());
        assert!(summary("Running", "Running", 3, 3).is_running());
    }

    #[test]
    fn stopped_or_empty_phase_is_not_running() {
        assert!(!summary("Stopped", "Stopped", 0, 0).is_running());
        assert!(!summary("", "", 0, 0).is_running());
        // serverGroupPhase still tearing down -> not up yet.
        assert!(!summary("Reconciling", "Stopped", 0, 3).is_running());
    }
}

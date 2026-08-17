use std::sync::Arc;

use chrono_tz::Tz;

use crate::admin::MqPublisher;
use crate::kubectl::{BattlegroupCli, ClusterCache, KubectlClient};
use crate::postgres::PgClient;

pub mod restart;
pub mod restart_notice;
pub mod welcome_package;

/// Heavy-weight resources shared by all scheduled tasks. Constructed once in
/// `main.rs` from [`crate::config::ServiceConfig`] and dropped into the
/// scheduler so each `Task::run` call can borrow what it needs.
pub struct TaskEnv {
    pub kubectl: KubectlClient,
    pub cluster: ClusterCache,
    pub bg_cli: BattlegroupCli,
    pub mq: Arc<MqPublisher>,
    pub pg: Arc<PgClient>,
    /// Master switch for the daily restart and its pre-restart warning
    /// broadcast. Defaults to true; existing installs (no stored row) keep the
    /// prior always-on behavior.
    pub restart_enabled: bool,
    /// Restart-notice + restart wall-clock target (default 05:00).
    pub restart_hour: u32,
    pub restart_minute: u32,
    /// Restart broadcast frequency / declared shutdown duration.
    pub restart_warning_frequency_secs: u64,
    pub restart_warning_duration_secs: u64,
    pub restart_tz: Tz,
    /// Enables the opt-in new-player welcome-package worker.
    pub welcome_package_enabled: bool,
    /// Enables the welcome whisper worker independently from item/package
    /// grants.
    pub welcome_message_enabled: bool,
    /// Operator-controlled package version. Changing it grants the package
    /// again because the package ledger key is
    /// `(player_id, package_version, account_id)`.
    pub welcome_package_version: String,
    /// JSON config for welcome-package actions. Kept as parsed data in the env
    /// so scheduled fires don't re-parse sqlite state.
    pub welcome_package_actions: Vec<welcome_package::WelcomePackageAction>,
    /// Verbatim JSON string for UI echo/restart-required checks.
    pub welcome_package_actions_json: String,
    /// Player lookup used as the visible sender for welcome whispers. Empty
    /// falls back to the recipient for self-sourced whispers.
    pub welcome_whisper_source_player: String,
    /// Welcome whisper text used by the automated action and manual send.
    pub welcome_message: String,
}

/// All task implementations registered for the scheduler.
pub fn build_all(env: Arc<TaskEnv>) -> Vec<Arc<dyn crate::scheduler::Task>> {
    vec![
        Arc::new(restart_notice::RestartNoticeTask::new(env.clone()))
            as Arc<dyn crate::scheduler::Task>,
        Arc::new(restart::RestartTask::new(env.clone())),
        Arc::new(welcome_package::WelcomePackageTask::new(env.clone())),
        Arc::new(welcome_package::WelcomeMessageTask::new(env)),
    ]
}

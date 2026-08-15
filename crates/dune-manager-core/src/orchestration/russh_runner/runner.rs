//! Sync [`RemoteCommandRunner`] backed by russh with a cached session.

use std::sync::Arc;
use std::{future::Future, time::Duration};

use tokio::sync::Mutex as AsyncMutex;

use crate::models::CommandResult;
use crate::orchestration::RemoteCommandRunner;

use super::session::{
    close as close_session, connect_and_authenticate, exec_capture, shared_runtime, SessionHandle,
};
use super::target::RusshTarget;

/// Generous default for commands outside the dashboard refresh path. Callers
/// that perform short, read-only probes should opt into a smaller deadline via
/// [`RusshRunner::with_command_timeout_seconds`].
const DEFAULT_COMMAND_TIMEOUT_SECONDS: u64 = 15 * 60;
const COMMAND_TIMEOUT_PREFIX: &str = "ssh command timed out";

/// Remote command runner that exposes a sync interface backed by a cached
/// russh session.
///
/// The runner keeps one SSH session alive per instance. The session is
/// established lazily on the first call and reconnected automatically if a
/// command fails (e.g. the server dropped the connection). Cloned runners
/// share the cached session, so commands issued through multiple clones are
/// serialized over a single SSH connection.
#[derive(Clone)]
pub struct RusshRunner {
    target: RusshTarget,
    session: Arc<AsyncMutex<Option<SessionHandle>>>,
    command_timeout: Duration,
}

impl std::fmt::Debug for RusshRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RusshRunner")
            .field("target", &self.target)
            .finish()
    }
}

impl RusshRunner {
    /// Creates a runner that will lazily open a session to the given target.
    pub fn new(target: RusshTarget) -> Self {
        Self {
            target,
            session: Arc::new(AsyncMutex::new(None)),
            command_timeout: Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECONDS),
        }
    }

    /// Sets an absolute wall-clock deadline for each call to [`Self::run`],
    /// [`Self::run_script`], or [`Self::run_with_stdin`]. The deadline covers
    /// connection, authentication, retry, input transfer, and remote command
    /// completion so an active-but-stalled SSH session cannot block forever.
    pub fn with_command_timeout_seconds(mut self, seconds: u64) -> Self {
        self.command_timeout = Duration::from_secs(seconds.max(1));
        self
    }

    /// Returns the connection target used by this runner.
    pub fn target(&self) -> &RusshTarget {
        &self.target
    }

    /// Closes the cached session if one exists.
    pub fn close(&self) {
        let session = self.session.clone();
        shared_runtime().block_on(async move {
            if let Some(handle) = session.lock().await.take() {
                close_session(&handle).await;
            }
        });
    }

    /// Runs a command while streaming arbitrary stdin bytes to the remote
    /// process. This is intended for binary payload uploads where embedding
    /// base64 in a shell script would create a very large command body.
    pub fn run_with_stdin(&self, command: &str, stdin_body: &[u8]) -> CommandResult<String> {
        let runner = self.clone();
        let command = command.to_string();
        let stdin_body = stdin_body.to_vec();
        shared_runtime()
            .block_on(async move { runner.exec_with_deadline(&command, Some(&stdin_body)).await })
    }

    async fn exec_with_deadline(
        &self,
        command: &str,
        stdin_body: Option<&[u8]>,
    ) -> CommandResult<String> {
        let destination = self.target.destination();
        let result = await_command_deadline(
            self.exec_with_retry(command, stdin_body),
            self.command_timeout,
            &destination,
        )
        .await;
        if result
            .as_ref()
            .is_err_and(|err| err.message.starts_with(COMMAND_TIMEOUT_PREFIX))
        {
            self.discard_cached_session().await;
        }
        result
    }

    async fn discard_cached_session(&self) {
        if let Some(handle) = self.session.lock().await.take() {
            // Disconnect is best-effort. The important recovery behavior is
            // removing the suspect cached session so the next refresh starts
            // with a new SSH connection.
            let _ = tokio::time::timeout(Duration::from_secs(1), close_session(&handle)).await;
        }
    }

    async fn exec_with_retry(
        &self,
        command: &str,
        stdin_body: Option<&[u8]>,
    ) -> CommandResult<String> {
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            self.target.validate()?;
            *guard = Some(connect_and_authenticate(&self.target).await?);
        }
        let first_attempt = {
            let handle = guard.as_ref().expect("session populated above");
            exec_capture(handle, command, stdin_body).await
        };
        match first_attempt {
            Ok(text) => Ok(text),
            Err(err) if is_remote_command_error(&err) => Err(err),
            Err(_) => {
                if let Some(handle) = guard.take() {
                    close_session(&handle).await;
                }
                self.target.validate()?;
                *guard = Some(connect_and_authenticate(&self.target).await?);
                let handle = guard.as_ref().expect("session populated above");
                exec_capture(handle, command, stdin_body).await
            }
        }
    }
}

fn is_remote_command_error(err: &crate::models::CommandFailure) -> bool {
    err.code.is_some() || !err.stdout.is_empty() || !err.stderr.is_empty()
}

impl RemoteCommandRunner for RusshRunner {
    fn run(&self, command: &str) -> CommandResult<String> {
        let runner = self.clone();
        let command = command.to_string();
        shared_runtime().block_on(async move { runner.exec_with_deadline(&command, None).await })
    }

    fn run_script(&self, script: &str) -> CommandResult<String> {
        let runner = self.clone();
        let script = script.to_string();
        shared_runtime().block_on(async move {
            runner
                .exec_with_deadline("sh -s", Some(script.as_bytes()))
                .await
        })
    }
}

async fn await_command_deadline<T, F>(
    operation: F,
    timeout: Duration,
    destination: &str,
) -> CommandResult<T>
where
    F: Future<Output = CommandResult<T>>,
{
    tokio::time::timeout(timeout, operation)
        .await
        .map_err(|_| {
            crate::errors::failure(format!(
                "{COMMAND_TIMEOUT_PREFIX} on {destination} after {}s",
                timeout.as_secs()
            ))
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_is_clone_and_debug() {
        let target = RusshTarget::new("key", "dune", "10.0.0.4");
        let runner = RusshRunner::new(target.clone());
        let _clone = runner.clone();
        assert_eq!(runner.target(), &target);
        assert!(format!("{runner:?}").contains("dune"));
    }

    #[test]
    fn command_deadline_returns_success_before_timeout() {
        let result = shared_runtime().block_on(await_command_deadline(
            async { Ok::<_, crate::models::CommandFailure>("ready") },
            Duration::from_secs(1),
            "dune@example",
        ));
        assert_eq!(result.unwrap(), "ready");
    }

    #[test]
    fn command_deadline_interrupts_a_stalled_operation() {
        let result = shared_runtime().block_on(await_command_deadline(
            async {
                std::future::pending::<()>().await;
                Ok::<_, crate::models::CommandFailure>(())
            },
            Duration::from_millis(5),
            "dune@example",
        ));
        let error = result.unwrap_err();
        assert!(error.message.starts_with(COMMAND_TIMEOUT_PREFIX));
        assert!(error.message.contains("dune@example"));
        assert_eq!(error.code, None);
    }
}

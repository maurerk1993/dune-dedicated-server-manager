use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::kubectl::{ClusterCache, KubectlClient};

const EXCHANGE: &str = "heartbeats";
const ROUTING_KEY: &str = "notifications";
const WHISPER_EXCHANGE: &str = "chat.whispers";
const USER_ID: &str = "fls";
const APP_ID: &str = "fls_backend";

static SAFE_LABEL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9_-]{0,63}$").unwrap());

const RABBITMQ_EVAL_SHELL: &str = "set -eu; \
    export PATH=/opt/rabbitmq/sbin:/opt/erlang/lib/erlang/bin:/bin:/usr/bin:/usr/local/bin:$PATH; \
    expr=$(cat); \
    /opt/rabbitmq/sbin/rabbitmqctl eval \"$expr\"";

#[derive(Debug, Clone, Copy)]
pub enum ShutdownType {
    Restart,
    Maintenance,
    Update,
}

impl ShutdownType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Maintenance => "Maintenance",
            Self::Update => "Update",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishResult {
    pub ok: bool,
    pub output: String,
}

#[derive(Clone)]
pub struct MqPublisher {
    kubectl: KubectlClient,
    cluster: ClusterCache,
    token: Arc<String>,
    /// RabbitMQ commands are intentionally serialized. Besides keeping
    /// operator actions ordered, this prevents concurrent admin/background
    /// publishes from interfering with one another inside the MQ pod.
    publish_lock: Arc<Mutex<()>>,
}

impl MqPublisher {
    pub fn new(kubectl: KubectlClient, cluster: ClusterCache, token: String) -> Self {
        Self {
            kubectl,
            cluster,
            token: Arc::new(token),
            publish_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub async fn publish_inner(&self, inner: &Value, label: &str) -> Result<PublishResult> {
        publish_inner(self, inner, label).await
    }

    pub async fn publish_whisper(
        &self,
        routing_key: &str,
        sender_fls_id: &str,
        body: &Value,
        label: &str,
    ) -> Result<PublishResult> {
        publish_whisper(self, routing_key, sender_fls_id, body, label).await
    }

    pub async fn publish_service_broadcast(
        &self,
        title: &str,
        body: &str,
        duration_secs: u64,
    ) -> Result<PublishResult> {
        publish_service_broadcast(self, title, body, duration_secs).await
    }

    pub async fn publish_server_shutdown(
        &self,
        shutdown_type: ShutdownType,
        shutdown_timestamp: i64,
        frequency_secs: u64,
        duration_secs: u64,
        broadcast_duration_secs: u64,
    ) -> Result<PublishResult> {
        publish_server_shutdown(
            self,
            shutdown_type,
            shutdown_timestamp,
            frequency_secs,
            duration_secs,
            broadcast_duration_secs,
        )
        .await
    }

    pub async fn publish_server_shutdown_cancel(&self) -> Result<PublishResult> {
        publish_server_shutdown_cancel(self).await
    }
}

/// Build the base64-encoded outer envelope expected by the seabass server-
/// command handler.
pub fn envelope_for_command(inner: &Value, token: &str) -> String {
    let inner_str = serde_json::to_string(inner).unwrap_or_else(|_| "{}".to_string());
    let outer = json!({
        "Version": 2,
        "AuthToken": token,
        "MessageContent": inner_str,
    });
    let outer_bytes = serde_json::to_vec(&outer).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(outer_bytes)
}

/// Build the Erlang expression that `rabbitmqctl eval` executes inside the MQ
/// pod. Byte-equivalent to the publish snippet in `mq.ts` so the server-side
/// log line stays consistent.
pub fn build_erlang_publish(payload_b64: &str, label: &str) -> String {
    let label = if SAFE_LABEL_RE.is_match(label) {
        label
    } else {
        "smgmt"
    };
    format!(
        "Outer = base64:decode(<<\"{payload_b64}\">>),\n\
         XName = rabbit_misc:r(<<\"/\">>, exchange, <<\"{EXCHANGE}\">>),\n\
         X = rabbit_exchange:lookup_or_die(XName),\n\
         MsgId = list_to_binary(\"smgmt-{label}-\" ++ integer_to_list(erlang:system_time(millisecond))),\n\
         P = {{list_to_atom(\"P_basic\"), <<\"Content\">>, undefined, [], undefined, undefined, undefined, undefined, undefined, MsgId, undefined, undefined, <<\"{USER_ID}\">>, <<\"{APP_ID}\">>, undefined}},\n\
         Content = rabbit_basic:build_content(P, Outer),\n\
         {{ok, Msg}} = rabbit_basic:message(XName, <<\"{ROUTING_KEY}\">>, Content),\n\
         Result = rabbit_queue_type:publish_at_most_once(X, Msg),\n\
         io:format(\"publish=~p exchange={EXCHANGE} routing={ROUTING_KEY} app_id={APP_ID} user_id={USER_ID} label={label}~n\", [Result]).\n",
    )
}

pub fn build_erlang_whisper_publish(
    payload_b64: &str,
    routing_key_b64: &str,
    sender_fls_id_b64: &str,
    label: &str,
) -> String {
    let label = if SAFE_LABEL_RE.is_match(label) {
        label
    } else {
        "smgmt"
    };
    let exchange_b64 = base64::engine::general_purpose::STANDARD.encode(WHISPER_EXCHANGE);
    format!(
        "Body = base64:decode(<<\"{payload_b64}\">>),\n\
         Exchange = base64:decode(<<\"{exchange_b64}\">>),\n\
         RoutingKey = base64:decode(<<\"{routing_key_b64}\">>),\n\
         Sender = base64:decode(<<\"{sender_fls_id_b64}\">>),\n\
         XName = rabbit_misc:r(<<\"/\">>, exchange, Exchange),\n\
         X = rabbit_exchange:lookup_or_die(XName),\n\
         MsgId = list_to_binary(\"smgmt-{label}-\" ++ integer_to_list(erlang:system_time(millisecond))),\n\
         P = {{list_to_atom(\"P_basic\"), <<\"Content\">>, undefined, [], undefined, undefined, undefined, undefined, undefined, MsgId, undefined, <<\"text_chat\">>, Sender, <<\"{APP_ID}\">>, undefined}},\n\
         Content = rabbit_basic:build_content(P, Body),\n\
         {{ok, Msg}} = rabbit_basic:message(XName, RoutingKey, Content),\n\
         Result = rabbit_queue_type:publish_at_most_once(X, Msg),\n\
         io:format(\"publish=~p exchange={WHISPER_EXCHANGE} routing=~s app_id={APP_ID} user_id=~s label={label}~n\", [Result, RoutingKey, Sender]).\n",
    )
}

pub async fn publish_inner(
    publisher: &MqPublisher,
    inner: &Value,
    label: &str,
) -> Result<PublishResult> {
    let _publish_guard = publisher.publish_lock.lock().await;
    let cluster = publisher.cluster.get().await?;
    let payload_b64 = envelope_for_command(inner, publisher.token());
    let erlang = build_erlang_publish(&payload_b64, label);

    let result = publisher
        .kubectl
        .run_timeout(
            &[
                "exec",
                "-i",
                "-n",
                &cluster.namespace,
                &cluster.mq_pod,
                "--",
                "sh",
                "-lc",
                RABBITMQ_EVAL_SHELL,
            ],
            Some(&erlang),
            30,
        )
        .await
        .context("kubectl exec rabbitmqctl eval")?;

    let combined = if result.stderr.trim().is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };
    let scrubbed = crate::logger::redact(&combined).into_owned();
    if !result.ok() {
        return Err(anyhow!(
            "rabbitmqctl eval exited {}: {scrubbed}",
            result.exit_code
        ));
    }
    let ok = result.stdout.contains("publish=ok");
    Ok(PublishResult {
        ok,
        output: scrubbed,
    })
}

pub async fn publish_whisper(
    publisher: &MqPublisher,
    routing_key: &str,
    sender_fls_id: &str,
    body: &Value,
    label: &str,
) -> Result<PublishResult> {
    let _publish_guard = publisher.publish_lock.lock().await;
    let cluster = publisher.cluster.get().await?;
    let payload_b64 = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(body).context("serializing whisper body")?);
    let routing_key_b64 = base64::engine::general_purpose::STANDARD.encode(routing_key);
    let sender_fls_id_b64 = base64::engine::general_purpose::STANDARD.encode(sender_fls_id);
    let erlang =
        build_erlang_whisper_publish(&payload_b64, &routing_key_b64, &sender_fls_id_b64, label);

    let result = publisher
        .kubectl
        .run_timeout(
            &[
                "exec",
                "-i",
                "-n",
                &cluster.namespace,
                &cluster.mq_pod,
                "--",
                "sh",
                "-lc",
                RABBITMQ_EVAL_SHELL,
            ],
            Some(&erlang),
            30,
        )
        .await
        .context("kubectl exec rabbitmqctl eval whisper")?;

    let combined = if result.stderr.trim().is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };
    let scrubbed = crate::logger::redact(&combined).into_owned();
    if !result.ok() {
        return Err(anyhow!(
            "rabbitmqctl eval exited {}: {scrubbed}",
            result.exit_code
        ));
    }
    let ok = result.stdout.contains("publish=ok");
    Ok(PublishResult {
        ok,
        output: scrubbed,
    })
}

pub async fn publish_service_broadcast(
    publisher: &MqPublisher,
    title: &str,
    body: &str,
    duration_secs: u64,
) -> Result<PublishResult> {
    let entry = json!({"Title": title, "Body": body});
    let inner = json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "Generic",
        "BroadcastPayload": {
            "BroadcastDuration": duration_secs,
            "LocalizedText": [
                {"Key": "en",    "Title": title, "Body": body},
                {"Key": "en-US", "Title": title, "Body": body},
            ]
        }
    });
    let _ = entry;
    publish_inner(publisher, &inner, "service-broadcast").await
}

pub async fn publish_server_shutdown(
    publisher: &MqPublisher,
    shutdown_type: ShutdownType,
    shutdown_timestamp: i64,
    frequency_secs: u64,
    duration_secs: u64,
    broadcast_duration_secs: u64,
) -> Result<PublishResult> {
    let inner = json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "ServerShutdown",
        "BroadcastPayload": {
            "ShutdownType": shutdown_type.as_str(),
            "DateTimestamp": chrono::Utc::now().timestamp(),
            "ShutdownDuration": duration_secs,
            "ShutdownTimestamp": shutdown_timestamp,
            "BroadcastFrequency": frequency_secs,
            "BroadcastDuration": broadcast_duration_secs,
        }
    });
    publish_inner(publisher, &inner, "server-shutdown").await
}

/// Cancels an in-flight ServerShutdown countdown. The parser short-circuits
/// on `ShouldCancel: true` so no other shutdown metadata is sent.
pub async fn publish_server_shutdown_cancel(publisher: &MqPublisher) -> Result<PublishResult> {
    let inner = json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "ServerShutdown",
        "BroadcastPayload": { "ShouldCancel": true },
    });
    publish_inner(publisher, &inner, "server-shutdown-cancel").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips() {
        let inner = json!({"ServerCommand": "Foo", "X": 1});
        let b64 = envelope_for_command(&inner, "TOKEN123");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let outer: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(outer["Version"], 2);
        assert_eq!(outer["AuthToken"], "TOKEN123");
        let inner_str = outer["MessageContent"].as_str().unwrap();
        let recovered: Value = serde_json::from_str(inner_str).unwrap();
        assert_eq!(recovered, inner);
    }

    #[test]
    fn erlang_uses_safe_label() {
        let snippet = build_erlang_publish("PAYLOAD", "test-label");
        assert!(snippet.contains("smgmt-test-label-"));
        let snippet = build_erlang_publish("PAYLOAD", "weird; rm -rf /");
        assert!(snippet.contains("smgmt-smgmt-")); // unsafe label replaced
    }

    #[test]
    fn erlang_contains_envelope_constants() {
        let s = build_erlang_publish("PAYLOAD", "x");
        assert!(s.contains("<<\"heartbeats\">>"));
        assert!(s.contains("<<\"notifications\">>"));
        assert!(s.contains("<<\"fls\">>"));
        assert!(s.contains("<<\"fls_backend\">>"));
        assert!(s.contains("<<\"Content\">>"));
    }

    #[test]
    fn whisper_erlang_uses_chat_whispers_and_sender() {
        let routing = base64::engine::general_purpose::STANDARD.encode("recipient-chat-id");
        let sender = base64::engine::general_purpose::STANDARD.encode("sender-fls-id");
        let s = build_erlang_whisper_publish("PAYLOAD", &routing, &sender, "whisper");
        assert!(s.contains("chat.whispers"));
        assert!(s.contains("<<\"text_chat\">>"));
        assert!(s.contains("Sender"));
        assert!(s.contains("RoutingKey"));
    }

    #[test]
    fn rabbitmq_eval_reads_stdin_without_shared_temp_files() {
        assert!(RABBITMQ_EVAL_SHELL.contains("expr=$(cat)"));
        assert!(!RABBITMQ_EVAL_SHELL.contains("/tmp/"));
    }

    #[tokio::test]
    async fn cloned_publishers_share_serialization_lock() {
        let kubectl = KubectlClient::new(false, None, None, None);
        let publisher = MqPublisher::new(
            kubectl.clone(),
            ClusterCache::new(kubectl),
            "test-token".to_string(),
        );
        let cloned = publisher.clone();

        let _guard = publisher.publish_lock.lock().await;
        assert!(cloned.publish_lock.try_lock().is_err());
    }
}

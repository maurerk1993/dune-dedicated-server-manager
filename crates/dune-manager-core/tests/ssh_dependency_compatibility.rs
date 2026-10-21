//! Local protocol regressions for dependency upgrades. Keys are generated per
//! test; no saved profiles, vendor keys, shell commands or real servers are used.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

use dune_manager_core::orchestration::{
    LocalForwarder, RemoteCommandRunner, RusshRunner, RusshTarget,
};
use russh::keys::{Algorithm, EcdsaCurve, PrivateKey, PublicKey};
use russh::server::{self, ChannelOpenHandle, Msg, Session};
use russh::{Channel, ChannelId};

#[derive(Clone)]
struct FixtureHandler {
    authorized_key: PublicKey,
    uploads: HashMap<ChannelId, Vec<u8>>,
}

impl server::Handler for FixtureHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && key == &self.authorized_key {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        match data {
            b"fixture:upload" => {
                self.uploads.insert(channel, Vec::new());
            }
            b"fixture:status" => {
                session.data(channel, b"fixture-ready".to_vec())?;
                session.exit_status_request(channel, 0)?;
                session.eof(channel)?;
                session.close(channel)?;
            }
            _ => {
                session.extended_data(channel, 1, b"expected fixture failure".to_vec())?;
                session.exit_status_request(channel, 7)?;
                session.eof(channel)?;
                session.close(channel)?;
            }
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(upload) = self.uploads.get_mut(&channel) {
            upload.extend_from_slice(data);
        } else {
            // direct-tcpip fixture: echo locally; never open a remote socket.
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(upload) = self.uploads.remove(&channel) {
            session.data(channel, upload)?;
            session.exit_status_request(channel, 0)?;
            session.eof(channel)?;
            session.close(channel)?;
        }
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        _channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!((host, port), ("127.0.0.1", 9));
        reply.accept().await;
        Ok(())
    }
}

fn exercise_connection(algorithm: Algorithm) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let key_dir = tempfile::tempdir().unwrap();
    let client_key = PrivateKey::random(&mut rand::rng(), algorithm).unwrap();
    let key_path = key_dir.path().join("generated-fixture-key");
    client_key
        .write_openssh_file(&key_path, Default::default())
        .unwrap();
    let config = Arc::new(server::Config {
        keys: vec![PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()],
        inactivity_timeout: Some(Duration::from_secs(15)),
        auth_rejection_time: Duration::ZERO,
        ..Default::default()
    });
    let handler = FixtureHandler {
        authorized_key: client_key.public_key().clone(),
        uploads: HashMap::new(),
    };
    let (port, server_task) = runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let config = config.clone();
                let handler = handler.clone();
                tokio::spawn(async move {
                    let session = server::run_stream(config, stream, handler).await.unwrap();
                    let _ = session.await;
                });
            }
        });
        (port, task)
    });
    let mut target = RusshTarget::new(key_path, "fixture", "127.0.0.1");
    target.port = port;
    let runner = RusshRunner::new(target.clone()).with_command_timeout_seconds(10);
    assert_eq!(runner.run("fixture:status").unwrap(), "fixture-ready");
    // Reuse the cached session, including after a real remote nonzero exit.
    let failure = runner.run("fixture:fail").unwrap_err();
    assert_eq!(failure.code, Some(7));
    assert!(failure.stderr.contains("expected fixture failure"));
    let payload = "synthetic-upload-data:".repeat(4096);
    assert_eq!(
        runner
            .run_with_stdin("fixture:upload", payload.as_bytes())
            .unwrap(),
        payload
    );
    assert_eq!(runner.run("fixture:status").unwrap(), "fixture-ready");
    runner.close();

    let forwarder = LocalForwarder::start(&target, 0, "127.0.0.1", 9).unwrap();
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", forwarder.local_port())).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(b"fixture-tunnel").unwrap();
    let mut received = [0; 14];
    stream.read_exact(&mut received).unwrap();
    assert_eq!(&received, b"fixture-tunnel");
    drop(stream);
    forwarder.stop();
    server_task.abort();
}

#[test]
fn ed25519_key_supports_exec_upload_and_tunnel() {
    exercise_connection(Algorithm::Ed25519);
}

#[test]
fn rsa_key_supports_exec_upload_and_tunnel() {
    exercise_connection(Algorithm::Rsa { hash: None });
}

#[test]
fn ecdsa_key_supports_exec_upload_and_tunnel() {
    exercise_connection(Algorithm::Ecdsa {
        curve: EcdsaCurve::NistP256,
    });
}

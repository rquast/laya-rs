//! Server lifecycle (JEV-006): [`start_server`] / [`ServerHandle`].
//!
//! The proven axum server-handle pattern (docs/jev-server/architecture.md):
//! build the [`ServerState`] over the given [`Answerer`] seam, build the
//! router (JEV-004), bind the TCP listener (port 0 = an ephemeral port), and
//! run `axum::serve` with `with_graceful_shutdown` on a oneshot.
//! [`ServerHandle::stop`] triggers the shutdown: the listener stops accepting
//! new connections, in-flight forwards complete (they run to completion under
//! the model lock — an in-flight forward "cannot be interrupted", reference
//! behavior), and the serve task finishes.
//!
//! Bind failure is an `anyhow` error returned before any HTTP is served; the
//! startup banner is printed by the CLI (`main.rs`), not here.
//!
//! (The `server::server` module-inception naming is deliberate: it matches
//! the JEV-002 architecture's `server/server.rs` layout and the JEV-006
//! architecture note's file path.)

use std::sync::Arc;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::server::router::build_router;
use crate::server::state::{Answerer, ServerConfig, ServerState};

/// A live server (JEV-006): the actually-bound port, the model identity, and
/// the shutdown seam (oneshot sender + the serve task).
pub struct ServerHandle {
    pub port: u16,
    pub model_name: String,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl ServerHandle {
    /// Trigger a graceful shutdown: stop accepting new connections, let any
    /// in-flight forward complete, and await the serve task.
    pub async fn stop(self) {
        // If the serve task already exited (e.g. a serve error), the oneshot
        // receiver is gone — ignore the send error and just await the task.
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}

/// Start the Jev-protocol server on `host:port` (port 0 = an ephemeral port).
///
/// The checkpoint (the answerer's backing model) is already loaded by the
/// caller (the CLI loads it first, with stderr progress); this function builds
/// the state, binds, and serves. Returns the [`ServerHandle`] (with the
/// actual bound port); a bind failure is an `anyhow` error before any HTTP is
/// served.
pub async fn start_server(
    host: &str,
    port: u16,
    answerer: Arc<dyn Answerer>,
    model_name: impl Into<String>,
    config: ServerConfig,
) -> anyhow::Result<ServerHandle> {
    let model_name = model_name.into();
    let state = ServerState::new(answerer, model_name.clone(), config);
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind {host}:{port}: {e}"))?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| anyhow::anyhow!("listener has no local address: {e}"))?
        .port();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok(ServerHandle { port: actual_port, model_name, shutdown: shutdown_tx, task })
}

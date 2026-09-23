# AST Research: JEV-006 `laya serve` CLI + server lifecycle

Research performed with AstGrep + targeted reads before specifying the
`Command::Serve` + `start_server`/`ServerHandle` implementation.

## 1. Reference pattern: `start_server`/`ServerHandle` lifecycle (mirrored)

The server-handle pattern adopted for JEV-006 (axum + tokio reference
shape):

```rust
pub struct ServerHandle {
    pub port: u16,                       // actual bound port (0 → ephemeral)
    pub model_name: String,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}
impl ServerHandle {
    pub async fn stop(self) {
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}
pub async fn start_server(host: String, model_name: String,
                          agent: Arc<RLAgent>, config: ServerConfig)
                          -> anyhow::Result<ServerHandle> {
    let app = build_router(ServerState::new(Arc::new(RealAnswerer::new(agent)), model_name.clone(), config));
    let listener = TcpListener::bind((host.as_str(), config.port)).await?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = rx.await;
        });
        if let Err(err) = server.await {
            tracing::error!("laya server error: {err}");
        }
    });
    Ok(ServerHandle { port, model_name, shutdown: tx, task })
}
```

JEV-006's `start_server` mirrors this 1:1, except (a) it takes an explicit
`host` (default 127.0.0.1, user-configurable) and an already-loaded
`Arc<RLAgent>` (loading happens in the CLI, with stderr progress, so a
missing checkpoint fails *before* the bind), and (b) `ServerHandle` also
carries `model_name` for the banner/tests. `with_graceful_shutdown(rx.await)`
gives the ctrl-C → stop() behavior: in-flight forwards (spawn_blocking)
complete before the task ends — the reference's "in-flight forward cannot be
interrupted" rule.

## 2. Existing laya-rs seams (what JEV-006 wires together)

- `src/model_path.rs:68` — `pub fn resolve(default_variant_key: &str,
  explicit: Option<PathBuf>, requested_variant: Option<&str>,
  family_root: Option<&Path>) -> Result<PathBuf>`: explicit `--model`
  bypass → variant under family root → download. Identical call shape as
  `Ask`/`Answer` in `src/main.rs:100,125`.
- `src/agent.rs:62` — `RLAgent::load(model_dir) -> anyhow::Result<Self>`;
  `agent.rs:99` — `checkpoint_limits() -> (max_len, head_max_len)` (the
  clamp target; already consumed by `ServerState::effective_cap`,
  src/server/state.rs:306).
- `src/server/state.rs:220` — `RealAnswerer::new(agent: Arc<RLAgent>)` (the
  production Answerer; `ServerState::new(Arc<dyn Answerer>, model_name,
  ServerConfig)` takes it).
- `src/server/router.rs:33` — `build_router(state: ServerState) -> Router`
  (JEV-004).
- `src/main.rs` — clap derive `Args`/`Command`; `models_root` is a
  `global = true` arg; subcommands run `if let Some(Command::X) = ...`
  blocks; `Ask`/`Answer` use `#[arg(long, env = "LAYA_MODEL")] model:
  Option<PathBuf>` + variant-key defaults.

Integration decisions:
- `ServerHandle { port, model_name, shutdown, task }` +
  `async fn stop(self)`, `pub` fields for `port` and `model_name`.
- `start_server(host: String, model_name: String, agent: Arc<RLAgent>,
  config: ServerConfig) -> anyhow::Result<ServerHandle>`: build
  `ServerState::new(Arc::new(RealAnswerer::new(agent)), model_name,
  config)` → `build_router` → `TcpListener::bind((host, port))` →
  `local_addr()?.port()` → oneshot + `tokio::spawn(axum::serve(...).
  with_graceful_shutdown(rx.await))`. Bind error → `anyhow::Error` with the
  OS reason (propagates as non-zero exit in main).
- CLI `Command::Serve`: clap value validators for `--max-queued` /
  `--max-request-branches` / `--max-model-len` (`> 0`, non-zero port);
  model name = `--model` display if given, else the variant key
  (reference rule: "the model ID or local path used to start the server").
- `main.rs` serve block: `model_path::resolve` (same precedence) →
  eprintln progress → `RLAgent::load` → `start_server` → banner
  `laya serving '<model>' on http://<host>:<port>` →
  `tokio::signal::ctrl_c().await` → `handle.stop()`.
- New dev-dep needs: tokio `macros` + `net` + `rt-multi-thread`? No —
  the CLI uses `#[tokio::main]` (multi-thread) — tokio `full`? Minimal:
  the bin already links tokio (rt, sync) natively; add `macros` + `net` +
  `signal` to the native deps (the bin's runtime). `#[tokio::main]` needs
  `macros` + `rt-multi-thread`.

## 3. Test strategy (weight-free where possible)

`tests/serve_cli.rs`, binary at `env!("CARGO_BIN_EXE_laya")` (cli_ask.rs
pattern):
- Parse-time rejections (weight-free): `serve --max-queued 0`,
  `--max-request-branches -1`, `--port 0` → non-zero exit, clap usage
  error, no checkpoint load (stderr must NOT contain load progress).
- Load failure before HTTP (weight-free): `serve --model /nonexistent` →
  non-zero exit with readable error, before any bind (a missing checkpoint
  dir is the cleanest weight-free "cannot load" fixture).
- Clamp (weight-free, lib-level): `ServerState::effective_cap` with a mock
  answerer `checkpoint_limits() = (1024, 128)` +
  `ServerConfig { max_model_len: Some(4096), .. }` → 1024 (JEV-005 code
  path; JEV-006 owns the flag → `ServerConfig` wiring, proven by (a)
  parse tests + a lib-level assertion).
- Model-gated (LAYA_TEST_MODEL, like cli_ask.rs): full lifecycle —
  `start_server` on an ephemeral port, GET /health via a blocking client
  (std TcpStream or ureq) → `{"status":"ready","model":<supplied>}`, a
  model-mismatch 422, then `handle.stop()` and assert the task joined.

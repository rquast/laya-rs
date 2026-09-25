@done
@jev-server
@decision-engine
@JEV-006
Feature: `rlcd serve` CLI subcommand: checkpoint load, server lifecycle, graceful shutdown
  """
  Command::Serve in src/main.rs + start_server/ServerHandle in src/server/ (the start_server/ServerHandle lifecycle pattern from the JEV-002 architecture). CLI: rlcd serve [--model DIR | --model-variant KEY] [--models-root DIR] [--host 127.0.0.1] [--port 8000] [--max-model-len N (default: checkpoint max_len)] [--max-request-branches 100] [--max-queued 16]. Checkpoint resolution reuses model_path::resolve with identical precedence to ask/answer. Model name (health / request.model check / response echo) = the user-supplied value. max_model_len clamped to checkpoint native max_len. Lifecycle: load agent first (stderr progress), TcpListener::bind, axum::serve with with_graceful_shutdown on a oneshot; ctrl-c/SIGTERM -> handle.stop(); in-flight forwards complete before exit. ServerHandle { port, model_name, shutdown: oneshot::Sender, task: JoinHandle } with async stop(). Bind/load failures exit non-zero before HTTP is served.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. rlcd serve accepts --model DIR | --model-variant KEY [--models-root DIR] (identical precedence to ask/answer via model_path::resolve) and --host (default 127.0.0.1) / --port (default 8000) / --max-model-len (default: checkpoint max_len) / --max-request-branches (100) / --max-queued (16); --max-model-len is clamped to the checkpoint's native max_len
  #   2. The model name reported in /health, checked against request.model, and echoed in responses is the value the user supplied at startup (--model path or variant key) — 'the model ID or local path used to start the server' (reference rule); one checkpoint per process, no hot-swap
  #   3. Lifecycle (proven axum server-handle pattern): load the checkpoint first (readable stderr progress), then TcpListener::bind, then axum::serve with with_graceful_shutdown on a oneshot; ctrl-c/SIGTERM triggers handle.stop() and in-flight forwards complete before the process exits; bind/load failures exit non-zero before any HTTP is served
  #
  # EXAMPLES:
  #   1. Running `rlcd serve --port 8000` while another process owns port 8000 fails with a readable bind error before serving anything; running with --max-model-len 4096 against a checkpoint whose native max_len is 1024 silently clamps the effective cap to 1024
  #   2. Running `rlcd serve --model /path/to/laya-typed-decisions --port 8000` loads the checkpoint, prints `rlcd serving '/path/to/laya-typed-decisions' on http://127.0.0.1:8000`, serves the endpoints, and on Ctrl-C completes any in-flight forward before exiting 0
  #   3. Server started with `--model-variant typed-decisions` (no --model): /health returns {"status":"ready","model":"typed-decisions"}; POST /v1/classifier with model field "other-model" is rejected 422 with the message naming 'typed-decisions' as the loaded model
  #   4. `rlcd serve --max-queued 0` and `rlcd serve --max-request-branches -1` each exit non-zero immediately with a clap usage error naming the flag and NO checkpoint-load progress output — nothing is loaded and no HTTP is ever served
  #
  # ========================================
  Background: User Story
    As a ops engineer
    I want to launch the server with one rlcd serve command and checkpoint flags matching the existing subcommands
    So that deployment is a single static binary with a predictable lifecycle (load, serve, graceful stop)

  Scenario: Serve loads the checkpoint and serves on the requested port
    Given a checkpoint at /path/to/laya-typed-decisions
    When the user runs `rlcd serve --model /path/to/laya-typed-decisions --port 8000`
    Then the checkpoint loads with stderr progress, the server prints `rlcd serving '/path/to/laya-typed-decisions' on http://127.0.0.1:8000`, and /health reports that model name

  Scenario: Port conflicts and load failures fail cleanly before serving
    Given another process already owns port 8000
    When the user runs `rlcd serve --port 8000`
    Then the command fails with a readable bind error, no HTTP is served, and the process exits non-zero
    And a checkpoint that cannot be loaded (missing files) fails the same way before any HTTP is served

  Scenario: The effective sequence cap clamps to the checkpoint's native max_len
    Given a checkpoint whose native max_len is 1024
    When the user runs `rlcd serve --max-model-len 4096`
    Then the effective cap used for pre-inference admission is 1024 (the smaller of the flag and the checkpoint's native limit)

  Scenario: The supplied model value is the reported model identity
    Given the server started with `--model-variant typed-decisions` (or an explicit --model path)
    When /health is polled and a request with a different model field is POSTed
    Then /health reports the supplied value and the mismatched request is rejected with 422 naming the loaded model

  Scenario: Ctrl-C stops the server gracefully
    Given the server is running with a long in-flight forward
    When the user presses Ctrl-C (or sends SIGTERM)
    Then the in-flight forward completes, the listener stops accepting new requests, and the process exits 0

  Scenario: Invalid serve flags are rejected at parse time
    Given the flags --max-queued 0 or --max-request-branches -1
    When the user runs `rlcd serve`
    Then clap rejects the flags with a usage error before any checkpoint is loaded

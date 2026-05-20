# Contributing

This document covers how to build, test, and extend ivara.

## Prerequisites

- Rust stable toolchain (install via [rustup](https://rustup.rs/))
- No external dependencies required -- SQLite is bundled via `rusqlite` with the `bundled` feature

## Building

```bash
cargo build              # debug build
cargo build --release    # optimized build
```

## Testing

### Running Tests

```bash
cargo test --all-targets    # all unit + integration tests
cargo test -- <test_name>   # run a specific test
```

### Test Infrastructure

Integration tests live in two files. `tests/integration.rs` covers the telemetry commands and uses a `TestEnv` helper struct that isolates each test with its own data directory. `tests/hooks_install.rs` covers `install-hooks` / `uninstall-hooks` / `hooks status` with its own harness that points `IVARA_HOOKS_HOME` at a temp dir, so a real `~/.claude` is never touched.

The `TestEnv` struct below is the harness for `tests/integration.rs`.

```rust
struct TestEnv {
    dir: PathBuf,    // temp dir unique to this test
}
```

Key `TestEnv` methods:

| Method | Description |
|--------|-------------|
| `new(name)` | Creates a temp dir at `$TMPDIR/ivara-test-{name}-{pid}`. |
| `run(args)` | Runs `ivara` with the given args, setting `IVARA_HOME` to the temp dir. |
| `capture(json)` | Runs `ivara capture` with JSON piped to stdin. |
| `stdout(args)` | Runs `ivara` and returns stdout as a `String`. |

`TestEnv` implements `Drop` to clean up its temp directory automatically.

**Isolation mechanism**: Each test gets its own `IVARA_HOME` directory, so tests never share a database. This allows tests to run in parallel safely.

### Test Categories

| Category | Tests | What They Verify |
|----------|-------|------------------|
| Event parsing | `capture_all_25_event_types` | All 25 event types parse and store correctly. |
| Storage round-trip | `large_payload_goes_to_disk`, `small_payload_stays_inline` | Payload threshold (4096 bytes) routes correctly. |
| Query filtering | `query_by_event_type`, `query_by_tool`, `query_has_error` | Filter flags produce correct results. |
| Session prefix | `session_prefix_matching` | Prefix-based session ID lookup works. |
| Stats/summary | `stats_global_and_per_session`, `summary_output` | Aggregation and narrative output are correct. |
| Export | `export_session` | Full session export inlines payloads. |
| Active sessions | `active_command_*` (8 tests) | Live-session detection, in-flight tool, idle time, JSON shape, and `--limit`. |
| Stream | `stream_replays_and_tails_to_session_end`, `stream_unknown_session_exits_error` | JSONL replay-then-tail to `SessionEnd`, and unknown-session error exit. |
| Prune | `prune_deletes_old_data` | Data deletion works including orphan cleanup. |
| Concurrency | `concurrent_capture` | 10 parallel captures don't conflict (WAL mode). |
| Error handling | `capture_rejects_invalid_json`, `capture_rejects_unknown_event` | Invalid input exits with non-zero status. |
| Hook install | `install_*`, `hooks_status_*` (6 tests in `tests/hooks_install.rs`) | Wrapper + all 25 entries written, idempotency, roundtrip preserves other tools' hooks, `settings.json.bak` backup, status reporting. |

### Writing New Tests

Follow the existing pattern:

```rust
#[test]
fn my_test() {
    let env = TestEnv::new("my-test");

    // Capture events
    env.capture(r#"{"session_id":"s1","hook_event_name":"SessionStart","cwd":"/p"}"#);

    // Query and assert
    let out = env.stdout(&["sessions", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);
}
```

## CI Pipeline

CI runs on every push to `main` and every pull request. Configuration is in `.github/workflows/ci.yml`.

### Test Job (3-platform matrix)

Runs on: `ubuntu-latest`, `macos-latest`, `windows-latest`

| Step | Command | Purpose |
|------|---------|---------|
| Check | `cargo check --all-targets` | Verify compilation. |
| Test | `cargo test --all-targets` | Run all tests. |
| Clippy | `cargo clippy --all-targets -- -D warnings` | Lint with warnings-as-errors. |

### Format Job

Runs on: `ubuntu-latest` only.

| Step | Command | Purpose |
|------|---------|---------|
| Format check | `cargo fmt --check` | Verify code formatting. |

**Before submitting a PR**, run locally:

```bash
cargo fmt                                  # fix formatting
cargo clippy --all-targets -- -D warnings  # fix lint warnings
cargo test --all-targets                   # verify tests pass
```

## Release Process

Releases are triggered by pushing a version tag (e.g., `git tag v0.2.0 && git push --tags`). The release workflow (`.github/workflows/release.yml`) does:

1. **Build** -- compiles release binaries for 5 targets:

   | Binary Name | Target |
   |-------------|--------|
   | `ivara-linux-amd64` | `x86_64-unknown-linux-gnu` |
   | `ivara-linux-arm64` | `aarch64-unknown-linux-gnu` |
   | `ivara-darwin-amd64` | `x86_64-apple-darwin` |
   | `ivara-darwin-arm64` | `aarch64-apple-darwin` |
   | `ivara-windows-amd64.exe` | `x86_64-pc-windows-msvc` |

2. **Release** -- creates a GitHub Release with all binaries and a `checksums.txt`.

3. **Update tap** -- sends a repository dispatch to `simonspoon/homebrew-tap` to update the Homebrew formula.

## Adding a New Command

1. Add a variant to the `Commands` enum in `src/cli.rs` with clap attributes for args and help text.
2. Create or extend a command module in `src/commands/`. Follow the existing pattern: accept `&Connection`, return `Result<()>`, support `--json` where appropriate.
3. Add a match arm in the `run()` function in `src/main.rs`.
4. Write integration tests in `tests/integration.rs` using `TestEnv` (or `tests/hooks_install.rs` for hook-management commands).

## Adding a New Event Type

1. Add the variant to the `EventType` enum in `src/events.rs`.
2. Add a match arm in `EventType::from_hook_name()`.
3. Add a match arm in `EventType::as_str()`.
4. Add a match arm in the `Display` impl.
5. If the event carries new fields, add them to `HookInput` with `#[serde(default)]`.
6. If new fields should appear in metadata, add them to `HookInput::metadata()`.
7. Add a capture test case to `capture_all_25_event_types` in `tests/integration.rs`.

## Source References

- CLI definition: `src/cli.rs`
- Command dispatch: `src/main.rs` (`run()`)
- Event types: `src/events.rs`
- Integration tests: `tests/integration.rs` (telemetry commands), `tests/hooks_install.rs` (hook self-install)
- CI workflow: `.github/workflows/ci.yml`
- Release workflow: `.github/workflows/release.yml`

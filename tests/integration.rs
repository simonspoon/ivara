use std::path::PathBuf;
use std::process::Command;

struct TestEnv {
    dir: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ivara-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ivara"))
            .args(args)
            .env("IVARA_HOME", &self.dir)
            .output()
            .expect("failed to run ivara")
    }

    fn capture(&self, json: &str) -> std::process::Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ivara"))
            .args(["capture"])
            .env("IVARA_HOME", &self.dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn ivara capture");

        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(json.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn stdout(&self, args: &[&str]) -> String {
        let out = self.run(args);
        String::from_utf8(out.stdout).unwrap()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ---- Event Parsing Tests (all 25 types) ----

#[test]
fn capture_all_25_event_types() {
    let env = TestEnv::new("all-25");

    let event_types = vec![
        (
            r#"{"session_id":"s1","hook_event_name":"SessionStart","cwd":"/p","source":"cli","model":"opus"}"#,
            "SessionStart",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"SessionEnd","cwd":"/p","reason":"user_exit"}"#,
            "SessionEnd",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"CwdChanged","cwd":"/new"}"#,
            "CwdChanged",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"ConfigChange","cwd":"/p"}"#,
            "ConfigChange",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"InstructionsLoaded","cwd":"/p"}"#,
            "InstructionsLoaded",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"UserPromptSubmit","cwd":"/p","prompt":"hello"}"#,
            "UserPromptSubmit",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"Stop","cwd":"/p","stop_hook_active":true,"last_assistant_message":"done"}"#,
            "Stop",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"StopFailure","cwd":"/p","error":"timeout","error_details":"took too long"}"#,
            "StopFailure",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_input":{"cmd":"ls"},"tool_use_id":"tu1"}"#,
            "PreToolUse",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PostToolUse","cwd":"/p","tool_name":"Bash","tool_response":"files","tool_use_id":"tu1"}"#,
            "PostToolUse",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PostToolUseFailure","cwd":"/p","tool_name":"Bash","error":"fail","tool_use_id":"tu1"}"#,
            "PostToolUseFailure",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PermissionRequest","cwd":"/p","tool_name":"Write","tool_input":{},"tool_use_id":"tu2"}"#,
            "PermissionRequest",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"SubagentStart","cwd":"/p","agent_id":"a1","agent_type":"task"}"#,
            "SubagentStart",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"SubagentStop","cwd":"/p","agent_id":"a1","agent_type":"task","stop_hook_active":false}"#,
            "SubagentStop",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"TaskCreated","cwd":"/p","task_id":"t1","task_subject":"build","task_description":"build it","teammate_name":"dev","team_name":"eng"}"#,
            "TaskCreated",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"TaskCompleted","cwd":"/p","task_id":"t1","task_subject":"build","teammate_name":"dev","team_name":"eng"}"#,
            "TaskCompleted",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"TeammateIdle","cwd":"/p","teammate_name":"dev","team_name":"eng"}"#,
            "TeammateIdle",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"FileChanged","cwd":"/p","file_path":"/p/main.rs","change_type":"modified"}"#,
            "FileChanged",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"WorktreeCreate","cwd":"/p"}"#,
            "WorktreeCreate",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"WorktreeRemove","cwd":"/p"}"#,
            "WorktreeRemove",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PreCompact","cwd":"/p","compaction_trigger":"context_limit"}"#,
            "PreCompact",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"PostCompact","cwd":"/p","compaction_trigger":"context_limit"}"#,
            "PostCompact",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"Elicitation","cwd":"/p","mcp_server_name":"test","tool_use_id":"tu3","fields":[]}"#,
            "Elicitation",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"ElicitationResult","cwd":"/p","mcp_server_name":"test","tool_use_id":"tu3","result_action":"confirm","result_content":"yes"}"#,
            "ElicitationResult",
        ),
        (
            r#"{"session_id":"s1","hook_event_name":"Notification","cwd":"/p","message":"hi","title":"alert","notification_type":"info"}"#,
            "Notification",
        ),
    ];

    for (json, expected_type) in &event_types {
        let out = env.capture(json);
        assert!(
            out.status.success(),
            "Failed to capture {}: {}",
            expected_type,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Verify all 25 events are stored
    let sessions_out = env.stdout(&["sessions", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&sessions_out).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["event_count"], 25);

    // Verify each event type appears in timeline
    let timeline_out = env.stdout(&["timeline", "s1", "--json"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&timeline_out).unwrap();
    assert_eq!(events.len(), 25);

    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();
    for (_, expected_type) in &event_types {
        assert!(
            types.contains(expected_type),
            "Missing event type: {}",
            expected_type
        );
    }
}

// ---- Storage Round-Trip Tests ----

#[test]
fn large_payload_goes_to_disk() {
    let env = TestEnv::new("large-payload");

    // Create a payload larger than 4KB
    let large_input = format!(
        r#"{{"session_id":"s2","hook_event_name":"PostToolUse","cwd":"/p","tool_name":"Read","tool_response":"{}","tool_use_id":"tu_big"}}"#,
        "x".repeat(5000)
    );

    let out = env.capture(&large_input);
    assert!(out.status.success());

    // Verify payload was written to disk
    let show_out = env.stdout(&["show", "1", "--json"]);
    let event: serde_json::Value = serde_json::from_str(&show_out).unwrap();
    assert!(
        event["payload_path"].is_string(),
        "Expected file-backed payload for >4KB input"
    );
}

#[test]
fn small_payload_stays_inline() {
    let env = TestEnv::new("small-payload");

    let small_input =
        r#"{"session_id":"s3","hook_event_name":"SessionStart","cwd":"/p","source":"cli"}"#;

    let out = env.capture(small_input);
    assert!(out.status.success());

    let show_out = env.stdout(&["show", "1", "--json"]);
    let event: serde_json::Value = serde_json::from_str(&show_out).unwrap();
    assert!(
        event["payload_path"].is_null(),
        "Expected inline payload for <4KB input"
    );
    assert!(event["metadata_json"].is_string());
}

// ---- Query Filtering Tests ----

#[test]
fn query_by_event_type() {
    let env = TestEnv::new("query-type");

    env.capture(r#"{"session_id":"s4","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"s4","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"s4","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Read","tool_use_id":"tu2"}"#,
    );

    let out = env.stdout(&["query", "--event-type", "PreToolUse", "--json"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(events.len(), 2);
    for e in &events {
        assert_eq!(e["event_type"], "PreToolUse");
    }
}

#[test]
fn query_by_tool() {
    let env = TestEnv::new("query-tool");

    env.capture(
        r#"{"session_id":"s5","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"s5","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Read","tool_use_id":"tu2"}"#,
    );
    env.capture(
        r#"{"session_id":"s5","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu3"}"#,
    );

    let out = env.stdout(&["query", "--tool", "Bash", "--json"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(events.len(), 2);
    for e in &events {
        assert_eq!(e["tool_name"], "Bash");
    }
}

#[test]
fn query_has_error() {
    let env = TestEnv::new("query-error");

    env.capture(r#"{"session_id":"s6","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"s6","hook_event_name":"StopFailure","cwd":"/p","error":"timeout"}"#,
    );
    env.capture(
        r#"{"session_id":"s6","hook_event_name":"PostToolUseFailure","cwd":"/p","tool_name":"Bash","error":"fail","tool_use_id":"tu1"}"#,
    );

    let out = env.stdout(&["query", "--has-error", "--json"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(events.len(), 2);
}

// ---- Session Prefix Matching ----

#[test]
fn session_prefix_matching() {
    let env = TestEnv::new("prefix-match");

    env.capture(
        r#"{"session_id":"abc-long-session-id-12345","hook_event_name":"SessionStart","cwd":"/p"}"#,
    );

    // Should match by prefix
    let out = env.stdout(&["timeline", "abc", "--json"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(events.len(), 1);
}

// ---- Stats and Summary ----

#[test]
fn stats_global_and_per_session() {
    let env = TestEnv::new("stats");

    env.capture(
        r#"{"session_id":"s7","hook_event_name":"SessionStart","cwd":"/p","model":"opus"}"#,
    );
    env.capture(
        r#"{"session_id":"s7","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"s7","hook_event_name":"StopFailure","cwd":"/p","error":"timeout"}"#,
    );

    // Global stats
    let out = env.stdout(&["stats", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(stats["total_events"], 3);
    assert_eq!(stats["error_events"], 1);

    // Per-session stats
    let out = env.stdout(&["stats", "s7", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(stats["total_events"], 3);
    assert_eq!(stats["scope"], "s7");
}

#[test]
fn summary_output() {
    let env = TestEnv::new("summary");

    env.capture(
        r#"{"session_id":"s8","hook_event_name":"SessionStart","cwd":"/proj","model":"opus"}"#,
    );
    env.capture(
        r#"{"session_id":"s8","hook_event_name":"PreToolUse","cwd":"/proj","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"s8","hook_event_name":"PostToolUse","cwd":"/proj","tool_name":"Bash","tool_response":"ok","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"s8","hook_event_name":"SessionEnd","cwd":"/proj","reason":"done"}"#,
    );

    let out = env.stdout(&["summary", "s8", "--json"]);
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(summary["session_id"], "s8");
    assert_eq!(summary["model"], "opus");
    assert_eq!(summary["cwd"], "/proj");
    assert_eq!(summary["total_events"], 4);
    assert_eq!(summary["errors"], 0);
}

// ---- Export ----

#[test]
fn export_session() {
    let env = TestEnv::new("export");

    env.capture(r#"{"session_id":"s9","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"s9","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );

    let out = env.stdout(&["export", "s9"]);
    let events: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(events.len(), 2);
    // Each event should have an inlined payload
    for e in &events {
        assert!(e.get("payload").is_some());
    }
}

// ---- Prune ----

#[test]
fn prune_deletes_old_data() {
    let env = TestEnv::new("prune");

    env.capture(r#"{"session_id":"s10","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"s10","hook_event_name":"SessionEnd","cwd":"/p","reason":"done"}"#,
    );

    // Verify data exists
    let out = env.stdout(&["sessions", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);

    // Prune with --days 0 deletes everything
    let prune_out = env.run(&["prune", "--days", "0"]);
    assert!(prune_out.status.success());

    let out = env.stdout(&["sessions", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 0);
}

// ---- Active Sessions ----

#[test]
fn active_command_lists_started_but_not_ended() {
    let env = TestEnv::new("active-basic");

    // Live: SessionStart, no SessionEnd
    env.capture(
        r#"{"session_id":"live1","hook_event_name":"SessionStart","cwd":"/p","source":"cli","model":"opus"}"#,
    );

    // Dead: SessionStart + SessionEnd
    env.capture(
        r#"{"session_id":"dead1","hook_event_name":"SessionStart","cwd":"/p","model":"sonnet"}"#,
    );
    env.capture(
        r#"{"session_id":"dead1","hook_event_name":"SessionEnd","cwd":"/p","reason":"user_exit"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["session_id"], "live1");
    assert_eq!(sessions[0]["model"], "opus");
}

#[test]
fn active_command_reports_in_flight_tool() {
    let env = TestEnv::new("active-tool");

    env.capture(r#"{"session_id":"live2","hook_event_name":"SessionStart","cwd":"/p"}"#);
    // Completed tool call — pre + post matched by tool_use_id
    env.capture(
        r#"{"session_id":"live2","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Read","tool_use_id":"done"}"#,
    );
    env.capture(
        r#"{"session_id":"live2","hook_event_name":"PostToolUse","cwd":"/p","tool_name":"Read","tool_response":"ok","tool_use_id":"done"}"#,
    );
    // In-flight tool call — pre without post
    env.capture(
        r#"{"session_id":"live2","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"pending"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["tool"], "Bash");
}

#[test]
fn active_command_blank_tool_when_no_in_flight() {
    let env = TestEnv::new("active-no-tool");

    env.capture(r#"{"session_id":"live3","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"live3","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"live3","hook_event_name":"PostToolUse","cwd":"/p","tool_name":"Bash","tool_response":"ok","tool_use_id":"tu1"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(
        sessions[0]["tool"].is_null(),
        "expected null tool when nothing in flight, got {:?}",
        sessions[0]["tool"]
    );
}

#[test]
fn active_command_post_tool_use_failure_counts_as_finalizer() {
    let env = TestEnv::new("active-tool-fail");

    env.capture(r#"{"session_id":"live4","hook_event_name":"SessionStart","cwd":"/p"}"#);
    env.capture(
        r#"{"session_id":"live4","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
    );
    env.capture(
        r#"{"session_id":"live4","hook_event_name":"PostToolUseFailure","cwd":"/p","tool_name":"Bash","error":"x","tool_use_id":"tu1"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert!(sessions[0]["tool"].is_null());
}

#[test]
fn active_command_empty_prints_one_line() {
    let env = TestEnv::new("active-empty");

    let out = env.run(&["active"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("No active sessions"),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn active_command_json_shape() {
    let env = TestEnv::new("active-shape");

    env.capture(
        r#"{"session_id":"shape1","hook_event_name":"SessionStart","cwd":"/proj","model":"sonnet"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    for key in &[
        "session_id",
        "last_seen",
        "duration",
        "event_count",
        "cwd",
        "idle",
        "tool",
        "model",
    ] {
        assert!(s.get(key).is_some(), "missing key: {}", key);
    }
    assert!(s["idle"].is_string());
    assert_eq!(s["cwd"], "/proj");
    assert_eq!(s["model"], "sonnet");
}

#[test]
fn active_command_respects_limit() {
    let env = TestEnv::new("active-limit");

    for i in 0..5 {
        let json = format!(
            r#"{{"session_id":"limit{}","hook_event_name":"SessionStart","cwd":"/p"}}"#,
            i
        );
        env.capture(&json);
    }

    let out = env.stdout(&["active", "--limit", "3", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 3);
}

#[test]
fn active_command_ignores_sessions_with_no_session_start() {
    // Only captures that lack a SessionStart event should not appear.
    let env = TestEnv::new("active-no-start");

    // Pre-hook captured event — session exists in sessions table but no SessionStart row.
    env.capture(
        r#"{"session_id":"orphan","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"t1"}"#,
    );

    let out = env.stdout(&["active", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 0);
}

// ---- Stream ----

#[test]
fn stream_replays_and_tails_to_session_end() {
    use std::io::{BufRead, BufReader};
    use std::time::Duration;

    let env = TestEnv::new("stream-basic");

    // Seed two events before starting stream — they should appear in the replay phase.
    env.capture(
        r#"{"session_id":"stream1","hook_event_name":"SessionStart","cwd":"/p","source":"cli","model":"opus"}"#,
    );
    env.capture(
        r#"{"session_id":"stream1","hook_event_name":"UserPromptSubmit","cwd":"/p","prompt":"hi"}"#,
    );

    // Spawn `ivara stream` with stdout piped.
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivara"))
        .args(["stream", "stream1"])
        .env("IVARA_HOME", &env.dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ivara stream");

    let stdout = child.stdout.take().expect("stdout pipe");
    let reader = BufReader::new(stdout);

    // Background thread writes a third event mid-stream, then SessionEnd to terminate.
    let dir = env.dir.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));

        let mut c = Command::new(env!("CARGO_BIN_EXE_ivara"))
            .args(["capture"])
            .env("IVARA_HOME", &dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn mid capture");
        use std::io::Write;
        c.stdin
            .take()
            .unwrap()
            .write_all(
                br#"{"session_id":"stream1","hook_event_name":"PreToolUse","cwd":"/p","tool_name":"Bash","tool_use_id":"tu1"}"#,
            )
            .unwrap();
        c.wait_with_output().unwrap();

        std::thread::sleep(Duration::from_millis(500));

        let mut c = Command::new(env!("CARGO_BIN_EXE_ivara"))
            .args(["capture"])
            .env("IVARA_HOME", &dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn end capture");
        c.stdin
            .take()
            .unwrap()
            .write_all(
                br#"{"session_id":"stream1","hook_event_name":"SessionEnd","cwd":"/p","reason":"user_exit"}"#,
            )
            .unwrap();
        c.wait_with_output().unwrap();
    });

    // Read every line from the stream until the process exits. Each line = one event JSON.
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        let l = line.expect("read stream line");
        lines.push(l);
    }

    writer.join().expect("writer thread panicked");
    let status = child.wait().expect("wait on stream process");

    assert!(status.success(), "stream exited non-zero: {status}");
    assert_eq!(lines.len(), 4, "expected 4 JSONL lines, got: {lines:?}");

    // Each line parses as JSON and carries the required fields.
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("line is JSON"))
        .collect();

    let types: Vec<&str> = parsed
        .iter()
        .map(|v| v["event_type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        vec![
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "SessionEnd",
        ]
    );

    // Payload must be inlined on every line — same shape as `ivara export`.
    for v in &parsed {
        assert!(
            v.get("payload").is_some(),
            "missing payload on stream line: {v}"
        );
        assert!(v.get("session_id").is_some());
        assert_eq!(v["session_id"], "stream1");
    }
}

#[test]
fn stream_unknown_session_exits_error() {
    let env = TestEnv::new("stream-unknown");

    let out = env.run(&["stream", "nope"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit for unknown session"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("No session matching"),
        "stderr was: {stderr}"
    );
}

// ---- Concurrent Capture ----

#[test]
fn concurrent_capture() {
    let env = TestEnv::new("concurrent");

    let threads: Vec<_> = (0..10)
        .map(|i| {
            let dir = env.dir.clone();
            std::thread::spawn(move || {
                let json = format!(
                    r#"{{"session_id":"concurrent-{}","hook_event_name":"SessionStart","cwd":"/p"}}"#,
                    i
                );
                let mut child = Command::new(env!("CARGO_BIN_EXE_ivara"))
                    .args(["capture"])
                    .env("IVARA_HOME", &dir)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("spawn failed");

                use std::io::Write;
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(json.as_bytes())
                    .unwrap();
                let out = child.wait_with_output().unwrap();
                assert!(
                    out.status.success(),
                    "Concurrent capture {} failed: {}",
                    i,
                    String::from_utf8_lossy(&out.stderr)
                );
            })
        })
        .collect();

    for t in threads {
        t.join().expect("thread panicked");
    }

    // All 10 events should be stored
    let out = env.stdout(&["sessions", "--json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(sessions.len(), 10);
}

// ---- Invalid Input ----

#[test]
fn capture_rejects_invalid_json() {
    let env = TestEnv::new("invalid-json");

    let out = env.capture("not json");
    assert!(!out.status.success());
}

#[test]
fn capture_rejects_unknown_event() {
    let env = TestEnv::new("unknown-event");

    let out = env.capture(r#"{"session_id":"s","hook_event_name":"FakeEvent","cwd":"/"}"#);
    assert!(!out.status.success());
}

// ---- Token Usage ----

/// Write a transcript JSONL fixture. Two real assistant turns plus a synthetic
/// entry and a duplicate uuid (both must be ignored) and a malformed line.
/// Expected aggregate: input 110, output 130, cache-write 200, cache-read 3000,
/// web_search 2, api_calls 2, model "claude-sonnet-4-6".
fn write_transcript_fixture(env: &TestEnv, name: &str) -> PathBuf {
    let lines = [
        r#"{"type":"user","uuid":"u1"}"#,
        r#"{"type":"assistant","uuid":"a1","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":200,"cache_read_input_tokens":1000}}}"#,
        r#"{"type":"assistant","uuid":"a2","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":10,"output_tokens":80,"cache_creation_input_tokens":0,"cache_read_input_tokens":2000,"server_tool_use":{"web_search_requests":2,"web_fetch_requests":0}}}}"#,
        r#"{"type":"assistant","uuid":"syn1","message":{"model":"<synthetic>","usage":{"input_tokens":9999,"output_tokens":9999,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
        r#"{"type":"assistant","uuid":"a1","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":200,"cache_read_input_tokens":1000}}}"#,
        "not valid json",
    ];
    let path = env.dir.join(format!("transcript-{name}.jsonl"));
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}

#[test]
fn session_end_auto_captures_token_usage() {
    let env = TestEnv::new("usage-sessionend");
    let transcript = write_transcript_fixture(&env, "ue1");

    env.capture(
        r#"{"session_id":"ue1","hook_event_name":"SessionStart","cwd":"/p","model":"opus"}"#,
    );

    let session_end = serde_json::json!({
        "session_id": "ue1",
        "hook_event_name": "SessionEnd",
        "cwd": "/p",
        "reason": "done",
        "transcript_path": transcript.to_str().unwrap(),
    })
    .to_string();
    let out = env.capture(&session_end);
    assert!(out.status.success());

    let stats_out = env.stdout(&["stats", "ue1", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&stats_out).unwrap();
    let u = &stats["token_usage"];
    assert_eq!(u["input_tokens"], 110);
    assert_eq!(u["output_tokens"], 130);
    assert_eq!(u["cache_creation_tokens"], 200);
    assert_eq!(u["cache_read_tokens"], 3000);
    assert_eq!(u["web_search_requests"], 2);
    assert_eq!(u["api_calls"], 2);
    assert_eq!(u["model"], "claude-sonnet-4-6");
}

#[test]
fn backfill_usage_populates_from_transcript() {
    let env = TestEnv::new("usage-backfill");
    let transcript = write_transcript_fixture(&env, "bf1");

    // Event carries transcript_path but no SessionEnd fires — usage stays unrecorded.
    let start = serde_json::json!({
        "session_id": "bf1",
        "hook_event_name": "SessionStart",
        "cwd": "/p",
        "transcript_path": transcript.to_str().unwrap(),
    })
    .to_string();
    env.capture(&start);

    let stats_out = env.stdout(&["stats", "bf1", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&stats_out).unwrap();
    assert!(
        stats.get("token_usage").is_none(),
        "token_usage should be absent before backfill"
    );

    let out = env.run(&["backfill-usage"]);
    assert!(
        out.status.success(),
        "backfill failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stats_out = env.stdout(&["stats", "bf1", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&stats_out).unwrap();
    assert_eq!(stats["token_usage"]["output_tokens"], 130);
    assert_eq!(stats["token_usage"]["api_calls"], 2);
}

#[test]
fn summary_json_includes_token_usage() {
    let env = TestEnv::new("usage-summary");
    let transcript = write_transcript_fixture(&env, "sm1");

    env.capture(
        r#"{"session_id":"sm1","hook_event_name":"SessionStart","cwd":"/p","model":"opus"}"#,
    );
    let session_end = serde_json::json!({
        "session_id": "sm1",
        "hook_event_name": "SessionEnd",
        "cwd": "/p",
        "reason": "done",
        "transcript_path": transcript.to_str().unwrap(),
    })
    .to_string();
    env.capture(&session_end);

    let out = env.stdout(&["summary", "sm1", "--json"]);
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(summary["token_usage"]["input_tokens"], 110);
    assert_eq!(summary["token_usage"]["output_tokens"], 130);
}

#[test]
fn session_end_with_missing_transcript_still_succeeds() {
    let env = TestEnv::new("usage-missing");

    // Usage capture is best-effort — an unreachable transcript must not fail capture.
    let session_end = serde_json::json!({
        "session_id": "mt1",
        "hook_event_name": "SessionEnd",
        "cwd": "/p",
        "reason": "done",
        "transcript_path": "/nonexistent/ivara/transcript.jsonl",
    })
    .to_string();
    let out = env.capture(&session_end);
    assert!(
        out.status.success(),
        "capture must succeed despite a missing transcript"
    );

    let stats_out = env.stdout(&["stats", "mt1", "--json"]);
    let stats: serde_json::Value = serde_json::from_str(&stats_out).unwrap();
    assert!(stats.get("token_usage").is_none());
}

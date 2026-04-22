//! End-to-end test for install-hooks / uninstall-hooks / hooks status.
//!
//! Drives the ivara binary with `IVARA_HOOKS_HOME` pointed at a tmp dir so
//! nothing in `~/.claude` is touched.

use std::path::PathBuf;
use std::process::Command;

struct HookEnv {
    dir: PathBuf,
}

impl HookEnv {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ivara-hooks-test-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // IVARA_HOME isolates the telemetry DB; IVARA_HOOKS_HOME isolates ~/.claude.
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        Self { dir }
    }

    fn settings_path(&self) -> PathBuf {
        self.dir.join(".claude").join("settings.json")
    }

    fn script_path(&self) -> PathBuf {
        self.dir
            .join(".claude")
            .join("hook-scripts")
            .join("ivara-capture.sh")
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ivara"))
            .args(args)
            .env("IVARA_HOOKS_HOME", &self.dir)
            .env("IVARA_HOME", &self.dir)
            .output()
            .expect("failed to run ivara")
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "ivara {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn read_settings(&self) -> serde_json::Value {
        let text = std::fs::read_to_string(self.settings_path()).unwrap();
        serde_json::from_str(&text).unwrap()
    }
}

impl Drop for HookEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn install_creates_wrapper_and_all_25_entries() {
    let env = HookEnv::new("install-basic");

    env.run_ok(&["install-hooks", "--scope", "user"]);

    // Wrapper written, executable.
    let script = env.script_path();
    assert!(script.exists(), "wrapper missing at {}", script.display());
    let body = std::fs::read_to_string(&script).unwrap();
    assert!(body.contains("command -v ivara"), "wrapper content drift");

    // Settings has 25 canonical events.
    let settings = env.read_settings();
    let hooks = settings["hooks"].as_object().unwrap();
    assert_eq!(hooks.len(), 25, "expected 25 canonical events");
    for event in hooks.keys() {
        let groups = hooks[event].as_array().unwrap();
        let has = groups.iter().any(|g| {
            g["hooks"]
                .as_array()
                .map(|hs| {
                    hs.iter().any(|h| {
                        h["command"]
                            .as_str()
                            .map(|c| c.contains("ivara-capture.sh"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        assert!(has, "event {event} missing ivara hook");
    }
}

#[test]
fn install_is_idempotent() {
    let env = HookEnv::new("idempotent");

    env.run_ok(&["install-hooks", "--scope", "user"]);
    let first = std::fs::read_to_string(env.settings_path()).unwrap();

    env.run_ok(&["install-hooks", "--scope", "user"]);
    let second = std::fs::read_to_string(env.settings_path()).unwrap();

    assert_eq!(first, second, "second install produced a diff");
}

#[test]
fn install_uninstall_roundtrip_preserves_other_tools() {
    let env = HookEnv::new("roundtrip");

    // Seed settings.json with another tool's entry.
    let seed = serde_json::json!({
        "hooks": {
            "Stop": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": "bash /other/tool.sh",
                    "timeout": 10
                }]
            }],
            "PreToolUse": [{
                "matcher": "AskUserQuestion",
                "hooks": [{
                    "type": "command",
                    "command": "bash /other/matcher.sh",
                    "timeout": 5
                }]
            }]
        },
        "otherKey": "preserved"
    });
    std::fs::write(
        env.settings_path(),
        serde_json::to_string_pretty(&seed).unwrap(),
    )
    .unwrap();

    env.run_ok(&["install-hooks", "--scope", "user"]);
    env.run_ok(&["uninstall-hooks", "--scope", "user"]);

    let after = env.read_settings();
    assert_eq!(after, seed, "roundtrip must restore original settings");
}

#[test]
fn uninstall_on_missing_settings_is_graceful() {
    let env = HookEnv::new("missing-settings");
    // No settings.json exists.
    let out = env.run_ok(&["uninstall-hooks", "--scope", "user"]);
    assert!(out.contains("nothing to remove"));
}

#[test]
fn hooks_status_reports_wired_vs_missing() {
    let env = HookEnv::new("status");

    // Before install: nothing wired.
    let out = env.run_ok(&["hooks", "status", "--scope", "user"]);
    assert!(out.contains("wired: 0/25"), "unexpected: {out}");

    env.run_ok(&["install-hooks", "--scope", "user"]);

    let out = env.run_ok(&["hooks", "status", "--scope", "user"]);
    assert!(out.contains("wired: 25/25"), "unexpected: {out}");
}

#[test]
fn install_creates_backup_file() {
    let env = HookEnv::new("backup");

    let seed = serde_json::json!({"hooks": {}});
    std::fs::write(
        env.settings_path(),
        serde_json::to_string_pretty(&seed).unwrap(),
    )
    .unwrap();

    env.run_ok(&["install-hooks", "--scope", "user"]);

    let bak = env.dir.join(".claude").join("settings.json.bak");
    assert!(bak.exists(), "backup file missing");
}

//! Self-install Claude Code hooks.
//!
//! Subcommands wire ivara's capture wrapper into Claude Code's hook settings
//! without requiring users to hand-edit `settings.json`.
//!
//! * `install-hooks`   — write wrapper, merge canonical events into settings.
//! * `uninstall-hooks` — strip ivara entries from settings.
//! * `hooks status`    — report wired vs missing per event.
//!
//! Settings layout: `settings.json.hooks.<Event> = [{matcher, hooks: [{type, command, timeout}, ...]}, ...]`.
//! Install is append-only: never clobbers other tools' entries. Uninstall removes
//! only hook entries whose `command` matches ivara's wrapper path.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Embedded wrapper script — source of truth lives in `hooks/ivara-capture.sh`.
pub const HOOK_SCRIPT: &str = include_str!("../../hooks/ivara-capture.sh");

/// Filename for the wrapper when written to disk.
pub const SCRIPT_NAME: &str = "ivara-capture.sh";

/// Default timeout for each hook invocation (seconds). Matches existing entries.
pub const HOOK_TIMEOUT: u64 = 5;

/// Canonical Claude Code events ivara captures. Binary is source of truth.
pub const CANONICAL_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "CwdChanged",
    "ConfigChange",
    "InstructionsLoaded",
    "UserPromptSubmit",
    "Stop",
    "StopFailure",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "TeammateIdle",
    "FileChanged",
    "WorktreeCreate",
    "WorktreeRemove",
    "PreCompact",
    "PostCompact",
    "Elicitation",
    "ElicitationResult",
    "Notification",
];

/// Install scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// `~/.claude/settings.json` (or `$IVARA_HOOKS_HOME/.claude/settings.json`).
    User,
    /// `<cwd>/.claude/settings.json`.
    Project,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Project => "project",
        }
    }
}

// ---------- paths ----------

/// Returns the `.claude` directory root for the given scope.
///
/// For `User`, honors `IVARA_HOOKS_HOME` env var (useful for isolated testing
/// and power-user relocation); falls back to `$HOME/.claude`.
/// For `Project`, always `<cwd>/.claude`.
fn claude_dir(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::User => {
            if let Ok(dir) = std::env::var("IVARA_HOOKS_HOME") {
                return Ok(PathBuf::from(dir).join(".claude"));
            }
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow!("could not determine home directory"))?;
            Ok(home.join(".claude"))
        }
        Scope::Project => Ok(std::env::current_dir()
            .context("could not determine current directory")?
            .join(".claude")),
    }
}

fn settings_path(scope: Scope) -> Result<PathBuf> {
    Ok(claude_dir(scope)?.join("settings.json"))
}

fn script_dir(scope: Scope) -> Result<PathBuf> {
    Ok(claude_dir(scope)?.join("hook-scripts"))
}

fn script_path(scope: Scope) -> Result<PathBuf> {
    Ok(script_dir(scope)?.join(SCRIPT_NAME))
}

/// Canonical hook command string written into settings.json entries.
fn hook_command(scope: Scope) -> Result<String> {
    let path = script_path(scope)?;
    Ok(format!("bash {}", path.display()))
}

// ---------- public entry points ----------

/// Install: write wrapper script, merge canonical entries into settings.json.
pub fn install(scope: Scope) -> Result<()> {
    let script_dir = script_dir(scope)?;
    fs::create_dir_all(&script_dir)
        .with_context(|| format!("creating {}", script_dir.display()))?;

    let script = script_path(scope)?;
    write_script(&script)?;

    let cmd = hook_command(scope)?;
    let settings = settings_path(scope)?;
    mutate_settings(&settings, |value| {
        merge_install(value, &cmd, HOOK_TIMEOUT)
    })?;

    println!("Installed ivara hooks ({} scope).", scope.label());
    println!("  wrapper:  {}", script.display());
    println!("  settings: {}", settings.display());
    println!("  events:   {}", CANONICAL_EVENTS.len());
    Ok(())
}

/// Uninstall: strip ivara entries from settings.json. Wrapper script left in place
/// (it is a no-op without ivara on PATH and may be referenced by older configs).
pub fn uninstall(scope: Scope) -> Result<()> {
    let cmd = hook_command(scope)?;
    let settings = settings_path(scope)?;

    if !settings.exists() {
        println!("No settings.json at {} — nothing to remove.", settings.display());
        return Ok(());
    }

    let removed = mutate_settings(&settings, |value| {
        Ok(merge_uninstall(value, &cmd))
    })?;

    println!("Uninstalled ivara hooks ({} scope).", scope.label());
    println!("  settings: {}", settings.display());
    println!("  entries removed: {}", removed);
    Ok(())
}

/// Status: report wired/missing per canonical event.
pub fn status(scope: Scope) -> Result<()> {
    let cmd = hook_command(scope)?;
    let settings = settings_path(scope)?;

    let value = if settings.exists() {
        load_settings(&settings)?
    } else {
        Value::Object(Map::new())
    };

    let rows = enumerate_status(&value, &cmd);
    let wired = rows.iter().filter(|(_, p)| *p).count();
    let total = rows.len();

    println!("ivara hooks status ({} scope)", scope.label());
    println!("  settings: {}", settings.display());
    println!("  wired: {}/{}", wired, total);
    println!();
    for (event, present) in &rows {
        let mark = if *present { "ok" } else { "--" };
        println!("  [{mark}] {event}");
    }

    // Flag unknown events present in settings but not in canonical list.
    let unknown = unknown_events(&value, &cmd);
    if !unknown.is_empty() {
        println!();
        println!("  unknown events with ivara entry (canonical list may be stale):");
        for e in &unknown {
            println!("    - {e}");
        }
    }
    Ok(())
}

// ---------- settings load / write ----------

fn load_settings(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parsing {} as JSON", path.display()))?;
    if !value.is_object() {
        bail!("settings.json root must be a JSON object");
    }
    Ok(value)
}

/// Load → backup → mutate → atomic write. Returns the mutator's return value.
fn mutate_settings<F, R>(path: &Path, f: F) -> Result<R>
where
    F: FnOnce(&mut Value) -> Result<R>,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut value = if path.exists() {
        load_settings(path)?
    } else {
        Value::Object(Map::new())
    };

    // Backup only if file exists with content.
    if path.exists() {
        let bak = path.with_extension("json.bak");
        fs::copy(path, &bak)
            .with_context(|| format!("writing backup {}", bak.display()))?;
    }

    let result = f(&mut value)?;

    atomic_write_json(path, &value)?;
    Ok(result)
}

fn atomic_write_json(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("settings path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("settings path has no file name: {}", path.display()))?;
    let tmp = parent.join(format!(".{file_name}.ivara.tmp"));

    let mut serialized = serde_json::to_string_pretty(value)?;
    serialized.push('\n');

    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("creating {}", tmp.display()))?;
        f.write_all(serialized.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

fn write_script(path: &Path) -> Result<()> {
    fs::write(path, HOOK_SCRIPT)
        .with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

// ---------- pure merge helpers ----------

/// Merge canonical events into `value.hooks`. Append-only; idempotent.
pub fn merge_install(value: &mut Value, command: &str, timeout: u64) -> Result<()> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings root must be object"))?;

    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.hooks must be object"))?;

    for event in CANONICAL_EVENTS {
        let groups = hooks_obj
            .entry((*event).to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let groups_arr = groups
            .as_array_mut()
            .ok_or_else(|| anyhow!("settings.hooks.{event} must be array"))?;

        // Locate matcher:"" group. If absent, append one.
        let idx = groups_arr.iter().position(|g| {
            g.get("matcher")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
        });
        let group_idx = match idx {
            Some(i) => i,
            None => {
                groups_arr.push(json!({
                    "matcher": "",
                    "hooks": []
                }));
                groups_arr.len() - 1
            }
        };

        let group = groups_arr[group_idx]
            .as_object_mut()
            .ok_or_else(|| anyhow!("settings.hooks.{event}[{group_idx}] must be object"))?;
        let hooks_in = group
            .entry("hooks".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let hooks_arr = hooks_in
            .as_array_mut()
            .ok_or_else(|| anyhow!("settings.hooks.{event}[{group_idx}].hooks must be array"))?;

        let already = hooks_arr.iter().any(|h| {
            h.get("command").and_then(Value::as_str) == Some(command)
        });
        if !already {
            hooks_arr.push(json!({
                "type": "command",
                "command": command,
                "timeout": timeout
            }));
        }
    }
    Ok(())
}

/// Strip ivara hook entries. Returns count of entries removed. Non-ivara entries preserved.
pub fn merge_uninstall(value: &mut Value, command: &str) -> usize {
    let mut removed = 0;
    let Some(root) = value.as_object_mut() else {
        return 0;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };

    let event_names: Vec<String> = hooks.keys().cloned().collect();
    for event in event_names {
        let Some(groups) = hooks.get_mut(&event).and_then(Value::as_array_mut) else {
            continue;
        };

        groups.retain_mut(|group| {
            let Some(g) = group.as_object_mut() else { return true };
            let Some(hooks_in) = g.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = hooks_in.len();
            hooks_in.retain(|h| {
                h.get("command").and_then(Value::as_str) != Some(command)
            });
            removed += before - hooks_in.len();
            !hooks_in.is_empty()
        });

        if groups.is_empty() {
            hooks.remove(&event);
        }
    }

    // If .hooks became empty, remove it for cleanliness.
    if hooks.is_empty() {
        root.remove("hooks");
    }

    removed
}

/// Enumerate canonical events: (event, wired?).
pub fn enumerate_status(value: &Value, command: &str) -> Vec<(&'static str, bool)> {
    CANONICAL_EVENTS
        .iter()
        .map(|e| (*e, event_has_command(value, e, command)))
        .collect()
}

/// Events present in settings with an ivara entry but NOT in canonical list.
/// Signals drift between binary's view and reality.
pub fn unknown_events(value: &Value, command: &str) -> Vec<String> {
    let Some(hooks) = value.get("hooks").and_then(Value::as_object) else {
        return Vec::new();
    };
    let canon: std::collections::HashSet<&&str> = CANONICAL_EVENTS.iter().collect();
    let mut out = Vec::new();
    for (event, groups) in hooks {
        if canon.contains(&event.as_str()) {
            continue;
        }
        if event_has_command_in_groups(groups, command) {
            out.push(event.clone());
        }
    }
    out.sort();
    out
}

fn event_has_command(value: &Value, event: &str, command: &str) -> bool {
    value
        .get("hooks")
        .and_then(|h| h.get(event))
        .map(|groups| event_has_command_in_groups(groups, command))
        .unwrap_or(false)
}

fn event_has_command_in_groups(groups: &Value, command: &str) -> bool {
    let Some(arr) = groups.as_array() else { return false };
    arr.iter().any(|group| {
        let Some(hooks) = group.get("hooks").and_then(Value::as_array) else {
            return false;
        };
        hooks.iter().any(|h| {
            h.get("command").and_then(Value::as_str) == Some(command)
        })
    })
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "bash /tmp/ivara-capture.sh";

    fn empty() -> Value {
        Value::Object(Map::new())
    }

    #[test]
    fn canonical_list_has_25_unique_events() {
        assert_eq!(CANONICAL_EVENTS.len(), 25);
        let mut seen = std::collections::HashSet::new();
        for e in CANONICAL_EVENTS {
            assert!(seen.insert(*e), "duplicate: {e}");
        }
    }

    #[test]
    fn merge_install_into_empty_creates_all_25_events() {
        let mut v = empty();
        merge_install(&mut v, CMD, 5).unwrap();
        let hooks = v["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), 25);
        for e in CANONICAL_EVENTS {
            let groups = hooks[*e].as_array().unwrap();
            assert_eq!(groups.len(), 1);
            let group = &groups[0];
            assert_eq!(group["matcher"], "");
            let h = group["hooks"].as_array().unwrap();
            assert_eq!(h.len(), 1);
            assert_eq!(h[0]["type"], "command");
            assert_eq!(h[0]["command"], CMD);
            assert_eq!(h[0]["timeout"], 5);
        }
    }

    #[test]
    fn merge_install_is_idempotent() {
        let mut v = empty();
        merge_install(&mut v, CMD, 5).unwrap();
        let first = v.clone();
        merge_install(&mut v, CMD, 5).unwrap();
        assert_eq!(v, first, "second install must not mutate settings");
    }

    #[test]
    fn merge_install_preserves_other_tools_in_same_group() {
        let mut v = json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "bash /other/tool.sh",
                        "timeout": 5
                    }]
                }]
            }
        });
        merge_install(&mut v, CMD, 5).unwrap();
        let stop = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1, "no new matcher group added");
        let hooks = stop[0]["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 2, "ivara appended alongside other tool");
        assert_eq!(hooks[0]["command"], "bash /other/tool.sh");
        assert_eq!(hooks[1]["command"], CMD);
    }

    #[test]
    fn merge_install_preserves_other_matchers() {
        let mut v = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "AskUserQuestion",
                    "hooks": [{
                        "type": "command",
                        "command": "bash /other/matcher.sh",
                        "timeout": 5
                    }]
                }]
            }
        });
        merge_install(&mut v, CMD, 5).unwrap();
        let groups = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(groups.len(), 2, "new matcher:\"\" group appended");
        assert_eq!(groups[0]["matcher"], "AskUserQuestion");
        assert_eq!(groups[1]["matcher"], "");
        assert_eq!(groups[1]["hooks"][0]["command"], CMD);
    }

    #[test]
    fn merge_uninstall_removes_ivara_entries_only() {
        let mut v = json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": "bash /other/tool.sh", "timeout": 5 },
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }]
            }
        });
        let n = merge_uninstall(&mut v, CMD);
        assert_eq!(n, 1);
        let hooks = v["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "bash /other/tool.sh");
    }

    #[test]
    fn merge_uninstall_removes_empty_groups_and_events() {
        let mut v = json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }]
            }
        });
        merge_uninstall(&mut v, CMD);
        // Empty group removed, then empty event, then empty hooks root removed.
        assert!(v.get("hooks").is_none(), "empty hooks root pruned");
    }

    #[test]
    fn merge_install_uninstall_roundtrip_is_clean() {
        let original = json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": "bash /other/tool.sh", "timeout": 5 }
                    ]
                }],
                "PreToolUse": [{
                    "matcher": "AskUserQuestion",
                    "hooks": [
                        { "type": "command", "command": "bash /other/matcher.sh", "timeout": 5 }
                    ]
                }]
            },
            "other_key": "preserved"
        });
        let mut v = original.clone();
        merge_install(&mut v, CMD, 5).unwrap();
        assert_ne!(v, original, "install mutated value");
        merge_uninstall(&mut v, CMD);
        assert_eq!(v, original, "uninstall restored original");
    }

    #[test]
    fn uninstall_on_empty_returns_zero() {
        let mut v = empty();
        assert_eq!(merge_uninstall(&mut v, CMD), 0);
    }

    #[test]
    fn enumerate_status_reports_partial_install() {
        // Install just one event manually.
        let v = json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }]
            }
        });
        let rows = enumerate_status(&v, CMD);
        assert_eq!(rows.len(), 25);
        let wired: Vec<_> = rows.iter().filter(|(_, p)| *p).map(|(e, _)| *e).collect();
        assert_eq!(wired, vec!["Stop"]);
    }

    #[test]
    fn unknown_events_flag_drift() {
        let v = json!({
            "hooks": {
                "FutureEvent": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }],
                "Stop": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }]
            }
        });
        let unknown = unknown_events(&v, CMD);
        assert_eq!(unknown, vec!["FutureEvent".to_string()]);
    }

    #[test]
    fn load_settings_rejects_non_object_root() {
        let tmp = std::env::temp_dir().join(format!("ivara-hooks-test-{}.json", std::process::id()));
        fs::write(&tmp, "[1,2,3]").unwrap();
        let err = load_settings(&tmp).unwrap_err();
        let _ = fs::remove_file(&tmp);
        assert!(err.to_string().contains("must be a JSON object"));
    }

    #[test]
    fn load_settings_rejects_malformed_json() {
        let tmp = std::env::temp_dir().join(format!("ivara-hooks-bad-{}.json", std::process::id()));
        fs::write(&tmp, "{not json").unwrap();
        let err = load_settings(&tmp).unwrap_err();
        let _ = fs::remove_file(&tmp);
        assert!(err.to_string().contains("parsing"));
    }

    #[test]
    fn load_settings_accepts_empty_file() {
        let tmp = std::env::temp_dir().join(format!("ivara-hooks-empty-{}.json", std::process::id()));
        fs::write(&tmp, "").unwrap();
        let v = load_settings(&tmp).unwrap();
        let _ = fs::remove_file(&tmp);
        assert!(v.is_object());
    }

    #[test]
    fn merge_install_preexisting_ivara_is_noop() {
        let mut v = json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [
                        { "type": "command", "command": CMD, "timeout": 5 }
                    ]
                }]
            }
        });
        let before = v.clone();
        merge_install(&mut v, CMD, 5).unwrap();
        // SessionStart hooks array still has exactly 1 entry for ivara.
        let hooks = v["hooks"]["SessionStart"][0]["hooks"].as_array().unwrap();
        let ivara_count = hooks.iter().filter(|h| h["command"] == CMD).count();
        assert_eq!(ivara_count, 1);
        assert_ne!(v, before, "other 24 events added");
    }
}

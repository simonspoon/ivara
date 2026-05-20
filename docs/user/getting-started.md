# Getting Started

This guide covers installing ivara and configuring it as a Claude Code hook to start logging sessions.

## Installation

### Homebrew (macOS)

```bash
brew install simonspoon/tap/ivara
```

### Cargo (any platform with Rust)

```bash
cargo install ivara
```

### Binary Download

Download a prebuilt binary from the [GitHub Releases](https://github.com/simonspoon/ivara/releases) page. Available binaries:

| Binary | Platform |
|--------|----------|
| `ivara-linux-amd64` | Linux x86_64 |
| `ivara-linux-arm64` | Linux ARM64 |
| `ivara-darwin-amd64` | macOS x86_64 (Intel) |
| `ivara-darwin-arm64` | macOS ARM64 (Apple Silicon) |
| `ivara-windows-amd64.exe` | Windows x86_64 |

After downloading, make the binary executable (Linux/macOS) and move it to a directory on your `PATH`:

```bash
chmod +x ivara-darwin-arm64
mv ivara-darwin-arm64 /usr/local/bin/ivara
```

## Claude Code Hook Configuration

ivara captures events by running as a Claude Code hook. The fastest way to wire it up is the built-in installer:

```bash
ivara install-hooks                  # user scope: ~/.claude/settings.json
ivara install-hooks --scope project  # project scope: <cwd>/.claude/settings.json
```

`install-hooks` writes a capture wrapper script to `<.claude>/hook-scripts/ivara-capture.sh` and merges hook entries for all 25 canonical events into `settings.json`. The merge is append-only and idempotent — it never clobbers other tools' hooks, and any existing `settings.json` is backed up to `settings.json.bak` first.

Check the wiring at any time with `ivara hooks status`, and remove it with `ivara uninstall-hooks`. See [commands.md](commands.md) for details on all three.

### Manual configuration

To wire hooks by hand instead, add entries to `.claude/settings.json` (project) or `~/.claude/settings.json` (user). Each event maps to an array of matcher groups, and each group holds the hook commands:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "ivara capture", "timeout": 5 }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "ivara capture", "timeout": 5 }
        ]
      }
    ]
  }
}
```

You can register any subset of the 25 supported event types. Each hook sends a JSON object to ivara's stdin containing the event data.

## How Capture Works

When Claude Code fires a hook, it pipes a JSON object to `ivara capture` via stdin. The JSON includes at minimum:

```json
{
  "session_id": "abc-123-def",
  "hook_event_name": "PreToolUse",
  "cwd": "/path/to/project"
}
```

Tool-related events include additional fields like `tool_name`, `tool_input`, and `tool_use_id`. Session events include `source` and `model`. See [data-model.md](../dev/data-model.md) for the complete field reference.

ivara processes each event quickly (it runs in the hook's blocking path):
1. Deserializes the JSON and validates the event type.
2. Stores the event in SQLite -- small payloads (<= 4096 bytes) go inline; larger payloads are written to disk.
3. Updates the session summary table.

## First Capture Walkthrough

After installing and configuring the hook:

1. Start a Claude Code session in any project.
2. Interact with Claude -- ask it to read a file, run a command, etc.
3. Open a separate terminal and verify events were captured:

```bash
# List sessions
ivara sessions

# Show the timeline for your session (prefix match)
ivara timeline abc

# See detailed stats
ivara stats abc
```

## Data Location

By default, ivara stores everything under `~/.ivara/`:

```
~/.ivara/
  ivara.db       # SQLite database
  payloads/      # Large event payloads (>4KB)
    <session_id>/
      <uuid>.json
```

Override the data directory by setting `IVARA_HOME`:

```bash
export IVARA_HOME=/custom/path
```

See [configuration.md](configuration.md) for full configuration details.

## Next Steps

- [Commands Reference](commands.md) -- all 15 commands with flags and examples.
- [Configuration](configuration.md) -- environment variables, data layout, and tuning.
- [Architecture](../dev/architecture.md) -- internal design and data flow.

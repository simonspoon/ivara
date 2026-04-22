use clap::{Parser, Subcommand, ValueEnum};

use crate::commands::hooks::Scope as HookScope;

#[derive(Parser)]
#[command(
    name = "ivara",
    about = "Claude Code session logging and analysis CLI",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ScopeArg {
    /// User-level settings: ~/.claude/settings.json
    User,
    /// Project-level settings: <cwd>/.claude/settings.json
    Project,
}

impl From<ScopeArg> for HookScope {
    fn from(s: ScopeArg) -> Self {
        match s {
            ScopeArg::User => HookScope::User,
            ScopeArg::Project => HookScope::Project,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Capture a hook event from stdin JSON
    Capture,

    /// List sessions
    Sessions {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },

    /// Show chronological events for a session
    Timeline {
        /// Session ID (prefix match)
        session: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,
    },

    /// Show full event detail including payload
    Show {
        /// Event ID
        event_id: i64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Query and filter events
    Query {
        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,
        /// Filter by tool name
        #[arg(long)]
        tool: Option<String>,
        /// Filter by session ID
        #[arg(long)]
        session: Option<String>,
        /// Events after this timestamp (ISO 8601 or relative like "1h", "2d")
        #[arg(long)]
        since: Option<String>,
        /// Events before this timestamp
        #[arg(long)]
        until: Option<String>,
        /// Only events with errors
        #[arg(long)]
        has_error: bool,
        /// Maximum results
        #[arg(long, default_value = "50")]
        limit: i64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show statistics (global or per-session)
    Stats {
        /// Session ID (optional — global if omitted)
        session: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a concise session narrative
    Summary {
        /// Session ID (prefix match)
        session: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Delete old data
    Prune {
        /// Delete events before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,
        /// Delete events older than N days
        #[arg(long)]
        days: Option<u64>,
    },

    /// Export a full session as JSON
    Export {
        /// Session ID (prefix match)
        session: String,
    },

    /// Install ivara hook wrapper + canonical event entries into Claude Code settings.json
    InstallHooks {
        /// Settings scope to install into
        #[arg(long, value_enum, default_value_t = ScopeArg::User)]
        scope: ScopeArg,
    },

    /// Remove ivara hook entries from Claude Code settings.json
    UninstallHooks {
        /// Settings scope to uninstall from
        #[arg(long, value_enum, default_value_t = ScopeArg::User)]
        scope: ScopeArg,
    },

    /// Report hook wiring status (wired vs missing per canonical event)
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
}

#[derive(Subcommand)]
pub enum HooksAction {
    /// Show which canonical events are currently wired in settings.json
    Status {
        /// Settings scope to inspect
        #[arg(long, value_enum, default_value_t = ScopeArg::User)]
        scope: ScopeArg,
    },
}

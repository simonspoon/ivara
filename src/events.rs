use serde::{Deserialize, Serialize};
use std::fmt;

/// All 25 Claude Code hook event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    SessionStart,
    SessionEnd,
    CwdChanged,
    ConfigChange,
    InstructionsLoaded,
    UserPromptSubmit,
    Stop,
    StopFailure,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PermissionRequest,
    SubagentStart,
    SubagentStop,
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
    FileChanged,
    WorktreeCreate,
    WorktreeRemove,
    PreCompact,
    PostCompact,
    Elicitation,
    ElicitationResult,
    Notification,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EventType::SessionStart => "SessionStart",
            EventType::SessionEnd => "SessionEnd",
            EventType::CwdChanged => "CwdChanged",
            EventType::ConfigChange => "ConfigChange",
            EventType::InstructionsLoaded => "InstructionsLoaded",
            EventType::UserPromptSubmit => "UserPromptSubmit",
            EventType::Stop => "Stop",
            EventType::StopFailure => "StopFailure",
            EventType::PreToolUse => "PreToolUse",
            EventType::PostToolUse => "PostToolUse",
            EventType::PostToolUseFailure => "PostToolUseFailure",
            EventType::PermissionRequest => "PermissionRequest",
            EventType::SubagentStart => "SubagentStart",
            EventType::SubagentStop => "SubagentStop",
            EventType::TaskCreated => "TaskCreated",
            EventType::TaskCompleted => "TaskCompleted",
            EventType::TeammateIdle => "TeammateIdle",
            EventType::FileChanged => "FileChanged",
            EventType::WorktreeCreate => "WorktreeCreate",
            EventType::WorktreeRemove => "WorktreeRemove",
            EventType::PreCompact => "PreCompact",
            EventType::PostCompact => "PostCompact",
            EventType::Elicitation => "Elicitation",
            EventType::ElicitationResult => "ElicitationResult",
            EventType::Notification => "Notification",
        };
        write!(f, "{}", s)
    }
}

impl EventType {
    pub fn from_hook_name(name: &str) -> Option<EventType> {
        match name {
            "SessionStart" => Some(EventType::SessionStart),
            "SessionEnd" => Some(EventType::SessionEnd),
            "CwdChanged" => Some(EventType::CwdChanged),
            "ConfigChange" => Some(EventType::ConfigChange),
            "InstructionsLoaded" => Some(EventType::InstructionsLoaded),
            "UserPromptSubmit" => Some(EventType::UserPromptSubmit),
            "Stop" => Some(EventType::Stop),
            "StopFailure" => Some(EventType::StopFailure),
            "PreToolUse" => Some(EventType::PreToolUse),
            "PostToolUse" => Some(EventType::PostToolUse),
            "PostToolUseFailure" => Some(EventType::PostToolUseFailure),
            "PermissionRequest" => Some(EventType::PermissionRequest),
            "SubagentStart" => Some(EventType::SubagentStart),
            "SubagentStop" => Some(EventType::SubagentStop),
            "TaskCreated" => Some(EventType::TaskCreated),
            "TaskCompleted" => Some(EventType::TaskCompleted),
            "TeammateIdle" => Some(EventType::TeammateIdle),
            "FileChanged" => Some(EventType::FileChanged),
            "WorktreeCreate" => Some(EventType::WorktreeCreate),
            "WorktreeRemove" => Some(EventType::WorktreeRemove),
            "PreCompact" => Some(EventType::PreCompact),
            "PostCompact" => Some(EventType::PostCompact),
            "Elicitation" => Some(EventType::Elicitation),
            "ElicitationResult" => Some(EventType::ElicitationResult),
            "Notification" => Some(EventType::Notification),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::SessionStart => "SessionStart",
            EventType::SessionEnd => "SessionEnd",
            EventType::CwdChanged => "CwdChanged",
            EventType::ConfigChange => "ConfigChange",
            EventType::InstructionsLoaded => "InstructionsLoaded",
            EventType::UserPromptSubmit => "UserPromptSubmit",
            EventType::Stop => "Stop",
            EventType::StopFailure => "StopFailure",
            EventType::PreToolUse => "PreToolUse",
            EventType::PostToolUse => "PostToolUse",
            EventType::PostToolUseFailure => "PostToolUseFailure",
            EventType::PermissionRequest => "PermissionRequest",
            EventType::SubagentStart => "SubagentStart",
            EventType::SubagentStop => "SubagentStop",
            EventType::TaskCreated => "TaskCreated",
            EventType::TaskCompleted => "TaskCompleted",
            EventType::TeammateIdle => "TeammateIdle",
            EventType::FileChanged => "FileChanged",
            EventType::WorktreeCreate => "WorktreeCreate",
            EventType::WorktreeRemove => "WorktreeRemove",
            EventType::PreCompact => "PreCompact",
            EventType::PostCompact => "PostCompact",
            EventType::Elicitation => "Elicitation",
            EventType::ElicitationResult => "ElicitationResult",
            EventType::Notification => "Notification",
        }
    }
}

/// Stored event row — what comes back from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub event_uuid: String,
    pub session_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub cwd: Option<String>,
    pub payload_path: Option<String>,
    pub metadata_json: Option<String>,
}

/// Session summary row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub first_seen: String,
    pub last_seen: String,
    pub event_count: i64,
    pub cwd: Option<String>,
    pub model: Option<String>,
}

/// Raw hook input from stdin — the full JSON blob Claude Code sends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    pub session_id: String,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    pub hook_event_name: String,

    // Tool events
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub tool_response: Option<serde_json::Value>,

    // Session events
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,

    // Stop events
    #[serde(default)]
    pub stop_hook_active: Option<bool>,
    #[serde(default)]
    pub last_assistant_message: Option<String>,

    // StopFailure
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_details: Option<String>,
    #[serde(default)]
    pub is_interrupt: Option<bool>,

    // User prompt
    #[serde(default)]
    pub prompt: Option<String>,

    // Subagent events
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub agent_transcript_path: Option<String>,

    // Task events
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub task_subject: Option<String>,
    #[serde(default)]
    pub task_description: Option<String>,
    #[serde(default)]
    pub teammate_name: Option<String>,
    #[serde(default)]
    pub team_name: Option<String>,

    // File events
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub change_type: Option<String>,

    // Compaction events
    #[serde(default)]
    pub compaction_trigger: Option<String>,

    // Elicitation events
    #[serde(default)]
    pub mcp_server_name: Option<String>,
    #[serde(default)]
    pub fields: Option<Vec<serde_json::Value>>,

    // ElicitationResult
    #[serde(default)]
    pub result_action: Option<String>,
    #[serde(default)]
    pub result_content: Option<serde_json::Value>,

    // Notification
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub notification_type: Option<String>,
}

impl HookInput {
    /// Parse hook input from a JSON string (stdin).
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let input: HookInput = serde_json::from_str(json)?;
        Ok(input)
    }

    /// Extract the event type from the hook_event_name field.
    pub fn event_type(&self) -> anyhow::Result<EventType> {
        EventType::from_hook_name(&self.hook_event_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown hook event: {}", self.hook_event_name))
    }

    /// Build a metadata JSON object with only the relevant fields for this event type,
    /// excluding large payloads (tool_input, tool_response, last_assistant_message).
    pub fn metadata(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        // Always include base fields
        if let Some(ref v) = self.transcript_path {
            map.insert("transcript_path".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.permission_mode {
            map.insert("permission_mode".into(), serde_json::json!(v));
        }

        // Event-specific small fields
        if let Some(ref v) = self.source {
            map.insert("source".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.model {
            map.insert("model".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.reason {
            map.insert("reason".into(), serde_json::json!(v));
        }
        if let Some(v) = self.stop_hook_active {
            map.insert("stop_hook_active".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.error {
            map.insert("error".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.error_details {
            map.insert("error_details".into(), serde_json::json!(v));
        }
        if let Some(v) = self.is_interrupt {
            map.insert("is_interrupt".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.prompt {
            map.insert("prompt".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.agent_id {
            map.insert("agent_id".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.agent_type {
            map.insert("agent_type".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.agent_transcript_path {
            map.insert("agent_transcript_path".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.task_id {
            map.insert("task_id".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.task_subject {
            map.insert("task_subject".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.task_description {
            map.insert("task_description".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.teammate_name {
            map.insert("teammate_name".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.team_name {
            map.insert("team_name".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.file_path {
            map.insert("file_path".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.change_type {
            map.insert("change_type".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.compaction_trigger {
            map.insert("compaction_trigger".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.mcp_server_name {
            map.insert("mcp_server_name".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.fields {
            map.insert("fields".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.result_action {
            map.insert("result_action".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.result_content {
            map.insert("result_content".into(), v.clone());
        }
        if let Some(ref v) = self.message {
            map.insert("message".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.title {
            map.insert("title".into(), serde_json::json!(v));
        }
        if let Some(ref v) = self.notification_type {
            map.insert("notification_type".into(), serde_json::json!(v));
        }

        serde_json::Value::Object(map)
    }

    /// Estimate the size of the full payload in bytes.
    pub fn payload_size(&self) -> usize {
        // Approximate by serializing
        serde_json::to_string(self).map(|s| s.len()).unwrap_or(0)
    }
}

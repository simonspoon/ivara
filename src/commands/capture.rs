use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use std::io::Read;

use crate::events::HookInput;
use crate::storage;

/// Read stdin JSON and store the event. Must be fast — hooks block on this.
pub fn run(conn: &Connection) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook = HookInput::from_json(&input)?;
    let event_type = hook.event_type()?;
    let event_uuid = uuid::Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();

    // Decide: inline metadata vs file-backed payload
    let payload_size = hook.payload_size();
    let (payload_path, metadata_json) = if payload_size > storage::PAYLOAD_THRESHOLD {
        // Large payload: write to file, store metadata inline
        let path = storage::write_payload(&hook.session_id, &event_uuid, &input)?;
        let meta = hook.metadata();
        (Some(path), Some(serde_json::to_string(&meta)?))
    } else {
        // Small payload: keep everything inline
        (None, Some(input))
    };

    crate::db::insert_event(
        conn,
        &event_uuid,
        &hook.session_id,
        event_type.as_str(),
        &timestamp,
        hook.tool_name.as_deref(),
        hook.tool_use_id.as_deref(),
        hook.cwd.as_deref(),
        payload_path.as_deref(),
        metadata_json.as_deref(),
    )?;

    // Upsert session
    crate::db::upsert_session(
        conn,
        &hook.session_id,
        &timestamp,
        hook.cwd.as_deref(),
        hook.model.as_deref(),
    )?;

    Ok(())
}

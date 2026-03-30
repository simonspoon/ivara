use anyhow::Result;
use rusqlite::Connection;

use crate::events::Event;
use crate::storage;

/// Show chronological events for a session.
pub fn timeline(
    conn: &Connection,
    session: &str,
    json: bool,
    event_type: Option<&str>,
) -> Result<()> {
    let session_id = crate::db::resolve_session(conn, session)?
        .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", session))?;

    let events = crate::db::session_events(conn, &session_id, event_type)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
        return Ok(());
    }

    if events.is_empty() {
        println!("No events found for session {}", session_id);
        return Ok(());
    }

    println!("Session: {}", session_id);
    println!("Events: {}", events.len());
    println!();
    println!("{:<6} {:<22} {:<22} DETAIL", "ID", "TIME", "EVENT TYPE");
    println!("{}", "-".repeat(80));

    for e in &events {
        let detail = event_detail(e);
        let time_display = truncate_timestamp(&e.timestamp);
        println!(
            "{:<6} {:<22} {:<22} {}",
            e.id, time_display, e.event_type, detail,
        );
    }

    Ok(())
}

/// Show full event detail including payload.
pub fn show(conn: &Connection, event_id: i64, json: bool) -> Result<()> {
    let event = crate::db::get_event(conn, event_id)?
        .ok_or_else(|| anyhow::anyhow!("No event with ID {}", event_id))?;

    if json {
        let mut value = serde_json::to_value(&event)?;
        // Include inline payload if present
        if let Some(ref path) = event.payload_path {
            if let Ok(payload) = storage::read_payload(path) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload) {
                    value["payload"] = parsed;
                }
            }
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    println!("Event ID:    {}", event.id);
    println!("UUID:        {}", event.event_uuid);
    println!("Session:     {}", event.session_id);
    println!("Type:        {}", event.event_type);
    println!("Timestamp:   {}", event.timestamp);
    if let Some(ref tool) = event.tool_name {
        println!("Tool:        {}", tool);
    }
    if let Some(ref tid) = event.tool_use_id {
        println!("Tool Use ID: {}", tid);
    }
    if let Some(ref cwd) = event.cwd {
        println!("CWD:         {}", cwd);
    }

    // Show payload
    if let Some(ref path) = event.payload_path {
        println!("\nPayload (file: {}):", path);
        match storage::read_payload(path) {
            Ok(payload) => println!("{}", payload),
            Err(e) => println!("  [Error reading payload: {}]", e),
        }
    } else if let Some(ref meta) = event.metadata_json {
        println!("\nPayload:");
        // Try to pretty-print JSON
        match serde_json::from_str::<serde_json::Value>(meta) {
            Ok(v) => println!("{}", serde_json::to_string_pretty(&v)?),
            Err(_) => println!("{}", meta),
        }
    }

    Ok(())
}

/// Query events with filters.
#[allow(clippy::too_many_arguments)]
pub fn query(
    conn: &Connection,
    event_type: Option<&str>,
    tool: Option<&str>,
    session: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    has_error: bool,
    limit: i64,
    json: bool,
) -> Result<()> {
    let filter = crate::query::QueryFilter {
        event_type: event_type.map(String::from),
        tool: tool.map(String::from),
        session: session.map(String::from),
        since: since.map(String::from),
        until: until.map(String::from),
        has_error,
        limit,
    };

    let events = crate::query::query_events(conn, &filter)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
        return Ok(());
    }

    if events.is_empty() {
        println!("No events match the query.");
        return Ok(());
    }

    println!(
        "{:<6} {:<22} {:<22} {:<16} SESSION",
        "ID", "TIME", "EVENT TYPE", "TOOL"
    );
    println!("{}", "-".repeat(90));

    for e in &events {
        let tool_str = e.tool_name.as_deref().unwrap_or("-");
        let sid = if e.session_id.len() > 12 {
            &e.session_id[..12]
        } else {
            &e.session_id
        };
        let time_display = truncate_timestamp(&e.timestamp);
        println!(
            "{:<6} {:<22} {:<22} {:<16} {}",
            e.id, time_display, e.event_type, tool_str, sid,
        );
    }

    Ok(())
}

/// Truncate an RFC 3339 timestamp to a readable display format.
fn truncate_timestamp(ts: &str) -> &str {
    // Show up to 19 chars: "2026-03-30T12:34:56"
    if ts.len() > 19 {
        &ts[..19]
    } else {
        ts
    }
}

/// Extract a short detail string from an event for timeline display.
fn event_detail(e: &Event) -> String {
    if let Some(ref tool) = e.tool_name {
        return tool.clone();
    }

    // Try to extract a useful field from metadata
    if let Some(ref meta) = e.metadata_json {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(meta) {
            // Look for common useful fields
            for key in &["reason", "source", "model", "error", "prompt", "message"] {
                if let Some(val) = v.get(key) {
                    if let Some(s) = val.as_str() {
                        let truncated = if s.len() > 50 { &s[..50] } else { s };
                        return format!("{}: {}", key, truncated);
                    }
                }
            }
        }
    }

    String::new()
}

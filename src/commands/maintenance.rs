use anyhow::{bail, Result};
use chrono::{Duration, Utc};
use rusqlite::Connection;

use crate::storage;

/// Prune old data — delete events before a date or older than N days.
pub fn prune(conn: &Connection, before: Option<&str>, days: Option<u64>) -> Result<()> {
    let cutoff = match (before, days) {
        (Some(date), _) => {
            // Parse YYYY-MM-DD date
            format!("{}T00:00:00Z", date)
        }
        (_, Some(d)) => {
            let ts = Utc::now() - Duration::days(d as i64);
            ts.to_rfc3339()
        }
        (None, None) => {
            bail!("Specify --before <date> or --days <N>");
        }
    };

    // Get payload paths before deleting rows
    let paths = crate::db::payload_paths_before(conn, &cutoff)?;

    // Delete DB rows
    let count = crate::db::prune_before(conn, &cutoff)?;

    // Delete payload files
    let mut file_errors = 0;
    for path in &paths {
        if storage::delete_payload(path).is_err() {
            file_errors += 1;
        }
    }

    // Cleanup empty directories
    storage::cleanup_empty_dirs()?;

    println!("Pruned {} events.", count);
    if !paths.is_empty() {
        println!(
            "Deleted {} payload files ({} errors).",
            paths.len() - file_errors,
            file_errors
        );
    }

    Ok(())
}

/// Convert a stored `Event` row to a JSON value with the `payload` field inlined.
///
/// Shared by `ivara export` and `ivara stream` so both emit the same shape per event.
/// If the event has a `payload_path`, the file is read and parsed as JSON. Otherwise
/// `metadata_json` is parsed as the payload. On any read/parse failure, the base
/// serialized `Event` is returned without a `payload` field — matching prior export
/// behavior which silently skipped unreadable payloads.
pub fn event_to_export_value(e: &crate::events::Event) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(e)?;

    if let Some(ref path) = e.payload_path {
        if let Ok(payload) = storage::read_payload(path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload) {
                value["payload"] = parsed;
            }
        }
    } else if let Some(ref meta) = e.metadata_json {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta) {
            value["payload"] = parsed;
        }
    }

    Ok(value)
}

/// Export a full session as a JSON array with inline payloads.
pub fn export(conn: &Connection, session: &str) -> Result<()> {
    let session_id = crate::db::resolve_session(conn, session)?
        .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", session))?;

    let events = crate::db::session_events(conn, &session_id, None)?;

    let mut output = Vec::new();
    for e in &events {
        output.push(event_to_export_value(e)?);
    }

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

use anyhow::Result;
use chrono::DateTime;
use rusqlite::Connection;

/// List sessions.
pub fn run(conn: &Connection, json: bool, limit: i64) -> Result<()> {
    let sessions = crate::db::list_sessions(conn, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    println!(
        "{:<40} {:<22} {:<12} {:<8} CWD",
        "SESSION", "LAST SEEN", "DURATION", "EVENTS"
    );
    println!("{}", "-".repeat(110));

    for s in &sessions {
        let dur_str = match (
            DateTime::parse_from_rfc3339(&s.first_seen),
            DateTime::parse_from_rfc3339(&s.last_seen),
        ) {
            (Ok(first), Ok(last)) => format_duration_secs((last - first).num_seconds()),
            _ => "-".to_string(),
        };
        let cwd = s.cwd.as_deref().unwrap_or("-");
        // Truncate session ID for display
        let sid = if s.session_id.len() > 38 {
            &s.session_id[..38]
        } else {
            &s.session_id
        };
        // Truncate last_seen for display
        let last = if s.last_seen.len() > 19 {
            &s.last_seen[..19]
        } else {
            &s.last_seen
        };
        println!(
            "{:<40} {:<22} {:<12} {:<8} {}",
            sid, last, dur_str, s.event_count, cwd,
        );
    }

    Ok(())
}

pub fn format_duration_secs(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

use anyhow::Result;
use chrono::DateTime;
use rusqlite::Connection;
use serde::Serialize;

use super::sessions::format_duration_secs;
use crate::usage::SessionUsage;

/// Stats output structure.
#[derive(Debug, Serialize)]
struct Stats {
    scope: String,
    total_events: i64,
    error_events: i64,
    error_rate: f64,
    event_types: Vec<TypeCount>,
    tool_usage: Vec<TypeCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<String>,
    /// Token usage — present once captured at SessionEnd or via `backfill-usage`.
    #[serde(skip_serializing_if = "Option::is_none")]
    token_usage: Option<SessionUsage>,
}

#[derive(Debug, Serialize)]
struct TypeCount {
    name: String,
    count: i64,
}

/// Show statistics.
pub fn stats(conn: &Connection, session: Option<&str>, json: bool) -> Result<()> {
    let session_id = match session {
        Some(s) => Some(
            crate::db::resolve_session(conn, s)?
                .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", s))?,
        ),
        None => None,
    };

    let event_counts = crate::query::event_counts_by_type(conn, session_id.as_deref())?;
    let tool_counts = crate::query::tool_frequency(conn, session_id.as_deref())?;
    let (errors, total) = crate::query::error_rate(conn, session_id.as_deref())?;

    let rate = if total > 0 {
        errors as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let duration = if let Some(ref sid) = session_id {
        let events = crate::db::session_events(conn, sid, None)?;
        if events.len() >= 2 {
            duration_between(&events[0].timestamp, &events[events.len() - 1].timestamp)
        } else {
            None
        }
    } else {
        None
    };

    let token_usage = match session_id {
        Some(ref sid) => crate::db::get_session_usage(conn, sid)?,
        None => crate::db::total_usage(conn)?,
    };

    let stats = Stats {
        scope: session_id.as_deref().unwrap_or("global").to_string(),
        total_events: total,
        error_events: errors,
        error_rate: rate,
        event_types: event_counts
            .into_iter()
            .map(|(name, count)| TypeCount { name, count })
            .collect(),
        tool_usage: tool_counts
            .into_iter()
            .map(|(name, count)| TypeCount { name, count })
            .collect(),
        duration,
        token_usage,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("Stats: {}", stats.scope);
    println!("{}", "=".repeat(50));
    println!("Total events:  {}", stats.total_events);
    println!("Error events:  {}", stats.error_events);
    println!("Error rate:    {:.1}%", stats.error_rate);
    if let Some(ref d) = stats.duration {
        println!("Duration:      {}", d);
    }

    println!("\nEvent types:");
    for tc in &stats.event_types {
        println!("  {:<25} {}", tc.name, tc.count);
    }

    if !stats.tool_usage.is_empty() {
        println!("\nTool usage:");
        for tc in &stats.tool_usage {
            println!("  {:<25} {}", tc.name, tc.count);
        }
    }

    if let Some(ref u) = stats.token_usage {
        print_token_usage(u);
    }

    Ok(())
}

/// Print the token-usage block shared by `stats` text output.
fn print_token_usage(u: &SessionUsage) {
    println!("\nToken usage:");
    println!("  {:<16} {}", "Input", group_thousands(u.input_tokens));
    println!("  {:<16} {}", "Output", group_thousands(u.output_tokens));
    println!(
        "  {:<16} {}",
        "Cache write",
        group_thousands(u.cache_creation_tokens)
    );
    println!(
        "  {:<16} {}",
        "Cache read",
        group_thousands(u.cache_read_tokens)
    );
    println!("  {:<16} {}", "Total", group_thousands(u.total_tokens()));
    println!("  {:<16} {}", "API calls", group_thousands(u.api_calls));
    if u.web_search_requests > 0 {
        println!(
            "  {:<16} {}",
            "Web searches",
            group_thousands(u.web_search_requests)
        );
    }
    if u.web_fetch_requests > 0 {
        println!(
            "  {:<16} {}",
            "Web fetches",
            group_thousands(u.web_fetch_requests)
        );
    }
}

/// Format an integer with thousands separators — token counts get large.
fn group_thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if n < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Generate a concise session narrative.
pub fn summary(conn: &Connection, session: &str, json: bool) -> Result<()> {
    let session_id = crate::db::resolve_session(conn, session)?
        .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", session))?;

    let events = crate::db::session_events(conn, &session_id, None)?;

    if events.is_empty() {
        println!("No events found for session {}", session_id);
        return Ok(());
    }

    let first = &events[0];
    let last = &events[events.len() - 1];
    let dur_str = duration_between(&first.timestamp, &last.timestamp).unwrap_or("-".to_string());

    // Count types
    let mut tool_counts = std::collections::HashMap::new();
    let mut errors = 0;

    for e in &events {
        if let Some(ref tool) = e.tool_name {
            *tool_counts.entry(tool.clone()).or_insert(0) += 1;
        }
        if e.event_type == "StopFailure" || e.event_type == "PostToolUseFailure" {
            errors += 1;
        }
    }

    // Find CWD
    let cwd = first.cwd.as_deref().unwrap_or("unknown");

    // Find model from SessionStart metadata
    let model = events
        .iter()
        .find(|e| e.event_type == "SessionStart")
        .and_then(|e| e.metadata_json.as_ref())
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .and_then(|v| v.get("model").and_then(|m| m.as_str().map(String::from)));

    // Token usage — present once captured at SessionEnd or via `backfill-usage`.
    let token_usage = crate::db::get_session_usage(conn, &session_id)?;

    // Top tools
    let mut top_tools: Vec<_> = tool_counts.into_iter().collect();
    top_tools.sort_by_key(|t| std::cmp::Reverse(t.1));
    let top_tools: Vec<_> = top_tools.into_iter().take(5).collect();

    // Truncate timestamps for display
    let start_display = if first.timestamp.len() > 19 {
        &first.timestamp[..19]
    } else {
        &first.timestamp
    };
    let end_display = if last.timestamp.len() > 8 {
        // Show just time portion
        &last.timestamp[11..19.min(last.timestamp.len())]
    } else {
        &last.timestamp
    };

    if json {
        let summary = serde_json::json!({
            "session_id": session_id,
            "cwd": cwd,
            "model": model,
            "start": first.timestamp,
            "end": last.timestamp,
            "duration": dur_str,
            "total_events": events.len(),
            "errors": errors,
            "top_tools": top_tools.iter().map(|(name, count)| {
                serde_json::json!({"tool": name, "count": count})
            }).collect::<Vec<_>>(),
            "token_usage": token_usage,
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("Session: {}", session_id);
    if let Some(ref m) = model {
        println!("Model:   {}", m);
    }
    println!("CWD:     {}", cwd);
    println!(
        "Period:  {} to {} ({})",
        start_display, end_display, dur_str
    );
    println!("Events:  {} total, {} errors", events.len(), errors);
    if let Some(ref u) = token_usage {
        println!(
            "Tokens:  {} in / {} out / {} total ({} API calls)",
            group_thousands(u.input_tokens),
            group_thousands(u.output_tokens),
            group_thousands(u.total_tokens()),
            group_thousands(u.api_calls),
        );
    }

    if !top_tools.is_empty() {
        println!("\nTop tools:");
        for (tool, count) in &top_tools {
            println!("  {} ({})", tool, count);
        }
    }

    Ok(())
}

fn duration_between(start: &str, end: &str) -> Option<String> {
    let s = DateTime::parse_from_rfc3339(start).ok()?;
    let e = DateTime::parse_from_rfc3339(end).ok()?;
    Some(format_duration_secs((e - s).num_seconds()))
}

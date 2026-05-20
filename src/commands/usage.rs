use anyhow::Result;
use rusqlite::Connection;

/// Backfill token usage by parsing session transcripts.
///
/// New sessions get usage recorded automatically when their `SessionEnd` hook
/// fires. This command fills the gap for historical sessions and for sessions
/// whose `SessionEnd` never ran (crash, kill, hook not wired).
///
/// `session` limits the run to one session (prefix match); omitted means every
/// session. Sessions that already have usage data are skipped unless `force`.
pub fn backfill(conn: &Connection, session: Option<&str>, force: bool) -> Result<()> {
    let session_ids = match session {
        Some(s) => {
            let id = crate::db::resolve_session(conn, s)?
                .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", s))?;
            vec![id]
        }
        None => crate::db::all_session_ids(conn)?,
    };

    let mut updated = 0;
    let mut skipped_existing = 0;
    let mut no_transcript = 0;
    let mut missing_file = 0;
    let mut parse_errors = 0;

    for sid in &session_ids {
        if !force && crate::db::get_session_usage(conn, sid)?.is_some() {
            skipped_existing += 1;
            continue;
        }
        let path = match crate::db::session_transcript_path(conn, sid)? {
            Some(p) => p,
            None => {
                no_transcript += 1;
                continue;
            }
        };
        let p = std::path::Path::new(&path);
        if !p.exists() {
            missing_file += 1;
            continue;
        }
        match crate::usage::parse_transcript(p) {
            Ok(usage) => {
                crate::db::upsert_session_usage(conn, sid, &usage)?;
                updated += 1;
            }
            Err(_) => {
                parse_errors += 1;
            }
        }
    }

    println!("Backfilled token usage for {} session(s).", updated);
    if skipped_existing > 0 {
        println!(
            "  {} already had usage data (use --force to re-parse).",
            skipped_existing
        );
    }
    if no_transcript > 0 {
        println!("  {} had no transcript path recorded.", no_transcript);
    }
    if missing_file > 0 {
        println!(
            "  {} referenced a transcript file that no longer exists.",
            missing_file
        );
    }
    if parse_errors > 0 {
        println!(
            "  {} transcript(s) could not be read or parsed.",
            parse_errors
        );
    }

    Ok(())
}

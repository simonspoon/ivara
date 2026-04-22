use anyhow::Result;
use rusqlite::Connection;
use std::io::{self, ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::commands::maintenance::event_to_export_value;
use crate::events::Event;

/// Poll interval while tailing new events.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Stream a session's events as JSONL to stdout: replay existing events, then tail new ones.
///
/// Exit conditions:
/// - A `SessionEnd` event is emitted → flush stdout, return Ok (exit 0).
/// - SIGINT (Ctrl-C) → flush stdout, return Ok (exit 0).
/// - Consumer closes the pipe (BrokenPipe) → return Ok (exit 0).
/// - Unknown session prefix → error (caller maps to exit 1).
pub fn run(conn: &Connection, session: &str) -> Result<()> {
    let session_id = crate::db::resolve_session(conn, session)?
        .ok_or_else(|| anyhow::anyhow!("No session matching '{}'", session))?;

    // Install Ctrl-C handler. Subsequent SIGINTs flip the flag; the loop exits on next poll tick.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = stop.clone();
    // `set_handler` returns Err if another handler was already installed. Ignore — we still
    // honor SIGINT via the default terminate behavior in that case.
    let _ = ctrlc::set_handler(move || {
        stop_handler.store(true, Ordering::SeqCst);
    });

    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Phase 1: replay all existing events in timestamp order.
    let events = crate::db::session_events(conn, &session_id, None)?;
    let mut last_id: i64 = 0;
    for e in &events {
        last_id = last_id.max(e.id);
        if !emit_event(&mut out, e)? {
            return Ok(());
        }
        if e.event_type == "SessionEnd" {
            out.flush().ok();
            return Ok(());
        }
    }

    // Phase 2: poll for new events until SessionEnd, Ctrl-C, or broken pipe.
    loop {
        if stop.load(Ordering::SeqCst) {
            out.flush().ok();
            return Ok(());
        }

        std::thread::sleep(POLL_INTERVAL);

        if stop.load(Ordering::SeqCst) {
            out.flush().ok();
            return Ok(());
        }

        let new_events = crate::db::session_events_after_id(conn, &session_id, last_id)?;
        for e in &new_events {
            last_id = last_id.max(e.id);
            if !emit_event(&mut out, e)? {
                return Ok(());
            }
            if e.event_type == "SessionEnd" {
                out.flush().ok();
                return Ok(());
            }
        }
    }
}

/// Emit a single event as one compact JSON line, flushing stdout.
///
/// Returns `Ok(false)` when the consumer has closed the pipe (BrokenPipe) — caller should
/// exit cleanly. Returns `Ok(true)` on success. Propagates other IO / serialization errors.
fn emit_event<W: Write>(out: &mut W, e: &Event) -> Result<bool> {
    let value = event_to_export_value(e)?;
    let line = serde_json::to_string(&value)?;
    match writeln!(out, "{line}") {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::BrokenPipe => return Ok(false),
        Err(err) => return Err(err.into()),
    }
    match out.flush() {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == ErrorKind::BrokenPipe => Ok(false),
        Err(err) => Err(err.into()),
    }
}

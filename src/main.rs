mod cli;
mod commands;
mod db;
mod events;
mod query;
mod storage;

use clap::Parser;
use std::process;

fn main() {
    let cli = cli::Cli::parse();

    let conn = match db::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to initialize database: {e}");
            process::exit(1);
        }
    };

    let result = run(cli.command, &conn);
    if let Err(e) = result {
        let msg = e.to_string();
        if !msg.is_empty() {
            eprintln!("Error: {e}");
        }
        process::exit(1);
    }
}

fn run(command: cli::Commands, conn: &rusqlite::Connection) -> anyhow::Result<()> {
    match command {
        cli::Commands::Capture => commands::capture::run(conn),

        cli::Commands::Sessions { json, limit } => commands::sessions::run(conn, json, limit),

        cli::Commands::Timeline {
            session,
            json,
            event_type,
        } => commands::query::timeline(conn, &session, json, event_type.as_deref()),

        cli::Commands::Show { event_id, json } => commands::query::show(conn, event_id, json),

        cli::Commands::Query {
            event_type,
            tool,
            session,
            since,
            until,
            has_error,
            limit,
            json,
        } => commands::query::query(
            conn,
            event_type.as_deref(),
            tool.as_deref(),
            session.as_deref(),
            since.as_deref(),
            until.as_deref(),
            has_error,
            limit,
            json,
        ),

        cli::Commands::Stats { session, json } => {
            commands::analysis::stats(conn, session.as_deref(), json)
        }

        cli::Commands::Summary { session, json } => {
            commands::analysis::summary(conn, &session, json)
        }

        cli::Commands::Prune { before, days } => {
            commands::maintenance::prune(conn, before.as_deref(), days)
        }

        cli::Commands::Export { session } => commands::maintenance::export(conn, &session),
    }
}

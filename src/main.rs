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

    let result = run(cli.command);
    if let Err(e) = result {
        let msg = e.to_string();
        if !msg.is_empty() {
            eprintln!("Error: {e}");
        }
        process::exit(1);
    }
}

fn run(command: cli::Commands) -> anyhow::Result<()> {
    // Hook lifecycle commands do not touch the telemetry DB.
    match command {
        cli::Commands::InstallHooks { scope } => return commands::hooks::install(scope.into()),
        cli::Commands::UninstallHooks { scope } => return commands::hooks::uninstall(scope.into()),
        cli::Commands::Hooks { action } => {
            return match action {
                cli::HooksAction::Status { scope } => commands::hooks::status(scope.into()),
            };
        }
        _ => {}
    }

    let conn = db::connect().map_err(|e| anyhow::anyhow!("Failed to initialize database: {e}"))?;

    match command {
        cli::Commands::Capture => commands::capture::run(&conn),

        cli::Commands::Sessions { json, limit } => commands::sessions::run(&conn, json, limit),

        cli::Commands::Active { json, limit } => commands::active::run(&conn, json, limit),

        cli::Commands::Timeline {
            session,
            json,
            event_type,
        } => commands::query::timeline(&conn, &session, json, event_type.as_deref()),

        cli::Commands::Show { event_id, json } => commands::query::show(&conn, event_id, json),

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
            &conn,
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
            commands::analysis::stats(&conn, session.as_deref(), json)
        }

        cli::Commands::Summary { session, json } => {
            commands::analysis::summary(&conn, &session, json)
        }

        cli::Commands::Prune { before, days } => {
            commands::maintenance::prune(&conn, before.as_deref(), days)
        }

        cli::Commands::Export { session } => commands::maintenance::export(&conn, &session),

        cli::Commands::Stream { session } => commands::stream::run(&conn, &session),

        // Hook commands already handled above.
        cli::Commands::InstallHooks { .. }
        | cli::Commands::UninstallHooks { .. }
        | cli::Commands::Hooks { .. } => unreachable!("handled before db connect"),
    }
}

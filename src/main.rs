mod config;
mod language;
mod logging;
mod lsp;
mod mcp;
mod project;
mod server;
mod stats;
mod uri;

use std::sync::Arc;

use clap::{Parser, Subcommand};
use config::SymtraceConfig;
use mcp::tools::McpServer;
use project::registry::ProjectRegistry;
use stats::StatsRecorder;

#[derive(Parser)]
#[command(name = "symtrace-mcp")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print usage statistics for the current project
    Stats,
}

fn main() {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().expect("failed to determine current directory");

    match cli.command {
        None => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(run_server(&cwd));
        }
        Some(Commands::Stats) => {
            // The MCP server holds fcntl locks on WAL files. Disabling
            // file locking lets the stats CLI read the database without
            // conflicting with a running server. Must happen before the
            // runtime spawns any worker threads.
            unsafe { std::env::set_var("LIMBO_DISABLE_FILE_LOCK", "1") };
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(stats::print_stats(&cwd));
        }
    }
}

async fn run_server(cwd: &std::path::Path) {
    // Load config first so we can pass the logging level to init_logging().
    // Config parse errors are fatal and exit immediately (before logging is
    // available), so they still use eprintln!.
    let config_path = cwd.join(".symtrace.toml");

    let config = match SymtraceConfig::load(&config_path) {
        Ok(config) => config,
        Err(config::ConfigError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            SymtraceConfig::implicit(cwd)
        }
        Err(e) => {
            eprintln!("error loading {}: {e}", config_path.display());
            std::process::exit(1);
        }
    };

    // Initialize logging — hold the guard for the entire server lifetime
    // so logs are flushed on drop when the process exits.
    let _log_guard = logging::init_logging(cwd, config.logging.level.as_deref());

    tracing::info!(
        cwd = %cwd.display(),
        pid = std::process::id(),
        "Server started"
    );

    let stats = match StatsRecorder::new(cwd).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize stats database");
            std::process::exit(1);
        }
    };

    let registry = ProjectRegistry::new(&config, cwd, stats.clone()).unwrap_or_else(|e| {
        tracing::error!(error = %e, "error building project registry");
        std::process::exit(1);
    });

    let mut server = McpServer::new(registry, stats);
    if let Err(e) = server.run().await {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    }
}

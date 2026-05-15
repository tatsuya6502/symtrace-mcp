mod config;
mod language;
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().expect("failed to determine current directory");

    match cli.command {
        None => run_server(&cwd).await,
        Some(Commands::Stats) => stats::print_stats(&cwd).await,
    }
}

async fn run_server(cwd: &std::path::Path) {
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

    let stats = match StatsRecorder::new(cwd).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("error: failed to initialize stats database: {e}");
            std::process::exit(1);
        }
    };

    let registry = ProjectRegistry::new(&config, cwd, stats.clone()).unwrap_or_else(|e| {
        eprintln!("error building project registry: {e}");
        std::process::exit(1);
    });

    let mut server = McpServer::new(registry, stats);
    if let Err(e) = server.run().await {
        eprintln!("symtrace-mcp error: {e}");
        std::process::exit(1);
    }
}

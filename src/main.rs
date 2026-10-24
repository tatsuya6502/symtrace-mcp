mod config;
mod hook;
mod language;
mod logging;
mod lsp;
mod mcp;
mod project;
mod server;
mod setup;
mod stats;
mod uri;

use std::sync::Arc;

use clap::{Parser, Subcommand};
use config::SymtraceConfig;
use mcp::tools::McpServer;
use project::registry::ProjectRegistry;
use stats::StatsRecorder;

#[derive(Parser)]
#[command(name = "symtrace-mcp", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print usage statistics for the current project
    Stats,
    /// Install, inspect or remove the Claude Code integration
    /// (CLAUDE.md block, Explore.md block, PreToolUse hook, MCP server entry)
    Setup(setup::SetupArgs),
    /// Claude Code PreToolUse hook: read one event from stdin and print a
    /// reminder to use symtrace-mcp tools when a text-search heuristic is used
    Hook,
}

fn main() {
    let cli = Cli::parse();
    // Resolved lazily: the `hook` path must never fail, even when the
    // working directory has been deleted under a running session.
    let cwd = || std::env::current_dir().expect("failed to determine current directory");

    match cli.command {
        None => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(run_server(&cwd()));
        }
        // `setup` and `hook` are plain CLI paths: no tokio runtime, no logging,
        // no stats database (the hook runs on every Bash/Grep call and must be
        // fast and side-effect free).
        Some(Commands::Setup(args)) => {
            let code = setup::run(args, &cwd());
            std::process::exit(code);
        }
        Some(Commands::Hook) => {
            let code = hook::run();
            std::process::exit(code);
        }
        Some(Commands::Stats) => {
            // The MCP server holds fcntl locks on WAL files. Disabling
            // file locking lets the stats CLI read the database without
            // conflicting with a running server. Must happen before the
            // runtime spawns any worker threads.
            unsafe { std::env::set_var("LIMBO_DISABLE_FILE_LOCK", "1") };
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(stats::print_stats(&cwd()));
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

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    use super::*;

    #[test]
    fn version_comes_from_cargo() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn version_flags_print_name_and_version() {
        for flag in ["--version", "-V"] {
            let err = match Cli::try_parse_from(["symtrace-mcp", flag]) {
                Ok(_) => panic!("{flag} should not parse into a command"),
                Err(e) => e,
            };
            assert_eq!(err.kind(), ErrorKind::DisplayVersion, "{flag}");
            assert_eq!(
                err.render().to_string(),
                format!("symtrace-mcp {}\n", env!("CARGO_PKG_VERSION")),
                "{flag}"
            );
        }
    }

    #[test]
    fn no_arguments_runs_the_server() {
        let cli = Cli::try_parse_from(["symtrace-mcp"]).unwrap();
        assert!(cli.command.is_none());
    }
}

mod config;
mod language;
mod lsp;
mod mcp;
mod project;
mod server;
mod uri;

use config::SymtraceConfig;
use mcp::tools::McpServer;
use project::registry::ProjectRegistry;

#[tokio::main]
async fn main() {
    let cwd = std::env::current_dir().expect("failed to determine current directory");
    let config_path = cwd.join(".symtrace.toml");

    let config = match SymtraceConfig::load(&config_path) {
        Ok(config) => config,
        Err(config::ConfigError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            SymtraceConfig::implicit(&cwd)
        }
        Err(e) => {
            eprintln!("error loading {}: {e}", config_path.display());
            std::process::exit(1);
        }
    };

    let registry = ProjectRegistry::new(&config, &cwd).unwrap_or_else(|e| {
        eprintln!("error building project registry: {e}");
        std::process::exit(1);
    });

    let mut server = McpServer::new(registry);
    if let Err(e) = server.run().await {
        eprintln!("symtrace-mcp error: {e}");
        std::process::exit(1);
    }
}

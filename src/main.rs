mod language;
mod lsp;
mod mcp;
mod server;

use mcp::tools::McpServer;

#[tokio::main]
async fn main() {
    let root =
        std::env::current_dir().expect("failed to determine current directory");
    let mut server = McpServer::new(root);
    if let Err(e) = server.run().await {
        eprintln!("symtrace-mcp error: {e}");
        std::process::exit(1);
    }
}

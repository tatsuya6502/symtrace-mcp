mod language;
mod lsp;
mod mcp;
mod server;

use mcp::tools::McpServer;

#[tokio::main]
async fn main() {
    let mut server = McpServer::new();
    if let Err(e) = server.run().await {
        eprintln!("symtrace-mcp error: {e}");
        std::process::exit(1);
    }
}

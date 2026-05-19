use std::path::Path;
use std::process::Stdio;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{Duration, sleep};

pub struct McpClient {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    /// Spawn `symtrace-mcp` as a subprocess with CWD set to `cwd`.
    pub async fn spawn(cwd: &Path) -> Result<Self, String> {
        let binary = std::env::var("SYMTRACE_MCP_BINARY").unwrap_or_else(|_| {
            let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            p.push("target/debug/symtrace-mcp");
            p.to_str().unwrap().to_string()
        });

        let mut child = Command::new(&binary)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("failed to spawn symtrace-mcp: {e}"))?;

        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;

        let mut client = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };

        // MCP initialize handshake
        let _init = client
            .send_request(
                "initialize",
                Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "integration-test", "version": "0.1.0" }
                })),
            )
            .await?;

        // Send initialized notification (no response expected)
        client
            .send_notification("notifications/initialized", None)
            .await?;

        Ok(client)
    }

    /// Send a JSON-RPC request and return the response.
    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if let Some(p) = params {
            request["params"] = p;
        }

        self.write_message(&request).await?;

        // Read responses, skipping notifications (no "id" or different id)
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let msg = self.read_message().await?;
                if msg.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    if let Some(error) = msg.get("error") {
                        return Err(format!("JSON-RPC error: {error}"));
                    }
                    return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
                }
            }
        })
        .await
        .map_err(|_| format!("timeout waiting for response: method={method}, id={id}"))?
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), String> {
        let mut notif = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(p) = params {
            notif["params"] = p;
        }
        self.write_message(&notif).await
    }

    /// Convenience: call `tools/list`.
    pub async fn tools_list(&mut self) -> Result<Value, String> {
        self.send_request("tools/list", None).await
    }

    /// Convenience: call `tools/call` with a tool name and arguments.
    pub async fn tools_call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.send_request(
            "tools/call",
            Some(json!({ "name": name, "arguments": arguments })),
        )
        .await
    }

    /// Poll until the LSP server is ready by sending a known query.
    /// Uses exponential backoff (100ms initial, 2x cap) up to `timeout`.
    pub async fn wait_for_ready(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
        timeout: Duration,
    ) -> Result<(), String> {
        let start = tokio::time::Instant::now();
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self
                .tools_call(
                    "goto_definition",
                    json!({
                        "file_path": file_path,
                        "line": line,
                        "column": column,
                    }),
                )
                .await
            {
                Ok(_) => return Ok(()),
                Err(_) if start.elapsed() < timeout => {
                    sleep(delay).await;
                    delay = (delay * 2).min(max_delay);
                }
                Err(e) => {
                    return Err(format!(
                        "LSP server not ready after {:.1}s: {e}",
                        start.elapsed().as_secs_f64()
                    ));
                }
            }
        }
    }

    /// Terminate the subprocess.
    pub async fn shutdown(&mut self) -> Result<(), String> {
        let _ = self.send_notification("shutdown", None).await;

        match tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            _ => {
                let _ = self.child.start_kill();
                Ok(())
            }
        }
    }

    async fn write_message(&mut self, msg: &Value) -> Result<(), String> {
        let mut line = serde_json::to_string(msg).map_err(|e| format!("serialize: {e}"))?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("write: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("flush: {e}"))?;
        Ok(())
    }

    async fn read_message(&mut self) -> Result<Value, String> {
        let mut line = Vec::new();
        self.stdout
            .read_until(b'\n', &mut line)
            .await
            .map_err(|e| format!("read: {e}"))?;
        if line.is_empty() {
            return Err("EOF from symtrace-mcp".to_string());
        }
        let text = String::from_utf8_lossy(&line);
        serde_json::from_str(text.trim()).map_err(|e| format!("parse: {e}\n  raw: {text}"))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

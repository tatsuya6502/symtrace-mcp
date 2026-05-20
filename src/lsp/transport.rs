use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

// --- Error type ---

#[derive(Debug)]
pub enum LspError {
    Transport(String),
    JsonRpc { code: i32, message: String },
    ProcessExited,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
            Self::JsonRpc { code, message } => {
                write!(f, "JSON-RPC error ({code}): {message}")
            }
            Self::ProcessExited => write!(f, "language server process exited"),
        }
    }
}

impl std::error::Error for LspError {}

// --- Pending-request map (sync Mutex — never held across .await) ---

type PendingMap = Arc<std::sync::Mutex<HashMap<i64, oneshot::Sender<Result<Value, LspError>>>>>;

// --- LspTransport (3.3) ---

pub struct LspTransport {
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    pending: PendingMap,
    next_id: AtomicI64,
    child: Arc<Mutex<Child>>,
    _reader_handle: tokio::task::JoinHandle<()>,
    closed: Arc<AtomicBool>,
}

impl LspTransport {
    /// Spawn a language server child process and create the transport.
    /// Returns the transport and a receiver for server notifications.
    pub async fn spawn(
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<(String, Value)>), LspError> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| LspError::Transport(format!("failed to spawn '{command}': {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Transport("child stdin not captured".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Transport("child stdout not captured".into()))?;

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let pending: PendingMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let child = Arc::new(Mutex::new(child));
        let closed = Arc::new(AtomicBool::new(false));
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        let reader_handle = tokio::spawn(reader_task(
            BufReader::new(stdout),
            pending.clone(),
            child.clone(),
            writer.clone(),
            closed.clone(),
            notification_tx,
        ));

        Ok((
            Self {
                writer,
                pending,
                next_id: AtomicI64::new(1),
                child,
                _reader_handle: reader_handle,
                closed,
            },
            notification_rx,
        ))
    }

    /// Send a JSON-RPC request and await the response (3.4).
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending.lock().unwrap();
            if self.closed.load(Ordering::Acquire) {
                return Err(LspError::ProcessExited);
            }
            map.insert(id, tx);
        }

        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        if let Err(e) = write_message(&self.writer, &message).await {
            let mut map = self.pending.lock().unwrap();
            map.remove(&id);
            return Err(e);
        }

        rx.await.map_err(|_| LspError::ProcessExited)?
    }

    /// Send a JSON-RPC notification — fire-and-forget (3.5).
    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), LspError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(LspError::ProcessExited);
        }
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        write_message(&self.writer, &message).await
    }
}

impl Drop for LspTransport {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
        if let Ok(mut pending) = self.pending.lock() {
            for (_, tx) in pending.drain() {
                let _ = tx.send(Err(LspError::ProcessExited));
            }
        }
    }
}

// --- Background reader task (3.6 + 3.7) ---

async fn reader_task(
    mut reader: BufReader<ChildStdout>,
    pending: PendingMap,
    child: Arc<Mutex<Child>>,
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    closed: Arc<AtomicBool>,
    notification_tx: mpsc::UnboundedSender<(String, Value)>,
) {
    loop {
        let msg = match read_message(&mut reader).await {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("[lsp] reader error: {e}");
                break;
            }
        };
        let raw_id = msg.get("id");
        if let Some(id_val) = raw_id {
            if let Some(id) = id_val.as_i64() {
                // Integer ID — try to match a pending client request.
                let matched = { pending.lock().unwrap().remove(&id) };
                if let Some(tx) = matched {
                    if let Some(error) = msg.get("error") {
                        let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
                        let message = error
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        let _ = tx.send(Err(LspError::JsonRpc { code, message }));
                    } else {
                        let result = msg.get("result").cloned().unwrap_or(Value::Null);
                        let _ = tx.send(Ok(result));
                    }
                } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                    // Server-initiated request (integer ID).
                    let response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("unsupported server request: {method}") }
                    });
                    let _ = write_message(&writer, &response).await;
                }
            } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                // Server-initiated request (string or other ID).
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "error": { "code": -32601, "message": format!("unsupported server request: {method}") }
                });
                let _ = write_message(&writer, &response).await;
            }
        } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
            let params = msg
                .get("params")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            let _ = notification_tx.send((method.to_string(), params));
        }
    }

    // Error all remaining pending requests (3.7).
    {
        let mut map = pending.lock().unwrap();
        closed.store(true, Ordering::Release);
        for (_, tx) in map.drain() {
            let _ = tx.send(Err(LspError::ProcessExited));
        }
    }

    // Reap the child process.
    if let Ok(mut child) = child.try_lock() {
        let _ = child.wait().await;
    }
}

// --- Content-Length framing (3.1 write, 3.2 read) ---

/// Write a JSON-RPC message with `Content-Length` framing.
async fn write_message(
    writer: &Arc<Mutex<BufWriter<ChildStdin>>>,
    message: &Value,
) -> Result<(), LspError> {
    let body = serde_json::to_string(message)
        .map_err(|e| LspError::Transport(format!("serialization failed: {e}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());

    let mut w = writer.lock().await;
    w.write_all(header.as_bytes())
        .await
        .map_err(|e| LspError::Transport(format!("write failed: {e}")))?;
    w.write_all(body.as_bytes())
        .await
        .map_err(|e| LspError::Transport(format!("write failed: {e}")))?;
    w.flush()
        .await
        .map_err(|e| LspError::Transport(format!("flush failed: {e}")))?;
    Ok(())
}

/// Read a JSON-RPC message with `Content-Length` framing.
async fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<Value, LspError> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| LspError::Transport(format!("header read failed: {e}")))?;
        if n == 0 {
            return Err(LspError::ProcessExited);
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                rest.trim()
                    .parse::<usize>()
                    .map_err(|e| LspError::Transport(format!("invalid Content-Length: {e}")))?,
            );
        }
    }

    let length = content_length
        .ok_or_else(|| LspError::Transport("missing Content-Length header".into()))?;

    const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
    if length > MAX_BODY_BYTES {
        return Err(LspError::Transport(format!(
            "Content-Length {length} exceeds maximum {MAX_BODY_BYTES}"
        )));
    }

    let mut buf = vec![0u8; length];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| LspError::Transport(format!("body read failed: {e}")))?;

    let body =
        String::from_utf8(buf).map_err(|e| LspError::Transport(format!("invalid UTF-8: {e}")))?;

    serde_json::from_str(&body).map_err(|e| LspError::Transport(format!("JSON parse failed: {e}")))
}

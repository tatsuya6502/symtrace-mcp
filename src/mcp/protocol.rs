//! JSON-RPC 2.0 message types and Content-Length framing for MCP.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
};

// --- JSON-RPC 2.0 message types ---

/// JSON-RPC request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC notification (no id, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC success response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

/// JSON-RPC error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub jsonrpc: String,
    pub id: Value,
    pub error: ErrorBody,
}

/// Error details within a JSON-RPC error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

// --- Standard JSON-RPC error codes ---

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INTERNAL_ERROR: i64 = -32603;

// --- Response builders ---

pub fn success_response(id: &Value, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

pub fn error_response(id: Value, code: i64, message: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

// --- Content-Length framing ---

/// Write a JSON-RPC message with `Content-Length` framing.
pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &Value,
) -> io::Result<()> {
    let body = serde_json::to_string(message).expect("serialization should not fail");
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(body.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a JSON-RPC message with `Content-Length` framing.
pub async fn read_message<R: AsyncBufRead + AsyncRead + Unpin>(
    reader: &mut R,
) -> io::Result<Value> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                rest.trim()
                    .parse::<usize>()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
        }
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
    if length > MAX_BODY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Content-Length {length} exceeds maximum {MAX_BODY_BYTES}"),
        ));
    }

    let mut buf = vec![0u8; length];
    reader.read_exact(&mut buf).await?;
    let body = String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    serde_json::from_str(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

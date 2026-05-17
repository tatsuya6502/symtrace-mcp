use serde_json::{Value, json};

/// rust-analyzer–specific initialization parameters.
pub fn client_capabilities() -> Value {
    json!({
        "textDocument": {
            "hover": { "contentFormat": ["plaintext", "markdown"] },
            "references": { "dynamicRegistration": false },
            "definition": { "dynamicRegistration": false, "linkSupport": true },
            "implementation": { "dynamicRegistration": false },
            "documentSymbol": { "dynamicRegistration": false },
            "rename": { "dynamicRegistration": false, "prepareSupport": false },
            "diagnostic": { "dynamicRegistration": false }
        },
        "workspace": {
            "symbol": { "dynamicRegistration": false },
            "workspaceEdit": { "documentChanges": true }
        }
    })
}

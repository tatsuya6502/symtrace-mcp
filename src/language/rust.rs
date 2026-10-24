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
            "diagnostic": { "dynamicRegistration": false },
            "callHierarchy": { "dynamicRegistration": false }
        },
        "workspace": {
            "symbol": { "dynamicRegistration": false },
            "workspaceEdit": { "documentChanges": true }
        },
        "experimental": {
            "serverStatusNotification": true
        }
    })
}

/// rust-analyzer initialization options.
///
/// Always `None`: rust-analyzer treats `initializationOptions` as its own
/// configuration, so symtrace-mcp sends none and the field is omitted from
/// the `initialize` request.
pub fn initialization_options() -> Option<Value> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_call_hierarchy_capability() {
        let caps = client_capabilities();
        let text_doc = caps.get("textDocument").unwrap();
        assert_eq!(
            text_doc["callHierarchy"]["dynamicRegistration"],
            Value::Bool(false),
            "Rust capabilities should declare call hierarchy"
        );
        assert!(
            text_doc.get("diagnostic").is_some(),
            "Rust capabilities should still include pull diagnostics"
        );
    }

    #[test]
    fn requests_server_status_notification() {
        let caps = client_capabilities();
        assert_eq!(
            caps["experimental"]["serverStatusNotification"],
            Value::Bool(true),
            "Rust capabilities should opt in to experimental/serverStatus"
        );
    }

    #[test]
    fn no_initialization_options() {
        assert!(initialization_options().is_none());
    }
}

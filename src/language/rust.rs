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
        }
    })
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
}

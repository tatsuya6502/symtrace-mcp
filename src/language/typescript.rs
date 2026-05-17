use serde_json::{Value, json};

/// typescript-language-server capabilities.
pub fn client_capabilities() -> Value {
    json!({
        "textDocument": {
            "hover": { "contentFormat": ["plaintext", "markdown"] },
            "references": { "dynamicRegistration": false },
            "definition": { "dynamicRegistration": false, "linkSupport": true },
            "implementation": { "dynamicRegistration": false },
            "documentSymbol": { "dynamicRegistration": false },
            "rename": { "dynamicRegistration": false, "prepareSupport": false }
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
    fn no_pull_diagnostics_capability() {
        let caps = client_capabilities();
        let text_doc = caps.get("textDocument").unwrap();
        assert!(
            text_doc.get("diagnostic").is_none(),
            "TypeScript capabilities should not include pull diagnostics"
        );
    }
}

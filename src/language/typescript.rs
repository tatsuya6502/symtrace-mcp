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
            "rename": { "dynamicRegistration": false, "prepareSupport": false },
            "callHierarchy": { "dynamicRegistration": false }
        },
        "workspace": {
            "symbol": { "dynamicRegistration": false },
            "workspaceEdit": { "documentChanges": true }
        }
    })
}

/// typescript-language-server initialization options.
///
/// Disables the syntax-only tsserver (`useSyntaxServer: "never"`). With the
/// default `"auto"`, typescript-language-server routes definition,
/// references, hover, rename and similar requests to a syntax server that
/// knows only the open file while the semantic project is still loading.
/// The project starts loading at the first `didOpen`, which symtrace-mcp
/// sends right before the first request, so the first query would otherwise
/// always get a file-local answer (Issue #30). With `"never"`, requests are
/// queued until the semantic tsserver has loaded the project.
pub fn initialization_options() -> Option<Value> {
    Some(json!({ "tsserver": { "useSyntaxServer": "never" } }))
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

    #[test]
    fn declares_call_hierarchy_capability() {
        let caps = client_capabilities();
        assert_eq!(
            caps["textDocument"]["callHierarchy"]["dynamicRegistration"],
            Value::Bool(false),
            "TypeScript capabilities should declare call hierarchy"
        );
    }

    #[test]
    fn no_server_status_notification_capability() {
        let caps = client_capabilities();
        assert!(
            caps.pointer("/experimental/serverStatusNotification")
                .is_none(),
            "TypeScript capabilities should not request experimental/serverStatus"
        );
    }

    #[test]
    fn initialization_options_disable_syntax_server() {
        let opts = initialization_options().expect("TypeScript should send initialization options");
        assert_eq!(opts["tsserver"]["useSyntaxServer"], "never");
    }
}

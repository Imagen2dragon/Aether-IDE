use serde_json::{json, Value};

/// Generates standard LSP 3.17 ClientCapabilities payload.
pub fn default_client_capabilities() -> Value {
    json!({
        "workspace": {
            "applyEdit": true,
            "workspaceEdit": {
                "documentChanges": true
            },
            "didChangeConfiguration": {
                "dynamicRegistration": true
            },
            "workspaceFolders": true,
            "configuration": true
        },
        "textDocument": {
            "synchronization": {
                "dynamicRegistration": true,
                "willSave": false,
                "willSaveWaitUntil": false,
                "didSave": true
            },
            "completion": {
                "dynamicRegistration": true,
                "completionItem": {
                    "snippetSupport": true,
                    "commitCharactersSupport": true,
                    "documentationFormat": ["markdown", "plaintext"],
                    "deprecatedSupport": true,
                    "preselectSupport": true
                },
                "completionItemKind": {
                    "valueSet": [
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25
                    ]
                },
                "contextSupport": true
            },
            "hover": {
                "dynamicRegistration": true,
                "contentFormat": ["markdown", "plaintext"]
            },
            "definition": {
                "dynamicRegistration": true,
                "linkSupport": true
            },
            "publishDiagnostics": {
                "relatedInformation": true,
                "versionSupport": true,
                "codeDescriptionSupport": true,
                "dataSupport": true
            }
        },
        "general": {
            "positionEncodings": ["utf-16", "utf-8"]
        }
    })
}

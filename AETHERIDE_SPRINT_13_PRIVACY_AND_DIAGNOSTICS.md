# AetherIDE Sprint 13 — Privacy Guarantees & Diagnostic Architecture Report

## 1. Privacy Principles & Architecture

AetherIDE is designed around developer data sovereignty and zero involuntary telemetry:

1. **Zero Unsolicited Telemetry:** The application does not collect, aggregate, or phone home developer telemetry, file names, source code, or usage statistics.
2. **Local Processing First:** AST indexing, syntax parsing, Project Brain graph construction, checkpoint storage, and CRDT synchronizations occur exclusively on the local workstation.
3. **Transparent Network Boundaries:** Outbound network connections are strictly limited to user-configured AI model provider endpoints (Anthropic, OpenAI, Google) or explicitly initiated peer-to-peer collaboration sessions.
4. **Local Model Option:** Developers working under air-gap or strict confidentiality compliance can use local Ollama/LM Studio endpoints with zero outbound internet traffic.

---

## 2. Privacy-Preserving Diagnostic Export

When troubleshooting issues during the beta pilot, developers can generate an exportable diagnostic bundle. The diagnostic system enforces rigorous sanitization before creating the bundle:

```
┌────────────────────────────────────────────────────────┐
│               Raw Log & Error Stream                   │
└──────────────────────────┬─────────────────────────────┘
                           │
┌──────────────────────────▼─────────────────────────────┐
│                 SecretRedactor Engine                  │
│                                                        │
│  - Redacts Bearer Tokens: [REDACTED_BEARER_TOKEN]      │
│  - Redacts API Keys (Anthropic, OpenAI, Google)        │
│  - Redacts Private Paths: /Users/<user> -> /Users/...  │
│  - Scrubs Memory Addresses & Connection Nonces         │
└──────────────────────────┬─────────────────────────────┘
                           │
┌──────────────────────────▼─────────────────────────────┐
│             Sanitized Diagnostic Bundle                │
│  - OS version, architecture, Rust runtime metrics      │
│  - Daemon error logs (sanitized)                       │
│  - WorkspaceScanReport summary (aggregate numbers only)│
└────────────────────────────────────────────────────────┘
```

---

## 3. Secret Redaction Telemetry Engine (`SecretRedactor`)

The `diagnostics` and `security-core` crates use deterministic regex pattern matching to sanitize all string outputs:
- **API Keys:** Patterns matching `sk-ant-[a-zA-Z0-9_\-]{20,}`, `sk-[a-zA-Z0-9_\-]{20,}`, and `AIza[0-9A-Za-z\-_]{35}` are scrubbed.
- **Bearer Tokens:** All HTTP `Authorization: Bearer <token>` headers are replaced with `[REDACTED_SECRET]`.
- **Operating System Usernames:** System paths containing user profile names (`C:\Users\<Name>\...` or `/home/<name>/...`) are replaced with standardized relative tokens (`<USER_HOME>/...`).

---

## 4. Verification

- `security-core::tests::test_secret_redactor`: Validates that sensitive strings containing combinations of tokens, keys, and private data are completely neutralized without altering the diagnostic context of log messages.

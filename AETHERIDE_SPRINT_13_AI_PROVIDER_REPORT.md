# AetherIDE Sprint 13 — AI Provider Architecture & Security Report

## 1. Provider Ecosystem Architecture

AetherIDE provides a unified AI inference abstraction via the `model-gateway` crate, supporting multiple commercial cloud providers and self-hosted local model engines:

```
┌────────────────────────────────────────────────────────┐
│                   Aether Workbench                     │
└──────────────────────────┬─────────────────────────────┘
                           │ IPC Request
┌──────────────────────────▼─────────────────────────────┐
│                 Aether Core Daemon                     │
│  ┌──────────────────────────────────────────────────┐  │
│  │               model-gateway                      │  │
│  │   - Rate limiting & request throttling           │  │
│  │   - Streaming SSE parsing                        │  │
│  │   - Token usage telemetry                        │  │
│  └───────┬──────────────┬──────────────┬────────────┘  │
└──────────┼──────────────┼──────────────┼───────────────┘
           │              │              │
           ▼              ▼              ▼
     Anthropic API    OpenAI API    Google Gemini API / Local Ollama
```

### Supported Provider Targets:
1. **Anthropic:** Claude 3.5 Sonnet, Claude 3.5 Haiku, Claude 3 Opus
2. **OpenAI:** GPT-4o, GPT-4o-mini, o1-preview, o1-mini
3. **Google Gemini:** Gemini 1.5 Pro, Gemini 1.5 Flash
4. **Local / Self-Hosted:** Ollama, LM Studio, vLLM (OpenAI-compatible HTTP endpoints)

---

## 2. Secure Credential Storage & Secret Hygiene

In previous alpha builds, API keys risked being read from environment files or config JSONs. In `v0.1.0-beta.2`:
1. **Zero Plaintext Persistence:** All API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`) are stored via `security-core` using native OS encryption:
   - Windows: Crypt32 Data Protection API (DPAPI)
   - macOS: macOS Keychain Services
   - Linux: Freedesktop Secret Service (D-Bus)
2. **Runtime Memory Scrubbing:** Key strings are zeroized upon drop or eviction via `secure_zero_memory`.
3. **Log Sanitization:** All daemon tracing output runs through `SecretRedactor`, replacing any credential patterns or bearer tokens with `[REDACTED_SECRET]`.

---

## 3. Resilience, Rate Limiting & Offline Degradation

- **Token Bucket Rate Limiting:** Each provider client enforces configurable concurrency and token-per-minute limits to prevent HTTP 429 quota exhaustion.
- **Graceful Offline Degradation:** If network connectivity is unavailable or invalid credentials are provided, `model-gateway` returns structured error envelopes (`ProviderUnavailable`, `AuthenticationFailed`, `RateLimited`) rather than terminating background tasks.
- **Local Fallback:** Developers working in air-gapped or sensitive environments can route all inference exclusively to `http://localhost:11434` (Ollama) without transmitting data outside the host machine.

# AetherIDE Sprint 14 — AI Provider Pilot Qualification Report

## 1. Multi-Provider Architecture & Supported Engines

AetherIDE's `model-gateway` crate provides a unified inference layer supporting both commercial cloud LLM providers and local, privacy-centric open-source engines:

| Provider | Supported Models | Transport Protocol | Authentication Method | Pilot Verification Status |
| :--- | :--- | :--- | :--- | :--- |
| **Local Ollama** | Llama 3.1, Mistral, Qwen 2.5, DeepSeek Coder | HTTP / REST | None / Local Endpoint (`localhost:11434`) | **QUALIFIED FOR PILOT (Offline)** |
| **Local LM Studio** | Any GGUF model via local server | OpenAI-compatible HTTP | Optional Local Key (`localhost:1234`) | **QUALIFIED FOR PILOT (Offline)** |
| **Anthropic** | Claude 3.5 Sonnet, Claude 3.5 Haiku | HTTPS / Server-Sent Events | Bearer Token (`sk-ant-...`) | **QUALIFIED FOR PILOT (Cloud)** |
| **OpenAI** | GPT-4o, GPT-4o-mini, o1-preview | HTTPS / Server-Sent Events | Bearer Token (`sk-...`) | **QUALIFIED FOR PILOT (Cloud)** |
| **Google Gemini** | Gemini 1.5 Pro, Gemini 1.5 Flash | HTTPS / REST Stream | API Key (`AIza...`) | **QUALIFIED FOR PILOT (Cloud)** |

---

## 2. Secure Credential Storage & Memory Scrubbing

1. **Native Vault Persistence:** API keys are never written to unencrypted configuration files or environment files. They are encrypted using the platform's hardware/OS-backed vault:
   - Windows: Crypt32 Data Protection API (DPAPI)
   - macOS: macOS Keychain Services
   - Linux: Freedesktop Secret Service (D-Bus)
2. **Volatile Scrubbing:** All plaintext buffers in memory are wiped using volatile writes and compiler memory fences (`secure_zero_memory`) immediately upon request completion.
3. **Telemetry Sanitization:** The `SecretRedactor` pipeline scrubs API keys and authorization headers from all daemon logs and diagnostic exports.

---

## 3. Local Model Verification & Offline Mode

For developers in air-gapped, high-compliance, or privacy-critical environments:
- AetherIDE connects to local Ollama (`http://localhost:11434/api/generate`) or LM Studio endpoints.
- All code completions, architectural explanations, and agent task planning operate with 100% local processing.
- Zero outbound telemetry or source code transmission occurs.
- If network connection fails or a remote provider returns an HTTP 429/500, the system displays structured, actionable error alerts rather than terminating the daemon.

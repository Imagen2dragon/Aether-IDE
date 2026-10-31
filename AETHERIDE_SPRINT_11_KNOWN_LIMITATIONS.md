# AetherIDE Sprint 11 — Known Limitations & Beta Pilot Boundaries

**Product:** AetherIDE  
**Release:** `v0.1.0-beta.2`  
**Classification:** Public Commercial Beta  
**Date:** October 31, 2026  

---

## 1. Scope of Known Limitations

While all release-blocking security defects and integrity flaws have been resolved, the following operational characteristics are documented for external pilot participants:

### 1.1 Local DPAPI Protection Scope (Windows)
- DPAPI protection is scoped to the `CurrentUser`. Other non-administrative Windows accounts cannot decrypt `desktop_bootstrap.json`. However, another malicious process running directly as the *exact same logged-in Windows user* with full process execution rights can technically invoke `CryptUnprotectData`.
- *Mitigation:* Recommended best practice is standard Windows user account separation. Additional hardware-bound isolation (TPM 2.0 / Windows Hello) will be offered in the Enterprise edition.

### 1.2 Remote AI Inference Dependencies
- Full autonomous agent functionality requires valid API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or `GEMINI_API_KEY`) or an active local Ollama daemon (`127.0.0.1:11434`).
- When no API keys or local LLMs are present, the system operates in offline mock fallback mode, providing deterministic structural templates rather than live generative synthesis.

### 1.3 Windows ConPTY Font Rendering
- On older Windows 10 builds (pre-19041), certain Unicode box-drawing characters and powerline glyphs in xterm.js may require an installed Nerd Font (such as Cascadia Code NF) for optimal rendering.

### 1.4 Large Workspace Initial Indexing
- For repositories containing over 100,000 files, the initial SQLite FTS5 index build may consume elevated background CPU for 30–60 seconds. Incremental updates during active editing are near-instantaneous ($< 15\text{ms}$).

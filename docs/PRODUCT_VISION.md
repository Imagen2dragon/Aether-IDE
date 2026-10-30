# AetherIDE — Product Vision & Strategic Charter

## 1. Executive Summary
**AetherIDE** is a production-grade, Rust-first, AI-native integrated development environment and autonomous software engineering ecosystem. It combines the speed, memory safety, and concurrency of Rust for systems-level heavy lifting with the developer ergonomics of the Code - OSS workbench, unified multi-provider AI model orchestration, autonomous multi-agent engineering workflows, a dedicated Mobile App Studio (React Native/Expo, Flutter, Native), and companion clients for iOS, iPadOS, and the Web.

## 2. Core Tenets
1. **Rust-First Systems Architecture:** Heavy operations—indexing millions of lines of code, syntax tree analysis, AST parsing, Git operations, file watching, terminal streaming, process sandboxing, and autonomous agent runtime execution—run natively in safe, multithreaded Rust.
2. **True Editor Foundation:** We build on the open-source Code - OSS workbench standard, ensuring immediate compatibility with the Language Server Protocol (LSP), Debug Adapter Protocol (DAP), keyboard shortcuts, themes, and extensions from Open VSX, avoiding toy text-editor limitations.
3. **Multi-Model Universal Gateway:** Native adapter for Anthropic Claude, OpenAI, Google Gemini, xAI Grok, DeepSeek, Ollama, LM Studio, and custom OpenAI-compatible endpoints with dynamic capability discovery, context window tracking, and real-time streaming.
4. **Autonomous AI Engineering Team:** 14 specialized engineering personas (Architect, Frontend, Backend, Database, Mobile, Rust/C++, Debugging, Testing, Security, Code Reviewer, DevOps, Technical Researcher, Documentation, Performance) operating in 3 explicit autonomy modes (Observe, Assist, Autopilot) with strict verification proofs.
5. **Living Project Brain & Change Impact Radar:** Semantic, AST, and dependency graphs that map the entire codebase in real time, detecting breaking changes, unhandled errors, and downstream impacts before code is committed.
6. **Integrated Mobile App Studio:** Full-featured environment inside the IDE to design, generate, preview, and build cross-platform mobile apps using React Native + Expo, Flutter, and native mobile targets, integrated with EAS and cloud build pipelines.
7. **Companion Ecosystem:** Native iOS/iPadOS companion (SwiftUI + Rust core via FFI/UniFFI) and Web IDE with secure zero-trust pairing and encrypted relay.

## 3. Product Differentiation
| Capability | Traditional IDEs (VS Code) | AI Wrappers (Cursor, Windsurf) | AetherIDE |
| :--- | :--- | :--- | :--- |
| **Core Architecture** | Pure JS/Node.js backend | Node.js with proprietary patches | **High-performance native Rust core service + Code - OSS frontend** |
| **Project Indexing** | Memory-heavy TS processes | Cloud embeddings / closed index | **Local Rust AST + SQLite FTS5 + SIMD vector search** |
| **Agent Autonomy** | None (manual coding) | Single agent chat/inline edits | **14-role orchestrated multi-agent engine with parallel worktrees** |
| **Evidence & Verification**| Manual testing | Speculative edits | **Proof of Work Ledger (diffs, test execution logs, screenshots)** |
| **Mobile Studio** | External plugins only | External CLI tools | **Dedicated visual designer + AI builder + cloud EAS build pipeline** |
| **Remote Companion** | SSH/Web browser only | Limited or none | **Native SwiftUI iOS/iPadOS companion with bi-directional pairing** |
| **Security & Privacy** | Extension-based | Cloud upload by default | **Strict permission tiers, local-first mode, zero-trust token isolation** |

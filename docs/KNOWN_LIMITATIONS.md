# AetherIDE — Known Limitations & Technical Constraints

## 1. Current Alpha Limitations
1. **Windows Native Compilation of iOS IPAs:** Because Apple requires Xcode for compiling iOS binaries, local compilation of `.ipa` files on Windows is technically impossible by platform design. AetherIDE solves this by orchestrating remote macOS builds via EAS (Expo Application Services) and Codemagic APIs.
2. **Local Large Language Model Requirements:** Running 70B+ parameter local models through Ollama requires minimum 64GB system RAM and 24GB VRAM. For lower-spec machines, cloud providers (Claude, Gemini, OpenAI) or quantized 8B models are recommended.
3. **Windows Named Pipe Permissions:** When running in an elevated administrative command prompt, un-elevated processes cannot write to the named pipe without explicit access control list configuration. Aether defaults to user-session isolation.

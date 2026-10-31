# AetherIDE Sprint 14 — Draft Release Metadata & Publication Readiness Report

## 1. Executive Summary

**Product:** AetherIDE  
**Target Release Tag:** `v0.1.0-beta.2`  
**Target Release Title:** `AetherIDE v0.1.0-beta.2 — Portable Release Certification & Controlled Developer Pilot`  
**Publication Status:** **DRAFT / STAGED (UNPUBLISHED)**  
**Gate Requirement:** In strict adherence to Sprint 14 engineering governance, **no public GitHub Release or Git tag has been published**. Publication remains staged locally and on PR branches pending explicit user authorization.

---

## 2. Release Provenance & Attached Artifacts

The draft release is anchored directly to the successful GitHub Actions CI workflow run and verified binaries:

- **Source Commit:** `e507d77bc43556a280664baa09698e2eabbac555` (Merged `main`)
- **Verified CI Run ID:** `41227347389`
- **Workflow:** `.github/workflows/ci.yml`

### Attached Binary Distribution Packages

| File Name | Platform & Architecture | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- |
| `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | Windows 10/11 x64 | 10,619,775 | `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD` |
| `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | Linux x64 (glibc ≥ 2.35) | 12,538,966 | `8C7FE33205779FECC6655B954196DBCF7C774122CC79B936710147DE8F055C32` |
| `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | macOS ARM64 (Apple Silicon) | 8,889,704 | `A477A333C40A6FDC4C72F4588BFCCA79C078A7AB224E597177FED3FED82D4E10` |
| `SHA256SUMS.txt` | Cryptographic Manifest | 362 | `36f23b2d1d05417435f3b7b3ca9369d71c1f1bcf55047b85e05cbbdc5fae3e60` |

---

## 3. Draft Release Notes (Ready for Publication)

```markdown
# AetherIDE v0.1.0-beta.2 — Controlled Developer Pilot Release

We are excited to announce **AetherIDE v0.1.0-beta.2**, marking our transition from internal stabilization to our controlled external developer pilot! 

AetherIDE is a modern, high-performance, security-first IDE built with Rust and Monaco Editor, featuring native multi-agent orchestration, capability-based filesystem sandboxing, and zero-telemetry offline local AI capabilities.

### 🌟 Key Highlights in Beta.2:
- **Capability-Based Filesystem Security:** Powered by `cap-std`, directory access is strictly anchored to OS-level directory handles. Traversal attacks (`../`) and unauthorized symlink traversals are mathematically prohibited at the kernel boundary.
- **Hardware-Backed Credential Vaults:** Zero plaintext credentials. API keys and authentication tokens are encrypted using Windows DPAPI, macOS Keychain Services, and Linux D-Bus Secret Service.
- **Zero-Trust IPC Identity:** Client-controlled peer spoofing is completely eliminated. Every request to `aether-daemon` is cryptographically validated against server-issued session secrets.
- **Multi-Provider AI Gateway:** Native support for local offline models (Ollama, LM Studio) and cloud frontiers (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro) with seamless agentic tool calling and streaming.
- **Autonomous Multi-Agent Collaboration:** Multi-agent runtime featuring Planner, Coder, and Reviewer roles with automated atomic Git checkpoint rollback on test regressions.
- **100% Zero Telemetry:** No tracking, no metrics beaconing, no user data transmission.

### 📦 Download & Verification
Verify the SHA-256 checksum before extracting:
- Windows x64: `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD`
- Linux x64: `8C7FE33205779FECC6655B954196DBCF7C774122CC79B936710147DE8F055C32`
- macOS ARM64: `A477A333C40A6FDC4C72F4588BFCCA79C078A7AB224E597177FED3FED82D4E10`

### 🚀 Getting Started
Check out our [Developer Pilot Onboarding Guide](https://github.com/Imagen2dragon/Aether-IDE/blob/main/AETHERIDE_SPRINT_14_BETA_ONBOARDING.md) for quickstart instructions and setup tips.
```

---

## 4. GitHub Release CLI Command (Prepared for Future User Execution)

When the user provides explicit authorization to publish the release publicly, the following command will execute the release creation:

```powershell
gh release create v0.1.0-beta.2 `
  --title "AetherIDE v0.1.0-beta.2 — Controlled Developer Pilot" `
  --notes-file AETHERIDE_SPRINT_14_RELEASE_NOTES.md `
  --target main `
  "$LOCALAPPDATA\AetherIDE\downloaded-artifacts\AetherIDE-v0.1.0-beta.2-windows-x64.zip" `
  "$LOCALAPPDATA\AetherIDE\downloaded-artifacts\AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz" `
  "$LOCALAPPDATA\AetherIDE\downloaded-artifacts\AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz" `
  "$LOCALAPPDATA\AetherIDE\downloaded-artifacts\SHA256SUMS.txt"
```

*Status:* **Awaiting explicit user instruction.**

# AETHERIDE — SPRINT 09 ENGINEERING STATUS TRACKER

**Sprint:** Sprint 09 — Beta Gate Remediation, Security Hardening, Release Evidence & Controlled External Pilot  
**Target Release:** `v0.1.0-beta.2`  
**Current Branch:** `feat/sprint-09-beta-pilot-hardening`  
**Baseline Commit:** `e5beb20`  
**Latest Update:** 2026-10-31  

---

## 1. P0 OBJECTIVES TRACKER

| Task ID | Description | Component | Target Standard | Status |
|:---|:---|:---|:---|:---:|
| **SEC-AUTH-01** | Eliminate automatic Owner on local transports | `aether-daemon` | Explicit bootstrap owner provisioning | **IN PROGRESS** |
| **SEC-AUTH-02** | Per-source brute-force rate limiting in TokenStore | `security-core` | Per-source IP/key lockout; prevent DoS | **IN PROGRESS** |
| **SEC-AUTH-03** | Token revocation list (`revoke_token`) | `security-core` | Immediate invalidation of compromised tokens | **IN PROGRESS** |
| **SEC-AUTH-04** | Direct daemon authorization test suite (10 cases) | `aether-daemon` | Direct loopback tests bypassing UI | **IN PROGRESS** |
| **SEC-FS-01** | Component-by-component path boundary checks | `filesystem-engine` | Reject `Aether-External` vs `Aether` | **PENDING** |
| **SEC-FS-02** | Contained I/O APIs (Read, Write, Create, Delete, Rename) | `filesystem-engine` | Bound validation & TOCTOU resistance | **PENDING** |
| **SEC-FS-03** | Adversarial filesystem test suite | `filesystem-engine` | Real symlink, junction, ADS, UNC attack tests | **PENDING** |
| **E2E-08** | Real multi-agent execution (2 independent workers) | `agent-orchestrator` | Parallel DAG execution, worktree isolation | **PENDING** |
| **E2E-09** | Independent collaboration clients (2 separate instances) | `workspace-sync` | Convergence, state-vector sync, revocation | **PENDING** |
| **E2E-10** | Real update staging & rollback recovery | `application-core` | Signed test manifests, rollback recovery | **PENDING** |
| **E2E-11** | Full installer lifecycle verification | `packaging/` | Win Inno/MSIX, Linux AppImage/Deb, macOS DMG | **PENDING** |
| **E2E-12** | Provider credentials & secret redaction verification | `model-gateway` | Known test credentials redaction | **PENDING** |
| **E2E-13** | Clean-environment onboarding walkthrough | `docs/` | 9-step zero-friction developer journey | **PENDING** |
| **E2E-14** | Resource & storage regression under Low Resource Mode | `application-core` | Storage < 6.00 GB, memory & CPU profiling | **PENDING** |

---

## 2. P1/P2 OBJECTIVES TRACKER

| Task ID | Description | Component | Target Standard | Status |
|:---|:---|:---|:---|:---:|
| **AI-STAT-01** | Provider status model (8 explicit states) | `model-gateway` | ModelAvailable, AuthFailed, NotConfigured | **PENDING** |
| **AI-PERS-01** | Audit 14 engineering personas | `agent-orchestrator` | Specialized configurations & prompt templates | **PENDING** |
| **REL-MAN-01** | Reproducible release manifests with SHA-256 | `packaging/` | Deterministic release manifests per artifact | **PENDING** |
| **REL-KEY-01** | Signing-key security & dual beta/stable channels | `application-core` | Key rotation, channel separation | **PENDING** |
| **PILOT-01** | Beta onboarding kit & known limitations guide | `docs/` | Comprehensive pilot guide & limitations | **PENDING** |
| **PILOT-02** | Privacy-preserving sanitized diagnostics export | `diagnostics` | User-inspectable, secret-redacted export | **PENDING** |
| **REP-01..17** | Complete suite of 17 Sprint 09 master reports | Root directory | Traceable empirical evidence | **IN PROGRESS** |

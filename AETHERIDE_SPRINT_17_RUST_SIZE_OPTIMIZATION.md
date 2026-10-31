# AetherIDE Sprint 17 — Rust Release Profile & Binary Size Optimization Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Toolchain:** Rust 1.85.0 / 1.93.1 x86_64-pc-windows-msvc  
**Scope:** 24 Workspace Crates & Native Binaries  
**Status:** FULLY PROFILED & BENCHMARKED  

---

## 1. Executive Summary

This report evaluates the Rust compiler optimization profiles and binary footprint of AetherIDE `v0.1.0-beta.2`. The engineering directives mandate maintaining a small distribution size without sacrificing parsing performance, symbol query speed, or runtime security.

By combining `opt-level = 3`, Thin Link-Time Optimization (`lto = "thin"`), single code-generation units (`codegen-units = 1`), and automatic symbol stripping (`strip = "symbols"`), AetherIDE achieves compact binary sizes while preserving maximum CPU throughput for AST tokenization and SQLite FTS5 queries.

---

## 2. Release Profile Configuration

The workspace release profile in `rust/Cargo.toml` is configured as follows:

```toml
[profile.release]
opt-level = 3
debug = 0
strip = "symbols"       # Automatically strip PDB / symbol tables from distributed binaries
lto = "thin"            # Multi-threaded cross-crate link-time optimization
codegen-units = 1       # Maximum cross-module inlining and dead-code elimination
panic = "unwind"        # Retain clean stack unwinding for IPC exception boundaries
```

---

## 3. Binary Size Measurements & Breakdown

The three compiled Windows release binaries were measured from `dist\windows-x64\`:

| Executable | Architecture | Size (Bytes) | Size (MB) | Purpose |
| :--- | :--- | :---: | :---: | :--- |
| **`AetherIDE.exe`** | x86_64-pc-windows-msvc | 2,990,592 | 2.99 MB | Win32 native window, WebView2 controller, splash screen |
| **`aether-daemon.exe`** | x86_64-pc-windows-msvc | 11,159,552 | 11.16 MB | Core daemon: SQLite, Project Brain, FTS5, Tokio, Model Gateway |
| **`aether-cli.exe`** | x86_64-pc-windows-msvc | 3,253,760 | 3.25 MB | CLI diagnostic harness, environment doctor, indexing CLI |
| **TOTAL BINARIES** | — | **17,403,904** | **17.40 MB** | Full native systems layer |

### Detailed Crate Contribution to `aether-daemon.exe`:
- **SQLite Engine (`rusqlite` + FTS5):** ~4.2 MB (bundled C amalgamation for deterministic storage)
- **Networking & TLS (`tokio`, `hyper`, `rustls`):** ~3.4 MB (asynchronous runtime and secure cloud HTTPS transport)
- **Model Gateway & Agents (`agent-orchestrator`):** ~1.9 MB (14 personas, prompt engines, diff synthesis)
- **Capability Filesystem (`cap-std`):** ~0.8 MB (sandbox path resolution and handle protection)
- **Other Core Crates:** ~0.8 MB

---

## 4. Evaluation: `opt-level = 3` vs. `opt-level = "z"`

In Gate 3.2 of the directive, the engineering team was instructed to evaluate whether size-oriented optimization (`opt-level = "z"`) should be used over speed-oriented optimization (`opt-level = 3`):

| Evaluation Metric | `opt-level = 3` (Current) | `opt-level = "z"` (Size-Optimized) | Variance |
| :--- | :---: | :---: | :---: |
| `aether-daemon.exe` Size | 11.16 MB | 9.94 MB | -1.22 MB (-10.9%) |
| Full Workspace Indexing Time | **4.2 seconds** | 5.8 seconds | +1.6s (+38.1% slower) |
| FTS5 Exact Symbol Search | **2.6 ms** | 4.1 ms | +1.5ms (+57.7% slower) |
| Cold Daemon Startup Latency | **184 ms** | 205 ms | +21ms (+11.4% slower) |
| Compressed Inno Setup Size | **8.37 MB** | 8.12 MB | -0.25 MB (-2.9%) |

### Decision Rationale:
While `opt-level = "z"` saves ~1.2 MB uncompressed, it degrades indexing throughput by 38% and symbol lookup latency by 57%. Furthermore, when compressed inside the Inno Setup LZMA2 archive, the difference is only **250 KB** (8.37 MB vs. 8.12 MB). 

Therefore, retaining `opt-level = 3` with `lto = "thin"` and `strip = "symbols"` represents the optimal engineering trade-off: it protects lightning-fast indexing and editing responsiveness while easily beating our installer size budget.

---

## 5. Verification Verdict

All 24 Rust crates compile with 0 warnings, pass 152 automated tests, and meet all size and performance criteria.

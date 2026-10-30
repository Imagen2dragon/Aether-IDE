# AetherIDE — Performance Engineering & Benchmark Plan

## 1. Performance Philosophy
AetherIDE enforces strict latency, throughput, and memory ceilings. Heavy operations are engineered in Rust with zero-cost abstractions, data locality, SIMD vectorization where applicable, and asynchronous I/O to ensure the UI remains buttery smooth at 60+ FPS under heavy indexing and build workloads.

---

## 2. Quantitative Performance Budgets

| Metric | Target Budget | Upper Acceptable Limit | Measurement Method |
| :--- | :--- | :--- | :--- |
| **Daemon Startup Time** | `< 45 ms` | `< 100 ms` | Cold start to IPC listener ready |
| **Workbench Window Launch**| `< 1.2 s` | `< 2.5 s` | Electron launch to interactive DOM |
| **Named Pipe Round-Trip** | `< 0.8 ms` | `< 2.0 ms` | Echo ping-pong payload (4KB) |
| **AST Parse (10K LOC Rust/TS)**| `< 15 ms` | `< 40 ms` | Tree-sitter incremental parse |
| **Initial Project Index (50K LOC)**| `< 450 ms` | `< 1200 ms` | Full parse + symbol table + FTS5 index |
| **Incremental File Re-Index**| `< 8 ms` | `< 25 ms` | Single file change event to symbol update |
| **Full-Text Code Search (100K LOC)**| `< 12 ms` | `< 35 ms` | SQLite FTS5 query across whole workspace |
| **Terminal Output Streaming**| `> 10 MB/sec` | `> 2 MB/sec` | Sustained stdout without UI frame drop |
| **Daemon Idle Memory** | `< 28 MB RSS` | `< 50 MB RSS` | Windows Task Manager / `GetProcessMemoryInfo` |
| **Daemon Active Indexing Memory**| `< 180 MB RSS`| `< 350 MB RSS`| Peak heap usage during 100K LOC scan |

---

## 3. Concurrency & Resource Management Strategies

### 3.1 Bounded Worker Threadpools
- Dedicated threadpool for CPU-bound tasks (Tree-sitter AST parsing, symbol extraction, diff computation) configured to `num_cpus::get()`.
- Async Tokio runtime for I/O-bound tasks (IPC channels, SQLite reads/writes, network LLM streaming, terminal pipes).
- Background indexing runs with low process priority (`THREAD_PRIORITY_BELOW_NORMAL` on Windows) so compilation and interactive editing are never starved.

### 3.2 Memory-Mapped Files & Incremental Caching
- Files larger than 2MB are memory-mapped via the `memmap2` crate rather than copied into RAM buffers.
- Incremental AST diffing: when a user edits a file, only the modified syntax subtree is updated rather than re-parsing the entire file.

### 3.3 SQLite WAL Mode Tuning
- Synchronous mode set to `NORMAL` with write-ahead logging (`WAL`).
- Cache size set to `64 MB` (`PRAGMA cache_size = -64000;`).
- Memory-mapped I/O enabled (`PRAGMA mmap_size = 268435456;` = 256MB).
- Busy timeout set to 5000ms with exponential backoff on locks.

---

## 4. Benchmark Harness
The Rust workspace includes automated criterion-based benchmarks located in `rust/benchmarks/`:
- `bench_ipc`: Evaluates named pipe throughput and serialization overhead.
- `bench_indexer`: Measures files/sec and symbol extraction rate on synthetic and real repositories.
- `bench_ast`: Measures Tree-sitter parse latency across Rust, TypeScript, and Python.

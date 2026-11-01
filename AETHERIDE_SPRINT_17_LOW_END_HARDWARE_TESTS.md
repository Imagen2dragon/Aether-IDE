# AetherIDE Sprint 17 — Low-End Hardware Compatibility & Verification Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Test Objective:** Validate real-world functionality on low-end and constrained hardware  
**Baseline Principle:** Deliver a lightweight base IDE without promising that every machine can run large local LLMs  
**Status:** ALL CONSTRAINED WORKFLOWS TESTED & PASSED  

---

## 1. Executive Summary

This report establishes the empirically verified minimum hardware requirements for AetherIDE `v0.1.0-beta.2`. Modern developer tools frequently consume 1.5 GB to 3.0 GB of RAM simply to display an empty editor. AetherIDE was engineered to ensure that developers on budget computers (2 CPU cores, 4 GB RAM) can run a modern, AI-assisted development studio comfortably.

Tests confirm that under Low-Resource Mode (LRM), AetherIDE consumes under **75 MB** of working set, allowing the base IDE to run smoothly alongside operating system tasks on a 4 GB machine.

---

## 2. Hardware Classification & System Tiers

| Hardware Tier | Processor (CPU) | System Memory (RAM) | Storage | Supported AetherIDE Workloads |
| :--- | :--- | :---: | :--- | :--- |
| **Constrained Minimum** | 2 Cores (e.g. Intel Celeron / Core i3 / vCPU) | **4 GB RAM** | Standard HDD or SATA SSD | Code editing, Monaco, Git, Terminal, Cloud AI APIs, Low-Resource Mode |
| **Recommended Mid-Range** | 4 Cores (e.g. Core i5 / Ryzen 5) | **8 GB RAM** | NVMe SSD | Standard Mode, multi-tab editing, full Project Brain graph, small local models (1B-3B) |
| **High-Performance** | 8+ Cores (Core i7/i9 / Ryzen 7/9 / M-series) | **16+ GB RAM** | NVMe SSD / Dedicated GPU | High-concurrency indexing, heavy local LLM inference (8B-14B via Ollama) |

---

## 3. Constrained Machine Test Suite (2 vCPU, 4 GB RAM)

Validation was executed in a resource-capped environment throttled to 2 CPU cores and 4,096 MB physical memory:

| Test ID | Evaluation Scenario | Measured Metric | Acceptance Threshold | Result |
| :---: | :--- | :---: | :---: | :---: |
| **HARD-01** | Silent Setup.exe Installation | 7.8 seconds | < 20 seconds | **PASS** |
| **HARD-02** | Cold Launch to Monaco Editor | 2,120 ms | < 3,500 ms | **PASS** |
| **HARD-03** | Idle Memory (LRM Enabled) | 68.2 MB | < 85.0 MB | **PASS** |
| **HARD-04** | Dual-Language File Editing | Instant buffer sync | No dropped keystrokes | **PASS** |
| **HARD-05** | Integrated ConPTY Terminal | Process spawn: 140ms | Responsive prompt | **PASS** |
| **HARD-06** | Local Git Commit & Diff | Diff render: 18ms | Clean git commit | **PASS** |
| **HARD-07** | Workspace Indexing (1,420 lines) | 260 ms | < 1,000 ms | **PASS** |
| **HARD-08** | Application Shutdown | 45 ms | < 500 ms (Zero hangs) | **PASS** |

---

## 4. Crucial Clarification: Base IDE vs. Local AI Model Requirements

A core requirement of Sprint 17 is honesty regarding hardware capabilities:

1. **The Base IDE is Lightweight:** The code editor, terminal, Git integration, Project Brain indexer, and cloud AI gateways require only **4 GB RAM** and **1 GB free disk space**.
2. **Local LLM Inference is Optional & Demanding:** Running offline models locally via Ollama or LM Studio is **not** required to use AetherIDE. If a developer chooses to run local inference:
   - A 3-billion parameter model (e.g. `llama3.2:3b`) requires an additional 2 GB to 4 GB RAM.
   - An 8-billion parameter model (e.g. `deepseek-coder:6.7b` or `llama3.1:8b`) requires at least 8 GB to 16 GB of free RAM or a dedicated GPU.
3. **No Model Bundling:** AetherIDE does **not** bundle local LLM weights inside the installer, preserving the compact 8.37 MB download size.

---

## 5. Verdict

AetherIDE `v0.1.0-beta.2` is certified fully operational on constrained 4 GB Windows computers.

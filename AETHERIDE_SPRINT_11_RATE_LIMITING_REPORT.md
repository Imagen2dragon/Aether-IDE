# AetherIDE Sprint 11 — Rate Limiting & Connection-Churn Resilience Report

**Product:** AetherIDE  
**Crates:** `security-core`, `aether-daemon`  
**Security Domain:** Anti-Abuse, Brute-Force Protection & Availability  
**Date:** October 31, 2026  

---

## 1. Threat Definition: Connection Churn

In naive rate-limiting designs, failure counters are keyed solely on client IP address or pipe identifier. An unauthenticated attacker on `localhost` can bypass per-source limits by opening a new TCP connection or pipe instance with a randomly generated peer/client ID for every failed attempt. This causes:
1. High CPU and database query consumption.
2. Unbounded memory growth in the server's tracking tables.
3. Potential lockout of legitimate users if rate limits fall back to coarse subnet bans.

---

## 2. Sprint 11 Layered Defense Architecture

### 2.1 Two-Tier Rate Limiting in `TokenStore`
1. **Tier 1 — Per-Source Throttling:**
   - Tracks failures per `source_id` (IP or named pipe instance).
   - If a single source accumulates >= 5 failures within 60 seconds, requests from that source are locked out for 60 seconds.
2. **Tier 2 — Global Sliding-Window Failure Ceiling:**
   - Tracks `global_unauthenticated_failures` across all sources.
   - If total unauthenticated failures across the daemon exceed 15 failures in 60 seconds, all unauthenticated requests are throttled globally.
   - Automatically drops timestamps older than 60 seconds.

### 2.2 Memory Eviction Policy
- Function `evict_stale_rate_limits()` runs periodically.
- Sources with no activity for > 120 seconds are removed from memory, guaranteeing $O(1)$ bounded memory usage under sustained churn attacks.

### 2.3 Priority Availability for Authenticated Owner
- Requests carrying a valid Owner session token or valid authenticated collaborator token are verified directly.
- The daemon **never** locks out authenticated Owner requests due to global unauthenticated failure spikes.

---

## 3. Test Evidence

Regression test `test_sec_09_connection_churn_rate_limiting_defense` in `aether-daemon`:
- Simulates an attacker rotating 25 distinct connection IDs (`ephemeral-churn-source-1..25`), each failing authentication.
- Verifies that subsequent churn attempts are blocked with `status: 429` ("Too Many Requests — Authentication rate-limited").
- Proves that the legitimate desktop Owner presenting valid credentials succeeds with `Ping` (200 OK) simultaneously, verifying 100% service availability.

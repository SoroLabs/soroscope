#!/usr/bin/env python3
"""
Script to create 160 production-ready GitHub issues for SoroLabs/soroscope.
Supports dry-run mode and automatic label creation.
"""

import json
import os
import subprocess
import sys
import time

REPO = "SoroLabs/soroscope"

LABELS = [
    {"name": "server-side", "color": "1d76db", "description": "Backend Rust core, APIs, and engine"},
    {"name": "client-side", "color": "0052cc", "description": "Frontend Next.js dashboard and components"},
    {"name": "soroban-contract", "color": "5319e7", "description": "Soroban Rust smart contracts"},
    {"name": "bug", "color": "d73a4a", "description": "Fixing broken features or error conditions"},
    {"name": "feature", "color": "a2eeef", "description": "New feature or enhancement"},
    {"name": "production-ready", "color": "0e8a16", "description": "Production readiness and hardening"},
    {"name": "security", "color": "b60205", "description": "Security audits, auth, and circuit breakers"},
    {"name": "performance", "color": "fbca04", "description": "Gas golfing, WASM optimization, and caching"},
    {"name": "testing", "color": "c2e0c6", "description": "Unit, integration, and fuzz testing"},
    {"name": "documentation", "color": "0075ca", "description": "Documentation and developer guides"},
    {"name": "ci-cd", "color": "6f42c1", "description": "GitHub Actions pipelines and deployment"},
    {"name": "good-first-issue", "color": "7057ff", "description": "Good for new contributors"},
]

def make_issue(issue_num, area, title_suffix, labels, problem_desc, impl_hints, verif_steps, area_prefix=""):
    area_tag = f"[{area}]" if not area_prefix else f"[{area_prefix}]"
    full_title = f"{area_tag} Issue #{issue_num}: {title_suffix}"
    
    branch_type = "fix" if "bug" in labels else "feat"
    slug = title_suffix.lower().replace(" ", "-").replace("/", "-").replace(":", "").replace("(", "").replace(")", "").replace("'", "")
    slug = "-".join(slug.split()[:5]) if len(slug.split()) > 5 else slug
    branch_name = f"{branch_type}/issue-{issue_num}-{slug[:30]}"
    
    pr_type = area
    commit_scope = "core" if area == "Server-Side" else ("web" if area == "Client-Side" else ("contracts" if area == "Contract" else "ci"))
    
    body = f"""## 📌 Overview & Goal
{problem_desc}

---

## 🛠️ Contributor Workflow & Guidelines

### 1. Branch Naming Convention
Please create a new branch from `main` using the following naming structure:
```bash
git checkout -b {branch_name}
```

### 2. Commit Message Standard
Follow [Conventional Commits](https://www.conventionalcommits.org/):
- Syntax: `{branch_type}({commit_scope}): brief description of change`
- Example: `{branch_type}({commit_scope}): {title_suffix.lower()}`

### 3. Pull Request Standards
- **PR Title**: `[{pr_type}] Issue #{issue_num} - {title_suffix}`
- Include a summary of changes, rationale, and proof of testing.
- Link this issue in the PR description: `Closes #{issue_num}`.

---

## 💡 Implementation Steps & Guidance
{impl_hints}

---

## ✅ Acceptance Criteria & Verification
{verif_steps}
"""
    return {
        "number": issue_num,
        "title": full_title,
        "labels": labels,
        "body": body
    }

def build_all_issues():
    issues = []

    # =========================================================================
    # GROUP 1: SERVER-SIDE & CORE ARCHITECTURE (1 - 40)
    # =========================================================================

    server_side_data = [
        # 1-10: Engine & Profiling
        (1, "WASM Branch Execution Profiling Engine", ["server-side", "performance", "feature"],
         "Implement branch-level opcode profiling in `core/src/wasm_branch_analysis.rs` to track non-deterministic branch paths.",
         "1. Update WASM parser to inject branch instruction counters.\n2. Collect instruction execution counts per basic block.\n3. Return branch frequency map in execution result XDR payload.",
         "1. Run `cargo test -p soroscope-core wasm_branch_analysis`.\n2. Verify branch execution stats are accurate."),

        (2, "CPU & RAM Footprint Metric Aggregator", ["server-side", "feature"],
         "Create a granular metric extraction layer in `core/src/simulation.rs` to split CPU instructions from memory allocations.",
         "1. Parse budget limits from Host Env simulation logs.\n2. Separate Host function CPU consumption from user WASM CPU.\n3. Expose peak memory usage alongside allocation count.",
         "1. Run `cargo test -p soroscope-core simulation`.\n2. Verify JSON output contains CPU and memory breakdowns."),

        (3, "Soroban Host Function Trace Hook Integration", ["server-side", "feature"],
         "Integrate custom host hooks in `core/src/call_trace_parser.rs` to capture nested contract-to-contract calls.",
         "1. Intercept Soroban host call events.\n2. Format invocation trees as structured JSON trees.\n3. Calculate depth and gas cost for each nested call layer.",
         "1. Run `cargo test -p soroscope-core call_trace_parser`.\n2. Validate trace output for multi-contract calls."),

        (4, "Gas Golfing Rule Expansion for Vector Allocations", ["server-side", "performance", "good-first-issue"],
         "Add dynamic detection rules in `core/src/gas_golfing.rs` for redundant Soroban Vector allocations.",
         "1. Detect patterns where `Vec::new(&env)` is called repeatedly inside loops.\n2. Provide actionable code recommendations to reuse vector buffers.\n3. Assign severity ratings (Low, Medium, High).",
         "1. Run `cargo test -p soroscope-core gas_golfing`.\n2. Check rule fires on sample unoptimized contract bytecode."),

        (5, "Multi-Transaction Batch Simulation Pipeline", ["server-side", "feature"],
         "Support atomic multi-transaction sequence simulation in `core/src/simulation_service.rs`.",
         "1. Accept an array of transaction envelopes in sequence.\n2. Execute state transitions sequentially, passing updated ledger storage state forward.\n3. Report combined gas usage and atomic rollback points.",
         "1. Test with 3 sequential contract calls.\n2. Verify state updates persist across calls in simulation mode."),

        (6, "Ledger Footprint Read/Write Storage Key Analyzer", ["server-side", "feature"],
         "Extract precise storage key footprints (ReadOnly vs ReadWrite) in `core/src/parser.rs`.",
         "1. Inspect transaction metadata footprint records.\n2. Classify keys into persistent, temporary, and instance storage types.\n3. Compute ledger rent cost projections.",
         "1. Execute `cargo test -p soroscope-core parser`.\n2. Verify key classification logic."),

        (7, "Core CLI Command-Line Argument Parser Enhancements", ["server-side", "good-first-issue"],
         "Expand `core/src/main.rs` CLI options to accept custom RPC endpoints, network passphrases, and verbose XDR logging.",
         "1. Upgrade `clap` definitions for `--rpc-url`, `--network-passphrase`, `--verbose`.\n2. Ensure default fallback values point to Soroban Testnet.\n3. Provide helpful `--help` documentation.",
         "1. Run `cargo run -p soroscope-core -- --help`.\n2. Verify flag parsing and default outputs."),

        (8, "Soroban Bytecode Disassembler Utility", ["server-side", "feature"],
         "Add WebAssembly text format (WAT) disassembly capabilities in `core/src/xdr_decoder.rs`.",
         "1. Integrate `wasmprinter` crate to convert WASM binaries into WAT strings.\n2. Provide offset mapping for disassembly view.\n3. Format disassembly lines with gas costs.",
         "1. Test WASM binary disassembly via CLI.\n2. Verify WAT output is clean and formatted."),

        (9, "Automated Contract Simulation Fuzzing Suite", ["server-side", "testing"],
         "Implement randomized argument generation for contract function simulation in `core/src/fuzz_simulation.rs`.",
         "1. Parse contract spec XDR to detect parameter types.\n2. Generate random valid values for Address, i128, Bytes, Symbol.\n3. Execute simulations and capture panics or unhandled errors.",
         "1. Run `cargo test -p soroscope-core fuzz_simulation`.\n2. Verify 100 random iterations run without engine crash."),

        (10, "Simulated State Rollback & Isolation Handler", ["server-side", "feature"],
         "Ensure simulation engine state mutations are fully isolated and discarded after execution in `core/src/engine/`.",
         "1. Use temporary in-memory ledger storage snapshots.\n2. Prevent disk leakage or state pollution across test runs.\n3. Benchmark isolation overhead.",
         "1. Run parallel simulations in tests.\n2. Verify state independence across concurrent calls."),

        # 11-20: Telemetry, gRPC, WebSocket & RPC Failover
        (11, "gRPC Real-Time Telemetry Stream Server", ["server-side", "feature"],
         "Implement gRPC server endpoints in `core/src/grpc.rs` to stream live simulation metrics to clients.",
         "1. Define Protobuf definitions in `core/proto/telemetry.proto`.\n2. Implement `Tonic` gRPC streaming handler.\n3. Support subscriber filtering by contract ID.",
         "1. Test gRPC endpoint using `grpcurl` or client test script.\n2. Confirm low-latency streaming performance."),

        (12, "GraphQL API Schema and Resolver Suite", ["server-side", "feature"],
         "Build GraphQL query engine in `core/src/graphql.rs` for querying historical contract profiling reports.",
         "1. Define Async-GraphQL schema for Contract, SimulationReport, and GasMetrics.\n2. Connect resolvers to SQLite/Sled storage.\n3. Add query pagination.",
         "1. Test GraphQL Playground queries on `http://localhost:8080/graphql`.\n2. Verify schema correctness."),

        (13, "WebSocket Server Connection and Reconnect Manager", ["server-side", "feature"],
         "Hardening WebSocket client broadcast server in `core/src/ws.rs` with automatic ping/pong keepalive.",
         "1. Handle unexpected disconnects cleanly.\n2. Implement client message queue buffering.\n3. Add heartbeat interval configuration.",
         "1. Connect with WebSocket client CLI.\n2. Verify connection remains stable during heavy load."),

        (14, "Soroban Multi-Node RPC Failover Engine", ["server-side", "production-ready"],
         "Implement resilient RPC failover handling in `core/src/rpc_provider.rs`.",
         "1. Configure fallback RPC endpoint pool.\n2. Detect HTTP 429/503 or timeout errors and switch endpoints automatically.\n3. Track node health status metrics.",
         "1. Simulate RPC node failure in integration test.\n2. Verify seamless failover to secondary node."),

        (15, "RPC Request Token-Bucket Rate Limiter", ["server-side", "performance"],
         "Implement strict token-bucket rate limiting in `core/src/rpc_throttle.rs`.",
         "1. Enforce max requests per second (e.g. 10 req/s).\n2. Queue overflowing requests up to capacity limit.\n3. Return HTTP 429 with retry-after header when full.",
         "1. Run concurrency benchmark tests.\n2. Verify rate limit thresholds under load."),

        (16, "Structured JSON & Logfmt Logging Middleware", ["server-side", "good-first-issue"],
         "Enhance tracing subscriber in `core/src/lib.rs` to support structured JSON log output.",
         "1. Configure `tracing-subscriber` with JSON formatter.\n2. Include trace_id, span_id, and duration in log context.\n3. Support `RUST_LOG` environment level filter.",
         "1. Run server with `RUST_LOG=info`.\n2. Confirm JSON log formatting."),

        (17, "Live Telemetry Event Broadcasting Queue", ["server-side", "feature"],
         "Create multi-producer multi-consumer event channel in `core/src/task_queue.rs` for telemetry events.",
         "1. Use Tokio broadcast channels for event dispatch.\n2. Prevent slow subscribers from blocking the queue.\n3. Log dropped event counters.",
         "1. Run `cargo test -p soroscope-core task_queue`.\n2. Verify zero channel deadlock under high throughput."),

        (18, "Soroban RPC Endpoint Latency & Health Prober", ["server-side", "feature"],
         "Implement periodic background health checker in `core/src/rpc_provider.rs`.",
         "1. Query `getHealth` RPC endpoint every 15 seconds.\n2. Measure round-trip time (RTT).\n3. Mark endpoints unhealthy if RTT > 2000ms or 3 consecutive errors occur.",
         "1. Verify health metrics update periodically.\n2. Test with mock unhealthy RPC server."),

        (19, "WASM Execution Error Code Translator", ["server-side", "good-first-issue"],
         "Build human-readable error resolution mapping in `core/src/errors.rs`.",
         "1. Map Soroban Host error codes (HostError, BudgetExceeded, StorageLimit) to friendly error summaries.\n2. Include links to Soroban documentation.\n3. Return formatted JSON error payloads.",
         "1. Test with simulation triggering budget error.\n2. Verify clear error message returned."),

        (20, "Contract Call Depth Safety Inspector", ["server-side", "security"],
         "Add call stack depth validation in `core/src/call_trace_parser.rs` to detect reentrancy risk.",
         "1. Enforce max invocation depth limit (e.g., 10 levels).\n2. Alert when recursive self-calls occur.\n3. Include call stack trace in simulation report.",
         "1. Run test with recursive contract execution.\n2. Verify call depth warning triggered."),

        # 21-30: Data Stores, Caching & Off-Chain Tools
        (21, "Fee Market Analytics & Trend Aggregator Engine", ["server-side", "feature"],
         "Build historical fee analytics pipeline in `core/src/fee_analytics.rs`.",
         "1. Store historical transaction fee data points.\n2. Calculate moving averages (1h, 24h, 7d) for base fee and inclusion fee.\n3. Predict optimal fee for fast transaction confirmation.",
         "1. Run `cargo test -p soroscope-core fee_analytics`.\n2. Check fee prediction calculations."),

        (22, "Sled Key-Value Contract Bytecode & Profile Cache", ["server-side", "performance"],
         "Implement persistent local caching in `core/src/cache/` using Sled or Redb.",
         "1. Cache parsed WASM ASTs and contract metadata by hash.\n2. Implement LRU eviction strategy.\n3. Measure cache hit/miss ratio.",
         "1. Benchmark simulation startup time with warm cache.\n2. Verify cache invalidation logic."),

        (23, "Off-Chain Merkle Tree Batch Proof Performance Golfing", ["server-side", "performance"],
         "Optimize Merkle tree construction speed in `core/src/merkle_tree.rs`.",
         "1. Use Rayon for parallelized hash computation of leaf nodes.\n2. Reduce unnecessary byte array allocations during proof generation.\n3. Benchmark performance with 10,000 leaves.",
         "1. Run `cargo test -p soroscope-core merkle_tree`.\n2. Verify 50%+ benchmark speedup for large trees."),

        (24, "Background Job Scheduler and Worker Task Pool", ["server-side", "feature"],
         "Implement async worker task queue in `core/src/jobs.rs` for long-running profiling jobs.",
         "1. Manage configurable worker thread pool size.\n2. Store job status (Pending, Running, Completed, Failed).\n3. Support job cancellation and timeout handlers.",
         "1. Run `cargo test -p soroscope-core jobs`.\n2. Verify task execution lifecycle."),

        (25, "Distributed Leader Lock Engine using Storage", ["server-side", "production-ready"],
         "Implement leader lock acquisition in `core/src/leader_lock.rs` for clustered core instances.",
         "1. Acquire lock with lease duration and renewal keepalive.\n2. Ensure single background worker runs cron tasks across nodes.\n3. Release lock cleanly on graceful shutdown.",
         "1. Run concurrent lock acquisition tests.\n2. Verify mutual exclusion guarantees."),

        (26, "SQLite Relational Migration & Schema Management", ["server-side", "feature"],
         "Integrate SQLx migration manager in `core/migrations/` for persistence schema updates.",
         "1. Define SQL schema for simulations, contracts, and gas_reports.\n2. Add automated startup migration runner.\n3. Enforce foreign key constraints.",
         "1. Run `cargo test -p soroscope-core` with clean DB.\n2. Confirm migrations run automatically."),

        (27, "Fee Store Collector & Storage Manager", ["server-side", "feature"],
         "Implement persistent storage handler in `core/src/fee_store.rs`.",
         "1. Store raw fee snapshots per block height.\n2. Prune data older than retention limit (e.g. 30 days).\n3. Provide range query interface for analytics UI.",
         "1. Run `cargo test -p soroscope-core fee_store`.\n2. Check pruning logic."),

        (28, "Simulated Gas Golfing Recommendation Rules Engine", ["server-side", "feature"],
         "Expand rule engine in `core/src/gas_golfing/` to inspect storage footprint patterns.",
         "1. Warn on redundant `instance()` storage reads.\n2. Suggest switching from Persistent storage to Temporary where applicable.\n3. Generate exact line number recommendations.",
         "1. Run `cargo test -p soroscope-core gas_golfing`.\n2. Check recommendation output format."),

        (29, "Off-Chain Merkle Proof Verification Benchmark CLI", ["server-side", "good-first-issue"],
         "Add an example CLI tool in `core/examples/build_tree.rs` to generate verification benchmarks.",
         "1. Generate tree with variable leaf sizes (100, 1000, 10000).\n2. Export JSON benchmark reports.\n3. Print Markdown format performance summary.",
         "1. Run `cargo run -p soroscope-core --example build_tree`.\n2. Confirm summary printout."),

        (30, "Core Binary Memory Allocation Profiling Integration", ["server-side", "performance"],
         "Integrate `jemallocator` or `dtrace` hooks in `core/src/bin/server.rs` to monitor server RAM overhead.",
         "1. Enable optional jemalloc feature flag.\n2. Export heap usage statistics via internal metrics API.\n3. Detect potential memory leaks during continuous simulation runs.",
         "1. Run server under sustained simulation test load.\n2. Confirm stable memory footprint over time."),

        # 31-40: Security, Authentication, Webhooks & Infra
        (31, "JWT Authentication Middleware for Server Endpoints", ["server-side", "security"],
         "Implement JWT token validation middleware in `core/src/auth.rs`.",
         "1. Validate Bearer JWT tokens on protected REST/gRPC routes.\n2. Parse user roles and permissions from claims.\n3. Return HTTP 401 Unauthorized for invalid/expired tokens.",
         "1. Test endpoints with valid and invalid JWT tokens.\n2. Confirm permission enforcement."),

        (32, "API Key Rate Limiting & Auth Validation", ["server-side", "security"],
         "Add API key authorization and per-key rate limits in `core/src/auth.rs`.",
         "1. Validate `X-API-Key` headers against stored key hashes.\n2. Apply customized tier limits (Free: 100/min, Pro: 1000/min).\n3. Track key usage counters.",
         "1. Test API key header validation.\n2. Confirm tier enforcement."),

        (33, "CORS Middleware Configuration Manager", ["server-side", "security"],
         "Implement strict CORS header configuration in `core/src/cors.rs`.",
         "1. Support configurable allowed origins via environment variable.\n2. Explicitly specify allowed methods (GET, POST, OPTIONS) and headers.\n3. Disable wildcard (`*`) in production builds.",
         "1. Run `cargo test -p soroscope-core cors`.\n2. Verify preflight OPTIONS request responses."),

        (34, "Webhook Dispatcher & Exponential Retry Engine", ["server-side", "feature"],
         "Build robust webhook notifications dispatcher in `core/src/webhooks.rs`.",
         "1. Dispatch HTTP POST payloads when long simulations finish.\n2. Retry failed deliveries with exponential backoff (1s, 2s, 4s, 8s, 16s).\n3. Log delivery status and failure reasons.",
         "1. Run `cargo test -p soroscope-core webhooks`.\n2. Verify retries occur on target HTTP failure."),

        (35, "HMAC Webhook Payload Signature Verification", ["server-side", "security"],
         "Implement HMAC-SHA256 signature calculation in `core/src/webhook_validation.rs`.",
         "1. Compute `X-Signature-256` header using secret key.\n2. Provide constant-time signature comparison helper for receivers.\n3. Include timestamp to prevent replay attacks.",
         "1. Test signature generation against known test vectors.\n2. Verify invalid signature rejection."),

        (36, "Contract Bytecode Security Scanner", ["server-side", "security"],
         "Build WASM safety checker in `core/src/parser.rs` to flag dangerous WASM constructs.",
         "1. Detect unbounded memory growth or floating-point operations.\n2. Warn on missing export functions or non-standard entry points.\n3. Fail verification for malformed WASM binaries.",
         "1. Test scanner against malformed WASM files.\n2. Confirm security alerts produced."),

        (37, "Prometheus Operational Metrics Exporter Endpoint", ["server-side", "production-ready"],
         "Expose `/metrics` endpoint in `core/src/main.rs` using Prometheus registry format.",
         "1. Count total HTTP requests, status codes, and latencies.\n2. Expose active simulation gauge and RPC error counters.\n3. Include server uptime and system metrics.",
         "1. Query `http://localhost:8080/metrics`.\n2. Verify Prometheus metric format."),

        (38, "Graceful Shutdown & Signal Handling", ["server-side", "production-ready"],
         "Implement SIGINT/SIGTERM graceful shutdown handler in `core/src/main.rs`.",
         "1. Catch OS signals (`SIGINT`, `SIGTERM`).\n2. Stop accepting new requests and drain active worker pool tasks within 10s.\n3. Flush database connections cleanly.",
         "1. Send SIGTERM signal to running server binary.\n2. Confirm clean shutdown without state corruption."),

        (39, "Configuration Loader with Environment Fallbacks", ["server-side", "good-first-issue"],
         "Build unified configuration struct in `core/src/lib.rs` supporting `.env` files.",
         "1. Load parameters from environment variables with sensible defaults.\n2. Validate required production flags (e.g. database credentials).\n3. Print clean error diagnostics on invalid config.",
         "1. Run server without `.env` to test defaults.\n2. Verify env variable overrides work correctly."),

        (40, "Core API Documentation Generator Integration", ["server-side", "documentation"],
         "Integrate `utoipa` or OpenAPI doc generator in `core/src/main.rs`.",
         "1. Annotate REST route handlers with OpenAPI attributes.\n2. Expose interactive Swagger UI on `/docs`.\n3. Generate client OpenAPI JSON spec file.",
         "1. Access `/docs` in browser.\n2. Verify OpenAPI spec completeness.")
    ]

    for num, title, labels, prob, impl_h, verif in server_side_data:
        issues.append(make_issue(num, "Server-Side", title, labels, prob, impl_h, verif))

    # =========================================================================
    # GROUP 2: CLIENT-SIDE & FRONTEND WEB DASHBOARD (41 - 80)
    # =========================================================================

    client_side_data = [
        # 41-50: Visualizations & Interactive Components
        (41, "Interactive React Flow Cross-Contract Call Graph Visualizer", ["client-side", "feature"],
         "Build a node-based interactive graph component in `web/components/CallGraphVisualizer.tsx`.",
         "1. Render call nodes representing target smart contracts.\n2. Color-code edges based on gas consumption (Green < 100k, Yellow < 500k, Red > 500k).\n3. Allow clicking nodes to inspect call stack parameters.",
         "1. Run `npm run test` in `web/`.\n2. Verify visual graph rendering with mock call graph data."),

        (42, "High-Performance HTML5 Canvas Resource Heatmap Renderer", ["client-side", "performance"],
         "Optimize `web/components/ResourceHeatmap.tsx` rendering using HTML5 Canvas instead of DOM elements.",
         "1. Replace heavy SVG/DOM grid with HTML5 2D Canvas rendering context.\n2. Support smooth zooming and panning over 10,000 opcode cells.\n3. Display tooltip overlay on hover over any cell.",
         "1. Profile FPS during zoom/pan actions in web app.\n2. Confirm smooth 60fps performance."),

        (43, "Command+K Global Quick-Search Modal", ["client-side", "feature"],
         "Enhance `web/components/GlobalSearchModal.tsx` for searching contracts, functions, and reports.",
         "1. Listen for global `Cmd+K` / `Ctrl+K` keyboard shortcuts.\n2. Provide fuzzy search across saved contracts, historical logs, and settings.\n3. Support keyboard arrow navigation and Enter selection.",
         "1. Press Cmd+K on home dashboard page.\n2. Verify search modal opens and keyboard shortcuts function."),

        (44, "Contract Nutrition Label Component Overhaul", ["client-side", "feature"],
         "Redesign `web/components/NutritionLabel.tsx` to resemble an authentic nutritional facts label.",
         "1. Display CPU instructions, RAM bytes, Ledger reads/writes as daily value percentages.\n2. Add expandable sections for gas breakdown.\n3. Provide export as PNG feature using `html2canvas`.",
         "1. Inspect Nutrition Label component on index page.\n2. Test export image action."),

        (45, "Gas Usage Comparison Side-by-Side Chart", ["client-side", "feature"],
         "Create comparison visualizer in `web/components/GasUsageChart.tsx`.",
         "1. Allow selecting two simulation runs side-by-side.\n2. Highlight gas diff percentage (+12%, -8%) per function call.\n3. Render comparative bar chart using Recharts/Chart.js.",
         "1. Select two simulation reports.\n2. Confirm diff highlight rendering."),

        (46, "Interactive Subgraph Schema Visualizer Node Diagram", ["client-side", "feature"],
         "Enhance `web/components/SchemaVisualizer.tsx` to display ledger key read/write dependencies.",
         "1. Render visual schema nodes for instance, persistent, and temporary storage keys.\n2. Distinguish read-only dependencies from state mutations.\n3. Support node filtering by key namespace.",
         "1. Test schema tab in web dashboard.\n2. Verify ledger storage node display."),

        (47, "Gas Golfing Suggestions Table with One-Click Filters", ["client-side", "good-first-issue"],
         "Improve `web/components/GasGolfingSuggestionsTable.tsx` usability.",
         "1. Add filter pills by severity (High, Medium, Low, Info).\n2. Add code snippet preview modal for recommended fixes.\n3. Add 'Copy Suggestion' button.",
         "1. Interact with table filters in web dashboard.\n2. Verify filtering and copy functionality."),

        (48, "Skeleton Loading Placeholders for Async Dashboard Views", ["client-side", "good-first-issue"],
         "Implement clean skeleton screens in `web/components/NutritionLabelSkeleton.tsx` and `ResultViewerSkeleton.tsx`.",
         "1. Replace blank screens during simulation fetch with animated pulsing skeletons.\n2. Match exact layout dimensions of final loaded components.\n3. Ensure zero UI layout shifts (CLS).",
         "1. Trigger async contract simulation fetch.\n2. Observe smooth skeleton transition."),

        (49, "Contract Invocation History Table with Search & Pagination", ["client-side", "feature"],
         "Enhance `web/components/TransactionHistoryTable.tsx` for browsing past simulations.",
         "1. Add real-time text search filter by contract address or method name.\n2. Add column sorting by timestamp, total gas cost, and status.\n3. Add pagination controls (10, 25, 50 rows).",
         "1. Test search input and sorting controls.\n2. Verify row pagination."),

        (50, "Dynamic Form Parameter Auto-Generator from Spec XDR", ["client-side", "feature"],
         "Enhance `web/components/DynamicForm.tsx` to build input forms directly from contract spec XDR.",
         "1. Parse Soroban contract function parameters (Address, Vector, Struct, Symbol).\n2. Render appropriate input components with validation rules.\n3. Support custom JSON input mode for complex types.",
         "1. Upload WASM contract with struct parameters.\n2. Verify generated input form fields."),

        # 51-60: State Management, Web Workers & Offline Support
        (51, "Dedicated Web Worker for Off-Thread WASM Bytecode Decoding", ["client-side", "performance"],
         "Offload WASM decoding to a background Web Worker in `web/wasm-upload/`.",
         "1. Move heavy WASM parsing and validation logic into worker thread.\n2. Communicate with main thread via `postMessage`.\n3. Ensure UI thread remains responsive (60fps) during 10MB WASM uploads.",
         "1. Upload large WASM binary file.\n2. Verify UI main thread never freezes."),

        (52, "LocalStorage Soroban RPC Endpoint Encryption Manager", ["client-side", "security"],
         "Implement client-side endpoint encryption in `web/lib/localStorage.ts`.",
         "1. Encrypt custom Soroban RPC URLs and secret keys before saving to LocalStorage.\n2. Use Web Crypto API (`AES-GCM`).\n3. Add 'Clear All Stored Endpoints' button in settings.",
         "1. Save custom RPC endpoint in `/settings`.\n2. Inspect browser LocalStorage to verify encrypted data."),

        (53, "Infinite Scrolling Telemetry Log Viewer", ["client-side", "feature"],
         "Build high-performance log list in `web/components/InnovocationHistory.tsx` using `IntersectionObserver`.",
         "1. Load telemetry logs dynamically as user scrolls down.\n2. Virtualize log row elements to preserve low memory footprint.\n3. Support log filtering by log level (Info, Warn, Error).",
         "1. Scroll through 1,000 mock log entries.\n2. Verify smooth scrolling without DOM slowdown."),

        (54, "Stellar Wallet Connection Provider Integration", ["client-side", "feature"],
         "Enhance `web/components/ConnectButton.tsx` and `WalletModal.tsx` for multi-wallet support.",
         "1. Support Freighter, Albedo, Lobstr, and xBull Stellar wallet adapters.\n2. Display connected account public key, network, and XLM balance card (`WalletBalanceCard.tsx`).\n3. Handle wallet disconnect and account switch events gracefully.",
         "1. Click 'Connect Wallet' button in header.\n2. Test wallet adapter modal selection."),

        (55, "Offline Network Banner with Framer Motion Animations", ["client-side", "good-first-issue"],
         "Enhance `web/components/OfflineBanner.tsx` for network disconnect status.",
         "1. Monitor `navigator.onLine` state and browser events.\n2. Render top notification bar with smooth Framer Motion slide-in transition.\n3. Display 'Back online!' success banner when reconnected.",
         "1. Toggle browser offline mode in DevTools.\n2. Verify offline banner slide animation."),

        (56, "Global Application State Management via Zustand/React Context", ["client-side", "feature"],
         "Refactor frontend state management in `web/context/`.",
         "1. Create centralized app store for active network, selected contract, and simulation status.\n2. Eliminate unnecessary prop drilling across components.\n3. Add state persistence middleware for user settings.",
         "1. Run unit tests in `web/`.\n2. Verify state persistence across page navigation."),

        (57, "Network Switcher Component with Latency Diagnostics", ["client-side", "good-first-issue"],
         "Enhance `web/components/NetworkSwitcher.tsx` for network selection.",
         "1. Provide dropdown selection for Mainnet, Testnet, Futurenet, and Localhost.\n2. Measure and display live RPC ping latency (e.g. `45ms`).\n3. Save selected network globally.",
         "1. Select different networks in top navigation bar.\n2. Verify latency indicator updates."),

        (58, "WASM Drag-and-Drop File Upload Zone Hardening", ["client-side", "feature"],
         "Enhance `web/components/upload-zone.tsx` and `WasmUpload.tsx`.",
         "1. Support drag-and-drop of `.wasm` contract files.\n2. Validate file magic header (`\\0asm`) before processing.\n3. Display upload progress bar and file hash checksum.",
         "1. Drag invalid file onto dropzone.\n2. Verify validation error toast."),

        (59, "Client-Side Soroban Simulation Rate Limiter Queue", ["client-side", "performance"],
         "Implement client request queue manager in `web/lib/requestQueue.ts`.",
         "1. Throttle simulation requests to maximum 2 calls per second.\n2. Prevent hitting Soroban RPC HTTP 429 rate limit errors.\n3. Display queue status badge when requests are waiting.",
         "1. Trigger 5 rapid simulation requests.\n2. Verify requests execute sequentially with delay."),

        (60, "Dynamic Contract Header Navigation Bar", ["client-side", "good-first-issue"],
         "Refactor `web/components/HeaderNav.tsx` for responsive navigation.",
         "1. Render mobile responsive navigation drawer menu.\n2. Add quick navigation links to Simulator, Analytics, Staking Calculator, Settings.\n3. Highlight active page route link.",
         "1. Resize browser window to mobile width.\n2. Test mobile hamburger drawer menu."),

        # 61-70: Staking Calculator, Forms & Code Highlighting
        (61, "Interactive Token Yield & Staking Calculator Widget", ["client-side", "feature"],
         "Enhance `web/components/StakingCalculator.tsx` with APY compounding math.",
         "1. Sliders for Deposit Amount, Lock Duration (1-36 months), and Base APY.\n2. Compound frequency selector (Daily, Weekly, Monthly, Annually).\n3. Display cumulative milestone table and reward projection charts.",
         "1. Run `npm test StakingCalculator` in `web/`.\n2. Verify mathematical calculations match test expectations."),

        (62, "Syntax Highlighting Component for Soroban Rust & XDR", ["client-side", "feature"],
         "Enhance `web/components/SyntaxHighlighter.tsx` for syntax coloring.",
         "1. Tokenize Rust code and base64 XDR structure payloads.\n2. Apply theme colors (Cyan keywords, Yellow types, Green strings, Orange numbers).\n3. Add line numbers and hover row highlighting.",
         "1. View code tab in web dashboard.\n2. Verify syntax highlighting colors."),

        (63, "One-Click Code & Data Clipboard Copy Button Component", ["client-side", "good-first-issue"],
         "Improve `web/components/CopyButton.tsx` feedback states.",
         "1. Provide instant visual feedback ('Copied!' badge + checkmark icon) for 2 seconds.\n2. Handle copy failure states gracefully.\n3. Add accessible ARIA attributes.",
         "1. Click copy button next to contract address.\n2. Verify checkmark animation and clipboard content."),

        (64, "Interactive Contract Invocation Sidebar Drawer", ["client-side", "feature"],
         "Enhance `web/components/FunctionSidebar.tsx` and `InvocationHistorySidebar.tsx`.",
         "1. Display collapsible list of exported contract functions.\n2. Search functions by name or tag.\n3. Click function to auto-fill execution form.",
         "1. Open sidebar drawer in simulator view.\n2. Test function search input."),

        (65, "Confetti Animation Trigger for Successful Transactions", ["client-side", "good-first-issue"],
         "Enhance `web/components/TransactionConfetti.tsx` visual feedback.",
         "1. Trigger canvas particle confetti animation on successful on-chain invocation.\n2. Add sound toggle setting.\n3. Ensure animation auto-cleans memory after 3 seconds.",
         "1. Execute successful simulation test.\n2. Observe confetti animation emission."),

        (66, "Liquidity Pool Analytics & Fee Visualizer Panel", ["client-side", "feature"],
         "Enhance `web/components/LiquidityPoolAnalytics.tsx` with pool metrics.",
         "1. Render TVL, 24h Volume, and LP Fee Revenue charts.\n2. Display user share balance and pending accrued yield.\n3. Include fee APR vs trading APR breakdown.",
         "1. Navigate to Liquidity Pool Analytics page.\n2. Verify chart rendering."),

        (67, "Settings Panel with Endpoint Diagnostics & Connection Test", ["client-side", "good-first-issue"],
         "Enhance `web/pages/settings.tsx` with diagnostic ping tool.",
         "1. Input fields for Soroban RPC URL, Horizon URL, and Indexer URL.\n2. 'Test Connection' button with real-time health indicator badge.\n3. 'Reset to Defaults' button.",
         "1. Change RPC URL in `/settings` and click 'Test Connection'.\n2. Verify success/error badge response."),

        (68, "Dark & Light Mode Theme Toggle Switcher", ["client-side", "good-first-issue"],
         "Fix theme toggle implementation in `web/components/ui/themeToggle.tsx`.",
         "1. Support system preference detection (`prefers-color-scheme`).\n2. Persist theme choice in LocalStorage.\n3. Prevent theme flash during initial server-side hydration.",
         "1. Click theme toggle switch in header.\n2. Confirm smooth theme transition without layout flash."),

        (69, "Custom Accessible Modal Component Library", ["client-side", "good-first-issue"],
         "Refactor dialog modals in `web/components/ui/` for accessibility compliance.",
         "1. Implement focus trap inside active modals.\n2. Support Escape key closing handler.\n3. Add screen-reader accessible ARIA roles and labels.",
         "1. Open modal and press Tab key.\n2. Verify focus remains trapped within modal."),

        (70, "Soroban Contract Address Explorer Link Component", ["client-side", "good-first-issue"],
         "Build reusable link component for Stellar explorer external links.",
         "1. Render shortened address string (`CC3J...4KL9`).\n2. Link directly to Stellar Expert or Soroban Explorer based on active network.\n3. Open link safely in external browser tab (`rel=\"noopener noreferrer\"`).",
         "1. Click contract address link.\n2. Verify target URL opens Stellar Expert for correct network."),

        # 71-80: Export Utilities, UX & Responsiveness
        (71, "Export Profiling Summary Report to PDF & CSV", ["client-side", "feature"],
         "Implement client-side report export utility in `web/lib/exportReport.ts`.",
         "1. Export full simulation details to formatted PDF document using `jspdf`.\n2. Export gas breakdown raw data tables to CSV format.\n3. Include contract metadata, timestamp, and signature.",
         "1. Click 'Export Report' button on simulation result view.\n2. Verify generated PDF and CSV files."),

        (72, "Toast Notification System Queue and Stack Manager", ["client-side", "good-first-issue"],
         "Enhance `web/components/Toast.tsx` toast stack manager.",
         "1. Support stacked toast notifications (Success, Warning, Error, Info).\n2. Auto-dismiss toasts after 4 seconds with smooth fade animation.\n3. Allow swipe-to-dismiss gesture on mobile touch screens.",
         "1. Trigger multiple toast alerts.\n2. Verify stack ordering and auto-dismiss timing."),

        (73, "Custom React Error Boundary Fallback Screen", ["client-side", "feature"],
         "Enhance `web/components/ErrorBoundary.tsx` error fallback component.",
         "1. Catch runtime JavaScript rendering errors gracefully.\n2. Render diagnostic error stack trace drawer with 'Copy Diagnostics' button.\n3. Provide 'Reload Application' recovery button.",
         "1. Trigger intentional rendering error in test mode.\n2. Confirm Error Boundary screen renders."),

        (74, "Mobile Touch Gestures for Interactive Charts", ["client-side", "good-first-issue"],
         "Add mobile touch optimization for resource charts in `web/components/`.",
         "1. Pinch-to-zoom gesture support for gas charts.\n2. Touch drag pan support.\n3. Tooltip focus on single touch tap.",
         "1. Test chart interaction on touch screen simulator.\n2. Confirm pinch zoom gesture behavior."),

        (75, "Soroban Network Congestion Live Indicator Badge", ["client-side", "feature"],
         "Build network indicator badge component in `web/components/HeaderNav.tsx`.",
         "1. Query current ledger inclusion fee levels.\n2. Display congestion level badge (Low: Green, Medium: Yellow, High: Red).\n3. Hover tooltip showing recommended base fee.",
         "1. Inspect header status badge.\n2. Confirm color update based on fee level."),

        (76, "WASM Bytecode Decompiler & Disassembly Viewer", ["client-side", "feature"],
         "Add interactive WASM text viewer tab in `web/pages/index.tsx`.",
         "1. Render disassembly text view side-by-side with original Rust code.\n2. Highlight corresponding WASM opcode line when clicking Rust line.\n3. Filter search within WASM opcodes.",
         "1. Upload WASM binary and select Disassembly tab.\n2. Test line cross-highlighting feature."),

        (77, "User Preference LocalStorage Migrations Handler", ["client-side", "good-first-issue"],
         "Build migration utility in `web/lib/settingsMigration.ts`.",
         "1. Handle versioned LocalStorage schema upgrades automatically.\n2. Provide fallback default values for missing setting keys.\n3. Validate setting values on app startup.",
         "1. Write legacy LocalStorage structure in browser console.\n2. Refresh page and verify smooth data migration."),

        (78, "Tailwind CSS Design System Token Standardization", ["client-side", "good-first-issue"],
         "Consolidate color tokens in `web/tailwind.config.js` and `web/styles/globals.css`.",
         "1. Remove inline arbitrary color classes (`bg-[#12141d]`).\n2. Define standardized CSS variable design tokens (`bg-background`, `text-primary`).\n3. Ensure WCAG AA contrast compliance across dark and light themes.",
         "1. Run `npm run lint` in `web/`.\n2. Check color contrast compliance."),

        (79, "Next.js Image & Font Optimization Pipeline", ["client-side", "performance"],
         "Optimize asset loading in `web/pages/_app.tsx`.",
         "1. Replace default browser fonts with `next/font` Google Fonts (Inter / JetBrains Mono).\n2. Use `next/image` for logo assets with automatic WebP conversion.\n3. Reduce initial JS bundle size.",
         "1. Run `npm run build` in `web/`.\n2. Inspect Next.js build bundle output size report."),

        (80, "Web Accessibility (a11y) ARIA Attribute Audit", ["client-side", "good-first-issue"],
         "Audit accessibility across all web dashboard components.",
         "1. Add missing `aria-label`, `aria-expanded`, `role` attributes.\n2. Ensure keyboard navigation order is logical across forms.\n3. Run automated `axe-core` accessibility check.",
         "1. Run automated a11y test suite.\n2. Verify zero accessibility violations.")
    ]

    for num, title, labels, prob, impl_h, verif in client_side_data:
        issues.append(make_issue(num, "Client-Side", title, labels, prob, impl_h, verif))

    # =========================================================================
    # GROUP 3: SOROBAN SMART CONTRACTS (81 - 120)
    # =========================================================================

    contracts_data = [
        # 81-90: AMM, Liquidity Pools & LP Fees
        (81, "Liquidity Pool LP Mint/Burn Deposit Fee Implementation", ["soroban-contract", "feature"],
         "Implement basis-point deposit & withdrawal fee logic in `contracts/liquidity_pool/src/lib.rs`.",
         "1. Add `LpFeeBps` DataKey to pool storage.\n2. Deduct fee_shares on deposit and retain in pool reserves.\n3. Emit `LpDepositFeeEvent` and `LpWithdrawFeeEvent`.",
         "1. Run `cargo test -p liquidity_pool`.\n2. Verify fee deduction accuracy in test suite."),

        (82, "JIT Liquidity Attack Economic Cost Safeguard", ["soroban-contract", "security"],
         "Add dynamic deposit lock time window in `contracts/liquidity_pool/src/lib.rs`.",
         "1. Track deposit ledger sequence number per LP user.\n2. Enforce minimum holding period (e.g. 10 ledgers) before withdrawal without penalty fee.\n3. Make JIT front-running unprofitable.",
         "1. Run LP integration tests.\n2. Verify early withdrawal penalty enforced."),

        (83, "Concentrated Liquidity AMM Tick Math Precision Optimization", ["soroban-contract", "performance"],
         "Optimize fixed-point math calculations in `contracts/concentrated_amm/src/lib.rs`.",
         "1. Use bitwise shift operations for square root calculations.\n2. Prevent precision loss when calculating tick swap amounts.\n3. Benchmark gas reduction per swap.",
         "1. Run `cargo test -p concentrated_amm`.\n2. Check gas consumption benchmark output."),

        (84, "Hybrid AMM Orderbook Matching Engine Integration", ["soroban-contract", "feature"],
         "Build order matching logic in `contracts/hybrid_amm_lob/src/lib.rs`.",
         "1. Combine automated market maker pool liquidity with limit orderbook queue.\n2. Route swaps to best price source (AMM vs LOB).\n3. Maintain price priority execution.",
         "1. Run `cargo test -p hybrid_amm_lob`.\n2. Verify order execution routing."),

        (85, "Liquidity Pool Dynamic Fee Rate Adjustment", ["soroban-contract", "feature"],
         "Implement volatility-adjusted fee scaling in `contracts/liquidity_pool/src/lib.rs`.",
         "1. Increase swap fee during high market volatility.\n2. Read volatility metric from TWAP oracle contract.\n3. Cap fee rate below maximum allowed limit.",
         "1. Run fee scaling unit tests.\n2. Verify fee adjusts dynamically with volatility inputs."),

        (86, "Multi-Token Basket Liquidity Pool Vault", ["soroban-contract", "feature"],
         "Support arbitrary N-token liquidity pools in `contracts/liquidity_pool/`.",
         "1. Generalize pool math from 2-token pair to N-token array.\n2. Implement Balancer-style invariant curve calculation.\n3. Support single-asset deposit and withdrawal.",
         "1. Test 3-token pool deposit and swap operations.\n2. Verify balance invariant holds."),

        (87, "Flash Loan Vault Callback Verification Logic", ["soroban-contract", "security"],
         "Implement strict reentrancy and callback validation in `contracts/flash_loan_vault/src/lib.rs`.",
         "1. Verify borrowing contract returns loan amount + fee within same transaction.\n2. Revert transaction if return balance check fails.\n3. Enforce access control on callback dispatcher.",
         "1. Run `cargo test -p flash_loan_vault`.\n2. Verify loan failure reverts transaction."),

        (88, "Automated LP Share Auto-Compounding Vault", ["soroban-contract", "feature"],
         "Build auto-compounding yield manager in `contracts/multi_yield_vault/src/lib.rs`.",
         "1. Collect accrued trading fees periodically.\n2. Re-invest fee tokens into pool reserves automatically.\n3. Mint compounding vault shares to depositors.",
         "1. Test compounding yield execution.\n2. Check vault share value growth."),

        (89, "Liquidity Pool Emergency Pause Circuit Breaker", ["soroban-contract", "security"],
         "Integrate Emergency Guard contract in `contracts/liquidity_pool/src/lib.rs`.",
         "1. Allow authorized guardian address to trigger pause.\n2. Disable swaps and deposits during emergency pause state.\n3. Allow withdrawals only under emergency mode.",
         "1. Trigger emergency pause in unit test.\n2. Verify swap calls fail with `Error::Paused`."),

        (90, "Constant Product AMM Slippage Protection Inspector", ["soroban-contract", "good-first-issue"],
         "Add strict slippage tolerance checks in `contracts/liquidity_pool/src/lib.rs`.",
         "1. Require caller to pass `min_amount_out` parameter.\n2. Revert with `Error::SlippageExceeded` if received output is below minimum.\n3. Emit slippage metric event.",
         "1. Execute swap with high slippage constraint.\n2. Verify transaction reverts correctly."),

        # 91-100: Security, Cross-Chain & Emergency Guard
        (91, "Cross-Chain Merkle Proof Verifier Gas Golfing", ["soroban-contract", "performance"],
         "Optimize CPU gas consumption in `contracts/cross_chain_verifier/src/lib.rs`.",
         "1. Replace recursive proof verification with iterative loop.\n2. Minimize SHA-256 host function calls during leaf verification.\n3. Reduce stack memory allocation overhead.",
         "1. Run `cargo test -p cross_chain_verifier`.\n2. Confirm 25%+ CPU gas reduction in profiling report."),

        (92, "Emergency Guard Circuit Breaker Core Module", ["soroban-contract", "security"],
         "Refactor shared guard module in `contracts/emergency_guard/src/lib.rs`.",
         "1. Standardize `PauseType` enum (Unpaused, PartialPause, FullPause).\n2. Support multi-sig guardian role management.\n3. Add auto-unpause timelock delay.",
         "1. Run `cargo test -p emergency_guard`.\n2. Verify permission checks and timelock functionality."),

        (93, "Cross-Chain Payload Parser and Validation Safety Checks", ["soroban-contract", "security"],
         "Hardening payload verification in `contracts/cross_chain_payload/src/lib.rs`.",
         "1. Validate message nonce to prevent replay attacks.\n2. Parse origin chain ID and sender contract bytes.\n3. Enforce maximum payload size limit (10 KB).",
         "1. Run `cargo test -p cross_chain_payload`.\n2. Verify invalid nonce payloads are rejected."),

        (94, "Timelocked Escrow Multi-Sig Recovery Contract", ["soroban-contract", "security"],
         "Implement multi-signature escrow recovery in `contracts/timelocked_escrow/src/lib.rs`.",
         "1. Require N-of-M signatures to execute recovery transfer after timelock expiry.\n2. Support timelock extension by original owner.\n3. Emit escrow status change events.",
         "1. Run `cargo test -p timelocked_escrow`.\n2. Verify threshold signature verification."),

        (95, "Typed Data Signature Authentication Verifier (`typed_data_auth`)", ["soroban-contract", "security"],
         "Implement EIP-712 style typed data signing verification in `contracts/typed_data_auth/src/lib.rs`.",
         "1. Hash structured domain separator and payload struct.\n2. Verify ed25519 signature against expected signer address.\n3. Prevent cross-contract signature re-use.",
         "1. Run `cargo test -p typed_data_auth`.\n2. Verify signature verification passes for valid signatures."),

        (96, "Contract Upgradability Proxy & Implementation Slot Manager", ["soroban-contract", "security"],
         "Implement secure contract upgrade pattern in `contracts/proxy/src/lib.rs`.",
         "1. Store WASM hash in administrative storage slot.\n2. Forward function calls to active implementation WASM.\n3. Require multi-sig authorization to update implementation hash.",
         "1. Test upgrading contract implementation WASM.\n2. Verify storage data remains intact post-upgrade."),

        (97, "CPU Heavy Benchmark Contract Gas Analyzer", ["soroban-contract", "performance"],
         "Enhance benchmark suite in `contracts/cpu_heavy/src/lib.rs`.",
         "1. Add recursive math, matrix multiplication, and sorting routines.\n2. Benchmark CPU budget limit boundaries.\n3. Export gas consumption baseline table.",
         "1. Run `cargo test -p cpu_heavy`.\n2. Verify CPU limit detection."),

        (98, "Storage Heavy Footprint Benchmark Contract", ["soroban-contract", "performance"],
         "Enhance benchmark suite in `contracts/storage_heavy/src/lib.rs`.",
         "1. Measure host gas costs for writing 1,000 instance vs persistent keys.\n2. Test storage footprint rent decay calculation.\n3. Compare storage batch write vs individual write performance.",
         "1. Run `cargo test -p storage_heavy`.\n2. Verify storage gas reporting."),

        (99, "Cross-Contract Invocation Call Chain Benchmark", ["soroban-contract", "performance"],
         "Enhance multi-call benchmark in `contracts/cross_call/src/lib.rs`.",
         "1. Test performance overhead of 1 to 10 nested cross-contract invocations.\n2. Measure gas cost per hop.\n3. Document host invocation overhead.",
         "1. Run `cargo test -p cross_call`.\n2. Check nested call gas scaling curve."),

        (100, "Soroban Error Code Definition Library Standardization", ["soroban-contract", "good-first-issue"],
         "Consolidate error codes in `contracts/error_codes/src/lib.rs`.",
         "1. Define unified error enum used across all Soroban contracts.\n2. Ensure stable, non-overlapping error integer mappings.\n3. Add documentation comments for each error code variant.",
         "1. Build all contracts in repository.\n2. Confirm error code compilation across workspace."),

        # 101-110: DeFi Building Blocks & Oracles
        (101, "CDP Collateralized Debt Position Liquidation Engine", ["soroban-contract", "feature"],
         "Implement position liquidation logic in `contracts/cdp/src/lib.rs`.",
         "1. Calculate vault collateralization ratio against oracle price.\n2. Trigger liquidation when ratio drops below minimum threshold (e.g. 150%).\n3. Award liquidation bonus percentage to liquidator caller.",
         "1. Run `cargo test -p cdp`.\n2. Verify liquidation calculation logic."),

        (102, "Dutch Auction Price Decay Factory Contract", ["soroban-contract", "feature"],
         "Implement descending-price Dutch auction logic in `contracts/dutch_auction/src/lib.rs`.",
         "1. Linear/exponential price decay calculation over auction duration.\n2. Immediate settlement upon first valid buyer bid.\n3. Refund unspent funds automatically.",
         "1. Run `cargo test -p dutch_auction`.\n2. Verify price calculation at variable timestamps."),

        (103, "English Auction Ascending Bid Factory Contract", ["soroban-contract", "feature"],
         "Implement ascending-bid auction logic in `contracts/english_auction/src/lib.rs`.",
         "1. Track highest bidder address and current bid price.\n2. Auto-refund outbid users immediately.\n3. Extend auction deadline if bid placed in final 5 minutes.",
         "1. Run `cargo test -p english_auction`.\n2. Test bid outbidding and deadline extension."),

        (104, "Soulbound Identity NFT Registry Contract", ["soroban-contract", "feature"],
         "Implement non-transferable token logic in `contracts/soulbound_token/src/lib.rs`.",
         "1. Mint soulbound identity badges tied to user Address.\n2. Explicitly prevent token transfers (`Error::Unauthorized`).\n3. Support issuer revocation and metadata update.",
         "1. Run `cargo test -p soulbound_token`.\n2. Verify transfer attempt fails with error."),

        (105, "Oracle Aggregator Time-Weighted Average Price (TWAP) Engine", ["soroban-contract", "feature"],
         "Implement TWAP price feed filter in `contracts/oracle_aggregator/src/lib.rs`.",
         "1. Aggregate price observations across multiple oracle sources.\n2. Reject outlier prices exceeding standard deviation threshold.\n3. Compute rolling time-weighted average price.",
         "1. Run `cargo test -p oracle_aggregator`.\n2. Verify outlier filtering and TWAP calculation."),

        (106, "TWAP Standalone Oracle Price Storage Contract", ["soroban-contract", "feature"],
         "Implement historical price accumulator in `contracts/twap_oracle/src/lib.rs`.",
         "1. Store cumulative price observations indexed by timestamp.\n2. Provide query interface for time interval price delta.\n3. Prune old price records automatically.",
         "1. Run `cargo test -p twap_oracle`.\n2. Verify price accumulator accuracy."),

        (107, "Private Zero-Knowledge Stealth Transfer Receiver", ["soroban-contract", "security"],
         "Implement private transaction routing in `contracts/private_transfer/src/lib.rs`.",
         "1. Verify stealth address generation commitment.\n2. Support one-time disposable deposit key accounts.\n3. Emit obfuscated transfer events.",
         "1. Run `cargo test -p private_transfer`.\n2. Verify commitment validation logic."),

        (108, "Staking Rewards Distribution & Fee Escrow Contract", ["soroban-contract", "feature"],
         "Implement linear reward distribution in `contracts/staking_rewards/src/lib.rs`.",
         "1. Distribute reward tokens proportionally based on user stake and duration.\n2. Support emergency unbonding with penalty fee.\n3. Update user reward index on every deposit/withdraw.",
         "1. Run `cargo test -p staking_rewards`.\n2. Check reward index math correctness."),

        (109, "Gasless Transaction Sponsor Relayer (`crucible-example-gasless`)", ["soroban-contract", "feature"],
         "Implement fee-sponsorship logic in `contracts/crucible-example-gasless/src/lib.rs`.",
         "1. Verify relayer account pays transaction gas fee on behalf of user.\n2. Validate user signature authorization payload.\n3. Deduct sponsor allowance from internal vault.",
         "1. Run `cargo test -p crucible-example-gasless`.\n2. Verify gasless invocation execution."),

        (110, "Decentralized Identity (DID) Document Registry", ["soroban-contract", "feature"],
         "Implement W3C DID document store in `contracts/did_registry/src/lib.rs`.",
         "1. Bind DID URIs (`did:soroban:...`) to public key verification methods.\n2. Support DID document rotation and key revocation.\n3. Emit `DIDUpdated` events.",
         "1. Run `cargo test -p did_registry`.\n2. Verify DID record resolution."),

        # 111-120: Governance, Timelock & Utilities
        (111, "Governance Proposal Voting & Execution Timelock Engine", ["soroban-contract", "feature"],
         "Implement DAO proposal lifecycle in `contracts/governance/src/lib.rs`.",
         "1. Proposal creation, voting delay, voting period, and execution timelock.\n2. Quorum verification and vote weight calculation based on token balance snapshot.\n3. Execute proposal target contract payload.",
         "1. Run `cargo test -p governance`.\n2. Test full proposal lifecycle from vote to execution."),

        (112, "Batch Token Transfer Gas Golfing Utility", ["soroban-contract", "performance"],
         "Optimize multi-recipient token transfers in `contracts/batch_transfer/src/lib.rs`.",
         "1. Accept arrays of recipient addresses and amounts in single call.\n2. Execute internal balance updates efficiently.\n3. Reduce overall transaction envelope overhead.",
         "1. Run `cargo test -p batch_transfer`.\n2. Compare gas cost vs separate individual transfers."),

        (113, "Soroban Fixed-Point High-Precision Math Library", ["soroban-contract", "good-first-issue"],
         "Enhance fixed-point math operations in `contracts/math/src/lib.rs`.",
         "1. Implement `wad_mul`, `wad_div`, `ray_mul`, `ray_div` routines.\n2. Prevent integer overflow/underflow panics.\n3. Add comprehensive unit test coverage for boundary values.",
         "1. Run `cargo test -p math`.\n2. Verify zero precision loss on fixed-point division."),

        (114, "Auction Factory Contract Deployment Manager", ["soroban-contract", "feature"],
         "Implement factory contract pattern in `contracts/auction_factory/src/lib.rs`.",
         "1. Deploy new Dutch or English auction contract instances programmatically.\n2. Maintain registry index of active auction contracts.\n3. Enforce protocol deployment fee.",
         "1. Run `cargo test -p auction_factory`.\n2. Verify contract instantiation via factory."),

        (115, "Universal Contract Factory Instance Registrar", ["soroban-contract", "feature"],
         "Implement central factory registry in `contracts/factory/src/lib.rs`.",
         "1. Track created pools, vaults, and token contracts across ecosystem.\n2. Map contract IDs to creator address and creation timestamp.\n3. Expose query functions for indexers.",
         "1. Run `cargo test -p factory`.\n2. Verify registry search functions."),

        (116, "Standard SEP-41 Token Contract Hardening", ["soroban-contract", "good-first-issue"],
         "Hardening SEP-41 token interface implementation in `contracts/token/src/lib.rs`.",
         "1. Verify compliance with official Soroban SEP-41 token specification.\n2. Add mint, burn, transfer, and allowance methods.\n3. Emit standard SEP-41 events.",
         "1. Run `cargo test -p token`.\n2. Verify compliance with SEP-41 spec."),

        (117, "Gas-Golfing Rust Attribute Macro Generator Library", ["soroban-contract", "performance"],
         "Create macro attribute crate in `contracts/gas_golfing/` to automate gas checks.",
         "1. Create procedural macro `#[gas_monitored]`.\n2. Auto-inject gas measurement hooks at start/end of contract functions.\n3. Reduce boilerplate code in contract files.",
         "1. Apply macro to sample contract function.\n2. Verify macro expansion and test execution."),

        (118, "Soroban Contract Event Index Payload Serialization", ["soroban-contract", "good-first-issue"],
         "Standardize event topic structures across all contracts in `contracts/`.",
         "1. Ensure first event topic is always Symbol action name.\n2. Standardize event payload data types for indexed querying.\n3. Update contract documentation.",
         "1. Run `cargo test` across all contracts.\n2. Verify event topics match specification."),

        (119, "Contract Storage TTL Extension Helper Function", ["soroban-contract", "good-first-issue"],
         "Implement automated TTL extension utility in `contracts/`.",
         "1. Extend instance and persistent storage TTL when contracts are accessed.\n2. Ensure contracts do not expire unexpectedly on Testnet/Mainnet.\n3. Enforce threshold check before extending TTL.",
         "1. Run test checking TTL extension logic.\n2. Verify storage TTL updated after call."),

        (120, "Sample Hello Soroban Contract Refactoring & Cleanup", ["soroban-contract", "good-first-issue"],
         "Clean up `contracts/hello_soroban/src/lib.rs` as clean reference template.",
         "1. Simplify codebase and add step-by-step beginner inline comments.\n2. Add complete unit test suite.\n3. Ensure warnings-free build.",
         "1. Run `cargo test -p hello_soroban`.\n2. Verify clean build without warnings.")
    ]

    for num, title, labels, prob, impl_h, verif in contracts_data:
        issues.append(make_issue(num, "Contract", title, labels, prob, impl_h, verif, area_prefix="Soroban Contract"))

    # =========================================================================
    # GROUP 4: FIXING BROKEN STUFFS, TESTING & PRODUCTION READINESS (121 - 160)
    # =========================================================================

    fixing_stuffs_data = [
        # 121-130: Contract Code Cleanup & Bug Fixes
        (121, "Fix Duplicate Enum & Struct Definitions in Liquidity Pool Contract", ["bug", "soroban-contract", "production-ready"],
         "Clean up `contracts/liquidity_pool/src/lib.rs` which contains duplicate `enum Error` variants and duplicate struct definitions from past merge conflicts.",
         "1. Open `contracts/liquidity_pool/src/lib.rs` and inspect duplicate `enum Error` variants (e.g. `InsufficientLiquidity` defined twice with codes 5 and 2).\n2. Consolidate into a single, clean `enum Error` block with unique u32 discriminant codes.\n3. Remove duplicate helper function declarations and redundant import statements.",
         "1. Run `cd contracts/liquidity_pool && cargo check`.\n2. Verify zero compilation errors or duplicate symbol warnings."),

        (122, "Fix AMM Swap Rounding Precision Error in Constant Product Math", ["bug", "soroban-contract"],
         "Fix rounding direction bug in `contracts/liquidity_pool/src/lib.rs` during token output calculation.",
         "1. Ensure swaps always round in favor of the pool (round output down, round required input up).\n2. Prevent token extraction exploit via micro-swaps.\n3. Add unit test for 1-stroop swap amounts.",
         "1. Run `cargo test -p liquidity_pool`.\n2. Verify micro-swap precision test passes."),

        (123, "Fix Reentrancy Vulnerability in Flash Loan Vault Execution", ["bug", "soroban-contract", "security"],
         "Fix missing reentrancy guard lock state in `contracts/flash_loan_vault/src/lib.rs`.",
         "1. Set `DataKey::ReentrancyLock` state before dispatching flash loan callback.\n2. Revert if borrower attempts to re-enter vault functions during callback.\n3. Clear lock state upon completion.",
         "1. Run `cargo test -p flash_loan_vault`.\n2. Verify recursive flash loan attempt fails."),

        (124, "Fix Unchecked Math Underflow in Staking Reward Index Calculation", ["bug", "soroban-contract"],
         "Fix potential underflow panic in `contracts/staking_rewards/src/lib.rs` when user balance is zero.",
         "1. Guard against subtraction underflow when calculating `reward_per_token_stored`.\n2. Use `checked_sub` and `checked_mul` math helpers.\n3. Return `Error::InvalidAmount` instead of panicking.",
         "1. Run `cargo test -p staking_rewards`.\n2. Verify zero-stake edge case test passes."),

        (125, "Fix Emergency Guard Unpause Permission By-Pass", ["bug", "soroban-contract", "security"],
         "Fix unauthorized unpause vulnerability in `contracts/emergency_guard/src/lib.rs`.",
         "1. Require explicit `admin.require_auth()` signature verification on `unpause()` function.\n2. Prevent regular accounts from clearing emergency pause state.\n3. Add negative authorization test.",
         "1. Run `cargo test -p emergency_guard`.\n2. Verify unpause call by non-admin fails."),

        (126, "Fix Dutch Auction Price Decay Expiry Overflow", ["bug", "soroban-contract"],
         "Fix arithmetic overflow in `contracts/dutch_auction/src/lib.rs` when auction duration is long.",
         "1. Cast elapsed time to `i128` before multiplication.\n2. Ensure current price saturates at reserve floor price when auction expires.\n3. Add test for auctions spanning 30+ days.",
         "1. Run `cargo test -p dutch_auction`.\n2. Verify long auction test passes."),

        (127, "Fix Oracle Aggregator Stale Price Acceptance Bug", ["bug", "soroban-contract", "security"],
         "Fix bug in `contracts/oracle_aggregator/src/lib.rs` that accepts expired price data.",
         "1. Check price payload timestamp against `env.ledger().timestamp()`.\n2. Reject prices older than max age limit (e.g. 300 seconds) with `Error::InvalidOraclePrice`.\n3. Update test suite with expired timestamp vectors.",
         "1. Run `cargo test -p oracle_aggregator`.\n2. Verify stale price rejected."),

        (128, "Fix CDP Liquidation Ratio Calculation Rounding", ["bug", "soroban-contract"],
         "Fix rounding error in `contracts/cdp/src/lib.rs` that prevents valid liquidations.",
         "1. Standardize collateralization ratio precision to 4 decimal places (bps).\n2. Prevent division-by-zero when debt is small.\n3. Add liquidation boundary unit tests.",
         "1. Run `cargo test -p cdp`.\n2. Verify edge case liquidations execute correctly."),

        (129, "Fix Governance Vote Double-Counting Vulnerability", ["bug", "soroban-contract", "security"],
         "Fix vulnerability in `contracts/governance/src/lib.rs` allowing double voting via token transfers.",
         "1. Use checkpoint snapshot balance at proposal creation ledger height.\n2. Prevent voters from transferring tokens to another address to vote twice.\n3. Add double-voting prevention test.",
         "1. Run `cargo test -p governance`.\n2. Verify second vote attempt with transferred tokens fails."),

        (130, "Fix Cross-Chain Payload Deserialization Panic", ["bug", "soroban-contract"],
         "Fix host panic in `contracts/cross_chain_payload/src/lib.rs` on truncated byte payloads.",
         "1. Validate input slice length before attempting deserialization.\n2. Return `Result::Err` instead of causing unhandled WASM panic.\n3. Add corrupt payload fuzz tests.",
         "1. Run `cargo test -p cross_chain_payload`.\n2. Verify graceful error return on corrupt payload."),

        # 131-140: Frontend Bug Fixes & React/SSR Fixes
        (131, "Fix Next.js React Hydration Mismatch in NutritionLabel Component", ["bug", "client-side"],
         "Fix SSR hydration warning in `web/components/NutritionLabel.tsx` caused by dynamic timestamp rendering.",
         "1. Move dynamic browser-only calculations inside `useEffect` hook.\n2. Ensure initial server render markup matches initial client render markup.\n3. Verify zero hydration warnings in browser console.",
         "1. Run `npm run dev` in `web/` and open dashboard.\n2. Inspect browser console for hydration warning elimination."),

        (132, "Fix Broken Jest Component Unit Test Mocks in Frontend", ["bug", "client-side", "testing"],
         "Fix failing Jest unit tests in `web/components/FeeEstimationPreview.test.cjs` and `HeaderNav.test.cjs`.",
         "1. Update mocked router and wallet context providers in test setup files.\n2. Fix mismatched component prop keys.\n3. Ensure all frontend tests pass cleanly.",
         "1. Run `npm test` inside `web/` directory.\n2. Confirm 100% passing test suite."),

        (133, "Fix Memory Leak in WebSocket Event Listener Custom Hooks", ["bug", "client-side"],
         "Fix uncleaned WebSocket listeners in `web/hooks/useWebSocket.ts`.",
         "1. Return cleanup function in `useEffect` to unsubscribe listeners and close connection on component unmount.\n2. Prevent memory leak and duplicate message handler execution.\n3. Test rapid page switching.",
         "1. Navigate between dashboard tabs repeatedly.\n2. Verify WebSocket connection count remains 1."),

        (134, "Fix Mobile Viewport Horizontal Scroll Overflow", ["bug", "client-side", "good-first-issue"],
         "Fix layout overflow bug causing horizontal scrollbar on mobile screen widths.",
         "1. Add `max-w-full overflow-x-auto` to resource data table containers.\n2. Wrap responsive flex grids with proper breakpoint utility classes.\n3. Test layout at 375px mobile breakpoint.",
         "1. Inspect site using mobile view mode in DevTools.\n2. Confirm zero horizontal body scrollbar."),

        (135, "Fix Theme Toggle Flash of Unstyled Content (FOUC)", ["bug", "client-side", "good-first-issue"],
         "Fix white theme background flash on page load when dark mode is enabled.",
         "1. Inject inline theme initialization script in Next.js `_document.tsx`.\n2. Read LocalStorage theme preference before HTML body paint.\n3. Eliminate visual theme flicker.",
         "1. Enable dark mode and hard refresh page (Ctrl+F5).\n2. Confirm smooth dark background load without white flash."),

        (136, "Fix Wallet Modal Account Disconnect State Reset Bug", ["bug", "client-side"],
         "Fix stale wallet account state in `web/components/ConnectButton.tsx` after disconnect.",
         "1. Clear active account public key and cached balance state upon disconnect.\n2. Reset wallet provider context state cleanly.\n3. Re-render UI in disconnected state immediately.",
         "1. Click 'Disconnect' in wallet menu.\n2. Verify UI updates to 'Connect Wallet' state immediately."),

        (137, "Fix Staking Calculator Negative Input Boundary Bug", ["bug", "client-side", "good-first-issue"],
         "Fix calculation error in `web/components/StakingCalculator.tsx` when negative numbers are typed.",
         "1. Enforce min value constraints (`min=0`) on numerical input fields.\n2. Sanitize user text inputs to prevent `NaN` results.\n3. Add edge-case validation test.",
         "1. Run `npm test StakingCalculator` in `web/`.\n2. Type negative numbers into form inputs and verify fallback to 0."),

        (138, "Fix Global Search Modal Esc Key Listener Leak", ["bug", "client-side", "good-first-issue"],
         "Fix keydown listener duplication in `web/components/GlobalSearchModal.tsx`.",
         "1. Remove keydown event listener in `useEffect` cleanup return.\n2. Prevent multiple search modal instances from opening simultaneously.\n3. Verify Esc key closes modal cleanly.",
         "1. Open and close search modal multiple times.\n2. Verify keyboard shortcut handles correctly."),

        (139, "Fix WASM Drag-and-Drop Dropzone File Type Check Bug", ["bug", "client-side", "good-first-issue"],
         "Fix dropzone bug in `web/components/upload-zone.tsx` accepting non-WASM files.",
         "1. Check both MIME type and file extension (`.wasm`).\n2. Show clear user error message when non-WASM file is dropped.\n3. Clear invalid file selection state.",
         "1. Drag `.txt` file onto dropzone.\n2. Confirm error message shown and upload blocked."),

        (140, "Fix CopyButton Toast Notification Overlay Z-Index", ["bug", "client-side", "good-first-issue"],
         "Fix visual layering bug where copy toast gets hidden under top navbar.",
         "1. Adjust CSS `z-index` layer hierarchy for `Toast.tsx` component (`z-50`).\n2. Ensure toasts display prominently over all navigation elements.\n3. Test notification display across all pages.",
         "1. Click copy button in table.\n2. Verify toast displays above header navigation bar."),

        # 141-150: Core Backend & Simulation Engine Bug Fixes
        (141, "Fix Flaky Integration Tests in Background Job Manager", ["bug", "server-side", "testing"],
         "Fix race condition in `core/src/jobs.rs` causing intermittent test failures during CI runs.",
         "1. Replace fixed `tokio::time::sleep` calls with explicit async completion channel notifications.\n2. Ensure task state transitions complete predictably before test assertions.\n3. Run test loop 50 times to verify stability.",
         "1. Run `cargo test -p soroscope-core jobs -- --nocapture` 50 times.\n2. Verify 100% pass rate without intermittent timeout."),

        (142, "Fix Cargo Build Target Warnings for Rust WASM Compilation", ["bug", "server-side", "production-ready"],
         "Resolve cargo compilation warnings across workspace during `cargo build`.",
         "1. Fix unused variable and dead code warnings across `core/src/` crate.\n2. Update deprecated dependency usage in `Cargo.toml`.\n3. Enforce `#![deny(warnings)]` in CI builds.",
         "1. Run `cargo check --all-targets` from root.\n2. Confirm zero warning outputs."),

        (143, "Fix Core CLI Crash on Corrupted XDR Payload Decoding", ["bug", "server-side"],
         "Fix panic in `core/src/xdr_decoder.rs` when parsing malformed XDR string.",
         "1. Replace `.unwrap()` with proper `Result` error handling in XDR decoder parser.\n2. Return descriptive error struct with offset byte details.\n3. Add corrupt payload regression unit test.",
         "1. Pass malformed XDR string to CLI decoder.\n2. Verify graceful error output without binary panic."),

        (144, "Fix Leader Lock Storage Timeout Calculation Bug", ["bug", "server-side"],
         "Fix lease expiration calculation bug in `core/src/leader_lock.rs`.",
         "1. Use monotonic system clock (`tokio::time::Instant`) instead of system wall time to prevent clock drift issues.\n2. Ensure leader lease expires reliably.\n3. Add clock jump test.",
         "1. Run `cargo test -p soroscope-core leader_lock`.\n2. Verify lock renewal and expiry behavior."),

        (145, "Fix WebSocket Server Reconnect Buffer Overflow", ["bug", "server-side", "performance"],
         "Fix memory buffer overflow in `core/src/ws.rs` under slow client connection.",
         "1. Set maximum outbound channel queue capacity (e.g. 100 messages).\n2. Drop oldest unconsumed telemetry events when client buffer is full.\n3. Log slow subscriber warning.",
         "1. Run WebSocket stress test with slow client simulator.\n2. Verify server memory remains stable."),

        (146, "Fix gRPC Server TLS Certificate Validation Failure", ["bug", "server-side", "security"],
         "Fix gRPC connection failure in `core/src/grpc.rs` when TLS is enabled.",
         "1. Fix certificate chain loading logic in Tonic gRPC server builder.\n2. Add support for custom CA certificate paths via environment config.\n3. Add TLS connection integration test.",
         "1. Start gRPC server with TLS enabled.\n2. Confirm secure client connection."),

        (147, "Fix SQLite Persistence Database Connection Pool Lock Deadlock", ["bug", "server-side"],
         "Fix database connection lock in `core/src/fee_store.rs` under concurrent writes.",
         "1. Configure SQLx pool size and busy timeout limit (e.g. 5000ms).\n2. Enable Write-Ahead Logging (WAL) mode on SQLite database connection.\n3. Test concurrent write throughput.",
         "1. Run concurrent database write test.\n2. Verify zero `DatabaseLocked` errors."),

        (148, "Fix Soroban Host Budget Limit Extraction Parsing Error", ["bug", "server-side"],
         "Fix regex extraction bug in `core/src/simulation.rs` parsing Soroban CLI log output.",
         "1. Update regex patterns to support Soroban CLI v21+ budget log output changes.\n2. Add fallback parser for legacy log formats.\n3. Add log string parsing unit tests.",
         "1. Run `cargo test -p soroscope-core simulation`.\n2. Verify budget numbers extracted correctly."),

        (149, "Fix Gas Golfing Rule False Positives on Loop Invariants", ["bug", "server-side"],
         "Fix false positive warnings in `core/src/gas_golfing.rs`.",
         "1. Refine AST inspection to distinguish true loop-invariant allocations from dynamic mutations.\n2. Reduce false positive rule warnings.\n3. Add regression tests.",
         "1. Run gas golfing inspector over complex contract.\n2. Verify accurate recommendation report."),

        (150, "Fix Merkle Tree Hash Calculation Order Mismatch", ["bug", "server-side"],
         "Fix leaf ordering discrepancy in `core/src/merkle_tree.rs`.",
         "1. Sort sibling nodes deterministically before hashing to match Soroban contract verifier expectations.\n2. Update example script test vectors.\n3. Ensure on-chain proof verification succeeds.",
         "1. Generate Merkle proof using core CLI and verify with Soroban contract.\n2. Confirm verification returns true."),

        # 151-160: DevOps, CI/CD, Production Readiness & Health Monitoring
        (151, "Fix GitHub Actions CI Workflow Dependency Caching", ["ci-cd", "production-ready"],
         "Optimize Rust build times in `.github/workflows/ci.yml` using `swatinem/rust-cache`.",
         "1. Configure caching for Cargo registry, index, and build target outputs.\n2. Configure npm cache for `web/` dependencies.\n3. Reduce CI pipeline run duration by 50%+.",
         "1. Push commit to trigger GitHub Actions CI.\n2. Verify cache hit in workflow execution log."),

        (152, "Multi-Stage Dockerfile Optimization for Core Server", ["ci-cd", "production-ready"],
         "Optimize Docker container image size in `Dockerfile`.",
         "1. Use multi-stage build pattern with `cargo-chef` for dependency caching.\n2. Use minimal `debian:bookworm-slim` or `alpine` base image for final runtime image.\n3. Reduce container image size to < 50 MB.",
         "1. Run `docker build -t soroscope-core .`.\n2. Inspect final image size with `docker images`."),

        (153, "Implement End-to-End Playwright UI Test Suite", ["testing", "client-side", "production-ready"],
         "Add Playwright E2E browser tests in `web/e2e/`.",
         "1. Write automated E2E tests for WASM file upload, simulation execution, and report export.\n2. Run tests headlessly in CI pipeline.\n3. Capture failure screenshots and traces.",
         "1. Run `npx playwright test` in `web/`.\n2. Confirm all E2E tests pass."),

        (154, "GitHub Release Workflow for WASM & Core Binary Artifacts", ["ci-cd", "production-ready"],
         "Create automated GitHub release pipeline in `.github/workflows/release.yml`.",
         "1. Trigger pipeline on version git tags (`v*`).\n2. Cross-compile release binaries for Linux x86_64 and macOS arm64.\n3. Compile all Soroban contract WASM files and attach as release assets.",
         "1. Test release pipeline using git dry-run tag.\n2. Confirm artifact packaging."),

        (155, "Production Security Audit of HTTP Headers & Input Sanitization", ["security", "production-ready"],
         "Audit HTTP security headers across core server and Next.js frontend.",
         "1. Configure `Content-Security-Policy`, `X-Frame-Options`, `X-Content-Type-Options` headers.\n2. Sanitize all user inputs against XSS and injection attacks.\n3. Run `cargo audit` and `npm audit` to fix vulnerable dependencies.",
         "1. Run security audit tools.\n2. Confirm zero high/critical vulnerability findings."),

        (156, "Implement `/healthz` and `/readyz` Kubernetes Health Endpoints", ["server-side", "production-ready"],
         "Add standard liveness and readiness probe routes in `core/src/main.rs`.",
         "1. `/healthz` returns HTTP 200 if server process is running.\n2. `/readyz` checks RPC connection health and database connection before returning HTTP 200.\n3. Return HTTP 503 Service Unavailable if unhealthy.",
         "1. Query `/healthz` and `/readyz` endpoints via curl.\n2. Verify appropriate HTTP status responses."),

        (157, "Production Environment Docker Compose Infrastructure Stack", ["ci-cd", "production-ready"],
         "Create unified `docker-compose.yml` for running full stack locally.",
         "1. Service definitions for `core-server`, `web-dashboard`, `redis-cache`, and `soroban-preview`.\n2. Configure container healthchecks and restart policies.\n3. Support quick start command: `docker compose up -d`.",
         "1. Run `docker compose up -d`.\n2. Verify all services start up and communicate seamlessly."),

        (158, "Frontend Static Web Export Build Pipeline Hardening", ["client-side", "ci-cd", "production-ready"],
         "Configure static export build pipeline in `.github/workflows/deploy-frontend.yml`.",
         "1. Update `next.config.js` for standalone static HTML export where applicable.\n2. Configure Vercel / GitHub Pages deployment automation.\n3. Add pre-deployment linting and build checks.",
         "1. Run `npm run build` in `web/`.\n2. Verify static export assets build cleanly."),

        (159, "OpenTelemetry Distributed Tracing Collector Integration", ["server-side", "production-ready"],
         "Integrate OpenTelemetry tracing in `core/src/trace_propagation.rs`.",
         "1. Propagate trace context (`traceparent`) across HTTP, gRPC, and WebSocket requests.\n2. Export spans to Jaeger / OpenTelemetry Collector.\n3. Track request latency breakdown across internal services.",
         "1. Run trace test with OpenTelemetry collector active.\n2. Verify end-to-end trace span visualization."),

        (160, "Production Readiness Checklist & Architecture Guide Documentation", ["documentation", "production-ready"],
         "Create comprehensive production deployment documentation in `docs/PRODUCTION_READINESS.md`.",
         "1. Document system architecture, security hardening guidelines, monitoring setup, and scaling best practices.\n2. Provide troubleshooting guide for common RPC and simulation issues.\n3. Link all 160 roadmap issues to production milestones.",
         "1. View `docs/PRODUCTION_READINESS.md`.\n2. Confirm markdown formatting and link integrity.")
    ]

    for num, title, labels, prob, impl_h, verif in fixing_stuffs_data:
        issues.append(make_issue(num, "Fix", title, labels, prob, impl_h, verif, area_prefix="Fix"))

    return issues

def ensure_labels_exist(dry_run=False):
    print("📌 Checking and creating GitHub labels...")
    for label in LABELS:
        name = label["name"]
        color = label["color"]
        desc = label["description"]
        if dry_run:
            print(f" [DRY-RUN] Label '{name}' (color: #{color}, desc: {desc})")
        else:
            cmd = ["gh", "label", "create", name, "--color", color, "--description", desc, "--repo", REPO, "--force"]
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                print(f"  ✓ Label '{name}' ready.")
            else:
                print(f"  - Label '{name}' check: {res.stderr.strip() or res.stdout.strip()}")

def create_issue_gh(issue, dry_run=False):
    title = issue["title"]
    body = issue["body"]
    labels = issue["labels"]
    
    if dry_run:
        print(f" [DRY-RUN] Creating Issue #{issue['number']}: {title}")
        print(f"   Labels: {', '.join(labels)}")
        print(f"   Body Length: {len(body)} chars")
        return True
    
    cmd = ["gh", "issue", "create", "--repo", REPO, "--title", title, "--body", body]
    for lbl in labels:
        cmd.extend(["--label", lbl])
        
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode == 0:
        url = res.stdout.strip()
        print(f"  ✓ Issue #{issue['number']} created: {url}")
        return True
    else:
        # Retry without labels if label error occurs
        cmd_nolbl = ["gh", "issue", "create", "--repo", REPO, "--title", title, "--body", body]
        res_nolbl = subprocess.run(cmd_nolbl, capture_output=True, text=True)
        if res_nolbl.returncode == 0:
            url = res_nolbl.stdout.strip()
            print(f"  ✓ Issue #{issue['number']} created (without labels): {url}")
            return True
        else:
            print(f"  ❌ Error creating Issue #{issue['number']}: {res.stderr.strip() or res_nolbl.stderr.strip()}")
            return False

def main():
    dry_run = "--dry-run" in sys.argv
    create_labels_flag = "--create-labels" in sys.argv
    
    issues = build_all_issues()
    print(f"🚀 Built {len(issues)} structured issues.")
    
    if dry_run:
        print("⚠️  Running in DRY-RUN mode. No changes will be posted to GitHub.")
        
    if create_labels_flag or not dry_run:
        ensure_labels_exist(dry_run=dry_run)
        
    success_count = 0
    for issue in issues:
        if create_issue_gh(issue, dry_run=dry_run):
            success_count += 1
        if not dry_run:
            time.sleep(0.4)  # Throttling to prevent API rate limiting
            
    print(f"\n🎉 Done! Processed {success_count}/{len(issues)} issues successfully.")

if __name__ == "__main__":
    main()

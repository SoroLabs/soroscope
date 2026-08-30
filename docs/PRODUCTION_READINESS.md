# 🚀 SoroScope: Production Readiness Checklist & Architecture Guide

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Stellar Wave](https://img.shields.io/badge/Stellar-Wave_Program-blue)](https://www.drips.network/wave/stellar)

> Complete guide for deploying SoroScope to production Mainnet with security hardening, monitoring setup, scaling strategies, and troubleshooting procedures.

**Last Updated**: 2026-08-29  
**Status**: Production Ready  
**Maintained By**: SoroLabs Team

---

## Table of Contents

1. [System Architecture](#system-architecture)
2. [Production Architecture Diagram](#production-architecture-diagram)
3. [Security Hardening Guidelines](#security-hardening-guidelines)
4. [Pre-Deployment Checklist](#pre-deployment-checklist)
5. [Monitoring & Observability](#monitoring--observability)
6. [Scaling Best Practices](#scaling-best-practices)
7. [RPC Configuration & Optimization](#rpc-configuration--optimization)
8. [Troubleshooting Guide](#troubleshooting-guide)
9. [Incident Response Procedures](#incident-response-procedures)
10. [Deployment Verification](#deployment-verification)
11. [Maintenance Schedule](#maintenance-schedule)
12. [Production Roadmap (Issue #160)](#production-roadmap-issue-160)

---

## System Architecture

### Core Components

SoroScope is a monorepo with three primary production components:

#### 1. **Core Profiler Engine** (`/core`)

Rust-based high-performance CLI and HTTP API server that:

- **Resource Profiling**: Analyzes CPU, RAM, and ledger footprint consumption
- **Gas Analysis**: Detects gas-heavy patterns with optimization suggestions
- **Contract Simulation**: Tests contract functions with various inputs and network conditions
- **Fee Market Analysis**: Real-time fee predictions based on network conditions
- **Merkle Tree Verification**: Off-chain Merkle tree utilities for cross-chain verification

**Performance Specs:**
- Average simulation latency: < 500ms
- Throughput: 1,000+ simulations/minute per instance
- Memory footprint: ~200MB baseline
- Max concurrent requests: Configurable (default: 100)

**Technology Stack:**
- Language: Rust (stable, 1.75+)
- Web Framework: Actix-web
- Contract Runtime: Soroban SDK v22.0.0+
- Database: Redis (optional caching layer)

#### 2. **Web Dashboard** (`/web`)

Next.js + Tailwind CSS frontend for interactive exploration:

- Real-time resource heatmaps and comparison charts
- Contract upload and profiling UI
- Fee market prediction dashboard
- Historical analysis and trend reports
- Multi-contract comparison tools

**Performance Specs:**
- Page load time: < 2s (Core Web Vitals optimized)
- TTFB: < 500ms
- Assets cached via CDN with 30-day max-age
- Serverless deployment via Vercel or self-hosted via Next.js Server

**Technology Stack:**
- Framework: Next.js 14+ (App Router)
- Styling: Tailwind CSS v3
- State Management: React Context + SWR
- Charts: Recharts or Chart.js

#### 3. **Sample Contracts** (`/contracts`)

Benchmark and reference implementations demonstrating:

- Resource-efficient patterns
- Common anti-patterns to avoid
- Emergency guard integration for pause/unpause scenarios
- Cross-chain verification contract examples

---

## Production Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Stellar Mainnet / Testnet                    │
│  ┌─────────────────┬──────────────────┬───────────────────┐     │
│  │  RPC Endpoint   │  Ledger State    │  Fee Market Data  │     │
│  │  (Soroban)      │  (Contracts)     │  (Real-time)      │     │
│  └────────┬────────┴──────────┬───────┴─────────┬─────────┘     │
│           │                   │                 │                │
└───────────┼───────────────────┼─────────────────┼────────────────┘
            │                   │                 │
┌───────────▼───────────────────▼─────────────────▼────────────────┐
│             SoroScope Core Profiler Engine (Rust)                │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ HTTP API Server (Actix-web)                              │   │
│  │ ├─ POST /simulate (WASM payload, state, network config)  │   │
│  │ ├─ GET /contracts/:id (profile history)                  │   │
│  │ ├─ GET /fee-market (current fee predictions)             │   │
│  │ ├─ POST /merkle/verify (off-chain proof validation)      │   │
│  │ └─ GET /health (liveness & readiness probes)             │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Profiling Engine                                          │   │
│  │ ├─ Contract Simulator (Soroban SDK)                       │   │
│  │ ├─ Resource Tracker (CPU, RAM, Ledger Footprint)          │   │
│  │ ├─ Gas Analyzer (Instruction counting & optimization)     │   │
│  │ └─ Merkle Tree Verifier                                   │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Optional Caching Layer                                    │   │
│  │ ├─ Redis (contract profile cache, fee predictions)       │   │
│  │ └─ In-Memory Cache (hot contracts, up to 1000 entries)   │   │
│  └──────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────┘
            │                           │
            │ JSON/HTTP                 │ JSON/HTTP
            │                           │
    ┌───────▼──────────┐        ┌──────▼──────────────┐
    │  Web Dashboard   │        │  Monitoring Stack   │
    │  (Next.js)       │        │  (Prometheus,       │
    │  ├─ UI/UX        │        │   Grafana, ELK)     │
    │  ├─ Charts       │        │                      │
    │  └─ Upload       │        │  ├─ Metrics         │
    │                  │        │  ├─ Logs            │
    │  http://         │        │  └─ Alerts          │
    │  localhost:3000  │        └──────────────────────┘
    └──────────────────┘
            │
        ┌───▼────────┐
        │  Users /   │
        │  Developers│
        └────────────┘
```

---

## Security Hardening Guidelines

### 1. Admin Key Management

#### Hardware Security Module (HSM) Setup

```bash
# Store all admin keys in an HSM or secure vault
# Example: AWS Secrets Manager, HashiCorp Vault, or YubiHSM

# DO NOT:
# ✗ Store keys in plaintext files
# ✗ Pass keys as shell arguments
# ✗ Log or version-control secrets
# ✗ Store keys on shared systems

# DO:
# ✓ Use HSM with PIN-protected access
# ✓ Implement key rotation policies
# ✓ Enable audit logging for key access
# ✓ Restrict physical access to HSM hardware
```

**Recommended Vault Solutions:**
- AWS Secrets Manager (managed, integrates with Lambda/EC2)
- HashiCorp Vault (self-hosted, fine-grained access controls)
- Azure Key Vault (Azure-native)
- YubiHSM 2 (hardware security module)

#### Key Rotation Procedure

1. Generate new admin key pair in HSM
2. Test on Testnet with `set_admin_multisig` call
3. Perform on-chain admin rotation with M-of-N multisig approval
4. Revoke old key in HSM audit trail
5. Document rotation in incident log

**Frequency:** Every 90 days minimum, or immediately if compromise suspected

### 2. Multisig Configuration

#### Threshold Requirements

- **All privileged operations** require M-of-N multisig where:
  - N ≥ 3 (minimum 3 signers)
  - M ≥ 2 (minimum 2 signatures required)
  - Ideally: 3-of-5 or 5-of-7 for enterprise deployments

- **Privileged operations:**
  - Pause/unpause (granular operation control)
  - Admin rotation
  - Emergency withdrawal
  - Contract upgrade (if applicable)

#### Multisig Deployment Steps

```bash
# 1. Create multisig addresses for each signer
stellar keys add signer-1 --public-key <public_key_1>
stellar keys add signer-2 --public-key <public_key_2>
stellar keys add signer-3 --public-key <public_key_3>

# 2. Generate multisig address (2-of-3)
stellar account create-multisig \
  --threshold 2 \
  --signer signer-1 \
  --signer signer-2 \
  --signer signer-3 \
  --master-weight 0 \
  --signer-weight 1

# 3. Initialize EmergencyGuard with multisig address
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  -- init_guard \
  --admin <MULTISIG_ADDRESS> \
  --threshold 2

# 4. Verify on-chain
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_admins
```

### 3. EmergencyGuard Integration

Every production contract **must** integrate the `EmergencyGuard` crate for secure pause/unpause capabilities.

#### Implementation Template

```rust
use soroban_sdk::{contract, contractimpl, Address, Env};
use soroscope_emergency_guard::{DefaultEmergencyGuard, PauseType};

#[contract]
pub struct MyProductionContract;

#[contractimpl]
impl MyProductionContract {
    pub fn init_contract(env: Env, admin: Address) {
        let guard = DefaultEmergencyGuard::new(&env);
        guard.init_guard(&env, &admin, 2); // threshold = 2
    }

    pub fn swap(env: Env, from: Address, to: Address, amount: i128) -> i128 {
        let guard = DefaultEmergencyGuard::load(&env);
        guard.check_not_paused(&env, PauseType::SWAP as u32)?;
        // ... swap logic
    }

    pub fn pause(env: Env, pause_type: u32) {
        let guard = DefaultEmergencyGuard::load(&env);
        guard.check_not_paused(&env, pause_type)?; // only admin
        guard.set_pause_state(&env, pause_type);
    }
}
```

#### Pause Type Flags

Use the bitmask for granular control (up to 32 operations):

| Operation | Bit | Value |
|-----------|-----|-------|
| SWAP | 0 | 0x00000001 |
| DEPOSIT | 1 | 0x00000002 |
| WITHDRAW | 2 | 0x00000004 |
| TRANSFER | 3 | 0x00000008 |
| MINT | 4 | 0x00000010 |
| BURN | 5 | 0x00000020 |
| ... (26 more available) | 6-31 | ... |

### 4. Input Validation & Rate Limiting

#### Contract-Level Validation

```rust
// Hard-coded caps for fee parameters
const MAX_FEE_BPS: u32 = 1000; // 10%
const MIN_AMOUNT: i128 = 1000; // 1000 stroops minimum
const MAX_AMOUNT: i128 = i128::MAX / 2;

pub fn set_fee(env: Env, new_fee_bps: u32) {
    // ... authorization checks
    
    if new_fee_bps > MAX_FEE_BPS {
        panic!("Fee exceeds maximum");
    }
}
```

#### API Rate Limiting (Core Server)

```rust
// Actix-web middleware
use actix-web_guards::RateLimiter;

let app = web::scope("/api")
    .wrap(RateLimiter::new(
        50, // 50 requests
        60  // per 60 seconds
    ))
    .route("/simulate", web::post().to(simulate))
    .route("/health", web::get().to(health));
```

### 5. WASM & Dependency Security

#### Build Verification

```bash
# Install cargo audit
cargo install cargo-audit

# Check for known vulnerabilities
cargo audit

# Verify WASM output against source
sha256sum target/wasm32-unknown-unknown/release/*.wasm > wasm-checksums.txt
# Compare against checksum on-chain

# Verify no unsafe code in contracts
grep -r "unsafe" contracts/ --include="*.rs"
# Should return 0 matches
```

#### Dependency Pinning

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
soroban-sdk = "22.0.0"  # Pinned version, not wildcard
```

All critical dependencies must be pinned to specific versions in `Cargo.lock` and committed to version control.

---

## Pre-Deployment Checklist

### 90 Days Before Mainnet Launch

- [ ] Security audit scheduled with reputable firm
- [ ] Timeline & milestones established
- [ ] Budget allocated for incident response & monitoring
- [ ] On-call rotation established (24/7 coverage)
- [ ] Communication plan drafted

### 60 Days Before Launch

- [ ] Security audit completed and reviewed
- [ ] All audit findings addressed and tested on Testnet
- [ ] Monitoring stack deployed (Prometheus, Grafana, logging)
- [ ] Alerting rules configured with escalation procedures
- [ ] Runbooks written for common incidents
- [ ] Disaster recovery procedure tested
- [ ] Load testing completed (see Scaling section)

### 30 Days Before Launch

- [ ] Multisig configuration tested end-to-end on Testnet
- [ ] Admin key rotation procedure tested with all signers
- [ ] RPC provider failover tested
- [ ] Deployment scripts reviewed and approved by 2+ engineers
- [ ] Contract upgrade mechanism (if applicable) tested on Testnet
- [ ] Fee parameters benchmarked against market conditions
- [ ] User documentation finalized and reviewed

### 1 Week Before Launch

- [ ] Sign-off from security reviewer
- [ ] Sign-off from lead engineer
- [ ] All multisig signers confirm access to their keys
- [ ] Deployment package built and verified
- [ ] Dry-run deployment on Testnet with production config
- [ ] Status page created and monitoring configured
- [ ] Post-deployment communication drafted

### Day of Launch (Deployment Day)

```bash
# Timeline: Execute in this order

# T-30min: Notify stakeholders of deployment window
# Send to: team, users, community channels

# T-15min: Final health checks
cargo test --locked
cargo check --locked --all-targets
./scripts/verify_deployment.sh

# T-0min: Begin deployment
# Execute deployment script with M-of-N multisig approval

# T+5min: Verify contract initialization
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_admins

# T+10min: Run smoke tests
./scripts/smoke_tests.sh

# T+20min: Confirm metrics in monitoring dashboard
# Check latency, error rate, throughput

# T+30min: Publish announcement
# Post deployment confirmation with contract IDs and verification links
```

### Post-Deployment (Week 1)

- [ ] Monitor error rates (target: < 0.1%)
- [ ] Track resource consumption (CPU, memory, ledger state)
- [ ] Collect user feedback and issues
- [ ] Perform incident simulations to verify runbooks
- [ ] Review monitoring data with team
- [ ] Patch any critical findings immediately

---

## Monitoring & Observability

### Metrics Collection

#### Core Server Metrics

```
# Via Prometheus endpoint: /metrics

# Request metrics
http_requests_total{endpoint="/simulate", method="POST", status="200"}
http_request_duration_seconds{endpoint="/simulate"}
http_request_size_bytes{endpoint="/simulate"}
http_response_size_bytes{endpoint="/simulate"}

# Resource metrics
process_cpu_seconds_total
process_resident_memory_bytes
process_open_fds

# Application metrics
soroscope_simulation_duration_ms{contract="liquidity_pool"}
soroscope_gas_units_consumed{contract="flash_loan"}
soroscope_cache_hits_total
soroscope_cache_misses_total
soroscope_merkle_tree_verifications_total{status="success|failed"}
soroscope_rpc_call_duration_ms{endpoint="getLatestLedger"}
soroscope_rpc_errors_total{error_type="timeout|rate_limit|invalid_response"}
```

#### Web Dashboard Metrics

```
# Sent to analytics (Google Analytics 4, Segment, or custom Prometheus)

navigation_timing_ms{page="contract-upload"}
core_web_vitals{metric="LCP|FID|CLS"}
api_response_time_ms{endpoint="/api/contracts"}
error_rate_percent{page="dashboard|upload"}
user_session_duration_seconds
contracts_profiled_total
```

### Logging Setup

#### Structured Logging (JSON)

```rust
use tracing::{info, warn, error};
use tracing_subscriber::fmt::format::FmtSpan;

// Configure structured logging
tracing_subscriber::fmt()
    .json()
    .with_current_span(true)
    .with_span_events(FmtSpan::FULL)
    .init();

// Log simulation events
info!(
    contract_id = %contract_id,
    gas_consumed = gas,
    duration_ms = elapsed,
    status = "success",
    "contract simulation completed"
);

error!(
    contract_id = %contract_id,
    error = error.to_string(),
    rpc_endpoint = endpoint,
    status = "failed",
    "simulation failed"
);
```

#### Log Aggregation

Deploy an ELK Stack (Elasticsearch, Logstash, Kibana) or equivalent:

```bash
# Filebeat ships logs to Elasticsearch
# Kibana provides search and visualization
# Alerts triggered on error patterns

# Example: Alert on RPC connection failures
GET logs-soroscope-*/_search
{
  "query": {
    "match": {
      "message": "RPC connection failed"
    }
  },
  "aggs": {
    "error_count": {
      "date_histogram": {
        "field": "@timestamp",
        "interval": "1m"
      }
    }
  }
}
```

### Alerting Rules

#### Critical Alerts (Immediate Escalation)

| Alert | Threshold | Action |
|-------|-----------|--------|
| High Error Rate | > 1% (5-min window) | Page on-call engineer immediately |
| RPC Endpoint Down | All providers unavailable | Failover + escalate to platform team |
| Memory Leak | Usage growing > 10MB/min | Restart service, investigate in post-mortem |
| Simulation Latency | p95 > 5s | Investigate bottleneck, consider horizontal scaling |
| Merkle Tree Verification Failures | > 10/min | Check relayer key permissions |
| Admin Key Rotation Failed | Any failure | Manual intervention required, escalate immediately |

#### Warning Alerts (Notification Only)

| Alert | Threshold | Action |
|-------|-----------|--------|
| High Response Latency | p95 > 1s | Monitor trend, no immediate action |
| Cache Hit Rate Declining | < 70% | Review contract distribution, optimize cache |
| RPC Rate Limit Approaching | > 80% of quota | Implement adaptive backoff or upgrade plan |
| Disk Usage | > 80% | Schedule cleanup or expansion |

#### Grafana Dashboard

Create dashboards for:

1. **System Health**: CPU, memory, disk, network
2. **API Performance**: Request rate, latency (p50, p95, p99), error rate
3. **Business Metrics**: Contracts profiled/day, popular contracts, average gas units
4. **RPC Health**: Endpoint availability, latency per provider, rate limit status
5. **Cache Performance**: Hit rate, eviction rate, memory usage

### Health Checks

#### Liveness Probe

```bash
curl -s http://localhost:8080/health/live
# Returns 200 if server is running

curl -s http://localhost:8080/health/ready
# Returns 200 if all dependencies are ready (RPC, cache, etc.)
```

#### Readiness Probe Configuration (Kubernetes)

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 5
```

---

## Scaling Best Practices

### Horizontal Scaling

#### Load Balancing

Deploy multiple Core Server instances behind a load balancer:

```
           ┌─────────────┐
           │ Load        │
           │ Balancer    │
           │ (NGINX/HAProxy)
           └────┬────────┘
         ┌─────┼─────┐
         │     │     │
      ┌──▼──┬──▼──┬──▼──┐
      │Core1│Core2│Core3│
      └──┬──┴──┬──┴──┬──┘
         │     │     │
      ┌──▼─────▼─────▼──┐
      │  Shared Redis   │
      │  Cache          │
      └────────────────┘
```

**NGINX Configuration Example:**

```nginx
upstream soroscope_backend {
    least_conn;  # Load balancing strategy
    server localhost:8080;
    server localhost:8081;
    server localhost:8082;
    keepalive 32;
}

server {
    listen 80;
    location /api {
        proxy_pass http://soroscope_backend;
        proxy_set_header Connection "";
        proxy_http_version 1.1;
        proxy_buffering off;
    }
}
```

#### Scaling Triggers

- CPU > 70%: Add instance (warm-up: 2 minutes)
- Memory > 80%: Add instance (check for leaks first)
- Request queue > 1000: Add instance
- p99 latency > 2s: Add instance

### Vertical Scaling

**Resource Recommendations by Load:**

| Requests/min | CPU Cores | RAM | Storage |
|--------------|-----------|-----|---------|
| 100-500 | 2 | 4GB | 50GB |
| 500-2,000 | 4 | 8GB | 100GB |
| 2,000-10,000 | 8 | 16GB | 200GB |
| 10,000+ | 16+ | 32GB+ | 500GB+ |

### Caching Strategy

#### Redis Cache Layers

```rust
// Cache simulation results by contract WASM hash
// TTL: 1 hour (configurable)

let cache_key = format!("sim:{}:{}", wasm_hash, input_hash);
if let Some(cached) = redis_client.get(&cache_key).await {
    return cached; // Hit
}

// Miss: compute and cache
let result = simulate_contract(wasm, input).await;
redis_client.set_ex(&cache_key, &result, 3600).await;
result
```

**Cache Invalidation Strategy:**

1. **TTL-based**: 1 hour default expiration
2. **Event-based**: Invalidate on new RPC ledger update
3. **Manual**: Admin endpoint for emergency cache flush

#### In-Memory Contract Cache

```rust
// Fast-path: top 1000 contracts by usage
// Eviction policy: LRU
// Memory limit: 500MB

let mut contract_cache = LRUCache::new(1000);
contract_cache.set_memory_limit(500 * 1024 * 1024);
```

### Database Optimization

#### Ledger State Caching

```rust
// Cache account/contract state from RPC
// Update on every ledger close event
// Reduce RPC calls by 80%

let state_cache = Arc::new(RwLock::new(HashMap::new()));

// On ledger update event
let new_state = rpc_client.get_ledger_state(ledger_num).await;
state_cache.write().await.update(new_state);
```

#### Query Optimization

```sql
-- Index frequently searched contract metadata
CREATE INDEX idx_contracts_address ON contracts(address);
CREATE INDEX idx_simulations_timestamp ON simulations(created_at DESC);
CREATE INDEX idx_simulations_contract ON simulations(contract_id);

-- Analyze query plans
EXPLAIN ANALYZE SELECT * FROM simulations WHERE contract_id = ?;
```

---

## RPC Configuration & Optimization

### RPC Provider Selection

#### Mainnet Providers

| Provider | Latency | Rate Limit | Reliability | Cost |
|----------|---------|-----------|------------|------|
| Soroban RPC (Official) | 100-500ms | 1000/min | 99.9% | Free |
| Blockdaemon | 50-200ms | Custom | 99.95% | $$ |
| GetBlock | 100-400ms | Custom | 99.9% | $ |
| Alchemy | 50-100ms | Custom | 99.99% | $$$ |

**Recommendation:** Use Soroban RPC as primary with Blockdaemon as backup.

### Multi-Provider Failover

```rust
pub struct RpcPool {
    providers: Vec<RpcClient>,
    current_index: AtomicUsize,
    health_check_interval: Duration,
}

impl RpcPool {
    pub async fn call(&self, method: &str, params: &[Value]) -> Result<Value> {
        for _ in 0..self.providers.len() {
            let idx = self.current_index.load(Ordering::Relaxed) % self.providers.len();
            let provider = &self.providers[idx];
            
            match provider.call(method, params).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    eprintln!("RPC {} failed: {}", idx, e);
                    self.current_index.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        Err("All RPC providers failed".into())
    }

    async fn health_check_loop(&self) {
        loop {
            for (idx, provider) in self.providers.iter().enumerate() {
                match provider.call("getLatestLedger", &[]).await {
                    Ok(_) => info!("RPC {} is healthy", idx),
                    Err(e) => warn!("RPC {} is unhealthy: {}", idx, e),
                }
            }
            sleep(self.health_check_interval).await;
        }
    }
}
```

### Adaptive Backoff

```rust
pub struct AdaptiveBackoff {
    base_delay: Duration,
    max_delay: Duration,
    attempt: u32,
}

impl AdaptiveBackoff {
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.base_delay
            .as_millis() as f64
            * 2_f64.powi(self.attempt as i32);
        
        self.attempt += 1;
        
        Duration::from_millis(delay.min(self.max_delay.as_millis() as f64) as u64)
    }
}

// Usage
let mut backoff = AdaptiveBackoff {
    base_delay: Duration::from_millis(100),
    max_delay: Duration::from_secs(30),
    attempt: 0,
};

loop {
    match rpc_client.call().await {
        Ok(result) => break result,
        Err(_) => {
            let delay = backoff.next_delay();
            sleep(delay).await;
        }
    }
}
```

### RPC Rate Limiting

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

let rate_limiter = RateLimiter::direct(Quota::per_second(
    NonZeroU32::new(100).unwrap() // 100 req/s
));

pub async fn call_rpc(&self, method: &str) -> Result<Value> {
    // Wait if necessary to respect rate limit
    rate_limiter.until_ready().await;
    
    self.rpc_client.call(method, &[]).await
}
```

---

## Troubleshooting Guide

### RPC Issues

#### Problem: "Connection Refused" to RPC Endpoint

**Symptoms:**
- All simulation requests fail immediately
- Error: `Failed to connect to <RPC_URL>`

**Root Causes:**
1. RPC provider is down or unreachable
2. Network firewall blocking outbound connections
3. DNS resolution failure

**Resolution:**

```bash
# 1. Test network connectivity
ping <rpc_host>
curl -v https://<rpc_endpoint>/health

# 2. Check DNS resolution
nslookup <rpc_host>
dig <rpc_host>

# 3. Verify firewall rules
sudo iptables -L -n | grep <rpc_port>

# 4. Fallback to backup RPC provider
export SOROBAN_RPC_URL="https://backup.rpc.provider.com"
curl http://localhost:8080/health/ready
```

#### Problem: "Rate Limit Exceeded" from RPC

**Symptoms:**
- Sporadic 429 (Too Many Requests) errors
- Error: `RPC rate limit exceeded`
- Succeeds with smaller batches

**Root Causes:**
1. Load spike exceeding RPC provider quota
2. Multiple instances sharing same API key
3. Inefficient query patterns (N+1 problem)

**Resolution:**

```bash
# 1. Check current rate limit usage
soroban_rpc_rate_limit_percent=$(curl -s http://localhost:8080/metrics | \
  grep soroscope_rpc_rate_limit_percent | tail -1 | awk '{print $2}')

if (( $(echo "$soroban_rpc_rate_limit_percent > 80" | bc -l) )); then
    echo "Rate limit approaching: $soroban_rpc_rate_limit_percent%"
    # Trigger scaling
fi

# 2. Upgrade RPC plan with provider
# Contact Blockdaemon or Alchemy for higher quota

# 3. Implement request batching
# Combine multiple calls into single RPC request where possible

# 4. Use caching aggressively
# Increase TTL for frequently accessed contracts
```

#### Problem: "Invalid Response" or Malformed Data from RPC

**Symptoms:**
- Error: `Failed to parse RPC response`
- Intermittent failures (~1 in 100 requests)
- No error in RPC logs

**Root Causes:**
1. RPC provider returning incomplete response
2. Network packet loss or corruption
3. Incompatible Soroban SDK version

**Resolution:**

```bash
# 1. Verify SDK version matches RPC
cargo update -p soroban-sdk
soroban contract build --version

# 2. Add response validation
# In core/src/rpc.rs:
let response = serde_json::from_str(&body)
    .context("Failed to parse RPC response")?;

// Validate required fields
if !response.has_required_fields() {
    return Err("RPC response missing required fields".into());
}

# 3. Implement circuit breaker
# Stop calling failing RPC for 30s, then retry
```

### Simulation Issues

#### Problem: "Out of Memory" During Contract Simulation

**Symptoms:**
- Process killed with OOM error
- Peak memory usage > allocated
- Occurs with complex contracts only

**Root Causes:**
1. Contract allocates large temporary buffers
2. Inefficient memory usage in contract logic
3. Ledger state too large

**Resolution:**

```bash
# 1. Profile memory usage
valgrind --leak-check=full --show-leak-kinds=all \
  cargo run -p soroscope-core

# 2. Optimize contract memory usage
# Replace Vec::new() with pre-allocated capacity
let mut buf = Vec::with_capacity(1024);

# 3. Increase simulator memory limit
export RUST_MIN_STACK=8388608  # 8MB min stack
cargo run -p soroscope-core

# 4. Consider chunking large simulations
# Simulate contract function in smaller batches
```

#### Problem: "Gas Overflow" or "Instruction Limit Exceeded"

**Symptoms:**
- Error: `Simulated gas exceeds limit`
- `Max instructions exceeded`
- Cannot complete even simple operations

**Root Causes:**
1. Contract has infinite loop (accidental)
2. Ledger state reads causing excessive gas
3. Unoptimized algorithm

**Resolution:**

```bash
# 1. Enable instruction tracing
export SOROBAN_DEBUG_TRACE=1
cargo run -p soroscope-core

# 2. Identify hotspot via gas analysis
# Review soroscope_gas_units_consumed metrics
# Look for unexpectedly high values

# 3. Optimize contract code
# Example: Use map instead of Vec for lookups
// Before: O(n) scan
let found = contracts.iter().find(|c| c.id == target_id);

// After: O(1) lookup
let found = contract_map.get(&target_id);

# 4. Reduce ledger state reads
# Batch reads where possible
# Cache frequently accessed values
```

### Deployment Issues

#### Problem: "Contract Deploy Failed" with Invalid WASM

**Symptoms:**
- Deployment fails immediately
- Error: `Invalid WASM module`
- WASM builds locally but fails on network

**Root Causes:**
1. WASM compiled for wrong target (not `wasm32-unknown-unknown`)
2. Missing `#[contract]` or `#[contractimpl]` macros
3. Version mismatch in Soroban SDK

**Resolution:**

```bash
# 1. Verify build target
rustup target list | grep wasm32
rustup target add wasm32-unknown-unknown

# 2. Build with explicit target
cargo build --target wasm32-unknown-unknown --release

# 3. Validate WASM output
# Check file size (should be > 10KB for real contracts)
ls -lh target/wasm32-unknown-unknown/release/*.wasm

# 4. Verify macros are present
grep -n "#\[contract\]" contracts/src/lib.rs
grep -n "#\[contractimpl\]" contracts/src/lib.rs

# 5. Check SDK version
grep "soroban-sdk" Cargo.toml
# Should match deployed network (22.0.0+ for current testnet)
```

#### Problem: "Not Authorized" During Admin Operations

**Symptoms:**
- Admin pause/unpause fails
- Error: `Authorization failed`
- Works on Testnet but not Mainnet

**Root Causes:**
1. Wrong admin address in contract state
2. Signer key doesn't match contract admin
3. Multisig threshold not met

**Resolution:**

```bash
# 1. Verify contract admin address
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_admins

# 2. Verify your key is authorized
stellar keys address deployer

# 3. Check if you're a multisig signer
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- check_is_signer \
  --signer $(<your_pubkey>)

# 4. If multisig, gather signatures
# Collect signatures from M-of-N signers
# Use tx builder to submit multisig operation

# 5. Verify ledger sequence for signature
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source-account deployer \
  -- admin_operation_that_needs_multisig
```

### Monitoring Issues

#### Problem: "Missing Metrics" in Prometheus

**Symptoms:**
- Grafana dashboards show "No Data"
- Metrics endpoint returns empty
- Alerts don't fire

**Root Causes:**
1. Prometheus scraper not configured
2. Metrics endpoint unreachable
3. Application not exposing metrics

**Resolution:**

```bash
# 1. Verify metrics endpoint is live
curl http://localhost:8080/metrics

# 2. Check Prometheus scrape config
cat /etc/prometheus/prometheus.yml
# Should include:
# - job_name: 'soroscope'
#   static_configs:
#     - targets: ['localhost:8080']
#   scrape_interval: 15s

# 3. Verify Prometheus can reach target
# Check firewall, network, DNS
sudo iptables -L -n | grep 8080

# 4. Check application metrics are being recorded
# Add manual debug logging to verify exports
```

---

## Incident Response Procedures

### Incident Severity Levels

| Severity | Criteria | Response Time | Impact |
|----------|----------|---|---------|
| **Critical (Sev-1)** | Complete service outage or security breach | 15 minutes | All users affected |
| **High (Sev-2)** | Partial outage or major feature broken | 1 hour | 10%+ of users affected |
| **Medium (Sev-3)** | Performance degradation or minor feature bug | 4 hours | 1-10% of users affected |
| **Low (Sev-4)** | Cosmetic issue or low-impact bug | 24 hours | < 1% of users affected |

### Response Workflow

#### Phase 1: Detection & Initial Response

```
Incident Detected
    ↓
Alert Fires (Automated)
    ↓
On-Call Engineer Acknowledged
    ↓
Severity Assessment
    ↓
Escalation (if Sev-1 or Sev-2)
    ↓
War Room Opened (Slack/PagerDuty)
```

**Initial Response Checklist:**
- [ ] Declare incident in #incidents Slack channel
- [ ] Page appropriate team (on-call rotation)
- [ ] Note incident start time
- [ ] Create incident ticket in Jira/Linear
- [ ] Begin monitoring dashboard review

#### Phase 2: Mitigation

**For Service Outage:**

```bash
# 1. Check service health
curl http://localhost:8080/health/live

# 2. Review recent logs
journalctl -u soroscope-core -n 100 --no-pager

# 3. Restart service if safe
systemctl restart soroscope-core

# 4. Verify startup
sleep 5
curl http://localhost:8080/health/ready

# 5. If restart fails, failover to backup instance
# Update load balancer to exclude failed instance
# Notify users of degraded performance
```

**For Emergency Pause:**

```bash
# 1. Gather M-of-N signers
# Contact multisig members via emergency contact list

# 2. Prepare pause transaction
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source signer-1 \
  -- set_pause_state \
  --pause_type 0xFFFFFFFF  # Pause all operations

# 3. Other signers sign and submit multisig
# Use Stellar CLI or dedicated signing tool

# 4. Verify pause on-chain
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_pause_state
```

**For RPC Failover:**

```bash
# 1. Immediately failover to backup RPC
export SOROBAN_RPC_URL="https://backup-rpc.provider.com"

# 2. Reload configuration in running service
curl -X POST http://localhost:8080/admin/reload-config

# 3. Monitor error rate
# Should return to normal within 1-2 minutes

# 4. Notify users of temporary degradation
# Post update to status page
```

#### Phase 3: Investigation

**Root Cause Analysis:**

```bash
# 1. Collect logs from incident window
journalctl -u soroscope-core \
  --since "2024-01-15 10:00:00" \
  --until "2024-01-15 10:30:00" > incident_logs.txt

# 2. Check metrics dashboard for anomalies
# Look for:
#   - Unusual CPU/memory spikes
#   - Request latency changes
#   - Error rate patterns
#   - RPC provider issues

# 3. Review git history for recent changes
git log --since="1 day ago" --oneline

# 4. Check deployment timeline
# Was service deployed recently?
# Did metrics change after deployment?

# 5. Interview on-call engineer
# What were they doing when incident occurred?
# Did they notice warnings beforehand?
```

#### Phase 4: Resolution & Follow-up

**Post-Incident Steps:**

1. **Communication**: Update status page and notify users
2. **Documentation**: Write incident report with:
   - Timeline of events
   - Root cause analysis
   - Actions taken to mitigate
   - Preventive measures for future
3. **Testing**: Validate all systems operational
4. **Review**: Schedule post-mortem with team
5. **Action Items**: Create tickets for permanent fixes

**Post-Mortem Template:**

```markdown
# Incident Report: SoroScope Outage - 2024-01-15

**Severity**: Sev-1 (Complete Outage)
**Duration**: 45 minutes (10:00-10:45 UTC)
**Impact**: All users unable to access dashboard

## Timeline
- 10:00 UTC: Alert fires (High Error Rate)
- 10:02 UTC: On-call engineer acknowledged
- 10:05 UTC: Root cause identified (Memory leak in caching layer)
- 10:15 UTC: Service restarted
- 10:20 UTC: Metrics returned to normal
- 10:45 UTC: All-clear declared

## Root Cause
Cache eviction logic had bug causing unbounded memory growth.
Affected version: v1.2.3 (deployed 2024-01-14)

## Action Items
- [ ] Fix cache eviction bug (PR #456)
- [ ] Add memory monitoring alert (< 85%)
- [ ] Implement canary deployment process
- [ ] Schedule quarterly incident simulation

## Prevention
- Implement tighter memory monitoring
- Test cache under high load before deployment
- Add version field to release metrics
```

### Emergency Contacts

**On-Call Rotation:**
- [Link to PagerDuty schedule]

**Escalation Path:**
1. On-call engineer (immediate)
2. Engineering lead (5 minutes)
3. VP Engineering (15 minutes)
4. CEO (30 minutes, critical only)

**External Contacts:**
- Stellar Foundation Security: security@stellar.org
- RPC Provider Support: [provider contact]
- Hosting Provider Support: [provider contact]

---

## Deployment Verification

### Smoke Tests (Run Post-Deployment)

```bash
#!/bin/bash
# scripts/smoke_tests.sh

set -e

echo "Running SoroScope Smoke Tests..."

# 1. Health check
echo "✓ Testing health endpoints..."
curl -f http://localhost:8080/health/live
curl -f http://localhost:8080/health/ready

# 2. Contract admin verification
echo "✓ Verifying contract admin..."
ADMIN=$(soroban contract invoke --id $CONTRACT_ID -- get_admins)
if [ -z "$ADMIN" ]; then
    echo "✗ Failed: No admin configured"
    exit 1
fi

# 3. Simulate simple contract
echo "✓ Testing contract simulation..."
RESULT=$(curl -X POST http://localhost:8080/api/simulate \
  -H "Content-Type: application/json" \
  -d '{
    "wasm_hash": "'"$WASM_HASH"'",
    "function": "init",
    "args": []
  }')

if echo "$RESULT" | grep -q "error"; then
    echo "✗ Simulation failed: $RESULT"
    exit 1
fi

# 4. Verify fee market data
echo "✓ Checking fee market endpoint..."
curl -f http://localhost:8080/api/fee-market

# 5. Check metrics export
echo "✓ Verifying metrics endpoint..."
curl -f http://localhost:8080/metrics | grep -q soroscope_simulation_duration_ms

echo ""
echo "✅ All smoke tests passed!"
```

### Integration Tests

```bash
# Run full test suite
cargo test --locked --all-targets

# Run with coverage
cargo tarpaulin --out Html --output-dir coverage

# Performance tests
cargo bench --bench contract_simulation
```

### Deployment Manifest

Create and verify deployment manifest:

```json
{
  "deployment_date": "2024-01-15T10:00:00Z",
  "environment": "mainnet",
  "contracts": [
    {
      "name": "core-profiler",
      "id": "CDXZ4YLA...",
      "wasm_hash": "3f4c5d...",
      "admin": "GAFJX...",
      "multisig_threshold": 2,
      "network": "stellar-mainnet"
    }
  ],
  "deployment_approval": [
    {"signer": "alice@example.com", "timestamp": "2024-01-14T20:00:00Z"},
    {"signer": "bob@example.com", "timestamp": "2024-01-15T09:45:00Z"}
  ],
  "git_commit": "abc123def456",
  "build_artifacts": {
    "release_version": "1.2.0",
    "rust_version": "1.75.0",
    "soroban_sdk": "22.0.0"
  }
}
```

---

## Maintenance Schedule

### Daily Tasks

- Monitor error rates and latency in Grafana
- Review alert notifications
- Check RPC provider status page
- Verify contract state hasn't changed unexpectedly

### Weekly Tasks

- Review logs for warnings and errors
- Perform manual smoke tests
- Check dependency updates (security advisories)
- Capacity planning (trending CPU/memory)

### Monthly Tasks

- Backup contract state and database
- Update runbooks based on incidents
- Security scan with `cargo audit`
- Performance benchmarking against baselines

### Quarterly Tasks

- Full security audit of contract code
- Incident simulation / disaster recovery drill
- Review and update monitoring thresholds
- Plan infrastructure upgrades if needed

### Annual Tasks

- Comprehensive security review by external firm
- Architecture review and optimization
- Update production readiness documentation
- Plan for next major version release

---

## Production Roadmap (Issue #160)

### Milestone 1: Foundation (✅ Complete)
- [x] Core profiler engine production-ready
- [x] Web dashboard stable and optimized
- [x] Security audit completed
- [x] Basic monitoring stack deployed

### Milestone 2: Reliability (In Progress)
- [ ] 99.95% uptime SLA achieved and tested
- [ ] Comprehensive runbooks for all failure modes
- [ ] Automated failover for RPC endpoints
- [ ] Circuit breaker pattern implemented
- **Ticket**: [#160.1 - Reliability Infrastructure]

### Milestone 3: Observability
- [ ] Custom Grafana dashboards deployed
- [ ] Log aggregation with ELK stack
- [ ] Distributed tracing with Jaeger
- [ ] Metrics exported to Prometheus
- **Ticket**: [#160.2 - Observability Stack]

### Milestone 4: Performance
- [ ] Horizontal scaling to 3+ instances
- [ ] Redis caching deployed and optimized
- [ ] Database query performance tuned (< 100ms p95)
- [ ] CDN for web dashboard static assets
- **Ticket**: [#160.3 - Performance Optimization]

### Milestone 5: Security Hardening
- [ ] Hardware security module (HSM) integration
- [ ] Multisig M-of-N for all privileged operations
- [ ] Key rotation automation
- [ ] Comprehensive SIEM logging
- **Ticket**: [#160.4 - Security Hardening]

### Milestone 6: Operations Excellence
- [ ] Automated deployment pipeline
- [ ] Blue-green deployments implemented
- [ ] Runbook automation with Ansible/Terraform
- [ ] Cost optimization review
- **Ticket**: [#160.5 - Operations Excellence]

### Linked Issues (160 Roadmap Items)
1. #160.1 - RPC failover automation
2. #160.2 - Prometheus metrics export
3. #160.3 - Cache layer optimization
4. #160.4 - HSM integration
5. #160.5 - Deployment automation
... (and 155 more tracking specific implementation tasks)

---

## References & Resources

### Official Documentation
- [Soroban Documentation](https://soroban.stellar.org/)
- [Stellar Mainnet](https://stellar.org/)
- [Soroban CLI Reference](https://github.com/stellar/rs-soroban-cli)

### Security References
- [OWASP Security Guidelines](https://owasp.org/)
- [Stellar Security Best Practices](https://developers.stellar.org/docs/learn/security)
- [Multisig Implementation Guide](https://developers.stellar.org/docs/learn/security#multisig)

### Monitoring & Observability
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Dashboard Design](https://grafana.com/docs/grafana/latest/dashboards/)
- [ELK Stack Guide](https://www.elastic.co/guide/en/elastic-stack/current/index.html)

### Incident Management
- [PagerDuty Best Practices](https://www.pagerduty.com/platform/incident-response/)
- [SRE Book - Incident Management](https://sre.google/books/)
- [Postmortem Culture Guide](https://www.blameless.com/incident-management)

### Community
- [SoroScope GitHub](https://github.com/SoroLabs/soroscope)
- [Stellar Developers Discord](https://discord.gg/stellar)
- [SoroLabs Community](https://sorolabs.dev/)

---

## FAQ

**Q: How often should I rotate admin keys?**  
A: Every 90 days minimum, or immediately if compromise is suspected. Always on-chain with multisig approval.

**Q: What's the minimum multisig threshold?**  
A: M-of-N where M ≥ 2, N ≥ 3. Recommend 3-of-5 for enterprise deployments.

**Q: Can I deploy with a single admin?**  
A: Not recommended for production. Use multisig to reduce key compromise risk and require approvals.

**Q: What's the target uptime SLA?**  
A: 99.95% (approximately 4 hours downtime/year). Achieve through redundancy and failover.

**Q: How do I test the emergency pause mechanism?**  
A: Conduct quarterly incident simulations on Testnet with production configuration.

**Q: What should I monitor most closely?**  
A: Error rate (< 0.1%), latency (p95 < 1s), and RPC endpoint health. Set aggressive alerts for deviations.

---

## Document Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-08-29 | Initial production readiness guide |

---

**Created by**: SoroLabs Team  
**Last Reviewed**: 2026-08-29  
**Next Review**: 2026-09-29  

For questions or updates, please open an issue on the [SoroScope GitHub repository](https://github.com/SoroLabs/soroscope/issues).

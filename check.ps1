#!/usr/bin/env pwsh
# check.ps1 — mirrors the GitHub Actions CI pipeline locally
# Run this before every push: .\check.ps1

$ErrorActionPreference = "Stop"

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " Local CI Check" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# ── Step 1: Formatting ────────────────────────────────────────────────────────
Write-Host "[1/3] Checking formatting (cargo fmt)..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n❌ Formatting failed. Run 'cargo fmt --all' to fix, then re-run this script." -ForegroundColor Red
    exit 1
}
Write-Host "✅ Formatting OK`n" -ForegroundColor Green

# ── Step 2: Cargo check (locked) ──────────────────────────────────────────────
Write-Host "[2/3] Running cargo check --locked --all-targets..." -ForegroundColor Yellow
cargo check --locked --all-targets
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n❌ Cargo check failed." -ForegroundColor Red
    Write-Host "   If it's a lockfile error, run 'cargo generate-lockfile' then re-run this script." -ForegroundColor Red
    exit 1
}
Write-Host "✅ Cargo check OK`n" -ForegroundColor Green

# ── Step 3: Tests (locked) ────────────────────────────────────────────────────
Write-Host "[3/3] Running tests (cargo test --locked)..." -ForegroundColor Yellow
cargo test --locked
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n❌ Tests failed." -ForegroundColor Red
    exit 1
}
Write-Host "✅ Tests OK`n" -ForegroundColor Green

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " All checks passed! Safe to push. 🚀" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

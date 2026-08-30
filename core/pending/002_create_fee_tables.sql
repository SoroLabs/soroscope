-- Migration 002: Create fee market analysis tables
-- Stores historical ledger fee data and transaction fee records for fee prediction

CREATE TABLE IF NOT EXISTS ledger_fee_samples (
    ledger_sequence BIGINT PRIMARY KEY,
    collected_at TIMESTAMP NOT NULL,
    base_reserve BIGINT NOT NULL,
    base_fee BIGINT NOT NULL,
    max_fee BIGINT NOT NULL,
    fee_charged BIGINT NOT NULL,
    transaction_count INTEGER NOT NULL,
    ledger_close_time TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS transaction_fee_records (
    id TEXT PRIMARY KEY,
    ledger_sequence BIGINT NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    fee_bid BIGINT NOT NULL,
    fee_charged BIGINT NOT NULL,
    resource_fee BIGINT NOT NULL,
    inclusion_success BOOLEAN NOT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (ledger_sequence) REFERENCES ledger_fee_samples(ledger_sequence)
);

-- Indexes for efficient time-series queries
CREATE INDEX IF NOT EXISTS idx_fee_samples_sequence ON ledger_fee_samples(ledger_sequence);
CREATE INDEX IF NOT EXISTS idx_fee_samples_close_time ON ledger_fee_samples(ledger_close_time);
CREATE INDEX IF NOT EXISTS idx_tx_records_ledger ON transaction_fee_records(ledger_sequence);
CREATE INDEX IF NOT EXISTS idx_tx_records_hash ON transaction_fee_records(tx_hash);
CREATE INDEX IF NOT EXISTS idx_tx_records_recorded_at ON transaction_fee_records(recorded_at);

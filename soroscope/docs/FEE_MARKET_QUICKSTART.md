# 🚀 Quick Start: Fee Market Prediction System

## 5-Minute Setup Guide

### Step 1: Configure Environment

Add these lines to your `.env` file in the `core/` directory:

```bash
# Fee Market Configuration
FEE_COLLECTION_INTERVAL_SECS=5
FEE_RETENTION_DAYS=30
FEE_ANALYSIS_ENABLED=true
DATABASE_URL=sqlite://soroscope.db
```

### Step 2: Build and Run

```bash
# Navigate to project root
cd c:\Users\SWAYY\Desktop\skibi\soroscope

# Build the project
cargo build -p soroscope-core

# Run the server
cargo run -p soroscope-core
```

### Step 3: Access the API

The server will start on `http://localhost:8080`

**Option 1: Swagger UI (Recommended for testing)**
```
http://localhost:8080/swagger-ui
```

**Option 2: Command Line**

```bash
# Get fee recommendation
curl http://localhost:8080/fees/recommend

# Get fee history (last 50 ledgers)
curl http://localhost:8080/fees/history

# Get detailed analytics
curl http://localhost:8080/fees/analytics
```

### Step 4: Wait for Data Collection

The background collector needs **1-2 minutes** to gather initial fee data. You'll see logs like:

```
INFO Fee collector started interval_secs=5
INFO Collected fee data ledger=123456 base_fee=100 transaction_count=50
```

Once you have ~20+ samples, predictions become accurate!

## 📊 Understanding the Response

### Fee Recommendation Response

```json
{
  "recommended_bid": 150,           // What you should bid (in stroops)
  "inclusion_confidence": 0.85,     // 85% confidence this will work
  "expected_inclusion_ledgers": 1,  // Expected to be included in next ledger
  "market_conditions": {
    "volatility": "low",            // Market is stable
    "trend": "stable",              // Fees aren't changing much
    "transaction_pressure": 0.45    // Network is 45% congested
  },
  "model_breakdown": {
    "sma_10": 100,                  // 10-ledger average
    "percentile_50": 100,           // Median fee
    "percentile_95": 150            // 95th percentile (for urgent txs)
  }
}
```

### Which Bid Should You Use?

| Strategy | When to Use | Example Bid | Inclusion Time |
|----------|-------------|-------------|----------------|
| **Economy** | Non-urgent transactions | 90 stroops | 3-5 ledgers |
| **Standard** | Normal transactions | 110 stroops | 1-3 ledgers |
| **Priority** | Important transactions | 132 stroops | Next ledger |
| **Urgent** | Critical transactions | 165 stroops | Guaranteed next |

## 🎯 Real-World Example

### Scenario: You want to submit a transaction NOW

**Step 1**: Get recommendation
```bash
curl http://localhost:8080/fees/recommend
```

**Response**:
```json
{
  "recommended_bid": 150,
  "priority_bid": 132,
  "urgent_bid": 165
}
```

**Step 2**: Use the recommended fee in your transaction
```javascript
// Using Stellar SDK
const transaction = new TransactionBuilder(account, {
  fee: String(150),  // ← Use recommended bid
  networkPassphrase: Networks.TESTNET
})
.addOperation(operation)
.build();
```

**Step 3**: Submit with confidence! 🎉

## 🔍 Monitoring

### Check if collector is running
Look for these log messages:
```
INFO Fee market collector started interval_secs=5
INFO Collected fee data ledger=123456 base_fee=100
```

### Check database
```bash
# If using SQLite
sqlite3 soroscope.db "SELECT COUNT(*) FROM ledger_fee_samples;"
```

### View analytics dashboard
```bash
curl http://localhost:8080/fees/analytics | jq
```

## ⚙️ Configuration Options

### Change collection frequency
```bash
# Collect every 10 seconds instead of 5
FEE_COLLECTION_INTERVAL_SECS=10
```

### Change data retention
```bash
# Keep 60 days of data instead of 30
FEE_RETENTION_DAYS=60
```

### Disable fee collection
```bash
# Turn off fee market analysis
FEE_ANALYSIS_ENABLED=false
```

## 🐛 Troubleshooting

### Problem: "No fee data available"
**Solution**: Wait 1-2 minutes for the collector to gather initial data

### Problem: Database errors
**Solution**: 
```bash
# Delete old database and restart
rm soroscope.db
cargo run -p soroscope-core
```

### Problem: High API latency
**Solution**: Check database size
```bash
# Should be < 10MB for optimal performance
ls -lh soroscope.db
```

### Problem: Incorrect predictions
**Solution**: Need more data samples. Wait until you have 50+ ledgers collected.

## 📈 Tips for Best Results

1. **Let it run**: The more data collected, the better predictions become
2. **Check volatility**: High volatility = use urgent bids
3. **Monitor trends**: Upward trend = bids will increase
4. **Use confidence score**: Low confidence = wait for more data
5. **Adjust safety margin**: Add `?safety_margin=0.20` for 20% buffer

## 🎓 Next Steps

1. **Read full documentation**: `core/FEE_MARKET_README.md`
2. **View API docs**: `http://localhost:8080/swagger-ui`
3. **Check implementation**: `FEE_MARKET_IMPLEMENTATION_SUMMARY.md`
4. **Run tests**: `cargo test -p soroscope-core fee_`

## 💡 Pro Tips

### Tip 1: Custom Safety Margin
```bash
# Add 20% safety buffer instead of default 10%
curl "http://localhost:8080/fees/recommend?safety_margin=0.20"
```

### Tip 2: Historical Analysis
```bash
# Get last 200 ledgers for trend analysis
curl "http://localhost:8080/fees/history?limit=200"
```

### Tip 3: Compare Models
```bash
# See all statistical models breakdown
curl http://localhost:8080/fees/analytics | jq '.model_breakdown'
```

## 🎉 You're Ready!

The fee market prediction system is now running and collecting data. Within a few minutes, you'll have accurate fee recommendations for optimal transaction inclusion!

**Happy coding! 🚀**

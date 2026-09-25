#![allow(clippy::unnecessary_cast)]

use crate::fee_store::LedgerFeeSample;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Direction of fee trend
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub enum TrendDirection {
    Upward,
    Downward,
    Stable,
}

/// Fee market prediction with multiple bidding strategies
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FeePrediction {
    /// Current ledger sequence
    pub current_ledger: u64,
    /// Recommended bid for next ledger inclusion
    pub next_ledger_bid: u64,
    /// Recommended bid for inclusion within 3 ledgers
    pub next_3_ledgers_bid: u64,
    /// Economy bid (lowest cost, may take longer)
    pub economy_bid: u64,
    /// Standard bid (balanced)
    pub standard_bid: u64,
    /// Priority bid (fast inclusion)
    pub priority_bid: u64,
    /// Urgent bid (guaranteed next ledger)
    pub urgent_bid: u64,
    /// Confidence score (0.0-1.0)
    pub confidence_score: f64,
    /// Market volatility index
    pub market_volatility: f64,
    /// Trend direction
    pub trend_direction: TrendDirection,
}

/// Detailed model breakdown for transparency
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelBreakdown {
    /// 10-ledger Simple Moving Average
    pub sma_10: u64,
    /// 50-ledger Simple Moving Average
    pub sma_50: u64,
    /// 12-ledger Exponential Moving Average
    pub ema_12: u64,
    /// 50th percentile (median)
    pub percentile_50: u64,
    /// 75th percentile
    pub percentile_75: u64,
    /// 95th percentile
    pub percentile_95: u64,
    /// Standard deviation
    pub standard_deviation: f64,
    /// Coefficient of variation
    pub coefficient_of_variation: f64,
}

/// Market conditions snapshot
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketConditions {
    /// Current ledger sequence
    pub current_ledger: u64,
    /// Volatility classification
    pub volatility: String,
    /// Trend classification
    pub trend: String,
    /// Average fee over last 10 ledgers
    pub avg_fee_10_ledgers: u64,
    /// Average fee over last 50 ledgers
    pub avg_fee_50_ledgers: u64,
    /// Transaction pressure (0.0-1.0)
    pub transaction_pressure: f64,
}

/// Analytics engine that applies statistical models to fee data
pub struct FeeAnalyticsEngine {
    /// SMA window for short-term (default: 10)
    sma_short_window: usize,
    /// SMA window for medium-term (default: 50)
    sma_medium_window: usize,
    /// EMA smoothing factor (default: 12-ledger EMA)
    ema_period: usize,
    /// Safety margin multiplier (default: 1.10 = 10%)
    safety_margin: f64,
}

impl FeeAnalyticsEngine {
    /// Create a new analytics engine with default parameters
    pub fn new() -> Self {
        Self {
            sma_short_window: 10,
            sma_medium_window: 50,
            ema_period: 12,
            safety_margin: 1.10,
        }
    }

    /// Create with custom parameters
    pub fn with_params(
        sma_short_window: usize,
        sma_medium_window: usize,
        ema_period: usize,
        safety_margin: f64,
    ) -> Self {
        Self {
            sma_short_window,
            sma_medium_window,
            ema_period,
            safety_margin,
        }
    }

    /// Generate comprehensive fee prediction from historical samples
    pub fn predict(&self, samples: &[LedgerFeeSample], current_ledger: u64) -> FeePrediction {
        if samples.is_empty() {
            // Return defaults if no data available
            return FeePrediction {
                current_ledger,
                next_ledger_bid: 100,
                next_3_ledgers_bid: 100,
                economy_bid: 100,
                standard_bid: 100,
                priority_bid: 150,
                urgent_bid: 200,
                confidence_score: 0.0,
                market_volatility: 0.0,
                trend_direction: TrendDirection::Stable,
            };
        }

        // Extract base fees from samples (most recent first)
        let fees: Vec<i64> = samples.iter().map(|s| s.base_fee).collect();

        // Calculate statistical models
        let _sma_10 = self.calculate_sma(&fees, self.sma_short_window);
        let _sma_50 = self.calculate_sma(&fees, self.sma_medium_window);
        let _ema_12 = self.calculate_ema(&fees, self.ema_period);
        let p50 = self.calculate_percentile(&fees, 50.0);
        let p75 = self.calculate_percentile(&fees, 75.0);
        let p95 = self.calculate_percentile(&fees, 95.0);
        let std_dev = self.calculate_std_dev(&fees);
        let mean = self.calculate_mean(&fees);

        // Calculate coefficient of variation
        let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };

        // Determine trend direction
        let trend = self.detect_trend(&fees);

        // Calculate volatility (coefficient of variation capped at 1.0)
        let volatility = cv.min(1.0);

        // Generate bid recommendations
        let economy_bid = (p50 as f64 * 0.9).ceil() as u64; // 90% of median
        let standard_bid = (p50 as f64 * self.safety_margin).ceil() as u64;
        let priority_bid = (p75 as f64 * self.safety_margin).ceil() as u64;
        let urgent_bid = (p95 as f64 * self.safety_margin).ceil() as u64;

        // Next ledger bid should be aggressive (use 95th percentile)
        let next_ledger_bid = urgent_bid;

        // Next 3 ledgers can be more conservative
        let next_3_ledgers_bid = standard_bid;

        // Confidence score based on data quantity and volatility
        let data_confidence = (samples.len() as f64 / 100.0).min(1.0);
        let volatility_confidence = 1.0 - volatility;
        let confidence_score = (data_confidence * 0.6 + volatility_confidence * 0.4).min(1.0);

        FeePrediction {
            current_ledger,
            next_ledger_bid,
            next_3_ledgers_bid,
            economy_bid,
            standard_bid,
            priority_bid,
            urgent_bid,
            confidence_score,
            market_volatility: volatility,
            trend_direction: trend,
        }
    }

    /// Get detailed model breakdown
    pub fn get_model_breakdown(&self, samples: &[LedgerFeeSample]) -> ModelBreakdown {
        if samples.is_empty() {
            return ModelBreakdown {
                sma_10: 100,
                sma_50: 100,
                ema_12: 100,
                percentile_50: 100,
                percentile_75: 100,
                percentile_95: 100,
                standard_deviation: 0.0,
                coefficient_of_variation: 0.0,
            };
        }

        let fees: Vec<i64> = samples.iter().map(|s| s.base_fee).collect();
        let sma_10 = self.calculate_sma(&fees, self.sma_short_window);
        let sma_50 = self.calculate_sma(&fees, self.sma_medium_window);
        let ema_12 = self.calculate_ema(&fees, self.ema_period);
        let p50 = self.calculate_percentile(&fees, 50.0);
        let p75 = self.calculate_percentile(&fees, 75.0);
        let p95 = self.calculate_percentile(&fees, 95.0);
        let std_dev = self.calculate_std_dev(&fees);
        let mean = self.calculate_mean(&fees);
        let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };

        ModelBreakdown {
            sma_10,
            sma_50,
            ema_12,
            percentile_50: p50,
            percentile_75: p75,
            percentile_95: p95,
            standard_deviation: std_dev,
            coefficient_of_variation: cv,
        }
    }

    /// Get market conditions snapshot
    pub fn get_market_conditions(
        &self,
        samples: &[LedgerFeeSample],
        current_ledger: u64,
    ) -> MarketConditions {
        if samples.is_empty() {
            return MarketConditions {
                current_ledger,
                volatility: "unknown".to_string(),
                trend: "unknown".to_string(),
                avg_fee_10_ledgers: 100,
                avg_fee_50_ledgers: 100,
                transaction_pressure: 0.0,
            };
        }

        let fees: Vec<i64> = samples.iter().map(|s| s.base_fee).collect();
        let volatility = self.calculate_std_dev(&fees);
        let mean = self.calculate_mean(&fees);
        let cv = if mean > 0.0 { volatility / mean } else { 0.0 };

        let volatility_str = if cv < 0.1 {
            "low"
        } else if cv < 0.3 {
            "medium"
        } else {
            "high"
        }
        .to_string();

        let trend = self.detect_trend(&fees);
        let trend_str = match trend {
            TrendDirection::Upward => "upward",
            TrendDirection::Downward => "downward",
            TrendDirection::Stable => "stable",
        }
        .to_string();

        let avg_10 = self.calculate_sma(&fees, self.sma_short_window);
        let avg_50 = self.calculate_sma(&fees, self.sma_medium_window);

        // Calculate transaction pressure based on average tx count
        let avg_tx_count: f64 = samples
            .iter()
            .map(|s| s.transaction_count as f64)
            .sum::<f64>()
            / samples.len() as f64;

        // Normalize to 0-1 range (assuming max ~100 tx per ledger)
        let transaction_pressure = (avg_tx_count / 100.0).min(1.0);

        MarketConditions {
            current_ledger,
            volatility: volatility_str,
            trend: trend_str,
            avg_fee_10_ledgers: avg_10,
            avg_fee_50_ledgers: avg_50,
            transaction_pressure,
        }
    }

    // ── Statistical Methods ─────────────────────────────────────────────

    /// Calculate Simple Moving Average
    fn calculate_sma(&self, data: &[i64], window: usize) -> u64 {
        if data.is_empty() || window == 0 {
            return 0;
        }

        let window = window.min(data.len());
        let sum: i64 = data.iter().take(window).sum();
        (sum / window as i64).max(0) as u64
    }

    /// Calculate Exponential Moving Average
    fn calculate_ema(&self, data: &[i64], period: usize) -> u64 {
        if data.is_empty() || period == 0 {
            return 0;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = data[0] as f64;

        for &value in data.iter().skip(1).take(period.min(data.len())) {
            ema = (value as f64 - ema) * multiplier + ema;
        }

        ema.max(0.0).round() as u64
    }

    /// Calculate percentile value
    fn calculate_percentile(&self, data: &[i64], percentile: f64) -> u64 {
        if data.is_empty() {
            return 0;
        }

        let mut sorted = data.to_vec();
        sorted.sort();

        let index = (percentile / 100.0) * (sorted.len() - 1) as f64;
        let lower = index.floor() as usize;
        let upper = index.ceil() as usize;
        let weight = index - lower as f64;

        if upper >= sorted.len() {
            sorted[lower].max(0) as u64
        } else {
            let value = sorted[lower] as f64 * (1.0 - weight) + sorted[upper] as f64 * weight;
            value.max(0.0).round() as u64
        }
    }

    /// Calculate standard deviation
    fn calculate_std_dev(&self, data: &[i64]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }

        let mean = self.calculate_mean(data);
        let variance: f64 = data
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / (data.len() - 1) as f64;

        variance.sqrt()
    }

    /// Calculate mean (average)
    fn calculate_mean(&self, data: &[i64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        data.iter().sum::<i64>() as f64 / data.len() as f64
    }

    /// Detect trend direction by comparing recent vs older averages
    fn detect_trend(&self, data: &[i64]) -> TrendDirection {
        if data.len() < 10 {
            return TrendDirection::Stable;
        }

        let recent_window = 5.min(data.len() / 2);
        let older_window = 5.min(data.len() / 2);

        let older: Vec<i64> = data.iter().take(older_window).cloned().collect();
        let recent: Vec<i64> = data
            .iter()
            .skip(data.len() - recent_window)
            .take(older_window)
            .cloned()
            .collect();

        let recent_avg = self.calculate_mean(&recent);
        let older_avg = self.calculate_mean(&older);

        // 5% threshold for trend detection
        let threshold = older_avg * 0.05;

        if recent_avg > older_avg + threshold {
            TrendDirection::Upward
        } else if recent_avg < older_avg - threshold {
            TrendDirection::Downward
        } else {
            TrendDirection::Stable
        }
    }
}

impl Default for FeeAnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// One stored observation of network base fee and inclusion fee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeDataPoint {
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    /// Protocol base fee, in stroops.
    pub base_fee: u64,
    /// Fee paid to be included, in stroops.
    pub inclusion_fee: u64,
}

/// Windows used for fee moving averages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeAverageWindow {
    OneHour,
    TwentyFourHours,
    SevenDays,
}

impl FeeAverageWindow {
    pub fn duration(self) -> chrono::Duration {
        match self {
            FeeAverageWindow::OneHour => chrono::Duration::hours(1),
            FeeAverageWindow::TwentyFourHours => chrono::Duration::hours(24),
            FeeAverageWindow::SevenDays => chrono::Duration::days(7),
        }
    }

    pub fn all() -> [FeeAverageWindow; 3] {
        [
            FeeAverageWindow::OneHour,
            FeeAverageWindow::TwentyFourHours,
            FeeAverageWindow::SevenDays,
        ]
    }
}

/// Mean base fee and inclusion fee inside one window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowMovingAverage {
    pub window: FeeAverageWindow,
    pub base_fee: f64,
    pub inclusion_fee: f64,
    pub sample_count: usize,
}

/// Recommended fee for a transaction that should confirm quickly.
#[derive(Debug, Clone, PartialEq)]
pub struct FastFeePrediction {
    /// Bid that should clear in the current market, in stroops.
    pub optimal_fee: u64,
    pub base_fee_1h: f64,
    pub inclusion_fee_1h: f64,
    pub inclusion_fee_24h: f64,
    /// `true` when the 1h inclusion average is materially above the 24h average.
    pub rising: bool,
    /// 0.0 when no samples exist, otherwise scales with 1h sample count.
    pub confidence: f64,
}

/// In-memory history of fee data points and the trend aggregator on top of it.
#[derive(Debug, Clone, Default)]
pub struct FeeTrendAggregator {
    points: Vec<FeeDataPoint>,
}

impl FeeTrendAggregator {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// Append a historical fee observation. Points are kept in insertion order.
    pub fn record(&mut self, point: FeeDataPoint) {
        self.points.push(point);
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn points(&self) -> &[FeeDataPoint] {
        &self.points
    }

    /// Arithmetic mean of base fee and inclusion fee for samples inside `window`
    /// ending at `now`. Samples recorded after `now` are ignored.
    pub fn moving_average(
        &self,
        window: FeeAverageWindow,
        now: chrono::DateTime<chrono::Utc>,
    ) -> WindowMovingAverage {
        let start = now - window.duration();
        let mut base_sum: u128 = 0;
        let mut inclusion_sum: u128 = 0;
        let mut sample_count: usize = 0;

        for point in &self.points {
            if point.recorded_at > now || point.recorded_at < start {
                continue;
            }
            base_sum += point.base_fee as u128;
            inclusion_sum += point.inclusion_fee as u128;
            sample_count += 1;
        }

        let (base_fee, inclusion_fee) = if sample_count == 0 {
            (0.0, 0.0)
        } else {
            (
                base_sum as f64 / sample_count as f64,
                inclusion_sum as f64 / sample_count as f64,
            )
        };

        WindowMovingAverage {
            window,
            base_fee,
            inclusion_fee,
            sample_count,
        }
    }

    /// Moving averages for 1h, 24h, and 7d.
    pub fn moving_averages(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> [WindowMovingAverage; 3] {
        let windows = FeeAverageWindow::all();
        [
            self.moving_average(windows[0], now),
            self.moving_average(windows[1], now),
            self.moving_average(windows[2], now),
        ]
    }

    /// Predict the fee that should confirm quickly.
    ///
    /// The bid is the 1-hour inclusion-fee average (falling back to 24h, then 7d),
    /// and at least the 1-hour base-fee average. When the 1h inclusion average is
    /// more than 5% above the 24h average, a 25% surge premium is applied;
    /// otherwise the premium is 10%.
    pub fn predict_fast_fee(&self, now: chrono::DateTime<chrono::Utc>) -> FastFeePrediction {
        let [hour, day, week] = self.moving_averages(now);

        if hour.sample_count == 0 && day.sample_count == 0 && week.sample_count == 0 {
            return FastFeePrediction {
                optimal_fee: 100,
                base_fee_1h: 0.0,
                inclusion_fee_1h: 0.0,
                inclusion_fee_24h: 0.0,
                rising: false,
                confidence: 0.0,
            };
        }

        let inclusion_anchor = if hour.sample_count > 0 {
            hour.inclusion_fee
        } else if day.sample_count > 0 {
            day.inclusion_fee
        } else {
            week.inclusion_fee
        };

        let rising = day.sample_count > 0 && hour.inclusion_fee > day.inclusion_fee * 1.05;
        // 125% when the short window is rising, otherwise 110%. Integer basis
        // points avoid `ceil` turning an exact multiple (200 * 1.10) into 221.
        let premium_bps: u64 = if rising { 12_500 } else { 11_000 };
        let floor = hour.base_fee.max(inclusion_anchor).round().max(0.0) as u64;
        let optimal_fee = (floor * premium_bps).div_ceil(10_000).max(1);
        let confidence = (hour.sample_count as f64 / 12.0).min(1.0);

        FastFeePrediction {
            optimal_fee,
            base_fee_1h: hour.base_fee,
            inclusion_fee_1h: hour.inclusion_fee,
            inclusion_fee_24h: day.inclusion_fee,
            rising,
            confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee_store::LedgerFeeSample;
    use chrono::Utc;

    fn create_sample(ledger: i64, base_fee: i64) -> LedgerFeeSample {
        LedgerFeeSample {
            ledger_sequence: ledger,
            collected_at: Utc::now(),
            base_reserve: 0,
            base_fee,
            max_fee: base_fee,
            fee_charged: base_fee,
            transaction_count: 10,
            ledger_close_time: Utc::now(),
        }
    }

    #[test]
    fn test_sma_calculation() {
        let engine = FeeAnalyticsEngine::new();
        let data = vec![100, 110, 120, 130, 140];
        let sma = engine.calculate_sma(&data, 5);
        assert_eq!(sma, 120); // (100+110+120+130+140)/5 = 120
    }

    #[test]
    fn test_ema_calculation() {
        let engine = FeeAnalyticsEngine::new();
        let data = vec![100, 110, 120, 130, 140];
        let ema = engine.calculate_ema(&data, 5);
        // EMA should be higher than SMA (more weight on recent values)
        assert!(ema >= 120);
    }

    #[test]
    fn test_percentile_calculation() {
        let engine = FeeAnalyticsEngine::new();
        let data = vec![100, 200, 300, 400, 500];
        let p50 = engine.calculate_percentile(&data, 50.0);
        assert_eq!(p50, 300); // Median

        let p95 = engine.calculate_percentile(&data, 95.0);
        assert!(p95 >= 400);
    }

    #[test]
    fn test_std_dev_calculation() {
        let engine = FeeAnalyticsEngine::new();
        let data = vec![100, 100, 100, 100];
        let std_dev = engine.calculate_std_dev(&data);
        assert!(std_dev < 0.001); // Should be ~0 for identical values

        let data2 = vec![100, 200, 300, 400, 500];
        let std_dev2 = engine.calculate_std_dev(&data2);
        assert!(std_dev2 > 0.0); // Should be > 0 for varied values
    }

    #[test]
    fn test_trend_detection_upward() {
        let engine = FeeAnalyticsEngine::new();
        // Older values are lower, recent values are higher
        let data = vec![500, 400, 300, 200, 100, 150, 250, 350, 450, 550];
        let trend = engine.detect_trend(&data);
        assert_eq!(trend, TrendDirection::Upward);
    }

    #[test]
    fn test_trend_detection_downward() {
        let engine = FeeAnalyticsEngine::new();
        // Older values are higher, recent values are lower
        let data = vec![100, 200, 300, 400, 500, 450, 350, 250, 150, 50];
        let trend = engine.detect_trend(&data);
        assert_eq!(trend, TrendDirection::Downward);
    }

    #[test]
    fn test_trend_detection_stable() {
        let engine = FeeAnalyticsEngine::new();
        // All values similar
        let data = vec![100, 105, 102, 98, 103, 101, 99, 104, 100, 102];
        let trend = engine.detect_trend(&data);
        assert_eq!(trend, TrendDirection::Stable);
    }

    #[test]
    fn test_fee_prediction_with_data() {
        let engine = FeeAnalyticsEngine::new();
        let samples: Vec<LedgerFeeSample> = (0..50)
            .map(|i| create_sample(i as i64 + 1, 100 + (i % 10) * 5))
            .collect();

        let prediction = engine.predict(&samples, 100);

        assert_eq!(prediction.current_ledger, 100);
        assert!(prediction.economy_bid <= prediction.standard_bid);
        assert!(prediction.standard_bid <= prediction.priority_bid);
        assert!(prediction.priority_bid <= prediction.urgent_bid);
        assert!(prediction.confidence_score > 0.0);
    }

    #[test]
    fn test_fee_prediction_empty_data() {
        let engine = FeeAnalyticsEngine::new();
        let samples: Vec<LedgerFeeSample> = vec![];

        let prediction = engine.predict(&samples, 50);

        assert_eq!(prediction.current_ledger, 50);
        assert_eq!(prediction.next_ledger_bid, 100);
        assert_eq!(prediction.confidence_score, 0.0);
    }

    #[test]
    fn test_model_breakdown() {
        let engine = FeeAnalyticsEngine::new();
        let samples: Vec<LedgerFeeSample> = (0..20)
            .map(|i| create_sample(i as i64 + 1, 100 + i * 2))
            .collect();

        let breakdown = engine.get_model_breakdown(&samples);

        assert!(breakdown.sma_10 > 0);
        assert!(breakdown.sma_50 > 0);
        assert!(breakdown.ema_12 > 0);
        assert!(breakdown.percentile_50 > 0);
        assert!(breakdown.standard_deviation >= 0.0);
    }

    #[test]
    fn test_market_conditions() {
        let engine = FeeAnalyticsEngine::new();
        let samples: Vec<LedgerFeeSample> =
            (0..30).map(|i| create_sample(i as i64 + 1, 100)).collect();

        let conditions = engine.get_market_conditions(&samples, 100);

        assert_eq!(conditions.current_ledger, 100);
        assert_eq!(conditions.avg_fee_10_ledgers, 100);
        assert!(conditions.transaction_pressure >= 0.0);
        assert!(conditions.transaction_pressure <= 1.0);
    }

    fn point(at: chrono::DateTime<chrono::Utc>, base_fee: u64, inclusion_fee: u64) -> FeeDataPoint {
        FeeDataPoint {
            recorded_at: at,
            base_fee,
            inclusion_fee,
        }
    }

    #[test]
    fn fee_history_stores_data_points() {
        let mut agg = FeeTrendAggregator::new();
        let now = Utc::now();
        agg.record(point(now, 100, 180));
        agg.record(point(now, 120, 200));
        assert_eq!(agg.len(), 2);
        assert_eq!(agg.points()[1].inclusion_fee, 200);
    }

    #[test]
    fn moving_averages_respect_1h_24h_and_7d_windows() {
        let now = Utc::now();
        let mut agg = FeeTrendAggregator::new();
        agg.record(point(now - chrono::Duration::minutes(30), 100, 200));
        agg.record(point(now - chrono::Duration::minutes(10), 140, 300));
        agg.record(point(now - chrono::Duration::hours(2), 50, 80));
        agg.record(point(now - chrono::Duration::days(8), 999, 999));

        let [hour, day, week] = agg.moving_averages(now);

        assert_eq!(hour.sample_count, 2);
        assert!((hour.base_fee - 120.0).abs() < 1e-9);
        assert!((hour.inclusion_fee - 250.0).abs() < 1e-9);

        assert_eq!(day.sample_count, 3);
        assert!((day.base_fee - (290.0 / 3.0)).abs() < 1e-9);
        assert!((day.inclusion_fee - (580.0 / 3.0)).abs() < 1e-9);

        assert_eq!(week.sample_count, 3);
        assert_eq!(week.base_fee, day.base_fee);
        assert_eq!(week.inclusion_fee, day.inclusion_fee);
    }

    #[test]
    fn empty_window_average_is_zero() {
        let agg = FeeTrendAggregator::new();
        let avg = agg.moving_average(FeeAverageWindow::OneHour, Utc::now());
        assert_eq!(avg.sample_count, 0);
        assert_eq!(avg.base_fee, 0.0);
        assert_eq!(avg.inclusion_fee, 0.0);
    }

    #[test]
    fn predict_fast_fee_uses_inclusion_average_with_premium() {
        let now = Utc::now();
        let mut stable = FeeTrendAggregator::new();
        for _ in 0..12 {
            stable.record(point(now - chrono::Duration::minutes(20), 100, 200));
            stable.record(point(now - chrono::Duration::hours(6), 100, 200));
        }
        let stable_pred = stable.predict_fast_fee(now);
        assert!(!stable_pred.rising);
        assert_eq!(stable_pred.optimal_fee, 220);
        assert_eq!(stable_pred.confidence, 1.0);

        let mut rising = FeeTrendAggregator::new();
        rising.record(point(now - chrono::Duration::minutes(15), 100, 400));
        rising.record(point(now - chrono::Duration::hours(5), 100, 100));
        let rising_pred = rising.predict_fast_fee(now);
        assert!(rising_pred.rising);
        assert_eq!(rising_pred.optimal_fee, 500);
        assert!(rising_pred.optimal_fee > stable_pred.optimal_fee);
    }

    #[test]
    fn predict_fast_fee_without_history_is_base_fee_default() {
        let pred = FeeTrendAggregator::new().predict_fast_fee(Utc::now());
        assert_eq!(pred.optimal_fee, 100);
        assert_eq!(pred.confidence, 0.0);
        assert!(!pred.rising);
    }
}

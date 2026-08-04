use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    pub trade_id: String,
    pub symbol: String,
    pub price_paisa: u64,
    pub quantity: u64,
    pub buyer_acc_no: String,
    pub seller_acc_no: String,
    pub timestamp_millis: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEvent {
    pub exchange_name: String,
    pub symbol: String,
    pub ltp_paisa: u64,
    pub cumulative_trades: u64,
    pub timestamp_millis: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HftTelemetryEvent {
    pub total_balance_rupees: f64,
    pub realized_pnl_rupees: f64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub total_trades: u64,
    pub hft_latency_nanos: u128,
    pub hft_median_latency_nanos: u128,
    pub current_spread_paisa: u64,
    pub unified_inventory: i64,
    pub ayushse_ltp_paisa: u64,
    pub bohrase_ltp_paisa: u64,
    pub timestamp_millis: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    Trade(TradeEvent),
    Volume(VolumeEvent),
    HftTelemetry(HftTelemetryEvent),
}

pub mod redis_publisher;
pub use redis_publisher::RedisStreamPublisher;

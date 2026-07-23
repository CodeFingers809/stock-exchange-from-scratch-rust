use std::time::SystemTime;

use super::price::Price;

#[derive(Debug, Clone, Hash)]
pub enum BidOrAsk {
    Bid,
    Ask
}

#[derive(Debug, Clone, Hash)]
pub enum OrderType {
    Limit,
    Market
}

#[derive(Debug, Clone, Hash)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled
}

#[derive(Debug, Clone, Hash)]
pub struct Order {
    pub symbol: String,
    pub price: Price,
    pub size: u64,
    pub filled_size: u64,
    pub bid_or_ask: BidOrAsk,
    pub timestamp: SystemTime,
}

impl Order {
    pub fn new(symbol: String, price: Price, size: u64, bid_or_ask: BidOrAsk) -> Self {
        Self {
            symbol,
            price,
            size,
            bid_or_ask,
            filled_size: 0,
            timestamp: SystemTime::now(),
        }
    }
}
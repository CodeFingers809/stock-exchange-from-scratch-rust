use std::fmt;
use std::time::SystemTime;

use super::price::Price;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BidOrAsk {
    Bid,
    Ask,
}

impl fmt::Display for BidOrAsk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BidOrAsk::Bid => write!(f, "BID"),
            BidOrAsk::Ask => write!(f, "ASK"),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Hash)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
}

#[derive(Debug, Clone, Hash)]
pub struct Order {
    pub symbol: String,
    pub price: Option<Price>,
    pub size: u64,
    pub filled_size: u64,
    pub bid_or_ask: BidOrAsk,
    pub timestamp: SystemTime,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let price_str = match self.price {
            Some(price) => format!("{}", price), // Uses Price's Display (₹150.25)
            None => "MARKET".to_string(),
        };
        write!(
            f,
            "[{}] {} {} @ {} (Filled: {}/{})",
            self.symbol,
            self.bid_or_ask,
            self.size,
            price_str,
            self.filled_size,
            self.size
        )
    }
}

impl Order {
    pub fn new(symbol: String, price: Option<Price>, size: u64, bid_or_ask: BidOrAsk) -> Self {
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
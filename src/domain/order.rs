use std::fmt;
use std::sync::{Arc, Mutex};
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

#[derive(Debug, Clone)]
pub struct OrderInner {
    pub symbol: String,
    pub price: Option<Price>,
    pub size: u64,
    pub filled_size: u64,
    pub bid_or_ask: BidOrAsk,
    pub acc_no: String,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub struct Order(Arc<Mutex<OrderInner>>);

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.0.lock().unwrap();
        let price_str = match inner.price {
            Some(price) => format!("{}", price),
            None => "MARKET".to_string(),
        };
        write!(
            f,
            "[{}] Acc: {} | {} {} @ {} (Filled: {}/{})",
            inner.symbol,
            inner.acc_no,
            inner.bid_or_ask,
            inner.size,
            price_str,
            inner.filled_size,
            inner.size
        )
    }
}

impl Order {
    pub fn new(symbol: String, price: Option<Price>, size: u64, bid_or_ask: BidOrAsk, acc_no: String) -> Self {
        Self(Arc::new(Mutex::new(OrderInner {
            symbol,
            price,
            size,
            filled_size: 0,
            bid_or_ask,
            acc_no,
            timestamp: SystemTime::now(),
        })))
    }

    // Fill an order by a given quantity safely
    pub fn fill(&self, qty: u64) {
        let mut inner = self.0.lock().unwrap();
        inner.filled_size += qty;
    }

    // Helper accessors for thread-safe field access
    pub fn symbol(&self) -> String {
        self.0.lock().unwrap().symbol.clone()
    }

    pub fn price(&self) -> Option<Price> {
        self.0.lock().unwrap().price
    }

    pub fn size(&self) -> u64 {
        self.0.lock().unwrap().size
    }

    pub fn filled_size(&self) -> u64 {
        self.0.lock().unwrap().filled_size
    }

    pub fn remaining_size(&self) -> u64 {
        let inner = self.0.lock().unwrap();
        inner.size - inner.filled_size
    }

    pub fn is_filled(&self) -> bool {
        let inner = self.0.lock().unwrap();
        inner.filled_size >= inner.size
    }

    pub fn bid_or_ask(&self) -> BidOrAsk {
        self.0.lock().unwrap().bid_or_ask.clone()
    }

    pub fn acc_no(&self) -> String {
        self.0.lock().unwrap().acc_no.clone()
    }
}
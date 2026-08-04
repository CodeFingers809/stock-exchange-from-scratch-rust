use std::time::SystemTime;
use super::price::Price;
#[derive(Debug, Clone)]

pub struct Holding {
    pub symbol: String,
    pub quantity: u64,
    pub buy_price: Price,
    pub bought_at: SystemTime,
}
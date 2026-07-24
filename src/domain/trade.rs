use std::time::SystemTime;
use super::price::Price;

#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub symbol: String,
    pub price: Price,
    pub quantity: u64,
    pub buyer_acc_no: String,
    pub seller_acc_no: String,
    pub timestamp: SystemTime,
}

impl Trade {
    pub fn new(
        id: u64,
        symbol: String,
        price: Price,
        quantity: u64,
        buyer_acc_no: String,
        seller_acc_no: String,
    ) -> Self {
        Self {
            id,
            symbol,
            price,
            quantity,
            buyer_acc_no,
            seller_acc_no,
            timestamp: SystemTime::now(),
        }
    }
}

use super::price::Price;
use super::order::{Order, BidOrAsk};

#[derive(Debug, Clone, Hash)]
pub struct OrderBook {
    pub symbol: String,
    pub ltp: Price,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

impl OrderBook {
    pub fn new(symbol: String, ltp: Price) -> Self {
        Self {
            symbol,
            ltp,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }
    pub fn add_order(&mut self, order: Order) -> Result<String, String> {
        match order.bid_or_ask {
            BidOrAsk::Bid => {
                self.bids.push(order);
                Ok("Order accepted.".to_string())
            },
            BidOrAsk::Ask => {
                self.asks.push(order);
                Ok("Order Received.".to_string())
            },
        }
    }
}
use std::collections::HashMap;
use std::time::SystemTime;

use super::{
    holding::Holding,
    market::Market,
    order::{BidOrAsk, Order},
    price::Price,
    trade::Trade,
    user::User,
};

#[derive(Debug, Clone)]
pub struct Portfolio {
    pub user: User,
    pub acc_no: String,
    pub balance_paisa: u64,
    pub open_orders: HashMap<String, Vec<Order>>,
    pub holdings: HashMap<String, Vec<Holding>>,
}

impl Portfolio {
    pub fn new(user: User, acc_no: String, initial_balance_paisa: u64) -> Self {
        Self {
            user,
            acc_no,
            balance_paisa: initial_balance_paisa,
            open_orders: HashMap::new(),
            holdings: HashMap::new(),
        }
    }

    // Helper: calculate total available shares owned for a ticker
    pub fn total_shares(&self, symbol: &str) -> u64 {
        match self.holdings.get(symbol) {
            Some(list) => list.iter().map(|h| h.quantity).sum(),
            None => 0,
        }
    }

    // Helper: clean up fully filled orders from open_orders
    pub fn cleanup_open_orders(&mut self) {
        for orders in self.open_orders.values_mut() {
            orders.retain(|o| !o.is_filled());
        }
        self.open_orders.retain(|_, orders| !orders.is_empty());
    }

    // Pre-Trade Risk Checks before order placement
    pub fn validate_order(&self, order: &Order, market: &Market) -> Result<(), String> {
        match order.bid_or_ask() {
            BidOrAsk::Bid => {
                let required_price = match order.price() {
                    Some(price) => price,
                    None => {
                        let book = market.get_orderbook(&order.symbol()).ok_or_else(|| {
                            format!("Ticker '{}' not found in market.", order.symbol())
                        })?;
                        book.ltp
                    }
                };
                let total_cost = order.size() * required_price.paisa;
                if self.balance_paisa < total_cost {
                    return Err(format!(
                        "Insufficient funds: Required ₹{:.2}, Available ₹{:.2}",
                        total_cost as f64 / 100.0,
                        self.balance_paisa as f64 / 100.0
                    ));
                }
            }
            BidOrAsk::Ask => {
                let available_shares = self.total_shares(&order.symbol());
                if available_shares < order.size() {
                    return Err(format!(
                        "Insufficient shares for '{}': Required {}, Available {}",
                        order.symbol(),
                        order.size(),
                        available_shares
                    ));
                }
            }
        }
        Ok(())
    }

    // Dispatch Limit Order from Portfolio -> Market
    pub fn dispatch_limit_order(
        &mut self,
        order: Order,
        market: &mut Market,
    ) -> Result<(Vec<Trade>, String), String> {
        self.validate_order(&order, market)?;

        // Track in open orders
        self.open_orders
            .entry(order.symbol())
            .or_default()
            .push(order.clone());

        let result = market.place_limit_order(order)?;
        self.cleanup_open_orders();
        Ok(result)
    }

    // Dispatch Market Order from Portfolio -> Market
    pub fn dispatch_market_order(
        &mut self,
        order: Order,
        market: &mut Market,
    ) -> Result<(Vec<Trade>, String), String> {
        self.validate_order(&order, market)?;

        self.open_orders
            .entry(order.symbol())
            .or_default()
            .push(order.clone());

        let result = market.place_market_order(order)?;
        self.cleanup_open_orders();
        Ok(result)
    }

    // Process a Buy settlement (Anonymized trade execution)
    pub fn apply_buy_trade(&mut self, symbol: &str, qty: u64, price: Price) {
        let total_cost = qty * price.paisa;
        if self.balance_paisa >= total_cost {
            self.balance_paisa -= total_cost;
        }

        let holding = Holding {
            symbol: symbol.to_string(),
            quantity: qty,
            buy_price: price,
            bought_at: SystemTime::now(),
        };

        self.holdings
            .entry(symbol.to_string())
            .or_default()
            .push(holding);

        self.cleanup_open_orders();
    }

    // Process a Sell settlement (Anonymized trade execution)
    pub fn apply_sell_trade(&mut self, symbol: &str, mut qty_to_sell: u64, price: Price) {
        let total_earned = qty_to_sell * price.paisa;
        self.balance_paisa += total_earned;

        if let Some(list) = self.holdings.get_mut(symbol) {
            while qty_to_sell > 0 && !list.is_empty() {
                if list[0].quantity <= qty_to_sell {
                    qty_to_sell -= list[0].quantity;
                    list.remove(0);
                } else {
                    list[0].quantity -= qty_to_sell;
                    qty_to_sell = 0;
                }
            }
        }

        self.cleanup_open_orders();
    }
}
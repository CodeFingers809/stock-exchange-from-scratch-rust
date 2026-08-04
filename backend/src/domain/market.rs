use std::collections::HashMap;
use std::fmt;

use super::order::Order;
use super::orderbook::OrderBook;
use super::price::Price;

#[derive(Debug, Clone)]
pub struct Market {
    pub name: String,
    pub books: HashMap<String, OrderBook>,
}

impl fmt::Display for Market {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "==================================================")?;
        writeln!(f, "  EXCHANGE MARKET: {}", self.name)?;
        writeln!(f, "  Active Tickers: {}", self.books.len())?;
        writeln!(f, "==================================================")?;

        if self.books.is_empty() {
            writeln!(f, "  (No stock orderbooks registered yet)")?;
        } else {
            for book in self.books.values() {
                writeln!(f, "\n{}", book)?;
            }
        }

        Ok(())
    }
}

use super::trade::Trade;

impl Market {
    pub fn new(name: String) -> Self {
        Self {
            name,
            books: HashMap::new(),
        }
    }

    pub fn add_stock(&mut self, symbol: String, initial_ltp: Price) {
        let book = OrderBook::new(symbol.clone(), initial_ltp);
        self.books.insert(symbol, book);
    }

    pub fn place_limit_order(&mut self, order: Order) -> Result<(Vec<Trade>, String), String> {
        match self.books.get_mut(&order.symbol()) {
            Some(book) => book.add_limit_order(order),
            None => Err(format!("Ticker '{}' not found in market.", order.symbol())),
        }
    }

    pub fn place_market_order(&mut self, order: Order) -> Result<(Vec<Trade>, String), String> {
        match self.books.get_mut(&order.symbol()) {
            Some(book) => book.add_market_order(order),
            None => Err(format!("Ticker '{}' not found in market.", order.symbol())),
        }
    }

    pub fn get_orderbook(&self, symbol: &str) -> Option<&OrderBook> {
        self.books.get(symbol)
    }

    /// Subscribe to real-time ticker stream: LTP + best bid/ask for HFT depth checks
    pub fn subscribe_ticker(
        &self,
        symbol: &str,
        sender: std::sync::mpsc::Sender<MarketTick>,
    ) -> Result<(), String> {
        if let Some(book) = self.books.get(symbol) {
            let best_bid = book.bids.keys().next_back().copied();
            let best_ask = book.asks.keys().next().copied();
            let tick = MarketTick {
                exchange_name: self.name.clone(),
                symbol: symbol.to_string(),
                ltp: book.ltp,
                best_bid,
                best_ask,
                timestamp_instant: std::time::Instant::now(),
            };
            let _ = sender.send(tick);
            Ok(())
        } else {
            Err(format!("Stock ticker '{}' not found for subscription.", symbol))
        }
    }
}

/// Real-time Market Tick payload emitted via Market::subscribe_ticker
#[derive(Debug, Clone)]
pub struct MarketTick {
    pub exchange_name: String,
    pub symbol: String,
    pub ltp: Price,
    /// Best resting bid price — None if book has no buyers
    pub best_bid: Option<Price>,
    /// Best resting ask price — None if book has no sellers
    pub best_ask: Option<Price>,
    pub timestamp_instant: std::time::Instant,
}
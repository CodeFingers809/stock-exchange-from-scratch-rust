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
}
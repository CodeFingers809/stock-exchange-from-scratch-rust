use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use super::order::{BidOrAsk, Order};
use super::price::Price;
use super::trade::Trade;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub ltp: Price,
    pub bids: BTreeMap<Price, VecDeque<Order>>,
    pub asks: BTreeMap<Price, VecDeque<Order>>,
}

impl fmt::Display for OrderBook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "==================================================")?;
        writeln!(f, "  ORDER BOOK: {}  |  LTP: {}", self.symbol, self.ltp)?;
        writeln!(f, "==================================================")?;

        writeln!(f, "--- ASKS (SELLERS) ---")?;
        if self.asks.is_empty() {
            writeln!(f, "  (No asks resting on book)")?;
        } else {
            for (price, queue) in self.asks.iter().rev() {
                let total_volume: u64 = queue.iter().map(|o| o.remaining_size()).sum();
                writeln!(f, "  {:>10}  |  {} shares ({} orders)", price, total_volume, queue.len())?;
            }
        }

        writeln!(f, "--------------------------------------------------")?;

        writeln!(f, "--- BIDS (BUYERS) ---")?;
        if self.bids.is_empty() {
            writeln!(f, "  (No bids resting on book)")?;
        } else {
            for (price, queue) in self.bids.iter().rev() {
                let total_volume: u64 = queue.iter().map(|o| o.remaining_size()).sum();
                writeln!(f, "  {:>10}  |  {} shares ({} orders)", price, total_volume, queue.len())?;
            }
        }

        writeln!(f, "==================================================")
    }
}

impl OrderBook {
    pub fn new(symbol: String, ltp: Price) -> Self {
        Self {
            symbol,
            ltp,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn add_limit_order(&mut self, order: Order) -> Result<(Vec<Trade>, String), String> {
        let mut executed_trades = Vec::new();
        let mut trade_id_counter = 1000u64;

        match order.bid_or_ask() {
            BidOrAsk::Bid => {
                while order.remaining_size() > 0 {
                    let best_ask_price = match self.asks.keys().next().copied() {
                        Some(price) => price,
                        None => break,
                    };

                    match order.price().unwrap().cmp(&best_ask_price) {
                        Greater | Equal => {
                            if let Some(ask_queue) = self.asks.get_mut(&best_ask_price) {
                                while order.remaining_size() > 0 {
                                    let first_ask = match ask_queue.front_mut() {
                                        Some(ask) => ask,
                                        None => break,
                                    };
                                    let remaining_ask = first_ask.remaining_size();
                                    let remaining_bid = order.remaining_size();

                                    self.ltp = best_ask_price;
                                    let match_qty = std::cmp::min(remaining_bid, remaining_ask);

                                    order.fill(match_qty);
                                    first_ask.fill(match_qty);

                                    executed_trades.push(Trade::new(
                                        trade_id_counter,
                                        self.symbol.clone(),
                                        best_ask_price,
                                        match_qty,
                                        order.acc_no(),
                                        first_ask.acc_no(),
                                    ));
                                    trade_id_counter += 1;

                                    if first_ask.is_filled() {
                                        ask_queue.pop_front();
                                    }

                                    if order.is_filled() {
                                        break;
                                    }
                                }

                                if ask_queue.is_empty() {
                                    self.asks.remove(&best_ask_price);
                                }
                            }
                        }
                        Less => break,
                    }
                }

                if order.remaining_size() > 0 {
                    let price = order.price().unwrap();
                    self.bids.entry(price).or_default().push_back(order);
                }

                Ok((executed_trades, "Limit Bid Processed.".to_string()))
            }
            BidOrAsk::Ask => {
                while order.remaining_size() > 0 {
                    let best_bid_price = match self.bids.keys().next_back().copied() {
                        Some(price) => price,
                        None => break,
                    };

                    match order.price().unwrap().cmp(&best_bid_price) {
                        Less | Equal => {
                            if let Some(bid_queue) = self.bids.get_mut(&best_bid_price) {
                                while order.remaining_size() > 0 {
                                    let first_bid = match bid_queue.front_mut() {
                                        Some(bid) => bid,
                                        None => break,
                                    };
                                    let remaining_bid = first_bid.remaining_size();
                                    let remaining_ask = order.remaining_size();

                                    self.ltp = best_bid_price;
                                    let match_qty = std::cmp::min(remaining_ask, remaining_bid);

                                    order.fill(match_qty);
                                    first_bid.fill(match_qty);

                                    executed_trades.push(Trade::new(
                                        trade_id_counter,
                                        self.symbol.clone(),
                                        best_bid_price,
                                        match_qty,
                                        first_bid.acc_no(),
                                        order.acc_no(),
                                    ));
                                    trade_id_counter += 1;

                                    if first_bid.is_filled() {
                                        bid_queue.pop_front();
                                    }

                                    if order.is_filled() {
                                        break;
                                    }
                                }

                                if bid_queue.is_empty() {
                                    self.bids.remove(&best_bid_price);
                                }
                            }
                        }
                        Greater => break,
                    }
                }

                if order.remaining_size() > 0 {
                    let price = order.price().unwrap();
                    self.asks.entry(price).or_default().push_back(order);
                }

                Ok((executed_trades, "Limit Ask Processed.".to_string()))
            }
        }
    }

    pub fn add_market_order(&mut self, order: Order) -> Result<(Vec<Trade>, String), String> {
        let mut executed_trades = Vec::new();
        let mut trade_id_counter = 1000u64;

        match order.bid_or_ask() {
            BidOrAsk::Bid => {
                while order.remaining_size() > 0 {
                    let best_ask_price = match self.asks.keys().next().copied() {
                        Some(price) => price,
                        None => break,
                    };

                    if let Some(ask_queue) = self.asks.get_mut(&best_ask_price) {
                        while order.remaining_size() > 0 {
                            let first_ask = match ask_queue.front_mut() {
                                Some(ask) => ask,
                                None => break,
                            };
                            let remaining_ask = first_ask.remaining_size();
                            let remaining_bid = order.remaining_size();

                            self.ltp = best_ask_price;
                            let match_qty = std::cmp::min(remaining_bid, remaining_ask);

                            order.fill(match_qty);
                            first_ask.fill(match_qty);

                            executed_trades.push(Trade::new(
                                trade_id_counter,
                                self.symbol.clone(),
                                best_ask_price,
                                match_qty,
                                order.acc_no(),
                                first_ask.acc_no(),
                            ));
                            trade_id_counter += 1;

                            if first_ask.is_filled() {
                                ask_queue.pop_front();
                            }

                            if order.is_filled() {
                                break;
                            }
                        }

                        if ask_queue.is_empty() {
                            self.asks.remove(&best_ask_price);
                        }
                    }
                }

                if order.filled_size() == 0 {
                    return Ok((executed_trades, "Order Cannot Be Fulfilled.".to_string()));
                }

                Ok((executed_trades, "Order Processed.".to_string()))
            }
            BidOrAsk::Ask => {
                while order.remaining_size() > 0 {
                    let best_bid_price = match self.bids.keys().next_back().copied() {
                        Some(price) => price,
                        None => break,
                    };

                    if let Some(bid_queue) = self.bids.get_mut(&best_bid_price) {
                        while order.remaining_size() > 0 {
                            let first_bid = match bid_queue.front_mut() {
                                Some(bid) => bid,
                                None => break,
                            };
                            let remaining_bid = first_bid.remaining_size();
                            let remaining_ask = order.remaining_size();

                            self.ltp = best_bid_price;
                            let match_qty = std::cmp::min(remaining_ask, remaining_bid);

                            order.fill(match_qty);
                            first_bid.fill(match_qty);

                            executed_trades.push(Trade::new(
                                trade_id_counter,
                                self.symbol.clone(),
                                best_bid_price,
                                match_qty,
                                first_bid.acc_no(),
                                order.acc_no(),
                            ));
                            trade_id_counter += 1;

                            if first_bid.is_filled() {
                                bid_queue.pop_front();
                            }

                            if order.is_filled() {
                                break;
                            }
                        }

                        if bid_queue.is_empty() {
                            self.bids.remove(&best_bid_price);
                        }
                    }
                }

                if order.filled_size() == 0 {
                    return Ok((executed_trades, "Order Cannot Be Fulfilled.".to_string()));
                }

                Ok((executed_trades, "Order Processed.".to_string()))
            }
        }
    }
}
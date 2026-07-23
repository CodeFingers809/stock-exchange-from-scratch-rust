use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::{BTreeMap, VecDeque};

use super::price::Price;
use super::order::{Order, BidOrAsk};

use std::fmt;

#[derive(Debug, Clone, Hash)]
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
            // Print asks in reverse order (highest ask at top, best ask at bottom near spread)
            for (price, queue) in self.asks.iter().rev() {
                let total_volume: u64 = queue.iter().map(|o| o.size - o.filled_size).sum();
                writeln!(f, "  {:>10}  |  {} shares ({} orders)", price, total_volume, queue.len())?;
            }
        }

        writeln!(f, "--------------------------------------------------")?;

        writeln!(f, "--- BIDS (BUYERS) ---")?;
        if self.bids.is_empty() {
            writeln!(f, "  (No bids resting on book)")?;
        } else {
            // Print bids in descending order (best bid at top near spread)
            for (price, queue) in self.bids.iter().rev() {
                let total_volume: u64 = queue.iter().map(|o| o.size - o.filled_size).sum();
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
    pub fn add_limit_order(&mut self, mut order: Order) -> Result<String, String> {
        match order.bid_or_ask {
            BidOrAsk::Bid => {
                // keep matching until our limit bid is filled or seller price is too high
                while order.size - order.filled_size > 0 {
                    // step 1: find the cheapest seller price level available right now
                    let best_ask_price = match self.asks.keys().next().copied() {
                        Some(price) => price,
                        None => break, // no sellers left anywhere on the book! stop matching
                    };

                    // step 2: check limit condition (seller price must be <= our limit bid price)
                    match order.price.unwrap().cmp(&best_ask_price) {
                        Greater | Equal => {
                            // step 3: get the queue of sellers at this price level
                            if let Some(ask_queue) = self.asks.get_mut(&best_ask_price) {
                                while order.size - order.filled_size > 0 {
                                    // step 4: look at the first seller in line
                                    let first_ask = match ask_queue.front_mut() {
                                        Some(ask) => ask,
                                        None => break, // no more sellers at this price level
                                    };
                                    let remaining_ask = first_ask.size - first_ask.filled_size;
                                    let remaining_bid = order.size - order.filled_size;

                                    // step 5: update ltp whenever a trade happens
                                    self.ltp = best_ask_price;

                                    // step 6: compare remaining quantities to execute the fill
                                    match remaining_bid.cmp(&remaining_ask) {
                                        Greater => {
                                            // bid needs more than this seller has -> fill this seller completely and pop them off
                                            order.filled_size += remaining_ask;
                                            first_ask.filled_size = first_ask.size;
                                            ask_queue.pop_front();
                                        }
                                        Less => {
                                            // bid needs less than this seller has -> fill bid completely and finish
                                            order.filled_size += remaining_bid;
                                            first_ask.filled_size += remaining_bid;
                                            break;
                                        }
                                        Equal => {
                                            // exact match -> fill both completely and pop seller off
                                            order.filled_size += remaining_bid;
                                            first_ask.filled_size = first_ask.size;
                                            ask_queue.pop_front();
                                            break;
                                        }
                                    }
                                }

                                // step 7: if all sellers at this price level are gone, remove this empty price level
                                if ask_queue.is_empty() {
                                    self.asks.remove(&best_ask_price);
                                }
                            }
                        }
                        Less => break, // seller is asking more than our limit bid! stop matching
                    }
                }

                // step 8: if limit bid still has remaining unfilled size, add it to bids orderbook!
                if order.size - order.filled_size > 0 {
                    let price = order.price.unwrap();
                    self.bids.entry(price).or_default().push_back(order);
                }

                Ok("Limit Bid Processed.".to_string())
            }
            BidOrAsk::Ask => {
                // keep matching until our limit ask is filled or buyer price is too low
                while order.size - order.filled_size > 0 {
                    // step 1: find the highest buyer price level available right now
                    let best_bid_price = match self.bids.keys().next_back().copied() {
                        Some(price) => price,
                        None => break, // no buyers left anywhere on the book! stop matching
                    };

                    // step 2: check limit condition (buyer price must be >= our limit ask price)
                    match order.price.unwrap().cmp(&best_bid_price) {
                        Less | Equal => {
                            // step 3: get the queue of buyers at this price level
                            if let Some(bid_queue) = self.bids.get_mut(&best_bid_price) {
                                while order.size - order.filled_size > 0 {
                                    // step 4: look at the first buyer in line
                                    let first_bid = match bid_queue.front_mut() {
                                        Some(bid) => bid,
                                        None => break, // no more buyers at this price level
                                    };
                                    let remaining_bid = first_bid.size - first_bid.filled_size;
                                    let remaining_ask = order.size - order.filled_size;

                                    // step 5: update ltp whenever a trade happens
                                    self.ltp = best_bid_price;

                                    // step 6: compare remaining quantities to execute the fill
                                    match remaining_ask.cmp(&remaining_bid) {
                                        Greater => {
                                            // ask needs to sell more than this buyer wants -> fill buyer completely and pop them off
                                            order.filled_size += remaining_bid;
                                            first_bid.filled_size = first_bid.size;
                                            bid_queue.pop_front();
                                        }
                                        Less => {
                                            // ask needs to sell less than this buyer wants -> fill ask completely and finish
                                            order.filled_size += remaining_ask;
                                            first_bid.filled_size += remaining_ask;
                                            break;
                                        }
                                        Equal => {
                                            // exact match -> fill both completely and pop buyer off
                                            order.filled_size += remaining_ask;
                                            first_bid.filled_size = first_bid.size;
                                            bid_queue.pop_front();
                                            break;
                                        }
                                    }
                                }

                                // step 7: if all buyers at this price level are gone, remove this empty price level
                                if bid_queue.is_empty() {
                                    self.bids.remove(&best_bid_price);
                                }
                            }
                        }
                        Greater => break, // buyer is offering less than our limit ask! stop matching
                    }
                }

                // step 8: if limit ask still has remaining unfilled size, add it to asks orderbook!
                if order.size - order.filled_size > 0 {
                    let price = order.price.unwrap();
                    self.asks.entry(price).or_default().push_back(order);
                }

                Ok("Limit Ask Processed.".to_string())
            }
        }
    }
    pub fn add_market_order(&mut self, mut order: Order) -> Result<String, String> {
        match order.bid_or_ask {
            BidOrAsk::Bid => {
                // keep matching until our market bid is fully filled
                while order.size - order.filled_size > 0 {
                    // step 1: find the cheapest seller price level available right now
                    let best_ask_price = match self.asks.keys().next().copied() {
                        Some(price) => price,
                        None => break, // no sellers left anywhere on the book! stop matching
                    };

                    // step 2: get the queue of sellers at this price level
                    if let Some(ask_queue) = self.asks.get_mut(&best_ask_price) {
                        while order.size - order.filled_size > 0 {
                            // step 3: look at the first seller in line
                            let first_ask = match ask_queue.front_mut() {
                                Some(ask) => ask,
                                None => break, // no more sellers at this price level
                            };
                            let remaining_ask = first_ask.size - first_ask.filled_size;
                            let remaining_bid = order.size - order.filled_size;

                            // step 4: update ltp whenever a trade happens
                            self.ltp = best_ask_price;

                            // step 5: compare remaining quantities to execute the fill
                            match remaining_bid.cmp(&remaining_ask) {
                                Greater => {
                                    // bid needs more than this seller has -> fill this seller completely and pop them off
                                    order.filled_size += remaining_ask;
                                    first_ask.filled_size = first_ask.size;
                                    ask_queue.pop_front();
                                }
                                Less => {
                                    // bid needs less than this seller has -> fill bid completely and finish
                                    order.filled_size += remaining_bid;
                                    first_ask.filled_size += remaining_bid;
                                    break;
                                }
                                Equal => {
                                    // exact match -> fill both completely and pop seller off
                                    order.filled_size += remaining_bid;
                                    first_ask.filled_size = first_ask.size;
                                    ask_queue.pop_front();
                                    break;
                                }
                            }
                        }

                        // step 6: if all sellers at this price level are gone, remove this empty price level
                        if ask_queue.is_empty() {
                            self.asks.remove(&best_ask_price);
                        }
                    }
                }

                // step 7: if not a single share could be matched, return unfulfilled status
                if order.filled_size == 0 {
                    return Ok("Order Cannot Be Fulfilled.".to_string());
                }

                Ok("Order Processed.".to_string())
            }
            BidOrAsk::Ask => {
                // keep matching until our market ask is fully filled
                while order.size - order.filled_size > 0 {
                    // step 1: find the highest buyer price level available right now
                    let best_bid_price = match self.bids.keys().next_back().copied() {
                        Some(price) => price,
                        None => break, // no buyers left anywhere on the book! stop matching
                    };

                    // step 2: get the queue of buyers at this price level
                    if let Some(bid_queue) = self.bids.get_mut(&best_bid_price) {
                        while order.size - order.filled_size > 0 {
                            // step 3: look at the first buyer in line
                            let first_bid = match bid_queue.front_mut() {
                                Some(bid) => bid,
                                None => break, // no more buyers at this price level
                            };
                            let remaining_bid = first_bid.size - first_bid.filled_size;
                            let remaining_ask = order.size - order.filled_size;

                            // step 4: update ltp whenever a trade happens
                            self.ltp = best_bid_price;

                            // step 5: compare remaining quantities to execute the fill
                            match remaining_ask.cmp(&remaining_bid) {
                                Greater => {
                                    // ask needs to sell more than this buyer wants -> fill buyer completely and pop them off
                                    order.filled_size += remaining_bid;
                                    first_bid.filled_size = first_bid.size;
                                    bid_queue.pop_front();
                                }
                                Less => {
                                    // ask needs to sell less than this buyer wants -> fill ask completely and finish
                                    order.filled_size += remaining_ask;
                                    first_bid.filled_size += remaining_ask;
                                    break;
                                }
                                Equal => {
                                    // exact match -> fill both completely and pop buyer off
                                    order.filled_size += remaining_ask;
                                    first_bid.filled_size = first_bid.size;
                                    bid_queue.pop_front();
                                    break;
                                }
                            }
                        }

                        // step 6: if all buyers at this price level are gone, remove this empty price level
                        if bid_queue.is_empty() {
                            self.bids.remove(&best_bid_price);
                        }
                    }
                }

                // step 7: if not a single share could be matched, return unfulfilled status
                if order.filled_size == 0 {
                    return Ok("Order Cannot Be Fulfilled.".to_string());
                }

                Ok("Order Processed.".to_string())
            }
        }
    }
}
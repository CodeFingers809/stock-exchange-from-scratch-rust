use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::{BTreeMap, VecDeque};

use super::price::Price;
use super::order::{Order, BidOrAsk};

#[derive(Debug, Clone, Hash)]
pub struct OrderBook {
    pub symbol: String,
    pub ltp: Price,
    pub bids: BTreeMap<Price, VecDeque<Order>>,
    pub asks: BTreeMap<Price, VecDeque<Order>>,
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
                // keep matching until our limit bid is filled or price is too high
                // (YOUR CODE HERE)
                // Step 1: Find cheapest seller price level available right now (self.asks.keys().next())
                // Step 2: Check limit condition (seller price must be <= our limit bid price)
                // Step 3: Get the queue of sellers at this price level
                // Step 4: Look at the first seller in line (front_mut())
                // Step 5: Update ltp whenever a trade happens
                // Step 6: Compare remaining quantities to execute the fill using remaining_bid.cmp(&remaining_ask) (Greater / Less / Equal)
                // Step 7: If all sellers at this price level are gone, remove empty price level
                // Step 8: If limit bid still has remaining size, add it to bids orderbook!

                Ok("Limit Bid Processed.".to_string())
            }
            BidOrAsk::Ask => {
                // keep matching until our limit ask is filled or price is too low
                // (YOUR CODE HERE)
                // Step 1: Find highest buyer price level available right now (self.bids.keys().next_back())
                // Step 2: Check limit condition (buyer price must be >= our limit ask price)
                // Step 3: Get the queue of buyers at this price level
                // Step 4: Look at the first buyer in line (front_mut())
                // Step 5: Update ltp whenever a trade happens
                // Step 6: Compare remaining quantities to execute the fill using remaining_ask.cmp(&remaining_bid) (Greater / Less / Equal)
                // Step 7: If all buyers at this price level are gone, remove empty price level
                // Step 8: If limit ask still has remaining size, add it to asks orderbook!

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
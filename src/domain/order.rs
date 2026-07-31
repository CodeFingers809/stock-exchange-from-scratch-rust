use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
    StopLoss,
    Target,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
}

/// Shared Bracket State between Parent Order, Stop-Loss Order, and Target Order.
/// Enables O(1) activation (only active once Parent is filled) and O(1) OCO cancellation.
#[derive(Debug, Clone)]
pub struct BracketState {
    pub is_parent_filled: Arc<AtomicBool>,
    pub is_bracket_cancelled: Arc<AtomicBool>,
}

impl BracketState {
    pub fn new() -> Self {
        Self {
            is_parent_filled: Arc::new(AtomicBool::new(false)),
            is_bracket_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderInner {
    pub id: String,
    pub symbol: String,
    pub price: Option<Price>,
    pub size: u64,
    pub filled_size: u64,
    pub bid_or_ask: BidOrAsk,
    pub order_type: OrderType,
    pub acc_no: String,
    pub timestamp: SystemTime,
    pub bracket_state: Option<BracketState>,
    pub sl_child: Option<Order>,
    pub tp_child: Option<Order>,
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
            "[{}] Acc: {} | {:?} {} {} @ {} (Filled: {}/{})",
            inner.symbol,
            inner.acc_no,
            inner.order_type,
            inner.bid_or_ask,
            inner.size,
            price_str,
            inner.filled_size,
            inner.size
        )
    }
}

#[derive(Debug, Clone)]
pub struct OrderBuilder {
    pub symbol: String,
    pub price: Option<Price>,
    pub size: u64,
    pub bid_or_ask: BidOrAsk,
    pub acc_no: String,
    pub stop_loss: Option<Price>,
    pub target: Option<Price>,
}

impl OrderBuilder {
    pub fn new(symbol: String, price: Option<Price>, size: u64, bid_or_ask: BidOrAsk, acc_no: String) -> Self {
        Self {
            symbol,
            price,
            size,
            bid_or_ask,
            acc_no,
            stop_loss: None,
            target: None,
        }
    }

    pub fn stop_loss(mut self, sl: Price) -> Self {
        self.stop_loss = Some(sl);
        self
    }

    pub fn target(mut self, tp: Price) -> Self {
        self.target = Some(tp);
        self
    }

    pub fn build(self) -> Order {
        let mut rng = rand::rng();
        use rand::RngExt;
        
        let parent_id = format!("{:016}", rng.random_range(1000_0000_0000_0000u64..=9999_9999_9999_9999u64));
        let sl_id = format!("{:016}", rng.random_range(1000_0000_0000_0000u64..=9999_9999_9999_9999u64));
        let tp_id = format!("{:016}", rng.random_range(1000_0000_0000_0000u64..=9999_9999_9999_9999u64));
        
        let has_bracket = self.stop_loss.is_some() || self.target.is_some();
        let bracket_state = if has_bracket {
            Some(BracketState::new())
        } else {
            None
        };

        // Opposite side for SL / TP exit orders
        let exit_side = match self.bid_or_ask {
            BidOrAsk::Bid => BidOrAsk::Ask,
            BidOrAsk::Ask => BidOrAsk::Bid,
        };

        // Spin off distinct Stop-Loss Order with its own unique 16-digit ID
        let sl_child = self.stop_loss.map(|sl_price| {
            Order(Arc::new(Mutex::new(OrderInner {
                id: sl_id,
                symbol: self.symbol.clone(),
                price: Some(sl_price),
                size: self.size,
                filled_size: 0,
                bid_or_ask: exit_side.clone(),
                order_type: OrderType::StopLoss,
                acc_no: self.acc_no.clone(),
                timestamp: SystemTime::now(),
                bracket_state: bracket_state.clone(),
                sl_child: None,
                tp_child: None,
            })))
        });

        // Spin off distinct Target Order with its own unique 16-digit ID
        let tp_child = self.target.map(|tp_price| {
            Order(Arc::new(Mutex::new(OrderInner {
                id: tp_id,
                symbol: self.symbol.clone(),
                price: Some(tp_price),
                size: self.size,
                filled_size: 0,
                bid_or_ask: exit_side,
                order_type: OrderType::Target,
                acc_no: self.acc_no.clone(),
                timestamp: SystemTime::now(),
                bracket_state: bracket_state.clone(),
                sl_child: None,
                tp_child: None,
            })))
        });

        let parent_order = Order(Arc::new(Mutex::new(OrderInner {
            id: parent_id,
            symbol: self.symbol,
            price: self.price,
            size: self.size,
            filled_size: 0,
            bid_or_ask: self.bid_or_ask,
            order_type: OrderType::Limit,
            acc_no: self.acc_no,
            timestamp: SystemTime::now(),
            bracket_state,
            sl_child,
            tp_child,
        })));

        parent_order
    }
}

impl Order {
    pub fn new(symbol: String, price: Option<Price>, size: u64, bid_or_ask: BidOrAsk, acc_no: String) -> Self {
        OrderBuilder::new(symbol, price, size, bid_or_ask, acc_no).build()
    }

    pub fn builder(symbol: String, price: Option<Price>, size: u64, bid_or_ask: BidOrAsk, acc_no: String) -> OrderBuilder {
        OrderBuilder::new(symbol, price, size, bid_or_ask, acc_no)
    }

    // Fill an order by a given quantity safely
    pub fn fill(&self, qty: u64) {
        let mut inner = self.0.lock().unwrap();
        inner.filled_size += qty;
        if inner.filled_size >= inner.size {
            if let Some(ref bs) = inner.bracket_state {
                bs.is_parent_filled.store(true, Ordering::SeqCst);
            }
        }
    }

    // O(1) Cancellation of bracket family
    pub fn cancel(&self) {
        let inner = self.0.lock().unwrap();
        if let Some(ref bs) = inner.bracket_state {
            bs.is_bracket_cancelled.store(true, Ordering::SeqCst);
        }
    }

    pub fn is_active(&self) -> bool {
        let inner = self.0.lock().unwrap();
        match inner.order_type {
            OrderType::Limit | OrderType::Market => {
                !inner.bracket_state.as_ref().map_or(false, |bs| bs.is_bracket_cancelled.load(Ordering::SeqCst))
            }
            OrderType::StopLoss | OrderType::Target => {
                let bs = match inner.bracket_state.as_ref() {
                    Some(bs) => bs,
                    None => return true,
                };
                let parent_filled = bs.is_parent_filled.load(Ordering::SeqCst);
                let cancelled = bs.is_bracket_cancelled.load(Ordering::SeqCst);
                parent_filled && !cancelled
            }
        }
    }

    // Helper accessors for thread-safe field access
    pub fn id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }

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

    pub fn sl_child(&self) -> Option<Order> {
        self.0.lock().unwrap().sl_child.clone()
    }

    pub fn tp_child(&self) -> Option<Order> {
        self.0.lock().unwrap().tp_child.clone()
    }
}
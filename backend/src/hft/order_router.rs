use std::collections::HashMap;
use std::sync::mpsc::SyncSender;

use crate::domain::order::Order;

pub struct OrderRouter {
    pub exchange_channels: HashMap<String, SyncSender<Order>>,
    pub routed_order_count: u64,
}

impl OrderRouter {
    pub fn new() -> Self {
        Self {
            exchange_channels: HashMap::new(),
            routed_order_count: 0,
        }
    }

    pub fn register_exchange(&mut self, exchange_name: String, sender: SyncSender<Order>) {
        self.exchange_channels.insert(exchange_name, sender);
    }

    pub fn send_order(&mut self, exchange_name: &str, order: Order) -> Result<(), String> {
        if let Some(channel) = self.exchange_channels.get(exchange_name) {
            channel.send(order).map_err(|e| e.to_string())?;
            self.routed_order_count += 1;
            Ok(())
        } else {
            Err(format!("Exchange '{}' channel not found", exchange_name))
        }
    }
}
use std::time::{Duration, Instant};

use crate::domain::market::MarketTick;
use crate::domain::order::{BidOrAsk, Order};
use crate::domain::price::Price;
use super::account::HftUser;
use super::order_router::OrderRouter;

pub type MarketSubscriptionTick = MarketTick;

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Price,   // best_ask on buy exchange (actual cost)
    pub sell_price: Price,  // best_bid on sell exchange (actual revenue)
    pub spread_paisa: u64,  // net_profit = sell_price - buy_price (always positive here)
    pub max_executable_quantity: u64,
}

/// Real-Time Telemetry Payload sent from HFT Engine -> UI Box
#[derive(Debug, Clone)]
pub struct HftTelemetryUpdate {
    pub total_balance_rupees: f64,
    pub realized_pnl_rupees: f64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub total_trades: u64,
    pub hft_internal_latency: Duration,
    pub hft_median_latency: Duration,
    /// Actual actionable net spread: best_bid(sell_ex) - best_ask(buy_ex).
    /// Always >= 0 (0 = no profitable opportunity this tick).
    pub current_spread_paisa: u64,
    pub active_opportunity: Option<ArbitrageOpportunity>,
    pub unified_inventory: i64,
    pub batch_size: u64,
    pub ayushse_ltp: Price,
    pub bohrase_ltp: Price,
}

/// Core Real-Time Cross-Exchange Arbitrage Engine
pub struct CrossExchangeArbitrage {
    pub symbol: String,
    pub user: HftUser,
    pub router: OrderRouter,
    /// Minimum net profit per share in paisa to fire a trade (filters noise)
    pub min_profit_threshold_paisa: i64,
    /// Shares per arb leg — small enough to fit within typical bid/ask depth
    pub max_trade_size: u64,
    /// Maximum net long inventory before pausing buy leg
    pub max_net_long_shares: i64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub total_executed_arbitrages: u64,
    pub hft_internal_latency: Duration,
    pub latency_history: Vec<Duration>,
}

impl CrossExchangeArbitrage {
    pub fn new(symbol: String, user: HftUser, router: OrderRouter) -> Self {
        Self {
            symbol,
            user,
            router,
            min_profit_threshold_paisa: 100, // ₹1 minimum net profit per share
            max_trade_size: 100,             // 100 shares per leg (fits within typical depth)
            max_net_long_shares: 1000,       // pause buys if holding more than 1000 shares net long
            winning_trades: 0,
            losing_trades: 0,
            total_executed_arbitrages: 0,
            hft_internal_latency: Duration::from_nanos(450),
            latency_history: Vec::with_capacity(1000),
        }
    }

    /// Cross-exchange arbitrage using ACTUAL actionable prices.
    ///
    /// Spread = best_bid(sell_exchange) - best_ask(buy_exchange)
    /// This is the real net profit per share after execution — not LTP difference.
    ///
    /// Two directions are evaluated independently each tick:
    ///   Dir 1: Buy on A (pay A.best_ask), Sell on B (receive B.best_bid)
    ///   Dir 2: Buy on B (pay B.best_ask), Sell on A (receive A.best_bid)
    ///
    /// Only the direction with net_profit > threshold is fired.
    /// If neither is profitable, the engine checks if inventory needs flushing.
    pub fn on_market_tick(
        &mut self,
        tick_a: &MarketSubscriptionTick,
        tick_b: &MarketSubscriptionTick,
    ) -> Option<HftTelemetryUpdate> {
        let t0 = Instant::now();

        // Compute net profit for each direction using actual execution prices.
        // None means the required side of the book is empty → direction unavailable.
        let net_dir1: Option<i64> = match (tick_a.best_ask, tick_b.best_bid) {
            (Some(a_ask), Some(b_bid)) =>
                Some(b_bid.paisa as i64 - a_ask.paisa as i64), // Buy A, Sell B
            _ => None,
        };
        let net_dir2: Option<i64> = match (tick_b.best_ask, tick_a.best_bid) {
            (Some(b_ask), Some(a_bid)) =>
                Some(a_bid.paisa as i64 - b_ask.paisa as i64), // Buy B, Sell A
            _ => None,
        };

        // Best actionable spread for display (must be >= 0)
        let best_directional_profit: i64 = match (net_dir1, net_dir2) {
            (Some(d1), Some(d2)) => d1.max(d2),
            (Some(d1), None) => d1,
            (None, Some(d2)) => d2,
            (None, None) => 0,
        };
        let current_spread_paisa: u64 = best_directional_profit.max(0) as u64;

        let current_inventory = self.user.total_shares(&self.symbol);
        let mut opportunity = None;

        // Inventory guard: pause buy leg if already too long
        let inventory_ok = current_inventory < self.max_net_long_shares;

        let mut trade_executed = false;

        // 1. Evaluate whether a profitable arbitrage direction exists
        if inventory_ok {
            let chosen = if net_dir1.unwrap_or(i64::MIN) >= net_dir2.unwrap_or(i64::MIN)
                && net_dir1.unwrap_or(i64::MIN) >= self.min_profit_threshold_paisa
            {
                // Direction 1: Buy A at A.best_ask, Sell B at B.best_bid
                let buy_price = tick_a.best_ask.unwrap();
                let sell_price = tick_b.best_bid.unwrap();
                Some((tick_a, tick_b, buy_price, sell_price, net_dir1.unwrap() as u64))
            } else if net_dir2.unwrap_or(i64::MIN) >= self.min_profit_threshold_paisa {
                // Direction 2: Buy B at B.best_ask, Sell A at A.best_bid
                let buy_price = tick_b.best_ask.unwrap();
                let sell_price = tick_a.best_bid.unwrap();
                Some((tick_b, tick_a, buy_price, sell_price, net_dir2.unwrap() as u64))
            } else {
                None // No profitable direction this tick
            };

            if let Some((buy_tick, sell_tick, buy_price, sell_price, net_paisa)) = chosen {
                let qty = self.max_trade_size;

                opportunity = Some(ArbitrageOpportunity {
                    symbol: self.symbol.clone(),
                    buy_exchange: buy_tick.exchange_name.clone(),
                    sell_exchange: sell_tick.exchange_name.clone(),
                    buy_price,
                    sell_price,
                    spread_paisa: net_paisa,
                    max_executable_quantity: qty,
                });

                let acc_no = self.user.account_number();

                // Market BUY on cheap exchange — fills at their best_ask
                let buy_order = Order::new(
                    self.symbol.clone(),
                    None,
                    qty,
                    BidOrAsk::Bid,
                    acc_no.clone(),
                );

                // Market SELL on expensive exchange — fills at their best_bid
                let sell_order = Order::new(
                    self.symbol.clone(),
                    None,
                    qty,
                    BidOrAsk::Ask,
                    acc_no,
                );

                let _ = self.router.send_order(&buy_tick.exchange_name, buy_order);
                let _ = self.router.send_order(&sell_tick.exchange_name, sell_order);

                self.total_executed_arbitrages += 1;
                self.winning_trades += 1;
                trade_executed = true;
            }
        }

        // 2. HFT Self-Flushing / Inventory Unloading Algorithm
        // If no arb trade occurred this tick AND we have excess inventory (unhedged long/short position),
        // flush/rebalance the open position into the exchange with the best bid/ask.
        if !trade_executed && current_inventory != 0 {
            let acc_no = self.user.account_number();

            if current_inventory > 0 {
                // Excess long inventory -> issue market SELL on the exchange offering the best bid
                let flush_qty = (current_inventory as u64).min(self.max_trade_size);

                let best_ex = match (tick_a.best_bid, tick_b.best_bid) {
                    (Some(a_bid), Some(b_bid)) => {
                        if a_bid.paisa >= b_bid.paisa { &tick_a.exchange_name } else { &tick_b.exchange_name }
                    }
                    (Some(_), None) => &tick_a.exchange_name,
                    (None, Some(_)) => &tick_b.exchange_name,
                    (None, None) => &tick_a.exchange_name,
                };

                let sell_flush_order = Order::new(
                    self.symbol.clone(),
                    None,
                    flush_qty,
                    BidOrAsk::Ask,
                    acc_no,
                );

                let _ = self.router.send_order(best_ex, sell_flush_order);
            } else if current_inventory < 0 {
                // Excess short inventory -> issue market BUY on the exchange offering the best ask
                let flush_qty = ((-current_inventory) as u64).min(self.max_trade_size);

                let best_ex = match (tick_a.best_ask, tick_b.best_ask) {
                    (Some(a_ask), Some(b_ask)) => {
                        if a_ask.paisa <= b_ask.paisa { &tick_a.exchange_name } else { &tick_b.exchange_name }
                    }
                    (Some(_), None) => &tick_a.exchange_name,
                    (None, Some(_)) => &tick_b.exchange_name,
                    (None, None) => &tick_a.exchange_name,
                };

                let buy_flush_order = Order::new(
                    self.symbol.clone(),
                    None,
                    flush_qty,
                    BidOrAsk::Bid,
                    acc_no,
                );

                let _ = self.router.send_order(best_ex, buy_flush_order);
            }
        }

        let elapsed = t0.elapsed();
        self.hft_internal_latency = elapsed;
        self.latency_history.push(elapsed);
        if self.latency_history.len() > 1000 {
            self.latency_history.remove(0);
        }

        // Calculate median latency
        let mut sorted_lat = self.latency_history.clone();
        sorted_lat.sort_unstable();
        let median_lat = if sorted_lat.is_empty() {
            Duration::from_nanos(0)
        } else {
            sorted_lat[sorted_lat.len() / 2]
        };

        let total_balance_rupees = self.user.cash_balance_rupees();
        let realized_pnl_rupees = self.user.realized_pnl_rupees();

        Some(HftTelemetryUpdate {
            total_balance_rupees,
            realized_pnl_rupees,
            winning_trades: self.winning_trades,
            losing_trades: self.losing_trades,
            total_trades: self.total_executed_arbitrages,
            hft_internal_latency: self.hft_internal_latency,
            hft_median_latency: median_lat,
            current_spread_paisa,
            active_opportunity: opportunity.clone(),
            unified_inventory: current_inventory,
            batch_size: opportunity.map_or(0, |o| o.max_executable_quantity),
            ayushse_ltp: tick_a.ltp,
            bohrase_ltp: tick_b.ltp,
        })
    }
}

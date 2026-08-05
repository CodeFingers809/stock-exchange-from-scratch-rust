use rand::RngExt;
use rand_distr::{Distribution, Normal};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::domain::{
    market::Market,
    order::{BidOrAsk, Order},
    price::Price,
    trade::Trade,
};

/// Sentiment state shared across ALL simulators on the same exchange group.
/// buy_prob + regime timing are consensus data — both sims see the same market direction.
/// Volume and price offsets remain independent per simulator.
#[derive(Debug, Clone)]
pub struct SharedSentiment {
    pub buy_prob: f64,
    pub regime_started_at: Instant,
    pub regime_duration: Duration,
}

impl SharedSentiment {
    pub fn new() -> Arc<Mutex<Self>> {
        let mut rng = rand::rng();
        Arc::new(Mutex::new(Self {
            buy_prob: rng.random_range(0.42..=0.58),
            regime_started_at: Instant::now(),
            regime_duration: Duration::from_secs_f64(rng.random_range(4.0..=12.0)),
        }))
    }

    /// Rotate to a wild sentiment regime (20% to 80%) with a slight upside bias (0.47 to 0.57 base).
    fn rotate(rng: &mut impl rand::Rng, _current_price_vs_ref: f64) -> Self {
        let buy_prob = if rng.random_bool(0.70) {
            // Slight upside bias: 47% - 57%
            rng.random_range(0.47..=0.57_f64)
        } else {
            // Wild swing: 20% - 80%
            rng.random_range(0.20..=0.80_f64)
        };

        Self {
            buy_prob,
            regime_started_at: Instant::now(),
            regime_duration: Duration::from_secs_f64(rng.random_range(2.0..=6.0)),
        }
    }
}


pub struct StepMetrics {
    pub trades: Vec<Trade>,
    pub step_latency: Duration,
    pub order_latencies: Vec<Duration>,
}

pub struct Simulator {
    pub symbol: String,
    pub initial_reference_price: Price,
    pub shared_sentiment: Arc<Mutex<SharedSentiment>>,
}

impl Simulator {
    pub fn new(symbol: String, initial_price_paisa: u64, shared_sentiment: Arc<Mutex<SharedSentiment>>) -> Self {
        Self {
            symbol,
            initial_reference_price: Price::from_paisa(initial_price_paisa),
            shared_sentiment,
        }
    }

    pub fn step(&mut self, market: &mut Market) -> StepMetrics {
        let mut rng = rand::rng();

        let current_ltp = market
            .get_orderbook(&self.symbol)
            .map(|b| b.ltp)
            .unwrap_or(self.initial_reference_price);

        let price_vs_ref = current_ltp.paisa as f64 / self.initial_reference_price.paisa as f64;

        let buy_prob = {
            let mut s = self.shared_sentiment.lock().unwrap();
            if s.regime_started_at.elapsed() >= s.regime_duration {
                *s = SharedSentiment::rotate(&mut rng, price_vs_ref);
            }
            s.buy_prob
        };

        // --- Low speed volume: 1–3 random orders per step tick ---
        let target_volume: usize = rng.random_range(1..=3);

        let market_order_prob = 0.25_f64;

        let mut executed_trades = Vec::new();
        let mut order_latencies = Vec::new();

        // Pivot / support-resistance levels from current LTP
        let ltp_p = current_ltp.paisa as f64;
        let r1 = Price::from_paisa(((ltp_p * 1.010) / 5.0).round() as u64 * 5);
        let r2 = Price::from_paisa(((ltp_p * 1.025) / 5.0).round() as u64 * 5);
        let r3 = Price::from_paisa(((ltp_p * 1.050) / 5.0).round() as u64 * 5);
        let s1 = Price::from_paisa(((ltp_p * 0.990) / 5.0).round() as u64 * 5);
        let s2 = Price::from_paisa(((ltp_p * 0.975) / 5.0).round() as u64 * 5);
        let s3 = Price::from_paisa(((ltp_p * 0.950) / 5.0).round() as u64 * 5);

        // Independent price noise per simulator
        let std_dev = (current_ltp.paisa as f64 * 0.0015).max(5.0);
        let normal_dist = Normal::new(0.0, std_dev).unwrap();

        let step_start = Instant::now();

        for _ in 0..target_volume {
            let is_buy = rng.random_bool(buy_prob);
            let side = if is_buy { BidOrAsk::Bid } else { BidOrAsk::Ask };
            let is_market_order = rng.random_bool(market_order_prob);

            // Independent price offset per simulator (different std_dev noise seed)
            let price_offset = normal_dist.sample(&mut rng) as i64;
            let max_offset = (current_ltp.paisa as f64 * 0.005) as i64;
            let clamped_offset = price_offset.clamp(-max_offset, max_offset);

            // Enforce price bounds relative to initial reference price (0.75x min floor, 1.35x max ceiling)
            let min_paisa = (self.initial_reference_price.paisa as f64 * 0.75) as i64;
            let max_paisa = (self.initial_reference_price.paisa as f64 * 1.35) as i64;
            let target_paisa = (current_ltp.paisa as i64 + clamped_offset).clamp(min_paisa, max_paisa) as u64;
            let price_paisa = ((target_paisa / 5) * 5).max(500);

            let order_price = if is_market_order {
                None
            } else {
                Some(Price::from_paisa(price_paisa))
            };

            // Independent order size per simulator
            let size = rng.random_range(1..=200u64);
            let acc_no = format!("{}", rng.random_range(100..998u32));

            let (stop_loss, target) = if rng.random_bool(0.30) {
                match side {
                    BidOrAsk::Bid => {
                        let sl = match rng.random_range(1..=3u8) { 1 => s1, 2 => s2, _ => s3 };
                        let tp = match rng.random_range(1..=3u8) { 1 => r1, 2 => r2, _ => r3 };
                        (Some(sl), Some(tp))
                    }
                    BidOrAsk::Ask => {
                        let sl = match rng.random_range(1..=3u8) { 1 => r1, 2 => r2, _ => r3 };
                        let tp = match rng.random_range(1..=3u8) { 1 => s1, 2 => s2, _ => s3 };
                        (Some(sl), Some(tp))
                    }
                }
            } else {
                (None, None)
            };

            let order = Order::builder(self.symbol.clone(), order_price, size, side, acc_no);
            let order = match (stop_loss, target) {
                (Some(sl), Some(tp)) => order.stop_loss(sl).target(tp).build(),
                (Some(sl), None) => order.stop_loss(sl).build(),
                (None, Some(tp)) => order.target(tp).build(),
                (None, None) => order.build(),
            };

            let t0 = Instant::now();
            let res = if is_market_order {
                market.place_market_order(order)
            } else {
                market.place_limit_order(order)
            };
            order_latencies.push(t0.elapsed());

            if let Ok((trades, _)) = res {
                executed_trades.extend(trades);
            }
        }

        StepMetrics {
            trades: executed_trades,
            step_latency: step_start.elapsed(),
            order_latencies,
        }
    }
}

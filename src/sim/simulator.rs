use rand::{Rng, RngExt};
use rand_distr::{Distribution, Normal};
use std::time::{Duration, Instant};

use crate::domain::{
    market::Market,
    order::{BidOrAsk, Order},
    price::Price,
    trade::Trade,
};

#[derive(Debug, Clone, Copy)]
pub enum MarketSpeed {
    Normal, // 60% of steps
    Fast,   // 40% of steps
}

#[derive(Debug, Clone)]
pub struct RegimeState {
    pub speed: MarketSpeed,
    pub buy_prob: f64,        // Current interpolated Buy probability (0.10 to 0.90)
    pub target_buy_prob: f64, // Target Buy probability to gently move towards
    pub sell_prob: f64,       // 100% - Buy probability
    pub market_order_prob: f64, // Fixed at 70% (0.70)
    pub speed_started_at: Instant,
    pub speed_duration: Duration, // 10 seconds
    pub prob_started_at: Instant,
    pub prob_duration: Duration,  // Random 3 to 4 seconds
}

pub struct StepMetrics {
    pub trades: Vec<Trade>,
    pub step_latency: Duration,
    pub order_latencies: Vec<Duration>,
}

pub struct Simulator {
    pub symbol: String,
    pub current_regime: RegimeState,
}

impl Simulator {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            current_regime: Self::generate_new_regime(),
        }
    }

    fn generate_new_regime() -> RegimeState {
        let mut rng = rand::rng();
        let speed = if rng.random_bool(0.40) {
            MarketSpeed::Fast
        } else {
            MarketSpeed::Normal
        };
        let target_buy_prob = rng.random_range(0.15..=0.85);
        let prob_duration_secs = rng.random_range(3.0..=4.0);

        RegimeState {
            speed,
            buy_prob: 0.50,
            target_buy_prob,
            sell_prob: 0.50,
            market_order_prob: 0.70, // 70% market orders
            speed_started_at: Instant::now(),
            speed_duration: Duration::from_secs(10),
            prob_started_at: Instant::now(),
            prob_duration: Duration::from_secs_f64(prob_duration_secs),
        }
    }

    fn check_and_update_regime(&mut self) {
        let mut rng = rand::rng();

        // 1. Toggle speed regime every 10 seconds (Slow/Normal <-> Fast)
        if self.current_regime.speed_started_at.elapsed() >= self.current_regime.speed_duration {
            self.current_regime.speed = match self.current_regime.speed {
                MarketSpeed::Normal => MarketSpeed::Fast,
                MarketSpeed::Fast => MarketSpeed::Normal,
            };
            self.current_regime.speed_started_at = Instant::now();
        }

        // 2. Pick a new target probability every 3 to 4 seconds
        if self.current_regime.prob_started_at.elapsed() >= self.current_regime.prob_duration {
            self.current_regime.target_buy_prob = rng.random_range(0.15..=0.85);
            let prob_duration_secs = rng.random_range(3.0..=4.0);
            self.current_regime.prob_duration = Duration::from_secs_f64(prob_duration_secs);
            self.current_regime.prob_started_at = Instant::now();
        }

        // 3. Gently interpolate buy_prob towards target_buy_prob (smooth growth/shrink)
        let step = 0.05; // 5% shift per iteration towards target
        if (self.current_regime.buy_prob - self.current_regime.target_buy_prob).abs() > step {
            if self.current_regime.buy_prob < self.current_regime.target_buy_prob {
                self.current_regime.buy_prob += step;
            } else {
                self.current_regime.buy_prob -= step;
            }
        } else {
            self.current_regime.buy_prob = self.current_regime.target_buy_prob;
        }
        self.current_regime.sell_prob = 1.0 - self.current_regime.buy_prob;
    }

    pub fn step(&mut self, market: &mut Market) -> StepMetrics {
        self.check_and_update_regime();

        let mut rng = rand::rng();
        let mut executed_trades = Vec::new();
        let mut order_latencies = Vec::new();

        // Increased order counts: Normal (10..=30), Fast (50..=250)
        let order_count = match self.current_regime.speed {
            MarketSpeed::Normal => rng.random_range(10..=30),
            MarketSpeed::Fast => rng.random_range(50..=250),
        };

        let current_ltp = market
            .get_orderbook(&self.symbol)
            .map(|b| b.ltp)
            .unwrap_or_else(|| Price::from_rupees_paisa(2245, 0));

        // Constricted Gaussian distribution centered tightly at 0 (0.4% std dev) to prevent price runaway
        let std_dev = (current_ltp.paisa as f64 * 0.004).max(10.0);
        let normal_dist = Normal::new(0.0, std_dev).unwrap();

        let step_start = Instant::now();

        for _ in 0..order_count {
            let is_buy = rng.random_bool(self.current_regime.buy_prob);
            let side = if is_buy { BidOrAsk::Bid } else { BidOrAsk::Ask };
            let is_market_order = rng.random_bool(self.current_regime.market_order_prob);

            let price_offset = normal_dist.sample(&mut rng) as i64;
            // Tightly clamp limit prices within 1.5% range to keep market price stable
            let max_offset = (current_ltp.paisa as f64 * 0.015) as i64;
            let clamped_offset = price_offset.clamp(-max_offset, max_offset);

            let price_paisa = (current_ltp.paisa as i64 + clamped_offset).max(100) as u64;

            let order_price = if is_market_order {
                None
            } else {
                Some(Price::from_paisa(price_paisa))
            };

            let size = rng.random_range(1..=300);
            let acc_no = format!("SIM_ACC_{}", rng.random_range(100..999));

            let order = Order::new(self.symbol.clone(), order_price, size, side, acc_no);

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

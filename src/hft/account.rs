use std::sync::{Arc, Mutex};
use crate::domain::{portfolio::Portfolio, user::User};

/// HFT Trader Identity wrapping official domain User & Portfolio.
/// Portfolio is Arc<Mutex<>> so the same object is shared between
/// the market execution threads (which apply fills) and the HFT engine
/// (which reads balance / PnL for telemetry).
#[derive(Clone)]
pub struct HftUser {
    pub user: User,
    pub portfolio: Arc<Mutex<Portfolio>>,
    pub initial_cash_paisa: u64,
}

impl HftUser {
    /// ₹1,000,000,000 (1 Billion INR) starting capital.
    /// Account number is a pure numeric string "999" —
    /// no prefixes, treated identically by the market.
    pub fn new_with_billion_capital(user_id: String, name: String) -> Self {
        let user = User::new(user_id, name);
        let initial_cash_paisa = 100_000_000_000u64;
        let portfolio = Portfolio::new(user.clone(), "999".to_string(), initial_cash_paisa);

        Self {
            user,
            portfolio: Arc::new(Mutex::new(portfolio)),
            initial_cash_paisa,
        }
    }

    pub fn account_number(&self) -> String {
        self.portfolio.lock().unwrap().acc_no.clone()
    }

    pub fn cash_balance_rupees(&self) -> f64 {
        self.portfolio.lock().unwrap().balance_paisa as f64 / 100.0
    }

    pub fn realized_pnl_rupees(&self) -> f64 {
        let balance = self.portfolio.lock().unwrap().balance_paisa;
        (balance as i64 - self.initial_cash_paisa as i64) as f64 / 100.0
    }

    pub fn total_shares(&self, symbol: &str) -> i64 {
        self.portfolio.lock().unwrap().total_shares(symbol) as i64
    }
}

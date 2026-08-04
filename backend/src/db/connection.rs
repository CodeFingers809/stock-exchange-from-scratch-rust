use anyhow::Result;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub struct DbPool {
    pool: SqlitePool,
}

impl DbPool {
    pub async fn init(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5) // Max 5 active connections pool limit (RDS cost protection)
            .min_connections(1) // Keep idle connections low
            .idle_timeout(std::time::Duration::from_secs(30)) // Close idle connections promptly
            .connect(database_url)
            .await?;

        // Initialize schema tables if not exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS portfolios (
                acc_no TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                balance_paisa INTEGER NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS holdings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                acc_no TEXT NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                buy_price_paisa INTEGER NOT NULL,
                bought_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS trades (
                trade_id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                price_paisa INTEGER NOT NULL,
                quantity INTEGER NOT NULL,
                buyer_acc_no TEXT NOT NULL,
                seller_acc_no TEXT NOT NULL,
                timestamp_millis INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS stock_prices (
                symbol TEXT NOT NULL,
                exchange TEXT NOT NULL,
                price_paisa INTEGER NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (symbol, exchange)
            );
            CREATE TABLE IF NOT EXISTS candles (
                symbol TEXT NOT NULL,
                exchange TEXT NOT NULL,
                time_sec INTEGER NOT NULL,
                open_paisa INTEGER NOT NULL,
                high_paisa INTEGER NOT NULL,
                low_paisa INTEGER NOT NULL,
                close_paisa INTEGER NOT NULL,
                volume INTEGER NOT NULL,
                PRIMARY KEY (symbol, exchange, time_sec)
            );
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub fn get_pool(&self) -> SqlitePool {
        self.pool.clone()
    }

    pub async fn load_portfolio_balance(&self, acc_no: &str) -> Result<Option<u64>> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT balance_paisa FROM portfolios WHERE acc_no = ?")
            .bind(acc_no)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.0 as u64))
    }

    pub async fn save_portfolio_balance(&self, acc_no: &str, user_id: &str, balance_paisa: u64) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO portfolios (acc_no, user_id, balance_paisa)
            VALUES (?, ?, ?)
            ON CONFLICT(acc_no) DO UPDATE SET balance_paisa = excluded.balance_paisa, updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(acc_no)
        .bind(user_id)
        .bind(balance_paisa as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_stock_prices(&self) -> Result<Vec<(String, String, u64)>> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT symbol, exchange, price_paisa FROM stock_prices"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(sym, ex, px)| (sym, ex, px as u64)).collect())
    }

    pub async fn save_stock_price(&self, symbol: &str, exchange: &str, price_paisa: u64) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stock_prices (symbol, exchange, price_paisa)
            VALUES (?, ?, ?)
            ON CONFLICT(symbol, exchange) DO UPDATE SET price_paisa = excluded.price_paisa, updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(symbol)
        .bind(exchange)
        .bind(price_paisa as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_recent_candles(&self) -> Result<Vec<(String, String, i64, i64, i64, i64, i64, i64)>> {
        let rows: Vec<(String, String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT symbol, exchange, time_sec, open_paisa, high_paisa, low_paisa, close_paisa, volume
            FROM candles
            ORDER BY time_sec ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn load_holdings(&self, acc_no: &str) -> Result<Vec<(String, u64, u64)>> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT symbol, quantity, buy_price_paisa FROM holdings WHERE acc_no = ?"
        )
        .bind(acc_no)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(sym, qty, px)| (sym, qty as u64, px as u64)).collect())
    }
}

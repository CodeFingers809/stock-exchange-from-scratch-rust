use anyhow::Result;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::events::MarketEvent;

pub struct SqliteDbWriter {
    pool: SqlitePool,
}

impl SqliteDbWriter {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Spawns an async worker loop that consumes events non-blockingly and persists to SQLite database
    pub fn start_worker(self, mut rx: mpsc::UnboundedReceiver<MarketEvent>) {
        tokio::spawn(async move {
            println!("[SQLite Writer] Background Writer Worker started.");
            while let Some(event) = rx.recv().await {
                if let MarketEvent::Trade(trade) = event {
                    if let Err(e) = self.record_trade(&trade).await {
                        eprintln!("[SQLite Writer Error] Failed to persist trade record: {:?}", e);
                    }
                }
            }
        });
    }

    async fn record_trade(&self, trade: &crate::events::TradeEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO trades (trade_id, symbol, price_paisa, quantity, buyer_acc_no, seller_acc_no, timestamp_millis)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trade.trade_id)
        .bind(&trade.symbol)
        .bind(trade.price_paisa as i64)
        .bind(trade.quantity as i64)
        .bind(&trade.buyer_acc_no)
        .bind(&trade.seller_acc_no)
        .bind(trade.timestamp_millis as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

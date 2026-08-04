use anyhow::Result;
use redis::aio::ConnectionManager;
use tokio::sync::mpsc;
use super::MarketEvent;

pub struct RedisStreamPublisher {
    connection: ConnectionManager,
}

impl RedisStreamPublisher {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = ConnectionManager::new(client).await?;
        println!("[Redis Stream] Connected to Redis Server at {}", redis_url);
        Ok(Self { connection })
    }

    /// Spawns an async worker loop that consumes events non-blockingly and publishes to Redis Streams
    pub fn start_worker(mut self, mut rx: mpsc::UnboundedReceiver<MarketEvent>) {
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = self.publish_event(&event).await {
                    eprintln!("[Redis Worker Error] Failed to publish stream event: {:?}", e);
                }
            }
        });
    }

    async fn publish_event(&mut self, event: &MarketEvent) -> Result<()> {
        match event {
            MarketEvent::Trade(trade) => {
                let payload = serde_json::to_string(trade)?;
                let _: String = redis::cmd("XADD")
                    .arg("trades:stream")
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(1000) // Keep stream trimmed to last 1000 items (~100KB max memory)
                    .arg("*")
                    .arg("trade_id")
                    .arg(&trade.trade_id)
                    .arg("symbol")
                    .arg(&trade.symbol)
                    .arg("price_paisa")
                    .arg(&trade.price_paisa.to_string())
                    .arg("quantity")
                    .arg(&trade.quantity.to_string())
                    .arg("buyer")
                    .arg(&trade.buyer_acc_no)
                    .arg("seller")
                    .arg(&trade.seller_acc_no)
                    .arg("json")
                    .arg(&payload)
                    .query_async(&mut self.connection)
                    .await?;
            }
            MarketEvent::Volume(vol) => {
                let payload = serde_json::to_string(vol)?;
                let _: String = redis::cmd("XADD")
                    .arg("volume:stream")
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(500)
                    .arg("*")
                    .arg("exchange")
                    .arg(&vol.exchange_name)
                    .arg("symbol")
                    .arg(&vol.symbol)
                    .arg("ltp_paisa")
                    .arg(&vol.ltp_paisa.to_string())
                    .arg("cumulative_trades")
                    .arg(&vol.cumulative_trades.to_string())
                    .arg("json")
                    .arg(&payload)
                    .query_async(&mut self.connection)
                    .await?;
            }
            MarketEvent::HftTelemetry(hft) => {
                let payload = serde_json::to_string(hft)?;
                let _: String = redis::cmd("XADD")
                    .arg("hft:telemetry:stream")
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(500)
                    .arg("*")
                    .arg("realized_pnl")
                    .arg(&hft.realized_pnl_rupees.to_string())
                    .arg("current_spread")
                    .arg(&hft.current_spread_paisa.to_string())
                    .arg("inventory")
                    .arg(&hft.unified_inventory.to_string())
                    .arg("json")
                    .arg(&payload)
                    .query_async(&mut self.connection)
                    .await?;
            }
        }
        Ok(())
    }
}

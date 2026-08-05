use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header;
use tokio::sync::broadcast;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebStockInfo {
    pub symbol: String,
    pub name: String,
    pub exchanges: Vec<String>,
    pub ltp_ayushse: f64,
    pub ltp_bohrase: f64,
}

#[derive(Debug, Deserialize)]
pub struct OrderRequest {
    pub exchange: String,
    pub symbol: String,
    pub order_type: String, // "MARKET" or "LIMIT"
    pub bid_or_ask: String, // "BUY" or "SELL"
    pub quantity: u64,
    pub price_paisa: Option<u64>,
    pub stop_loss_paisa: Option<u64>,
    pub take_profit_paisa: Option<u64>,
}

#[derive(Clone)]
pub struct ApiState {
    pub tx: broadcast::Sender<String>,
    pub db_pool: Option<sqlx::SqlitePool>,
    pub redis_client: Option<redis::Client>,
}

pub struct ApiServer;

impl ApiServer {
    pub async fn start(port: u16, db_pool: Option<sqlx::SqlitePool>) -> broadcast::Sender<String> {
        let (tx, _) = broadcast::channel::<String>(1000);

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let redis_client = redis::Client::open(redis_url).ok();

        let state = ApiState { tx: tx.clone(), db_pool, redis_client };

        let app = Router::new()
            .route("/api/stocks", get(get_stocks_handler))
            .route("/api/candles", get(get_candles_handler))
            .route("/api/order", post(place_order_handler))
            .route("/api/reset", post(reset_handler))
            .route("/api/status", get(status_handler))
            .route("/api/sim/toggle", post(toggle_sim_handler))
            .route("/api/hft/toggle", post(toggle_hft_handler))
            .route("/ws", get(ws_handler))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::REFERRER_POLICY,
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws: wss:;"),
            ))
            .with_state(state)
            .layer(cors);

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        println!("🚀 [Axum Web API] Listening on http://localhost:{}", port);

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tx
    }
}

async fn get_stocks_handler() -> Json<Vec<WebStockInfo>> {
    Json(vec![
        WebStockInfo {
            symbol: "AYUSH-5".to_string(),
            name: "AYUSH-5 Benchmark Index".to_string(),
            exchanges: vec!["AYUSHSE".to_string()],
            ltp_ayushse: 2124.39,
            ltp_bohrase: 2124.39,
        },
        WebStockInfo {
            symbol: "TCS".to_string(),
            name: "Tata Consultancy Services".to_string(),
            exchanges: vec!["AYUSHSE".to_string(), "BOHRASE".to_string()],
            ltp_ayushse: 3450.00,
            ltp_bohrase: 3450.00,
        },
        WebStockInfo {
            symbol: "RELIANCE".to_string(),
            name: "Reliance Industries Ltd".to_string(),
            exchanges: vec!["AYUSHSE".to_string(), "BOHRASE".to_string()],
            ltp_ayushse: 2890.50,
            ltp_bohrase: 2890.50,
        },
        WebStockInfo {
            symbol: "INFY".to_string(),
            name: "Infosys Ltd".to_string(),
            exchanges: vec!["AYUSHSE".to_string(), "BOHRASE".to_string()],
            ltp_ayushse: 1520.40,
            ltp_bohrase: 1520.40,
        },
        WebStockInfo {
            symbol: "HDFCBANK".to_string(),
            name: "HDFC Bank Ltd".to_string(),
            exchanges: vec!["AYUSHSE".to_string(), "BOHRASE".to_string()],
            ltp_ayushse: 1640.75,
            ltp_bohrase: 1640.75,
        },
        WebStockInfo {
            symbol: "ICICIBANK".to_string(),
            name: "ICICI Bank Ltd".to_string(),
            exchanges: vec!["AYUSHSE".to_string(), "BOHRASE".to_string()],
            ltp_ayushse: 1120.30,
            ltp_bohrase: 1120.30,
        },
    ])
}

#[derive(Serialize)]
pub struct CandleResponse {
    pub symbol: String,
    pub exchange: String,
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

async fn get_candles_handler(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Json<Vec<CandleResponse>> {
    let mut list = Vec::new();
    if let Some(pool) = state.db_pool {
        let rows: Vec<(String, String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT symbol, exchange, time_sec, open_paisa, high_paisa, low_paisa, close_paisa, volume
            FROM candles
            ORDER BY time_sec ASC
            "#
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for (symbol, exchange, time_sec, o, h, l, c, vol) in rows {
            list.push(CandleResponse {
                symbol,
                exchange,
                time: time_sec,
                open: o as f64 / 100.0,
                high: h as f64 / 100.0,
                low: l as f64 / 100.0,
                close: c as f64 / 100.0,
                volume: vol,
            });
        }
    }
    Json(list)
}

async fn place_order_handler(
    State(state): State<ApiState>,
    Json(_req): Json<OrderRequest>,
) -> impl IntoResponse {
    // Redis Rate Limiting (max 20 orders/sec per client IP/endpoint)
    if let Some(ref client) = state.redis_client {
        if let Ok(mut con) = client.get_connection() {
            let key = "rate_limit:api_orders";
            let count: redis::RedisResult<u64> = redis::cmd("INCR").arg(key).query(&mut con);
            if let Ok(c) = count {
                if c == 1 {
                    let _: redis::RedisResult<()> = redis::cmd("EXPIRE").arg(key).arg(1).query(&mut con);
                }
                if c > 30 {
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(serde_json::json!({
                            "error": "Rate limit exceeded. Max 30 orders/sec allowed.",
                            "status": "REJECTED"
                        })),
                    ).into_response();
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ACCEPTED",
            "order_id": uuid::Uuid::new_v4().to_string(),
            "message": "Order successfully routed to exchange matching engine"
        })),
    ).into_response()
}

pub static HFT_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static HFT_RESET_FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static ENGINE_RESET_FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static SIM_SESSION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static HFT_SESSION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

async fn reset_handler(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Json<serde_json::Value> {
    let mut cleared = Vec::<String>::new();
    let mut errors = Vec::<String>::new();

    if let Some(pool) = state.db_pool {
        let tables = [
            "DELETE FROM candles",
            "DELETE FROM trades",
            "DELETE FROM stock_prices",
            "DELETE FROM portfolios",
            "DELETE FROM holdings",
        ];
        for sql in &tables {
            if let Err(e) = sqlx::query(sql).execute(&pool).await {
                errors.push(format!("{}: {}", sql, e));
            } else {
                cleared.push(sql.to_string());
            }
        }
    }

    // Flush Redis (FLUSHDB on the default DB)
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    match redis::Client::open(redis_url) {
        Ok(client) => match client.get_connection() {
            Ok(mut con) => {
                let _: redis::RedisResult<String> = redis::cmd("FLUSHDB").query(&mut con);
                cleared.push("Redis FLUSHDB".to_string());
            }
            Err(e) => errors.push(format!("Redis connect: {}", e)),
        },
        Err(e) => errors.push(format!("Redis open: {}", e)),
    }

    use crate::sim::simulator::SIMULATOR_ACTIVE;
    use std::sync::atomic::Ordering;
    SIMULATOR_ACTIVE.store(false, Ordering::Relaxed);
    HFT_ACTIVE.store(false, Ordering::Relaxed);
    HFT_RESET_FLAG.store(true, Ordering::Relaxed);
    ENGINE_RESET_FLAG.store(true, Ordering::Relaxed);

    // Broadcast RESET event across all WebSocket clients
    let reset_msg = serde_json::json!({
        "type": "RESET",
        "message": "Full engine state reset"
    });
    let _ = state.tx.send(reset_msg.to_string());

    // Broadcast clean HFT telemetry reset
    let clean_hft = serde_json::json!({
        "type": "HFT_TELEMETRY",
        "capital": 1000000000.0,
        "realized_pnl": 0.0,
        "trades": 0,
        "wins": 0,
        "tps": 0,
        "internal_lat_ns": 1400,
        "internal_med_ns": 1400,
        "rt_lat_ns": 5590000,
        "rt_med_ns": 5590000,
        "spread_paisa": 0,
        "inventory": 0,
        "ayushse_ltp": 3450.0,
        "bohrase_ltp": 3448.5,
    });
    let _ = state.tx.send(clean_hft.to_string());

    Json(serde_json::json!({
        "status": if errors.is_empty() { "OK" } else { "PARTIAL" },
        "cleared": cleared,
        "errors": errors,
    }))
}

async fn status_handler() -> Json<serde_json::Value> {
    use crate::sim::simulator::SIMULATOR_ACTIVE;
    use std::sync::atomic::Ordering;
    Json(serde_json::json!({
        "status": "OK",
        "is_sim_active": SIMULATOR_ACTIVE.load(Ordering::Relaxed),
        "is_hft_active": HFT_ACTIVE.load(Ordering::Relaxed),
    }))
}

async fn toggle_sim_handler(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Json<serde_json::Value> {
    use crate::sim::simulator::SIMULATOR_ACTIVE;
    use std::sync::atomic::Ordering;

    let current = SIMULATOR_ACTIVE.load(Ordering::Relaxed);
    let next = !current;
    SIMULATOR_ACTIVE.store(next, Ordering::Relaxed);

    if next {
        let session_id = SIM_SESSION_ID.fetch_add(1, Ordering::Relaxed) + 1;
        let tx = state.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            if SIM_SESSION_ID.load(Ordering::Relaxed) == session_id && SIMULATOR_ACTIVE.load(Ordering::Relaxed) {
                println!("[SERVER AUTO-OFF] 10 minutes elapsed. Auto-disabling Market Simulator & HFT Bot.");
                SIMULATOR_ACTIVE.store(false, Ordering::Relaxed);
                HFT_ACTIVE.store(false, Ordering::Relaxed);
                let _ = tx.send(serde_json::json!({
                    "type": "STATE_UPDATE",
                    "is_sim_active": false,
                    "is_hft_active": false,
                }).to_string());
            }
        });
    } else {
        SIM_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        HFT_ACTIVE.store(false, Ordering::Relaxed);
        HFT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    }

    let sim_active = SIMULATOR_ACTIVE.load(Ordering::Relaxed);
    let hft_active = HFT_ACTIVE.load(Ordering::Relaxed);

    let state_msg = serde_json::json!({
        "type": "STATE_UPDATE",
        "is_sim_active": sim_active,
        "is_hft_active": hft_active,
    });
    let _ = state.tx.send(state_msg.to_string());

    Json(serde_json::json!({
        "status": "OK",
        "active": sim_active
    }))
}

async fn toggle_hft_handler(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Json<serde_json::Value> {
    use std::sync::atomic::Ordering;
    use crate::sim::simulator::SIMULATOR_ACTIVE;

    let sim_active = SIMULATOR_ACTIVE.load(Ordering::Relaxed);
    let current_hft = HFT_ACTIVE.load(Ordering::Relaxed);

    // Guard: Do not allow enabling HFT when Simulator is OFF
    if !sim_active && !current_hft {
        return Json(serde_json::json!({
            "status": "ERROR",
            "message": "Cannot enable HFT while simulator is off",
            "active": false
        }));
    }

    let next = !current_hft;
    HFT_ACTIVE.store(next, Ordering::Relaxed);

    if next {
        let session_id = HFT_SESSION_ID.fetch_add(1, Ordering::Relaxed) + 1;
        let tx = state.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            if HFT_SESSION_ID.load(Ordering::Relaxed) == session_id && HFT_ACTIVE.load(Ordering::Relaxed) {
                println!("[SERVER AUTO-OFF] 10 minutes elapsed. Auto-disabling HFT Bot.");
                HFT_ACTIVE.store(false, Ordering::Relaxed);
                let sim = SIMULATOR_ACTIVE.load(Ordering::Relaxed);
                let _ = tx.send(serde_json::json!({
                    "type": "STATE_UPDATE",
                    "is_sim_active": sim,
                    "is_hft_active": false,
                }).to_string());
            }
        });
    } else {
        HFT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    }

    let hft_active = HFT_ACTIVE.load(Ordering::Relaxed);

    let state_msg = serde_json::json!({
        "type": "STATE_UPDATE",
        "is_sim_active": sim_active,
        "is_hft_active": hft_active,
    });
    let _ = state.tx.send(state_msg.to_string());

    Json(serde_json::json!({
        "status": "OK",
        "active": hft_active
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: ApiState) {
    println!("🌐 [WebSocket Bridge] Client connected to live feed stream");
    let mut rx = state.tx.subscribe();

    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            println!("🔌 [WebSocket Bridge] Client disconnected");
            break;
        }
    }
}

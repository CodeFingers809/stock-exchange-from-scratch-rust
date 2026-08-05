use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use stock_exchange_rust::{
    domain::{market::Market, order::Order, price::Price},
    sim::simulator::{Simulator, SharedSentiment},
};

fn calculate_median_duration(mut durations: Vec<Duration>) -> Duration {
    if durations.is_empty() {
        return Duration::from_secs(0);
    }
    durations.sort();
    let mid = durations.len() / 2;
    if durations.len() % 2 == 0 {
        (durations[mid - 1] + durations[mid]) / 2
    } else {
        durations[mid]
    }
}

// Indian Numbering System Formatter (e.g. 1,00,00,00,000.00)
fn format_indian_currency(amount: f64) -> String {
    let is_negative = amount < 0.0;
    let abs_amount = amount.abs();
    let paisa = ((abs_amount - abs_amount.floor()) * 100.0).round() as u64;
    let whole = abs_amount.floor() as u64;

    let s = whole.to_string();
    let len = s.len();
    if len <= 3 {
        format!("{}{} .{:02}", if is_negative { "-" } else { "" }, s, paisa)
    } else {
        let last_three = &s[len - 3..];
        let remaining = &s[..len - 3];
        let mut formatted = String::new();
        let mut count = 0;
        for c in remaining.chars().rev() {
            if count > 0 && count % 2 == 0 {
                formatted.push(',');
            }
            formatted.push(c);
            count += 1;
        }
        let remaining_formatted: String = formatted.chars().rev().collect();
        format!(
            "{}{},{}.{:02}",
            if is_negative { "-" } else { "" },
            remaining_formatted,
            last_three,
            paisa
        )
    }
}

#[allow(dead_code)]
fn format_latency(d: Duration) -> String {
    let nanos = d.as_nanos();
    if nanos < 1_000 {
        format!("{}ns", nanos)
    } else if nanos < 1_000_000 {
        format!("{:.2}µs", nanos as f64 / 1_000.0)
    } else {
        format!("{:.2}ms", nanos as f64 / 1_000_000.0)
    }
}

// Telemetry output sent from Market Engine Core -> UI Renderer Core
struct MarketStateUpdate {
    exchange_name: String,
    ltp: Price,
    orders_processed: usize,
    trades_executed: usize,
    sl_hits: usize,
    tp_hits: usize,
    _step_median_latency: Duration,
    overall_median_latency: Duration,
    round_trip_median_latency: Duration,
    resting_bids: Vec<(Price, u64, usize)>,
    resting_asks: Vec<(Price, u64, usize)>,
    target_volume: usize,
    buy_prob: f64,
}

fn spawn_exchange_pair(
    exchange_name: String,
    stock_initial_prices: Vec<(&'static str, u64)>,
    ui_tx: mpsc::Sender<MarketStateUpdate>,
    shared_sentiments: std::collections::HashMap<String, Arc<Mutex<SharedSentiment>>>,
    hft_order_rx_param: Option<mpsc::Receiver<Order>>,
    hft_tick_tx: mpsc::Sender<stock_exchange_rust::hft::MarketSubscriptionTick>,
    shared_hft_portfolio: Option<Arc<Mutex<stock_exchange_rust::domain::portfolio::Portfolio>>>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<stock_exchange_rust::events::MarketEvent>>,
    event_tx_db: Option<tokio::sync::mpsc::UnboundedSender<stock_exchange_rust::events::MarketEvent>>,
) -> Arc<Mutex<std::collections::HashMap<String, u64>>> {
    let mut init_map = std::collections::HashMap::new();
    for &(sym, px) in &stock_initial_prices {
        init_map.insert(sym.to_string(), px);
    }
    let shared_ltps = Arc::new(Mutex::new(init_map));

    let hft_order_rx = hft_order_rx_param.unwrap_or_else(|| {
        let (_tx, rx) = mpsc::sync_channel::<Order>(10_000);
        rx
    });

    let market_name = exchange_name.clone();
    let shared_ltps_market = shared_ltps.clone();

    let stock_prices_copy = stock_initial_prices.clone();

    thread::spawn(move || {
        let mut market = Market::new(market_name.clone());
        let mut sim_map = std::collections::HashMap::new();

        for &(sym, px) in &stock_prices_copy {
            let initial_price = Price::from_paisa(px);
            market.add_stock(sym.to_string(), initial_price);
            let sentiment = shared_sentiments.get(sym).unwrap().clone();
            sim_map.insert(sym.to_string(), Simulator::new(sym.to_string(), px, sentiment));
        }

        let mut orders_processed = 0usize;
        let mut trades_executed = 0usize;
        let mut all_latencies = Vec::new();
        let mut all_round_trip_latencies = Vec::new();

        loop {
            if stock_exchange_rust::api::ENGINE_RESET_FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                market = Market::new(market_name.clone());
                sim_map.clear();
                let mut map = shared_ltps_market.lock().unwrap();
                for &(sym, px) in &stock_prices_copy {
                    let initial_price = Price::from_paisa(px);
                    market.add_stock(sym.to_string(), initial_price);
                    let sentiment = shared_sentiments.get(sym).unwrap().clone();
                    sim_map.insert(sym.to_string(), Simulator::new(sym.to_string(), px, sentiment));
                    map.insert(sym.to_string(), px);
                }
            }

            let mut step_latencies = Vec::new();

            // Process HFT Direct Orders First
            while let Ok(hft_order) = hft_order_rx.try_recv() {
                let t0 = Instant::now();
                let is_mkt = hft_order.price().is_none();
                let res = if is_mkt {
                    market.place_market_order(hft_order)
                } else {
                    market.place_limit_order(hft_order)
                };
                let elapsed = t0.elapsed();
                step_latencies.push(elapsed);
                all_latencies.push(elapsed);
                orders_processed += 1;

                let round_trip = elapsed + Duration::from_nanos(1200);
                all_round_trip_latencies.push(round_trip);

                if let Ok((trades, _)) = res {
                    trades_executed += trades.len();
                    if let Some(ref port_arc) = shared_hft_portfolio {
                        let mut port = port_arc.lock().unwrap();
                        for trade in &trades {
                            if trade.buyer_acc_no == port.acc_no {
                                port.apply_buy_trade(&trade.symbol, trade.quantity, trade.price);
                            }
                            if trade.seller_acc_no == port.acc_no {
                                port.apply_sell_trade(&trade.symbol, trade.quantity, trade.price);
                            }
                        }
                    }
                }
            }

            // Run simulator step for ALL 10 stocks on this exchange
            let mut current_buy_prob = 0.5;
            let mut current_target_volume = 1;

            for (sym, sim) in sim_map.iter_mut() {
                let sim_metrics = sim.step(&mut market);
                current_buy_prob = sim.shared_sentiment.lock().unwrap().buy_prob;
                current_target_volume += sim_metrics.trades.len();

                for trade in &sim_metrics.trades {
                    trades_executed += 1;
                    if let Some(ref port_arc) = shared_hft_portfolio {
                        let mut port = port_arc.lock().unwrap();
                        if trade.buyer_acc_no == port.acc_no {
                            port.apply_buy_trade(&trade.symbol, trade.quantity, trade.price);
                        }
                        if trade.seller_acc_no == port.acc_no {
                            port.apply_sell_trade(&trade.symbol, trade.quantity, trade.price);
                        }
                    }

                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();

                    let trade_evt = stock_exchange_rust::events::TradeEvent {
                        trade_id: trade.id.to_string(),
                        symbol: trade.symbol.clone(),
                        price_paisa: trade.price.paisa,
                        quantity: trade.quantity,
                        buyer_acc_no: trade.buyer_acc_no.clone(),
                        seller_acc_no: trade.seller_acc_no.clone(),
                        timestamp_millis: now_ms,
                    };
                    
                    if let Some(ref tx) = event_tx {
                        let _ = tx.send(stock_exchange_rust::events::MarketEvent::Trade(trade_evt.clone()));
                    }
                    if let Some(ref tx_db) = event_tx_db {
                        let _ = tx_db.send(stock_exchange_rust::events::MarketEvent::Trade(trade_evt));
                    }
                }

                for lat in &sim_metrics.order_latencies {
                    all_latencies.push(*lat);
                    step_latencies.push(*lat);
                    orders_processed += 1;
                    all_round_trip_latencies.push(*lat + Duration::from_nanos(1200));
                }

                if let Some(book) = market.get_orderbook(sym) {
                    shared_ltps_market.lock().unwrap().insert(sym.clone(), book.ltp.paisa);
                    let _ = market.subscribe_ticker(sym, hft_tick_tx.clone());
                }
            }

            let default_book = market.get_orderbook("TCS").cloned();
            let book_ltp = default_book.as_ref().map(|b| b.ltp).unwrap_or(Price::from_paisa(345000));
            let sl_hits = default_book.as_ref().map(|b| b.sl_triggers.values().map(|v| v.len()).sum()).unwrap_or(0);
            let tp_hits = default_book.as_ref().map(|b| b.tp_triggers.values().map(|v| v.len()).sum()).unwrap_or(0);

            let step_lat = calculate_median_duration(step_latencies);
            let overall_lat = calculate_median_duration(all_latencies.clone());
            let round_trip_lat = calculate_median_duration(all_round_trip_latencies.clone());

            let resting_bids: Vec<(Price, u64, usize)> = default_book.as_ref().map(|b| {
                b.bids
                    .iter()
                    .rev()
                    .take(5)
                    .map(|(p, q)| (*p, q.iter().map(|o| o.remaining_size()).sum(), q.len()))
                    .collect()
            }).unwrap_or_default();

            let resting_asks: Vec<(Price, u64, usize)> = default_book.as_ref().map(|b| {
                b.asks
                    .iter()
                    .take(5)
                    .map(|(p, q)| (*p, q.iter().map(|o| o.remaining_size()).sum(), q.len()))
                    .collect()
            }).unwrap_or_default();

            let update = MarketStateUpdate {
                exchange_name: market_name.clone(),
                ltp: book_ltp,
                orders_processed,
                trades_executed,
                sl_hits,
                tp_hits,
                _step_median_latency: step_lat,
                overall_median_latency: overall_lat,
                round_trip_median_latency: round_trip_lat,
                resting_bids,
                resting_asks,
                target_volume: current_target_volume,
                buy_prob: current_buy_prob,
            };

            if ui_tx.send(update).is_err() {
                return;
            }

            thread::sleep(Duration::from_millis(10));
        }
    });

    shared_ltps
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "tui" || args[1] == "--tui") {
        return run_tui();
    }

    println!("--------------------------------------------------");
    println!(" 🚀 STOCK EXCHANGE BACKEND MATCHING SERVER");
    println!("--------------------------------------------------");

    let (ui_tx, ui_rx) = mpsc::channel::<MarketStateUpdate>();

    let (hft_tick_tx, hft_tick_rx) = mpsc::channel::<stock_exchange_rust::hft::MarketSubscriptionTick>();
    let (hft_telemetry_tx, hft_telemetry_rx) = mpsc::channel::<stock_exchange_rust::hft::HftTelemetryUpdate>();

    // Non-blocking Event Pipeline (unbounded channels -> async background workers)
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<stock_exchange_rust::events::MarketEvent>();
    let (event_tx_db, event_rx_db) = tokio::sync::mpsc::unbounded_channel::<stock_exchange_rust::events::MarketEvent>();

    // Initialize SQLite Database Pool and Async Writer
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://stock_exchange.db?mode=rwc".to_string());
    let mut restored_balance = None;
    let mut db_pool_opt = None;
    if let Ok(db_pool) = stock_exchange_rust::db::DbPool::init(&db_url).await {
        if let Ok(balance) = db_pool.load_portfolio_balance("999").await {
            restored_balance = balance;
        }
        let db_writer = stock_exchange_rust::db::SqliteDbWriter::new(db_pool.get_pool());
        db_writer.start_worker(event_rx_db);
        db_pool_opt = Some(db_pool);
    }

    // Start Axum REST + WebSocket API server on port 3001 for Web Frontend Dashboard
    let raw_pool = db_pool_opt.as_ref().map(|p| p.get_pool());
    let ws_broadcast = stock_exchange_rust::api::ApiServer::start(3001, raw_pool).await;

    // Attempt Redis Streams publisher connection asynchronously
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    if let Ok(redis_publisher) = stock_exchange_rust::events::RedisStreamPublisher::new(&redis_url).await {
        redis_publisher.start_worker(event_rx);
    } else {
        eprintln!("[Warning] Could not connect to Redis at {}. Event streaming skipped.", redis_url);
    }

    let (ayush_hft_tx, ayush_hft_rx) = mpsc::sync_channel::<Order>(10_000);
    let (bohra_hft_tx, bohra_hft_rx) = mpsc::sync_channel::<Order>(10_000);

    let mut router = stock_exchange_rust::hft::OrderRouter::new();
    router.register_exchange("AYUSHSE".to_string(), ayush_hft_tx);
    router.register_exchange("BOHRASE".to_string(), bohra_hft_tx);

    let hft_user = if let Some(bal) = restored_balance {
        stock_exchange_rust::hft::HftUser::new_with_balance(
            "HFT_BOT_01".to_string(),
            "Antigravity HFT Arbitrage".to_string(),
            bal,
        )
    } else {
        stock_exchange_rust::hft::HftUser::new_with_billion_capital(
            "HFT_BOT_01".to_string(),
            "Antigravity HFT Arbitrage".to_string(),
        )
    };

    let shared_hft_portfolio = hft_user.portfolio.clone();

    let mut hft_engine = stock_exchange_rust::hft::CrossExchangeArbitrage::new("TCS".to_string(), hft_user, router);

    let ws_broadcast_hft = ws_broadcast.clone();
    thread::spawn(move || {
        let mut ayush_ticks: std::collections::HashMap<String, stock_exchange_rust::hft::MarketSubscriptionTick> = std::collections::HashMap::new();
        let mut bohra_ticks: std::collections::HashMap<String, stock_exchange_rust::hft::MarketSubscriptionTick> = std::collections::HashMap::new();

        let mut last_tps_check = Instant::now();
        let mut trades_at_last_check = 0u64;
        let mut current_tps = 0u64;

        while let Ok(tick) = hft_tick_rx.recv() {
            let sym = tick.symbol.clone();
            if tick.exchange_name == "AYUSHSE" {
                ayush_ticks.insert(sym.clone(), tick);
            } else if tick.exchange_name == "BOHRASE" {
                bohra_ticks.insert(sym.clone(), tick);
            }

            if stock_exchange_rust::api::HFT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
                if let (Some(t_a), Some(t_b)) = (ayush_ticks.get(&sym), bohra_ticks.get(&sym)) {
                    if let Some(telemetry) = hft_engine.on_market_tick(t_a, t_b) {
                        let elapsed_sec = last_tps_check.elapsed().as_secs_f64();
                        if elapsed_sec >= 1.0 {
                            let diff = telemetry.total_trades.saturating_sub(trades_at_last_check);
                            current_tps = (diff as f64 / elapsed_sec).round() as u64;
                            trades_at_last_check = telemetry.total_trades;
                            last_tps_check = Instant::now();
                        }

                        let _ = hft_telemetry_tx.send(telemetry.clone());

                        let payload = serde_json::json!({
                            "type": "HFT_TELEMETRY",
                            "capital": telemetry.total_balance_rupees,
                            "realized_pnl": telemetry.realized_pnl_rupees,
                            "trades": telemetry.total_trades,
                            "wins": telemetry.winning_trades,
                            "tps": current_tps,
                            "internal_lat_ns": telemetry.hft_internal_latency.as_nanos(),
                            "internal_med_ns": telemetry.hft_median_latency.as_nanos(),
                            "rt_lat_ns": telemetry.hft_round_trip_latency.as_nanos(),
                            "rt_med_ns": telemetry.hft_median_round_trip_latency.as_nanos(),
                            "spread_paisa": telemetry.current_spread_paisa,
                            "inventory": telemetry.unified_inventory,
                            "ayushse_ltp": telemetry.ayushse_ltp.paisa as f64 / 100.0,
                            "bohrase_ltp": telemetry.bohrase_ltp.paisa as f64 / 100.0,
                        });
                        let _ = ws_broadcast_hft.send(payload.to_string());
                    }
                }
            } else {
                // If reset happened, reset hft_engine
                if stock_exchange_rust::api::HFT_RESET_FLAG.swap(false, std::sync::atomic::Ordering::Relaxed) {
                    hft_engine.reset();
                    trades_at_last_check = 0;
                    current_tps = 0;
                }
            }
        }
    });

    let stock_list = vec![
        ("TCS", 345000u64),
        ("RELIANCE", 289050u64),
        ("INFY", 152040u64),
        ("HDFCBANK", 164075u64),
        ("ICICIBANK", 112030u64),
    ];

    let mut ayush_stocks = stock_list.clone();
    let mut bohra_stocks = stock_list.clone();

    if let Some(ref pool) = db_pool_opt {
        if let Ok(prices) = pool.load_stock_prices().await {
            for (sym, ex, px) in prices {
                if px >= 50000 { // Only load prices >= ₹500.00
                    if ex == "AYUSHSE" {
                        if let Some(item) = ayush_stocks.iter_mut().find(|(s, _)| *s == sym) {
                            item.1 = px;
                        }
                    } else if ex == "BOHRASE" {
                        if let Some(item) = bohra_stocks.iter_mut().find(|(s, _)| *s == sym) {
                            item.1 = px;
                        }
                    }
                }
            }
        }
    }

    let mut shared_sentiments = std::collections::HashMap::new();
    for (sym, _) in &stock_list {
        shared_sentiments.insert(sym.to_string(), SharedSentiment::new());
    }

    let ayush_ltp_ref = spawn_exchange_pair(
        "AYUSHSE".to_string(),
        ayush_stocks,
        ui_tx.clone(),
        shared_sentiments.clone(),
        Some(ayush_hft_rx),
        hft_tick_tx.clone(),
        Some(shared_hft_portfolio.clone()),
        Some(event_tx.clone()),
        Some(event_tx_db.clone()),
    );

    let bohra_ltp_ref = spawn_exchange_pair(
        "BOHRASE".to_string(),
        bohra_stocks,
        ui_tx.clone(),
        shared_sentiments,
        Some(bohra_hft_rx),
        hft_tick_tx,
        Some(shared_hft_portfolio),
        Some(event_tx),
        Some(event_tx_db),
    );

    let mut latest_ayush_state: Option<MarketStateUpdate> = None;
    let mut latest_bohra_state: Option<MarketStateUpdate> = None;
    let mut last_broadcast_time = Instant::now();
    let mut last_db_portfolio_save = Instant::now();

    println!("✅ [Server Ready] Processing matching engine ticks & broadcasting live WebSocket feed...");

    loop {
        tokio::time::sleep(Duration::from_millis(10)).await;

        while let Ok(update) = ui_rx.try_recv() {
            if update.exchange_name == "AYUSHSE" {
                latest_ayush_state = Some(update);
            } else if update.exchange_name == "BOHRASE" {
                latest_bohra_state = Some(update);
            }
        }

        while let Ok(hft_update) = hft_telemetry_rx.try_recv() {
            if last_db_portfolio_save.elapsed() >= Duration::from_secs(2) {
                let ayush_map = ayush_ltp_ref.lock().unwrap().clone();
                let bohra_map = bohra_ltp_ref.lock().unwrap().clone();
                let now_sec = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let min_sec = (now_sec / 60) * 60;

                if let Some(ref db_pool) = db_pool_opt {
                    let pool = db_pool.get_pool();
                    let balance_paisa = (hft_update.total_balance_rupees * 100.0) as u64;
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO portfolios (acc_no, user_id, balance_paisa)
                            VALUES (?, ?, ?)
                            ON CONFLICT(acc_no) DO UPDATE SET balance_paisa = excluded.balance_paisa, updated_at = CURRENT_TIMESTAMP
                            "#,
                        )
                        .bind("999")
                        .bind("HFT_BOT_01")
                        .bind(balance_paisa as i64)
                        .execute(&pool)
                        .await;

                        for (sym, px) in &ayush_map {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO stock_prices (symbol, exchange, price_paisa)
                                VALUES (?, 'AYUSHSE', ?)
                                ON CONFLICT(symbol, exchange) DO UPDATE SET price_paisa = excluded.price_paisa, updated_at = CURRENT_TIMESTAMP
                                "#,
                            )
                            .bind(sym)
                            .bind(*px as i64)
                            .execute(&pool)
                            .await;

                            let _ = sqlx::query(
                                r#"
                                INSERT INTO candles (symbol, exchange, time_sec, open_paisa, high_paisa, low_paisa, close_paisa, volume)
                                VALUES (?, 'AYUSHSE', ?, ?, ?, ?, ?, 50)
                                ON CONFLICT(symbol, exchange, time_sec) DO UPDATE SET
                                    high_paisa = MAX(high_paisa, excluded.high_paisa),
                                    low_paisa = MIN(low_paisa, excluded.low_paisa),
                                    close_paisa = excluded.close_paisa,
                                    volume = volume + 50
                                "#,
                            )
                            .bind(sym)
                            .bind(min_sec)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .execute(&pool)
                            .await;
                        }

                        for (sym, px) in &bohra_map {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO stock_prices (symbol, exchange, price_paisa)
                                VALUES (?, 'BOHRASE', ?)
                                ON CONFLICT(symbol, exchange) DO UPDATE SET price_paisa = excluded.price_paisa, updated_at = CURRENT_TIMESTAMP
                                "#,
                            )
                            .bind(sym)
                            .bind(*px as i64)
                            .execute(&pool)
                            .await;

                            let _ = sqlx::query(
                                r#"
                                INSERT INTO candles (symbol, exchange, time_sec, open_paisa, high_paisa, low_paisa, close_paisa, volume)
                                VALUES (?, 'BOHRASE', ?, ?, ?, ?, ?, 50)
                                ON CONFLICT(symbol, exchange, time_sec) DO UPDATE SET
                                    high_paisa = MAX(high_paisa, excluded.high_paisa),
                                    low_paisa = MIN(low_paisa, excluded.low_paisa),
                                    close_paisa = excluded.close_paisa,
                                    volume = volume + 50
                                "#,
                            )
                            .bind(sym)
                            .bind(min_sec)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .bind(*px as i64)
                            .execute(&pool)
                            .await;
                        }
                    });
                }
                last_db_portfolio_save = Instant::now();
            }
        }

        if stock_exchange_rust::api::ENGINE_RESET_FLAG.swap(false, std::sync::atomic::Ordering::Relaxed) {
            // Give exchange threads a tick to process reset
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Broadcast tick updates for ALL stocks to web clients every 100ms
        if last_broadcast_time.elapsed() >= Duration::from_millis(100) {
            let ayush_map = ayush_ltp_ref.lock().unwrap().clone();
            let bohra_map = bohra_ltp_ref.lock().unwrap().clone();

            let ayush_lat = latest_ayush_state.as_ref().map(|s| s.overall_median_latency.as_nanos() as u64).unwrap_or(450);
            let ayush_rt_lat = latest_ayush_state.as_ref().map(|s| s.round_trip_median_latency.as_nanos() as u64).unwrap_or(1650);

            let ayush_bids = latest_ayush_state.as_ref().map(|s| s.resting_bids.iter().map(|(p, q, o)| serde_json::json!({ "price": p.paisa as f64 / 100.0, "qty": q, "orders": o })).collect::<Vec<_>>()).unwrap_or_default();
            let ayush_asks = latest_ayush_state.as_ref().map(|s| s.resting_asks.iter().map(|(p, q, o)| serde_json::json!({ "price": p.paisa as f64 / 100.0, "qty": q, "orders": o })).collect::<Vec<_>>()).unwrap_or_default();
            let bohra_bids = latest_bohra_state.as_ref().map(|s| s.resting_bids.iter().map(|(p, q, o)| serde_json::json!({ "price": p.paisa as f64 / 100.0, "qty": q, "orders": o })).collect::<Vec<_>>()).unwrap_or_default();
            let bohra_asks = latest_bohra_state.as_ref().map(|s| s.resting_asks.iter().map(|(p, q, o)| serde_json::json!({ "price": p.paisa as f64 / 100.0, "qty": q, "orders": o })).collect::<Vec<_>>()).unwrap_or_default();

            let mut ayush_sum = 0.0;
            let mut bohra_sum = 0.0;
            let mut constituent_count = 0;

            for (sym, a_paisa) in &ayush_map {
                let b_paisa = bohra_map.get(sym).cloned().unwrap_or(*a_paisa);
                let a_val = *a_paisa as f64 / 100.0;
                let b_val = b_paisa as f64 / 100.0;
                ayush_sum += a_val;
                bohra_sum += b_val;
                constituent_count += 1;

                let tick_payload = serde_json::json!({
                    "type": "TICK",
                    "symbol": sym,
                    "ayushse_ltp": a_val,
                    "bohrase_ltp": b_val,
                    "med_lat_ns": ayush_lat,
                    "rt_med_lat_ns": ayush_rt_lat,
                    "ayushse_bids": ayush_bids,
                    "ayushse_asks": ayush_asks,
                    "bohrase_bids": bohra_bids,
                    "bohrase_asks": bohra_asks,
                    "timestamp": chrono::Utc::now().timestamp_millis()
                });
                let _ = ws_broadcast.send(tick_payload.to_string());
            }

            // Broadcast AYUSH-5 Index calculation tick
            if constituent_count > 0 {
                let ayush_5_ayushse = ((ayush_sum / constituent_count as f64) * 100.0).round() / 100.0;
                let ayush_5_bohrase = ((bohra_sum / constituent_count as f64) * 100.0).round() / 100.0;
                let index_payload = serde_json::json!({
                    "type": "TICK",
                    "symbol": "AYUSH-5",
                    "ayushse_ltp": ayush_5_ayushse,
                    "bohrase_ltp": ayush_5_bohrase,
                    "med_lat_ns": ayush_lat,
                    "rt_med_lat_ns": ayush_rt_lat,
                    "ayushse_bids": [],
                    "ayushse_asks": [],
                    "bohrase_bids": [],
                    "bohrase_asks": [],
                    "timestamp": chrono::Utc::now().timestamp_millis()
                });
                let _ = ws_broadcast.send(index_payload.to_string());
            }

            // Broadcast server settings state update continuously over WebSocket
            let is_sim = stock_exchange_rust::sim::simulator::SIMULATOR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
            let is_hft = stock_exchange_rust::api::HFT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
            let state_payload = serde_json::json!({
                "type": "STATE_UPDATE",
                "is_sim_active": is_sim,
                "is_hft_active": is_hft,
            });
            let _ = ws_broadcast.send(state_payload.to_string());

            last_broadcast_time = Instant::now();
        }
    }
}

fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{
        event::{poll, read, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        symbols,
        text::{Line, Span},
        widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
        Terminal,
    };
    use std::io::stdout;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (ui_tx, ui_rx) = mpsc::channel::<MarketStateUpdate>();
    let (hft_tick_tx, hft_tick_rx) = mpsc::channel::<stock_exchange_rust::hft::MarketSubscriptionTick>();
    let (hft_telemetry_tx, hft_telemetry_rx) = mpsc::channel::<stock_exchange_rust::hft::HftTelemetryUpdate>();

    let (ayush_hft_tx, ayush_hft_rx) = mpsc::sync_channel::<Order>(10_000);
    let (bohra_hft_tx, bohra_hft_rx) = mpsc::sync_channel::<Order>(10_000);

    let mut router = stock_exchange_rust::hft::OrderRouter::new();
    router.register_exchange("AYUSHSE".to_string(), ayush_hft_tx);
    router.register_exchange("BOHRASE".to_string(), bohra_hft_tx);

    let hft_user = stock_exchange_rust::hft::HftUser::new_with_billion_capital(
        "HFT_BOT_01".to_string(),
        "Antigravity HFT Arbitrage".to_string(),
    );
    let shared_hft_portfolio = hft_user.portfolio.clone();
    let mut hft_engine = stock_exchange_rust::hft::CrossExchangeArbitrage::new("TCS".to_string(), hft_user, router);

    thread::spawn(move || {
        let mut latest_ayush_tick: Option<stock_exchange_rust::hft::MarketSubscriptionTick> = None;
        let mut latest_bohra_tick: Option<stock_exchange_rust::hft::MarketSubscriptionTick> = None;

        while let Ok(tick) = hft_tick_rx.recv() {
            if tick.exchange_name == "AYUSHSE" {
                latest_ayush_tick = Some(tick);
            } else if tick.exchange_name == "BOHRASE" {
                latest_bohra_tick = Some(tick);
            }

            if let (Some(ref t_a), Some(ref t_b)) = (&latest_ayush_tick, &latest_bohra_tick) {
                if let Some(telemetry) = hft_engine.on_market_tick(t_a, t_b) {
                    let _ = hft_telemetry_tx.send(telemetry);
                }
            }
        }
    });

    let stock_list = vec![
        ("TCS", 345000u64),
        ("RELIANCE", 289050u64),
        ("INFY", 152040u64),
        ("HDFCBANK", 164075u64),
        ("ICICIBANK", 112030u64),
        ("TATAMOTORS", 98060u64),
        ("BHARTIARTL", 141025u64),
        ("SBIN", 82540u64),
        ("ITC", 46580u64),
        ("LTIM", 512000u64),
    ];

    let mut shared_sentiments = std::collections::HashMap::new();
    for (sym, _) in &stock_list {
        shared_sentiments.insert(sym.to_string(), SharedSentiment::new());
    }

    let _ayush_ltp_ref = spawn_exchange_pair(
        "AYUSHSE".to_string(),
        stock_list.clone(),
        ui_tx.clone(),
        shared_sentiments.clone(),
        Some(ayush_hft_rx),
        hft_tick_tx.clone(),
        Some(shared_hft_portfolio.clone()),
        None,
        None,
    );

    let _bohra_ltp_ref = spawn_exchange_pair(
        "BOHRASE".to_string(),
        stock_list,
        ui_tx.clone(),
        shared_sentiments,
        Some(bohra_hft_rx),
        hft_tick_tx,
        Some(shared_hft_portfolio),
        None,
        None,
    );

    let mut ayush_history: Vec<(f64, f64)> = Vec::new();
    let mut bohra_history: Vec<(f64, f64)> = Vec::new();
    let mut hft_balance_history: Vec<(f64, f64)> = Vec::new();

    let mut latest_ayush_state: Option<MarketStateUpdate> = None;
    let mut latest_bohra_state: Option<MarketStateUpdate> = None;
    let mut latest_hft_telemetry: Option<stock_exchange_rust::hft::HftTelemetryUpdate> = None;
    let mut last_chart_update = Instant::now();

    loop {
        if poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = read()? {
                if key_event.code == KeyCode::Char('q') || key_event.code == KeyCode::Char('Q') {
                    break;
                }
            }
        }

        while let Ok(update) = ui_rx.try_recv() {
            if update.exchange_name == "AYUSHSE" {
                latest_ayush_state = Some(update);
            } else if update.exchange_name == "BOHRASE" {
                latest_bohra_state = Some(update);
            }
        }

        while let Ok(hft_update) = hft_telemetry_rx.try_recv() {
            latest_hft_telemetry = Some(hft_update);
        }

        if last_chart_update.elapsed() >= Duration::from_millis(200) {
            if let Some(ref st) = latest_ayush_state {
                let x = ayush_history.len() as f64;
                let y = st.ltp.paisa as f64 / 100.0;
                ayush_history.push((x, y));
            }
            if let Some(ref st) = latest_bohra_state {
                let x = bohra_history.len() as f64;
                let y = st.ltp.paisa as f64 / 100.0;
                bohra_history.push((x, y));
            }
            if let Some(ref hft) = latest_hft_telemetry {
                let x = hft_balance_history.len() as f64;
                let y = hft.total_balance_rupees;
                hft_balance_history.push((x, y));
            }
            last_chart_update = Instant::now();
        }

        terminal.draw(|f| {
            let size = f.area();
            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(14)])
                .split(size);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(main_layout[0]);

            let render_exchange_col = |f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &Option<MarketStateUpdate>, history: &[(f64, f64)], title_color: Color| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Percentage(50), Constraint::Percentage(43)])
                    .split(area);

                if let Some(st) = state {
                    let current_ltp = st.ltp.paisa as f64 / 100.0;
                    let step_us = st._step_median_latency.as_nanos() as f64 / 1000.0;
                    let overall_us = st.overall_median_latency.as_nanos() as f64 / 1000.0;
                    let buy_pct = st.buy_prob * 100.0;
                    let sell_pct = (1.0 - st.buy_prob) * 100.0;

                    let header_lines = vec![
                        Line::from(vec![
                            Span::styled("Stock: ", Style::default().fg(Color::Gray)),
                            Span::styled("TCS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                            Span::raw(" | "),
                            Span::styled("LTP: ", Style::default().fg(Color::Gray)),
                            Span::styled(format!("₹{}", format_indian_currency(current_ltp)), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::raw(" | "),
                            Span::styled(format!("Vol: {}/s", st.target_volume), Style::default().fg(Color::Yellow)),
                        ]),
                        Line::from(vec![
                            Span::styled("Sentiment Ratio: ", Style::default().fg(Color::Gray)),
                            Span::styled(format!("{:.0}% BUY", buy_pct), Style::default().fg(Color::Green)),
                            Span::raw(" / "),
                            Span::styled(format!("{:.0}% SELL", sell_pct), Style::default().fg(Color::Red)),
                        ]),
                        Line::from(vec![
                            Span::styled("Orders: ", Style::default().fg(Color::Gray)),
                            Span::raw(format!("{} | ", format_indian_currency(st.orders_processed as f64).split('.').next().unwrap())),
                            Span::styled("Trades: ", Style::default().fg(Color::Gray)),
                            Span::raw(format!("{} | ", format_indian_currency(st.trades_executed as f64).split('.').next().unwrap())),
                            Span::styled("SL: ", Style::default().fg(Color::Red)),
                            Span::raw(format!("{} | ", st.sl_hits)),
                            Span::styled("TP: ", Style::default().fg(Color::Green)),
                            Span::raw(format!("{}", st.tp_hits)),
                        ]),
                        Line::from(vec![
                            Span::styled("⚡ Step Med: ", Style::default().fg(Color::Yellow)),
                            Span::raw(format!("{:>7.3}µs | ", step_us)),
                            Span::styled("⚡ Overall Med: ", Style::default().fg(Color::Green)),
                            Span::raw(format!("{:>7.3}µs", overall_us)),
                        ]),
                    ];

                    let header = Paragraph::new(header_lines).block(
                        Block::default().title(format!(" 🏛️ EXCHANGE: {} ", st.exchange_name)).borders(Borders::ALL).border_style(Style::default().fg(title_color)),
                    );
                    f.render_widget(header, chunks[0]);

                    let x_max = (history.len() as f64).max(60.0);
                    let x_min = (x_max - 60.0).max(0.0);
                    let window_points: Vec<&(f64, f64)> = history.iter().filter(|(x, _)| *x >= x_min).collect();
                    let (y_min, y_max) = if window_points.is_empty() {
                        (current_ltp - 5.0, current_ltp + 5.0)
                    } else {
                        let mut min_val = f64::MAX;
                        let mut max_val = f64::MIN;
                        for (_, y) in &window_points {
                            if *y < min_val { min_val = *y; }
                            if *y > max_val { max_val = *y; }
                        }
                        if (max_val - min_val).abs() < 0.5 { (min_val - 1.0, max_val + 1.0) } else { (min_val - 0.5, max_val + 0.5) }
                    };

                    let dataset = vec![
                        Dataset::default().marker(symbols::Marker::Braille).graph_type(GraphType::Line).style(Style::default().fg(title_color)).data(history),
                    ];

                    let chart = Chart::new(dataset).block(Block::default().title(" 📈 REAL-TIME LTP CHART ").borders(Borders::ALL).border_style(Style::default().fg(title_color)))
                        .x_axis(Axis::default().bounds([x_min, x_max]))
                        .y_axis(Axis::default().bounds([y_min, y_max]).labels(vec![
                            Span::raw(format!("₹{}", format_indian_currency(y_min))),
                            Span::raw(format!("₹{}", format_indian_currency((y_min + y_max) / 2.0))),
                            Span::raw(format!("₹{}", format_indian_currency(y_max))),
                        ]));
                    f.render_widget(chart, chunks[1]);

                    let mut depth_lines = vec![Line::from(Span::styled("--- ASKS (SELLERS) ---", Style::default().fg(Color::Red)))];
                    for (price, qty, orders) in st.resting_asks.iter().rev() {
                        depth_lines.push(Line::from(format!("₹{} | {} shares ({} orders)", format_indian_currency(price.paisa as f64 / 100.0), format_indian_currency(*qty as f64).split('.').next().unwrap(), orders)));
                    }
                    depth_lines.push(Line::from(Span::styled("----------------------------------------", Style::default().fg(Color::DarkGray))));
                    depth_lines.push(Line::from(Span::styled("--- BIDS (BUYERS) ---", Style::default().fg(Color::Green))));
                    for (price, qty, orders) in &st.resting_bids {
                        depth_lines.push(Line::from(format!("₹{} | {} shares ({} orders)", format_indian_currency(price.paisa as f64 / 100.0), format_indian_currency(*qty as f64).split('.').next().unwrap(), orders)));
                    }

                    let depth_widget = Paragraph::new(depth_lines).block(Block::default().title(" 📖 L2 ORDER BOOK DEPTH ").borders(Borders::ALL).border_style(Style::default().fg(title_color)));
                    f.render_widget(depth_widget, chunks[2]);
                }
            };

            render_exchange_col(f, main_chunks[0], &latest_ayush_state, &ayush_history, Color::Cyan);
            render_exchange_col(f, main_chunks[1], &latest_bohra_state, &bohra_history, Color::Yellow);

            let hft_box_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
                .split(main_layout[1]);

            if let Some(ref hft) = latest_hft_telemetry {
                let win_rate = if hft.total_trades > 0 { (hft.winning_trades as f64 / hft.total_trades as f64) * 100.0 } else { 0.0 };
                let pnl_color = if hft.realized_pnl_rupees >= 0.0 { Color::Green } else { Color::Red };

                let hft_lines = vec![
                    Line::from(vec![Span::styled("Bot: ", Style::default().fg(Color::Gray)), Span::styled("Antigravity Arbitrage", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))]),
                    Line::from(vec![Span::styled("Capital: ", Style::default().fg(Color::Gray)), Span::styled(format!("₹{}", format_indian_currency(hft.total_balance_rupees)), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))]),
                    Line::from(vec![Span::styled("Realized PnL: ", Style::default().fg(Color::Gray)), Span::styled(format!("{}₹{}", if hft.realized_pnl_rupees >= 0.0 { "+" } else { "" }, format_indian_currency(hft.realized_pnl_rupees)), Style::default().fg(pnl_color).add_modifier(Modifier::BOLD))]),
                    Line::from(vec![Span::styled("Trades: ", Style::default().fg(Color::Gray)), Span::raw(format!("{} | ", hft.total_trades)), Span::styled("Wins: ", Style::default().fg(Color::Green)), Span::raw(format!("{} ({:.1}%)", hft.winning_trades, win_rate))]),
                    Line::from(vec![Span::styled("⚡ HFT Int Med: ", Style::default().fg(Color::Yellow)), Span::raw(format!("{:>7.3}µs", hft.hft_median_latency.as_nanos() as f64 / 1000.0))]),
                    Line::from(vec![Span::styled("⚡ HFT RT Med:  ", Style::default().fg(Color::LightMagenta)), Span::raw(format!("{:>7.3}µs", hft.hft_median_round_trip_latency.as_nanos() as f64 / 1000.0))]),
                ];
                let hft_widget = Paragraph::new(hft_lines).block(Block::default().title(" 🤖 HFT BOT PERFORMANCE ").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)));
                f.render_widget(hft_widget, hft_box_layout[0]);

                let ayush_tcs_ltp = latest_ayush_state.as_ref().map(|s| s.ltp.paisa as f64 / 100.0).unwrap_or(hft.ayushse_ltp.paisa as f64 / 100.0);
                let bohra_tcs_ltp = latest_bohra_state.as_ref().map(|s| s.ltp.paisa as f64 / 100.0).unwrap_or(hft.bohrase_ltp.paisa as f64 / 100.0);
                let current_spread = (ayush_tcs_ltp - bohra_tcs_ltp).abs();

                let market_lines = vec![
                    Line::from(vec![Span::styled("AYUSHSE LTP: ", Style::default().fg(Color::Cyan)), Span::raw(format!("₹{}", format_indian_currency(ayush_tcs_ltp)))]),
                    Line::from(vec![Span::styled("BOHRASE LTP: ", Style::default().fg(Color::Yellow)), Span::raw(format!("₹{}", format_indian_currency(bohra_tcs_ltp)))]),
                    Line::from(vec![Span::styled("Current Spread: ", Style::default().fg(Color::White)), Span::styled(format!("₹{:.2}", current_spread), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))]),
                    Line::from(vec![Span::styled("Inventory: ", Style::default().fg(Color::Gray)), Span::styled(format!("{} shares", hft.unified_inventory), Style::default().fg(Color::Yellow))]),
                ];
                let market_widget = Paragraph::new(market_lines).block(Block::default().title(" 📊 LIVE ARBITRAGE SPREAD ").borders(Borders::ALL).border_style(Style::default().fg(Color::LightBlue)));
                f.render_widget(market_widget, hft_box_layout[1]);

                let x_max = (hft_balance_history.len() as f64).max(60.0);
                let x_min = (x_max - 60.0).max(0.0);
                let window_points: Vec<&(f64, f64)> = hft_balance_history.iter().filter(|(x, _)| *x >= x_min).collect();
                let (y_min, y_max) = if window_points.is_empty() {
                    (hft.total_balance_rupees - 100.0, hft.total_balance_rupees + 100.0)
                } else {
                    let mut min_val = f64::MAX;
                    let mut max_val = f64::MIN;
                    for (_, y) in &window_points {
                        if *y < min_val { min_val = *y; }
                        if *y > max_val { max_val = *y; }
                    }
                    if (max_val - min_val).abs() < 10.0 { (min_val - 50.0, max_val + 50.0) } else { (min_val - 10.0, max_val + 10.0) }
                };

                let hft_dataset = vec![Dataset::default().marker(symbols::Marker::Braille).graph_type(GraphType::Line).style(Style::default().fg(Color::Magenta)).data(&hft_balance_history)];
                let hft_chart = Chart::new(hft_dataset).block(Block::default().title(" 💰 CAPITAL GROWTH ").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)))
                    .x_axis(Axis::default().bounds([x_min, x_max]))
                    .y_axis(Axis::default().bounds([y_min, y_max]).labels(vec![
                        Span::raw(format!("₹{}", format_compact(y_min))),
                        Span::raw(format!("₹{}", format_compact((y_min + y_max) / 2.0))),
                        Span::raw(format!("₹{}", format_compact(y_max))),
                    ]));
                f.render_widget(hft_chart, hft_box_layout[2]);
            }
        })?;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn format_compact(amount: f64) -> String {
    let abs_amount = amount.abs();
    if abs_amount >= 1_00_00_000.0 {
        format!("{:.2}Cr", amount / 1_00_00_000.0)
    } else if abs_amount >= 1_00_000.0 {
        format!("{:.2}L", amount / 1_00_000.0)
    } else {
        format!("{:.0}", amount)
    }
}

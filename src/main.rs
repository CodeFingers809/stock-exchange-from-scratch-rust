use std::io::stdout;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

// Telemetry output sent from Market Engine Core -> UI Renderer Core
struct MarketStateUpdate {
    exchange_name: String,
    ltp: Price,
    orders_processed: usize,
    trades_executed: usize,
    sl_hits: usize,
    tp_hits: usize,
    step_median_latency: Duration,
    overall_median_latency: Duration,
    resting_bids: Vec<(Price, u64, usize)>,
    resting_asks: Vec<(Price, u64, usize)>,
    target_volume: usize,
    buy_prob: f64,
}

fn spawn_exchange_pair(
    exchange_name: String,
    initial_price: Price,
    ui_tx: mpsc::Sender<MarketStateUpdate>,
    shared_simulator: Arc<Mutex<Simulator>>,
    hft_order_rx_param: Option<mpsc::Receiver<Order>>,
    hft_tick_tx: mpsc::Sender<stock_exchange_rust::hft::MarketSubscriptionTick>,
    shared_hft_portfolio: Option<Arc<Mutex<stock_exchange_rust::domain::portfolio::Portfolio>>>,
) -> Arc<Mutex<Price>> {
    let shared_ltp = Arc::new(Mutex::new(initial_price));

    let hft_order_rx = hft_order_rx_param.unwrap_or_else(|| {
        let (_tx, rx) = mpsc::sync_channel::<Order>(10_000);
        rx
    });

    // Simulator runs inside the market thread (calls step() directly on the Market)
    // This eliminates the SimOrderMsg channel and the duplicated inline sim logic.

    // 2. DEDICATED MARKET ENGINE + SIMULATOR OS THREAD / CORE
    let market_name = exchange_name.clone();
    let shared_ltp_market = shared_ltp.clone();
    thread::spawn(move || {
        let mut market = Market::new(market_name.clone());
        market.add_stock("TCS".to_string(), initial_price);

        let mut orders_processed = 0usize;
        let mut trades_executed = 0usize;
        let mut all_latencies = Vec::new();

        loop {
            let mut step_latencies = Vec::new();

            // Process HFT Direct Orders First (L2 NBBO subscriber, priority access)
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

            // Run simulator step directly on the market (shared sentiment, independent volume)
            let sim_metrics = {
                let mut sim = shared_simulator.lock().unwrap();
                sim.step(&mut market)
            };

            // Read buy_prob from the shared sentiment for the UI telemetry display
            let current_buy_prob = {
                shared_simulator.lock().unwrap().shared_sentiment.lock().unwrap().buy_prob
            };
            let current_target_volume = sim_metrics.trades.len().max(1);

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
            }

            for lat in &sim_metrics.order_latencies {
                all_latencies.push(*lat);
                step_latencies.push(*lat);
                orders_processed += 1;
            }

            let book = market.get_orderbook("TCS").unwrap();
            *shared_ltp_market.lock().unwrap() = book.ltp;

            // Emit real-time tick via Market::subscribe_ticker subscription contract
            let _ = market.subscribe_ticker("TCS", hft_tick_tx.clone());

            let sl_hits = book.sl_triggers.values().map(|v| v.len()).sum();
            let tp_hits = book.tp_triggers.values().map(|v| v.len()).sum();

            let step_lat = calculate_median_duration(step_latencies);
            let overall_lat = calculate_median_duration(all_latencies.clone());

            let resting_bids: Vec<(Price, u64, usize)> = book
                .bids
                .iter()
                .rev()
                .take(5)
                .map(|(p, q)| (*p, q.iter().map(|o| o.remaining_size()).sum(), q.len()))
                .collect();

            let resting_asks: Vec<(Price, u64, usize)> = book
                .asks
                .iter()
                .take(5)
                .map(|(p, q)| (*p, q.iter().map(|o| o.remaining_size()).sum(), q.len()))
                .collect();

            let update = MarketStateUpdate {
                exchange_name: market_name.clone(),
                ltp: book.ltp,
                orders_processed,
                trades_executed,
                sl_hits,
                tp_hits,
                step_median_latency: step_lat,
                overall_median_latency: overall_lat,
                resting_bids,
                resting_asks,
                target_volume: current_target_volume,
                buy_prob: current_buy_prob,
            };

            if ui_tx.send(update).is_err() {
                return;
            }

            thread::sleep(Duration::from_millis(100));
        }
    });

    shared_ltp
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // Clone the Arc<Mutex<Portfolio>> pointer — both exchange threads and the
    // HFT engine all share the exact same Portfolio object in memory.
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

    // Independent simulators — same sentiment regime, but separate volume/price randomness
    // One sentiment source — both exchanges see identical buy_prob / regime timing.
    // Each simulator still generates its own volume and price offsets independently.
    let shared_sentiment = SharedSentiment::new();

    let ayush_simulator = Arc::new(Mutex::new(Simulator::new("TCS".to_string(), shared_sentiment.clone())));
    let bohra_simulator = Arc::new(Mutex::new(Simulator::new("TCS".to_string(), shared_sentiment)));

    let ayush_ltp_ref = spawn_exchange_pair(
        "AYUSHSE".to_string(),
        Price::from_rupees_paisa(2250, 0),
        ui_tx.clone(),
        ayush_simulator,
        Some(ayush_hft_rx),
        hft_tick_tx.clone(),
        Some(shared_hft_portfolio.clone()),
    );

    let bohra_ltp_ref = spawn_exchange_pair(
        "BOHRASE".to_string(),
        Price::from_rupees_paisa(2250, 0),
        ui_tx.clone(),
        bohra_simulator,
        Some(bohra_hft_rx),
        hft_tick_tx,
        Some(shared_hft_portfolio),
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

        // Update line chart histories every 500ms (step-based timing)
        if last_chart_update.elapsed() >= Duration::from_millis(500) {
            let ayush_ltp_val = ayush_ltp_ref.lock().unwrap().paisa as f64 / 100.0;
            let bohra_ltp_val = bohra_ltp_ref.lock().unwrap().paisa as f64 / 100.0;

            ayush_history.push((ayush_history.len() as f64, ayush_ltp_val));
            bohra_history.push((bohra_history.len() as f64, bohra_ltp_val));

            let current_bal = latest_hft_telemetry.as_ref().map_or(1_000_000_000.0, |t| t.total_balance_rupees);
            hft_balance_history.push((hft_balance_history.len() as f64, current_bal));

            last_chart_update = Instant::now();
        }

        let _ = terminal.draw(|f| {
            let outer_layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
                .split(f.area());

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(outer_layout[0]);

            let render_exchange_col = |f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &Option<MarketStateUpdate>, history: &[(f64, f64)], title_color: Color| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(7),       // Telemetry Header
                        Constraint::Percentage(50), // LTP Chart
                        Constraint::Percentage(43), // L2 Depth
                    ])
                    .split(area);

                if let Some(st) = state {
                    let current_ltp = st.ltp.paisa as f64 / 100.0;
                    let step_us = st.step_median_latency.as_nanos() as f64 / 1000.0;
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
                        Block::default()
                            .title(format!(" 🏛️ EXCHANGE: {} ", st.exchange_name))
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(title_color)),
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
                        Dataset::default()
                            .marker(symbols::Marker::Braille)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(title_color))
                            .data(history),
                    ];

                    let chart = Chart::new(dataset)
                        .block(
                            Block::default()
                                .title(" 📈 REAL-TIME LTP CHART ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(title_color)),
                        )
                        .x_axis(Axis::default().bounds([x_min, x_max]))
                        .y_axis(
                            Axis::default()
                                .bounds([y_min, y_max])
                                .labels(vec![
                                    Span::raw(format!("₹{}", format_indian_currency(y_min))),
                                    Span::raw(format!("₹{}", format_indian_currency((y_min + y_max) / 2.0))),
                                    Span::raw(format!("₹{}", format_indian_currency(y_max))),
                                ]),
                        );
                    f.render_widget(chart, chunks[1]);

                    let mut depth_lines = vec![
                        Line::from(Span::styled("--- ASKS (SELLERS) ---", Style::default().fg(Color::Red))),
                    ];

                    for (price, qty, orders) in st.resting_asks.iter().rev() {
                        depth_lines.push(Line::from(format!(
                            "₹{} | {} shares ({} orders)",
                            format_indian_currency(price.paisa as f64 / 100.0),
                            format_indian_currency(*qty as f64).split('.').next().unwrap(),
                            orders
                        )));
                    }

                    depth_lines.push(Line::from(Span::styled(
                        "----------------------------------------",
                        Style::default().fg(Color::DarkGray),
                    )));
                    depth_lines.push(Line::from(Span::styled("--- BIDS (BUYERS) ---", Style::default().fg(Color::Green))));

                    for (price, qty, orders) in &st.resting_bids {
                        depth_lines.push(Line::from(format!(
                            "₹{} | {} shares ({} orders)",
                            format_indian_currency(price.paisa as f64 / 100.0),
                            format_indian_currency(*qty as f64).split('.').next().unwrap(),
                            orders
                        )));
                    }

                    let depth_widget = Paragraph::new(depth_lines).block(
                        Block::default()
                            .title(" 📖 L2 ORDER BOOK DEPTH ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(title_color)),
                    );
                    f.render_widget(depth_widget, chunks[2]);
                }
            };

            render_exchange_col(f, main_chunks[0], &latest_ayush_state, &ayush_history, Color::Cyan);
            render_exchange_col(f, main_chunks[1], &latest_bohra_state, &bohra_history, Color::Yellow);

            let hft_box_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                ])
                .split(outer_layout[1]);

            if let Some(ref hft) = latest_hft_telemetry {
                let hft_lat_ns = hft.hft_internal_latency.as_nanos();
                let spread_rupees = hft.current_spread_paisa as f64 / 100.0;

                let hft_med_ns = hft.hft_median_latency.as_nanos();
                let col1_text = vec![
                    Line::from(vec![
                        Span::styled("Capital: ", Style::default().fg(Color::Gray)),
                        Span::styled(format!("₹{}", format_indian_currency(hft.total_balance_rupees)), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("Realized PnL: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            format!("₹{}", format_indian_currency(hft.realized_pnl_rupees)),
                            Style::default().fg(if hft.realized_pnl_rupees >= 0.0 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Trades: ", Style::default().fg(Color::Gray)),
                        Span::raw(format!("{} | ", format_indian_currency(hft.total_trades as f64).split('.').next().unwrap())),
                        Span::styled("Wins: ", Style::default().fg(Color::Green)),
                        Span::raw(format!("{}", format_indian_currency(hft.winning_trades as f64).split('.').next().unwrap())),
                    ]),
                    Line::from(vec![
                        Span::styled("⚡ HFT Engine Latency: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{}ns", hft_lat_ns), Style::default().fg(Color::Yellow)),
                        Span::styled(" | Med: ", Style::default().fg(Color::Gray)),
                        Span::styled(format!("{}ns", hft_med_ns), Style::default().fg(Color::Cyan)),
                    ]),
                ];
                let col1_widget = Paragraph::new(col1_text).block(
                    Block::default()
                        .title(" ⚡ HFT ACCOUNT & PERFORMANCE ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                );
                f.render_widget(col1_widget, hft_box_layout[0]);

                let (spread_label, spread_color) = if hft.current_spread_paisa >= 100 {
                    // Real actionable profit: best_bid(sell) - best_ask(buy) > ₹1
                    (format!("ARBITRAGE LIVE (+₹{})", format_indian_currency(spread_rupees)), Color::Green)
                } else if hft.current_spread_paisa > 0 {
                    // Spread exists but below our ₹1 threshold
                    (format!("BELOW THRESHOLD (+₹{})", format_indian_currency(spread_rupees)), Color::Yellow)
                } else {
                    // Negative: buying would cost more than selling receives
                    (format!("NO EDGE (₹{})", format_indian_currency(spread_rupees)), Color::Red)
                };

                let inv_cap_hit = hft.unified_inventory >= 1000;

                let col2_text = vec![
                    Line::from(vec![
                        Span::styled("AYUSHSE LTP: ", Style::default().fg(Color::Cyan)),
                        Span::styled(format!("₹{} ", hft.ayushse_ltp), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::styled("| ", Style::default().fg(Color::Gray)),
                        Span::styled("BOHRASE LTP: ", Style::default().fg(Color::Yellow)),
                        Span::styled(format!("₹{}", hft.bohrase_ltp), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("Net Spread: ", Style::default().fg(Color::Gray)),
                        Span::styled(format!("₹{} ", format_indian_currency(spread_rupees)), Style::default().fg(spread_color).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("[{}]", spread_label), Style::default().fg(spread_color)),
                    ]),
                    Line::from(vec![
                        Span::styled("Dynamic Batch Size: ", Style::default().fg(Color::Gray)),
                        Span::styled(format!("{} shares", format_indian_currency(hft.batch_size as f64).split('.').next().unwrap()), Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::styled("Net Inventory: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            format!("{} shares{}", format_indian_currency(hft.unified_inventory as f64).split('.').next().unwrap(), if inv_cap_hit { " [CAP HIT — PAUSING BUYS]" } else { "" }),
                            Style::default().fg(if inv_cap_hit { Color::Red } else { Color::Yellow }).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                ];
                let col2_widget = Paragraph::new(col2_text).block(
                    Block::default()
                        .title(" 🎯 SPREAD & BATCH INVENTORY ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                );
                f.render_widget(col2_widget, hft_box_layout[1]);


                let x_max = (hft_balance_history.len() as f64).max(60.0);
                let x_min = (x_max - 60.0).max(0.0);
                let window_points: Vec<&(f64, f64)> = hft_balance_history.iter().filter(|(x, _)| *x >= x_min).collect();
                let (y_min, y_max) = if window_points.is_empty() {
                    (999_990_000.0, 1_000_010_000.0)
                } else {
                    let mut min_val = f64::MAX;
                    let mut max_val = f64::MIN;
                    for (_, y) in &window_points {
                        if *y < min_val { min_val = *y; }
                        if *y > max_val { max_val = *y; }
                    }
                    if (max_val - min_val).abs() < 10.0 { (min_val - 50.0, max_val + 50.0) } else { (min_val - 10.0, max_val + 10.0) }
                };

                let hft_dataset = vec![
                    Dataset::default()
                        .marker(symbols::Marker::Braille)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Green))
                        .data(if window_points.is_empty() { &hft_balance_history } else {
                            // Safety: window_points borrows from hft_balance_history;
                            // we need owned slice — collect into a temp vec via the full history slice
                            &hft_balance_history[hft_balance_history.len().saturating_sub(60)..]
                        }),
                ];

                let hft_chart = Chart::new(hft_dataset)
                    .block(
                        Block::default()
                            .title(" 📈 LIVE CAPITAL GROWTH (₹) ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Magenta)),
                    )
                    .x_axis(Axis::default().bounds([x_min, x_max]))
                    .y_axis(
                        Axis::default()
                            .bounds([y_min, y_max])
                            .labels(vec![
                                Span::raw(format!("{:.0}", y_min)),
                                Span::raw(format!("{:.0}", (y_min + y_max) / 2.0)),
                                Span::raw(format!("{:.0}", y_max)),
                            ]),
                    );
                f.render_widget(hft_chart, hft_box_layout[2]);
            }
        });

        thread::sleep(Duration::from_millis(50));
    }

    disable_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn disable_mode() -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    Ok(())
}

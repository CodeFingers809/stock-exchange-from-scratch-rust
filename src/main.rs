use std::io::stdout;
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
    domain::{market::Market, price::Price},
    sim::simulator::Simulator,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ayushse_market = Market::new("AYUSHSE".to_string());
    ayushse_market.add_stock("TCS".to_string(), Price::from_rupees_paisa(2245, 0));

    let mut simulator = Simulator::new("TCS".to_string());
    let mut total_orders_processed = 0usize;
    let mut all_order_latencies = Vec::new();

    // Data points for real-time scrolling line chart: (x_time_sec, y_price_rupees)
    let start_time = Instant::now();
    let mut ltp_history: Vec<(f64, f64)> = Vec::new();

    let mut step_count = 0usize;

    loop {
        if poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = read()? {
                if key_event.code == KeyCode::Char('q') || key_event.code == KeyCode::Char('Q') {
                    break;
                }
            }
        }

        step_count += 1;
        let metrics = simulator.step(&mut ayushse_market);
        let step_median_latency = calculate_median_duration(metrics.order_latencies.clone());
        total_orders_processed += metrics.order_latencies.len();
        all_order_latencies.extend(metrics.order_latencies);
        let overall_median_latency = calculate_median_duration(all_order_latencies.clone());

        let book = ayushse_market.get_orderbook("TCS").unwrap();
        let current_ltp_rupees = book.ltp.paisa as f64 / 100.0;
        let current_elapsed_secs = start_time.elapsed().as_secs_f64();

        // Push new LTP tick to line chart history once every 1 second (every 5th step of 200ms)
        if step_count % 5 == 0 || ltp_history.is_empty() {
            ltp_history.push((current_elapsed_secs, current_ltp_rupees));
        }

        let total_resting_orders = book.bids.values().map(|q| q.len()).sum::<usize>()
            + book.asks.values().map(|q| q.len()).sum::<usize>();

        // Use 1-second chart data points for scrolling window based on chart area width
        let chart_area_width = 80usize;
        let chart_step_index = ltp_history.len() as f64;
        let x_max = chart_step_index.max(chart_area_width as f64);
        let x_min = (x_max - chart_area_width as f64).max(0.0);

        // Convert ltp_history to step-indexed points: (sec_step_num, ltp_rupees)
        let chart_points: Vec<(f64, f64)> = ltp_history
            .iter()
            .enumerate()
            .map(|(idx, (_, price))| ((idx + 1) as f64, *price))
            .collect();

        // Compute y-axis bounds from points inside the active window
        let window_points: Vec<&(f64, f64)> = chart_points.iter().filter(|(x, _)| *x >= x_min).collect();
        let (y_min, y_max) = if window_points.is_empty() {
            (current_ltp_rupees - 5.0, current_ltp_rupees + 5.0)
        } else {
            let mut min_val = f64::MAX;
            let mut max_val = f64::MIN;
            for (_, y) in &window_points {
                if *y < min_val { min_val = *y; }
                if *y > max_val { max_val = *y; }
            }
            if (max_val - min_val).abs() < 0.5 {
                (min_val - 1.0, max_val + 1.0)
            } else {
                (min_val - 0.5, max_val + 0.5)
            }
        };

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(8), // Metrics Header
                    Constraint::Percentage(45), // Live Scrolling Chart
                    Constraint::Percentage(45), // Order Book Depth
                ])
                .split(f.area());

            // 1. Metrics Header Panel
            let header_text = vec![
                Line::from(vec![
                    Span::styled("Speed Regime: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:?} ({:.1}s)", simulator.current_regime.speed, simulator.current_regime.speed_started_at.elapsed().as_secs_f64())),
                    Span::raw(" | "),
                    Span::styled("Buy/Sell Ratio: ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{:.1}% Buy / {:.1}% Sell (Target: {:.1}%)", simulator.current_regime.buy_prob * 100.0, simulator.current_regime.sell_prob * 100.0, simulator.current_regime.target_buy_prob * 100.0)),
                ]),
                Line::from(vec![
                    Span::styled("Total Orders: ", Style::default().fg(Color::Green)),
                    Span::raw(format!("{} | ", total_orders_processed)),
                    Span::styled("Resting Orders: ", Style::default().fg(Color::Magenta)),
                    Span::raw(format!("{} | ", total_resting_orders)),
                    Span::styled("Executed Trades: ", Style::default().fg(Color::LightGreen)),
                    Span::raw(format!("{}", metrics.trades.len())),
                ]),
                Line::from(vec![
                    Span::styled("⚡ Step Median Latency: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:?} ({:.3} µs) | ", step_median_latency, step_median_latency.as_nanos() as f64 / 1000.0)),
                    Span::styled("⚡ Overall Median: ", Style::default().fg(Color::Green)),
                    Span::raw(format!("{:?} ({:.3} µs)", overall_median_latency, overall_median_latency.as_nanos() as f64 / 1000.0)),
                ]),
                Line::from(Span::styled("Press 'q' to exit simulation", Style::default().fg(Color::DarkGray))),
            ];

            let header = Paragraph::new(header_text).block(
                Block::default()
                    .title(" 🚀 REAL-TIME STOCK EXCHANGE ENGINE METRICS ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );
            f.render_widget(header, chunks[0]);

            // 2. Real-Time Scrolling Line Chart (1 sec step window)
            let chart_dataset = vec![Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(&chart_points)];

            let chart = Chart::new(chart_dataset)
                .block(
                    Block::default()
                        .title(format!(" 📈 TCS REAL-TIME LTP (1s Step Window) | Current: ₹{:.2} ", current_ltp_rupees))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .x_axis(
                    Axis::default()
                        .title("Time (s)")
                        .style(Style::default().fg(Color::Gray))
                        .bounds([x_min, x_max])
                        .labels(vec![
                            Span::raw(format!("{:.0}s", x_min)),
                            Span::raw(format!("{:.0}s", (x_min + x_max) / 2.0)),
                            Span::raw(format!("{:.0}s", x_max)),
                        ]),
                )
                .y_axis(
                    Axis::default()
                        .title("Price (₹)")
                        .style(Style::default().fg(Color::Gray))
                        .bounds([y_min, y_max])
                        .labels(vec![
                            Span::raw(format!("{:.2}", y_min)),
                            Span::raw(format!("{:.2}", (y_min + y_max) / 2.0)),
                            Span::raw(format!("{:.2}", y_max)),
                        ]),
                );
            f.render_widget(chart, chunks[1]);

            // 3. Order Book Depth Panel (Top 5 Asks + Top 5 Bids)
            let top_asks: Vec<_> = book.asks.iter().take(5).collect();
            let top_bids: Vec<_> = book.bids.iter().rev().take(5).collect();

            let mut book_lines = vec![
                Line::from(Span::styled("--- ASKS (SELLERS) ---", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
            ];

            if top_asks.is_empty() {
                book_lines.push(Line::from("  (No asks resting on book)"));
            } else {
                for (price, queue) in top_asks.iter().rev() {
                    let total_shares: u64 = queue.iter().map(|o| o.remaining_size()).sum();
                    book_lines.push(Line::from(vec![
                        Span::styled(format!("  {} ", price), Style::default().fg(Color::Red)),
                        Span::raw(format!("| {} shares ({} orders)", total_shares, queue.len())),
                    ]));
                }
            }

            book_lines.push(Line::from(Span::styled("----------------------------------------", Style::default().fg(Color::DarkGray))));
            book_lines.push(Line::from(Span::styled("--- BIDS (BUYERS) ---", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));

            if top_bids.is_empty() {
                book_lines.push(Line::from("  (No bids resting on book)"));
            } else {
                for (price, queue) in top_bids {
                    let total_shares: u64 = queue.iter().map(|o| o.remaining_size()).sum();
                    book_lines.push(Line::from(vec![
                        Span::styled(format!("  {} ", price), Style::default().fg(Color::Green)),
                        Span::raw(format!("| {} shares ({} orders)", total_shares, queue.len())),
                    ]));
                }
            }

            let book_paragraph = Paragraph::new(book_lines).block(
                Block::default()
                    .title(format!(" 📖 ORDER BOOK L2 DEPTH | LTP: {} ", book.ltp))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
            f.render_widget(book_paragraph, chunks[2]);
        })?;

        std::thread::sleep(Duration::from_millis(200));
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

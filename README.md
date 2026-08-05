# Multi-Exchange Order Matching Engine & Cross-Exchange Arbitrage Simulator (Rust)

A from-scratch limit order book, matching engine, and cross-exchange arbitrage bot, written in Rust with a live web dashboard. Built as a systems-programming exercise in low-latency order matching, lock-free state management, and concurrent market simulation — not a production trading system.

---

## What this actually is

This project simulates two independent exchanges (`AYUSHSE` and `BOHRASE`) running in the same process, each with its own limit order book, and a bot that watches both books for arbitrage opportunities and executes offsetting trades between them. Market data (order flow) is synthetically generated using Gaussian/normal distributions to approximate a mix of institutional and retail order sizes — it is **not connected to any real market data feed**.

The goal was to implement the core mechanics correctly (price-time priority matching, bracket orders, atomic state transitions) rather than to build something production-deployable. Any latency numbers below reflect in-process function call timing on a single machine with no network I/O, not real exchange round-trip latency.

---

## Architecture

### Order Book & Matching Engine
- **Price-time priority matching** using `BTreeMap` for $O(\log K)$ price-level lookup and $O(1)$ queue pop on execution, where $K$ is the number of distinct price levels.
- **Thread-safe order state** via `Arc<Mutex<OrderInner>>`, so order book and portfolio views observe consistent state without duplicating data.
- **Pre-trade risk checks**: validates available cash on buy orders and available holdings on sell orders before an order is routed to the book.

### Bracket Orders (Parent + Stop-Loss + Take-Profit)
- Parent, stop-loss, and take-profit orders share state through an atomic reference-counted flag (`Arc<AtomicBool>`).
- When the parent fills, the SL/TP children activate in constant time — no scan over the order queue is required.
- **One-Cancels-the-Other (OCO)** behavior: when either exit order triggers or is cancelled, the sibling updates atomically without a lock-based traversal.

### Cross-Exchange Arbitrage Bot
- Monitors L2 (top-of-book depth) from both simulated exchanges concurrently across all constituent stocks.
- Executes a two-leg trade only when `best_bid(Exchange B) − best_ask(Exchange A)` is positive after accounting for the simulated spread.
- Includes an inventory-rebalancing routine that detects unhedged partial fills (one leg filled, the other didn't) and unwinds the excess position against the opposite exchange's best resting quote.

### Market Data Persistence & Real-Time Event Pipeline
- **SQLite Storage**: Persistent storage of order books, trades, portfolio balances, and 1-minute OHLCV candles with WAL (Write-Ahead Logging) mode.
- **Redis Streams Publisher**: Non-blocking asynchronous event pipeline streaming trade events for external consumption.
- **WebSocket Broadcast Engine**: Sub-millisecond tick broadcast broadcasting order book L2 depth, last-traded prices, and HFT telemetry updates.

### Benchmark Index (`AYUSH-5`)
- Dynamically calculated benchmark index tracking the top 5 constituent stocks (`TCS`, `RELIANCE`, `INFY`, `HDFCBANK`, `ICICIBANK`).
- Computes exchange-specific benchmark ticks independently for `AYUSHSE` and `BOHRASE`.

### Terminal Interface (TUI Mode)
- Built with `ratatui` + `crossterm` as a lightweight CLI alternative to the web frontend.

---

## Performance notes (read before citing these numbers anywhere)

These are **micro-benchmarks of in-process function latency**, measured on synthetic order flow on a single developer machine — they are not representative of real exchange, real network, or real market-data latency, and should not be compared to production HFT infrastructure (which typically involves kernel bypass, FPGA, or colocated networking that this project doesn't touch).

| What was measured | Result |
|---|---|
| Time from receiving a synthetic tick to submitting the offsetting order (in-process, no I/O) | ~0.5–1.0 µs |
| Time for the simulated matching engine to process and confirm a fill | ~1.5–3.0 µs |
| Combined median, tick-to-fill | ~1.7 µs |
| Order throughput, single thread | >300,000 orders/sec |

*(If you want to actually defend these numbers in an interview, be ready to explain: sample size, how timing was captured — e.g. `Instant::now()` deltas — whether GC/allocator warmup was excluded, and that no real network stack is involved. That context matters more than the number itself.)*

---

## Tech Stack

### Core Engine & Backend (Rust)
- **Rust (2021 Edition)**: High-performance memory-safe systems language.
- **Axum**: Asynchronous web framework for high-throughput REST API endpoints and WebSocket servers.
- **Tokio**: Multi-threaded asynchronous I/O runtime for concurrency and background tasks.
- **SQLx (SQLite)**: Asynchronous SQL toolkit for database queries, schema migrations, and persistent WAL storage.
- **Redis Streams (`redis-rs`)**: High-speed message broker for async event streaming.
- **Serde / Serde JSON**: Zero-copy serialization and deserialization for JSON payloads and IPC.
- **Ratatui & Crossterm**: Terminal User Interface (TUI) libraries for terminal rendering.

### Frontend Dashboard (TypeScript & React)
- **Next.js 15 (App Router)**: Framework for React applications built with static export (`output: 'export'`).
- **React 19 & TypeScript**: Type-safe component UI architecture.
- **TradingView Lightweight Charts (`lightweight-charts`)**: Canvas-rendered financial charts for real-time candlestick and line series.
- **TailwindCSS**: CSS design system with custom dark mode and glassmorphism styling.
- **Lucide React**: Modern iconography library.

### Deployment & Infrastructure
- **AWS EC2 (Amazon Linux 2023)**: Cloud virtual machine hosting both backend binary and frontend static assets.
- **Nginx**: High-performance reverse proxy handling SSL termination (`TLS v1.2/v1.3`), WebSocket upgrading, and zero-overhead static asset serving.
- **Systemd**: Linux service manager for daemonizing and managing process lifecycle.

---

## Running It

### Prerequisites
- **Rust** (1.75+ recommended): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js** (v18+ recommended): for frontend dashboard build
- **Redis Server** (optional): for streaming event pipeline

### 1. Running the Backend Server
From the repository root:

```bash
# Build and run the matching engine & API server
cd backend
cargo run --release
```

The matching engine will initialize SQLite database storage and start listening for API and WebSocket requests on `http://localhost:3001`.

### 2. Running in Terminal Mode (TUI)
If you prefer terminal visualization instead of the web dashboard:

```bash
cd backend
cargo run --release -- tui
```

### 3. Running the Web Frontend Dashboard
From the repository root:

```bash
cd client
npm install
npm run dev
```

Open `http://localhost:3000` in your web browser.

### 4. Production Build & Export
To compile the frontend static export:

```bash
cd client
npm run build
```

The optimized static build will be generated in `client/out/`, ready to be served by Nginx or any web server.
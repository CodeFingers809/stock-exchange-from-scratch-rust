# Multi-Exchange Order Matching Engine & Cross-Exchange Arbitrage Simulator (Rust)

A from-scratch limit order book, matching engine, and cross-exchange arbitrage bot, written in Rust with a live terminal dashboard. Built as a systems-programming exercise in low-latency order matching, lock-free state management, and concurrent market simulation — not a production trading system.

---

## What this actually is

This project simulates two independent exchanges (`AYUSHSE` and `BOHRASE`) running in the same process, each with its own limit order book, and a bot that watches both books for arbitrage opportunities and executes offsetting trades between them. Market data (order flow) is synthetically generated using Gaussian/normal distributions to approximate a mix of institutional and retail order sizes — it is **not connected to any real market data feed**.

The goal was to implement the core mechanics correctly (price-time priority matching, bracket orders, atomic state transitions) rather than to build something production-deployable. Any latency numbers below reflect in-process function call timing on a single machine with no network I/O, not real exchange round-trip latency.

---

## Architecture

### Order Book & Matching Engine
- Price-priority FIFO matching using `BTreeMap` for O(log K) price-level lookup and O(1) queue pop on execution, where K is the number of distinct price levels.
- Thread-safe order state via `Arc<Mutex<OrderInner>>`, so order book and portfolio views observe consistent state without duplicating data.
- Pre-trade risk checks: validates available cash on buy orders and available holdings on sell orders before an order is routed to the book.

### Bracket Orders (Parent + Stop-Loss + Take-Profit)
- Parent, stop-loss, and take-profit orders share state through an atomic reference-counted flag (`Arc<AtomicBool>`).
- When the parent fills, the SL/TP children activate in constant time — no scan over the order queue is required.
- One-Cancels-the-Other (OCO) behavior: when either exit order triggers or is cancelled, the sibling updates atomically without a lock-based traversal.

### Cross-Exchange Arbitrage Bot
- Monitors L2 (top-of-book depth) from both simulated exchanges concurrently.
- Executes a two-leg trade only when `best_bid(Exchange B) − best_ask(Exchange A)` is positive after accounting for the simulated spread, i.e. a strictly profitable synthetic arbitrage.
- Includes an inventory-rebalancing routine that detects unhedged partial fills (one leg filled, the other didn't) and unwinds the excess position against the opposite exchange's best resting quote.

### Terminal Dashboard
- Built with `ratatui` + `crossterm`.
- Displays both exchanges' top-5 bid/ask depth and last-traded price side by side.
- Rolling price chart using Braille-character sub-pixel rendering for smoother terminal-native line charts.
- Live PnL, win/loss counters, and rolling median latency stats for the arbitrage bot.

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

- **Language**: Rust (2021 edition)
- **Concurrency**: `std::sync::mpsc` channels, `Arc<Mutex<_>>`, `Arc<AtomicBool>`
- **Terminal UI**: `ratatui`, `crossterm`
- **Market simulation**: `rand_distr` (Gaussian/normal order-size distributions)

## What's not here (yet)

- No real market data ingestion.
- No persistence — state is in-memory only.
- No network layer between the two "exchanges"; they run in the same process.
- No tests directory yet — see `TODO.md`.

## Running it

```bash
cargo run
```

See `.env.example` for configurable simulation parameters.
# 📌 Stock Exchange Project TODO List & Checkpoints

> **Note**: Everything is built incrementally by the developer. Work moves stage by stage with interactive CLI verification at each checkpoint.

---

## 🧮 Phase 1: In-Memory Core Matching Engine & Market Router

- [x] **Step 1.1: Fixed-Point Price Model (`src/domain/price.rs`)**
  - [x] Implement `Price` struct (`paisa: u64`).
  - [x] Implement constructors (`from_paisa`, `from_rupees_paisa`).
  - [x] Implement formatting for display using `₹` symbol (e.g., `₹150.25`).
  - [x] Implement comparison traits (`Ord`, `PartialOrd`, `Eq`, `PartialEq`).

- [x] **Step 1.2: Order Domain Model (`src/domain/order.rs`)**
  - [x] Implement `BidOrAsk` enum (`Bid`, `Ask`).
  - [x] Implement `OrderType` & `OrderStatus` enums.
  - [x] Implement `Order` struct (`symbol`, `price: Option<Price>`, `size`, `filled_size`, `bid_or_ask`, `timestamp`).
  - [x] Implement `Display` formatting for `Order`.

- [x] **Step 1.3: OrderBook Engine (`src/domain/orderbook.rs`)**
  - [x] Implement `OrderBook` struct using `BTreeMap<Price, VecDeque<Order>>`.
  - [x] Implement `add_market_order(&mut self, order: Order)` with multi-level matching and LTP updates.
  - [x] Implement `add_limit_order(&mut self, order: Order)` with limit condition checks and book insertion.
  - [x] Implement `Display` depth table formatting for `OrderBook`.

- [x] **Step 1.4: Market Router (`src/domain/market.rs`)**
  - [x] Implement `Market` struct (`name`, `books: HashMap<String, OrderBook>`).
  - [x] Implement ticker registration (`add_stock`).
  - [x] Implement order routing (`place_limit_order`, `place_market_order`).
  - [x] Implement `Display` for `Market`.

---

## 👤 Phase 2: User, Portfolio & Holding Infrastructure

- [x] **Step 2.1: Holding & User Models (`src/domain/holding.rs`, `src/domain/user.rs`)**
  - [x] Implement `User` struct (`id: String`, `name: String`).
  - [x] Implement `Holding` struct (`symbol: String`, `quantity: u64`, `buy_price: Price`, `bought_at: SystemTime`).

- [x] **Step 2.2: Portfolio & Risk Validation (`src/domain/portfolio.rs`)**
  - [x] Implement `Portfolio` struct (`user`, `acc_no`, `balance_paisa`, `open_orders`, `holdings`).
  - [x] Implement pre-trade risk validation for buy orders (cash check) and sell orders (share holdings check).
  - [x] Implement order dispatching to market (`dispatch_limit_order`, `dispatch_market_order`).

- [x] **Step 2.3: Trade Generation & Portfolio Settlement (`src/domain/trade.rs`)**
  - [x] Implement `Trade` struct emitted by `OrderBook` (`id`, `symbol`, `price`, `quantity`, `buyer_acc_no`, `seller_acc_no`, `timestamp`).
  - [x] Update `OrderBook::add_limit_order` and `add_market_order` to generate and return `Vec<Trade>`.
  - [x] Implement `Portfolio` trade settlement (`apply_buy_trade`, `apply_sell_trade`) and automatic filled `open_orders` cleanup.

---

## 🎲 Phase 3: Stochastic Simulator & Real-Time Terminal TUI

- [x] **Step 3.1: Stochastic Market Simulator (`src/sim/simulator.rs`)**
  - [x] Implement regime shifts (Fast burst vs Normal mode every 10s).
  - [x] Implement smooth linear interpolation (lerping) of Buy/Sell probabilities every 3-4s.
  - [x] Implement Gaussian (Normal) price distribution around LTP with 70% market order ratio and constricted limit variance.

- [x] **Step 3.2: Ratatui Full-Screen Terminal Dashboard (`src/main.rs`)**
  - [x] Implement real-time 1-second scrolling LTP line chart with sub-pixel Braille rendering.
  - [x] Implement top-5 color-coded Order Book L2 Depth rendering.
  - [x] Benchmarked median order latency (`3.042 µs` overall median across 275,000+ orders).
  - [x] Non-blocking keyboard handler (`q` to exit).

---

## ⚡ Phase 4: High-Frequency Trading (HFT) & Bracket Order Engine

- [x] **Step 4.1: O(1) Bracket Order State Machine (`src/domain/order.rs`)**
  - [x] Implement shared `BracketState` with `Arc<AtomicBool>` for `is_parent_filled` and `is_bracket_cancelled`.
  - [x] Enable O(1) activation of Stop-Loss (SL) and Target (TP) exit orders on parent fill without book traversal.
  - [x] Enable O(1) One-Cancels-the-Other (OCO) cancellation across bracket family.

- [x] **Step 4.2: Cross-Exchange Arbitrage Engine (`src/hft/arbitrage.rs`)**
  - [x] Real-time market tick subscription model (`Market::subscribe_ticker` over `mpsc` channel).
  - [x] Actionable spread computation (`sell_exchange.best_bid - buy_exchange.best_ask`) with non-negative edge guarantees (`≥ 0 paisa`).
  - [x] Dual-exchange execution routing (`OrderRouter`) with ₹1 minimum threshold.
  - [x] Autonomous HFT self-flushing / inventory unloading algorithm for unhedged position rebalancing.
  - [x] Microsecond/nanosecond engine tick latency & rolling median tracking (`latency_history`).

- [x] **Step 4.3: Real-Time Multi-Exchange & HFT Dashboard (`src/main.rs`)**
  - [x] Concurrent dual-exchange simulation (`AYUSHSE` & `BOHRASE`).
  - [x] Real-time HFT Account & Performance Telemetry rendering (Capital, Realized PnL, Win/Loss count, Latency & Median).
  - [x] Side-by-side exchange price monitor (`AYUSHSE LTP` vs `BOHRASE LTP`) and Spread/Inventory panel.
  - [x] Live HFT Capital Growth curve rendering.

---

## 🌐 Phase 5: Async Event Streaming & SQLite Persistence (`backend/src/db/`, `backend/src/events/`)

- [x] **Step 5.1: Non-Blocking Event Streaming Pipeline**
  - [x] Non-blocking Tokio channel dispatch (`mpsc::unbounded_channel`) from engine ticks to async background workers.
  - [x] Redis Stream Publisher with auto-flushing `XADD MAXLEN ~ 1000` to prevent memory leaks.
- [x] **Step 5.2: SQLite Database Persistence & State Recovery**
  - [x] Sqlite Pool & Background Writer (`SqliteDbWriter`) for asynchronous trade logging.
  - [x] Automatic portfolio state recovery on startup (`cargo run` loads initial user balance & holdings).
  - [x] DB pool cost protection (`max_connections(5)`, `idle_timeout(30s)` and debounced balance saving).

---

## 🎨 Phase 6: Next.js + Tailwind CSS + shadcn/ui Dashboard (`client/`)

- [ ] **Step 6.1: Next.js Frontend UI Development**
  - [ ] Connect Next.js frontend to Redis Streams / WebSocket server for real-time web UI dashboard visualization.
  - [ ] Build interactive trading interface, live depth viewer, and HFT telemetry graphs using `shadcn/ui` and Tailwind CSS.



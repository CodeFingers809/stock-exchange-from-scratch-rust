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

## ⚡ Phase 3: Async Tokio Server & Concurrency

- [ ] **Step 3.1: Thread-Safe State Management (`Arc<RwLock<Broker>>` / `Arc<Mutex<Market>>`)**
  - [ ] Wrap `Market` / `Broker` in Tokio thread-safe synchronization primitives for concurrent access.

- [ ] **Step 3.2: Async Task Runner & CLI Control Loop (`src/main.rs`)**
  - [ ] Convert `main.rs` to `#[tokio::main]`.
  - [ ] Build an interactive async CLI REPL to create users, deposit funds, submit orders, and view portfolios/orderbooks concurrently.

---

## 💾 Phase 4: Persistence Layer (SQLite via `sqlx`)

- [ ] **Step 4.1: Database Schema & Repositories (`src/db/`)**
  - [ ] SQLite tables for `users`, `portfolios`, `orders`, and `trades`.
  - [ ] Persist orderbook snapshots and executed trades asynchronously.

---

## 📡 Phase 5: Upstash Redis & Axum REST API

- [ ] **Step 5.1: Real-time Event Streaming (`src/events/`)**
  - [ ] Publish trade executions and book updates to Upstash Redis pub-sub channels.

- [ ] **Step 5.2: Axum HTTP REST Endpoints (`src/api/`)**
  - [ ] Expose REST endpoints for placing orders, inspecting orderbooks, and fetching user portfolios.


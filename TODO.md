# 📌 Stock Exchange Project TODO List & Checkpoints

> **Note**: Everything will be built incrementally by the developer. Work moves stage by stage with interactive CLI verification at each checkpoint.

---

## 🧮 Phase 1: Pure In-Memory Core Domain & OOP Objects

- [ ] **Step 1.1: Fixed-Point Price Model (`src/domain/price.rs`)**
  - [ ] Implement `Price` struct containing `rupees: i64` and `paisa: u8` (`0..=99`).
  - [ ] Implement constructors (`from_rupees_paisa`, `from_paisa_total`, `parse_str`).
  - [ ] Implement formatting for display using `₹` symbol (e.g., `₹150.25`).
  - [ ] Implement comparison traits (`Ord`, `PartialOrd`, `Eq`, `PartialEq`).
  - [ ] Implement arithmetic operations (`Add`, `Sub`) with overflow protection.
  - [ ] Add unit tests verifying zero floating-point imprecision.

- [ ] **Step 1.2: Order & Trade Domain Models (`src/domain/order.rs`, `src/domain/trade.rs`)**
  - [ ] Implement `OrderSide` enum (`Bid`, `Ask`).
  - [ ] Implement `OrderType` enum (`Limit`, `Market`).
  - [ ] Implement `OrderStatus` enum (`New`, `PartiallyFilled`, `Filled`, `Cancelled`).
  - [ ] Implement `Order` struct (`id`, `symbol`, `side`, `price`, `quantity`, `filled_quantity`, `timestamp`, `user_id`).
  - [ ] Implement helper methods on `Order` (`remaining_quantity()`, `is_filled()`, `fill()`).
  - [ ] Implement `Trade` struct (`id`, `symbol`, `bid_order_id`, `ask_order_id`, `price`, `quantity`, `timestamp`).

- [ ] **Step 1.3: Spread Object (`src/domain/spread.rs`)**
  - [ ] Implement `Spread` struct (`best_bid: Option<Price>`, `best_ask: Option<Price>`).
  - [ ] Implement methods: `difference() -> Option<Price>`, `mid_price() -> Option<Price>`, `display_inr()`.

- [ ] **Step 1.4: OrderBook & Matching Logic (`src/domain/orderbook.rs`)**
  - [ ] Implement `OrderBook` struct using `BTreeMap<Price, VecDeque<Order>>`.
    - Note: Bid side sorted descending (highest price first), Ask side sorted ascending (lowest price first).
  - [ ] Implement `add_limit_order(&mut self, order: Order) -> Vec<Trade>`.
  - [ ] Implement FIFO matching algorithm for overlapping bids/asks.
  - [ ] Implement `cancel_order(&mut self, order_id: u64) -> bool`.
  - [ ] Implement `get_spread(&self) -> Spread`.

- [ ] **Step 1.5: In-Memory CLI Checkpoint 1 (`src/cli/mod.rs` & `src/main.rs`)**
  - [ ] Build a interactive REPL / CLI sandbox to manually create orders, place them into the `OrderBook`, print `Spread` in `₹`, and display executed `Trades`.
  - [ ] Verify matching logic via manual CLI operations before adding any DB/network dependencies.

---

## 💾 Phase 2: Persistence & Storage Layer (SQLite)

- [ ] **Step 2.1: Database Schema & Migration Setup**
  - [ ] Design SQLite tables for `orders` and `trades`.
  - [ ] Create setup script / migrations.

- [ ] **Step 2.2: SQLite Repositories (`src/db/`)**
  - [ ] Implement `OrderRepository` to save & update orders.
  - [ ] Implement `TradeRepository` to record executed trades.

- [ ] **Step 2.3: CLI Checkpoint 2**
  - [ ] Verify order persistence and trade history recovery from SQLite via CLI.

---

## 📡 Phase 3: Event Broadcasting (Upstash Redis)

- [ ] **Step 3.1: Redis Client & Publisher (`src/events/`)**
  - [ ] Configure Upstash Redis client using `redis` crate.
  - [ ] Implement `EventPublisher` struct to broadcast `TradeExecuted` and `BookUpdated` events.

- [ ] **Step 3.2: Redis Subscriber Test CLI**
  - [ ] Create CLI command to listen to real-time events published to Upstash Redis.

---

## 🌐 Phase 4: Web API & Server (Axum + Tokio)

- [ ] **Step 4.1: Axum HTTP Endpoints (`src/api/`)**
  - [ ] Implement `POST /api/orders` - Submit new order.
  - [ ] Implement `DELETE /api/orders/:id` - Cancel order.
  - [ ] Implement `GET /api/orderbook/:symbol` - Get current book depth & spread.
  - [ ] Implement `GET /api/trades/:symbol` - Get trade history.

- [ ] **Step 4.2: End-to-End System Verification**
  - [ ] Verify full system under concurrent HTTP load with SQLite logging and Redis streaming.

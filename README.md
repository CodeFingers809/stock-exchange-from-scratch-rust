# 📈 Stock Exchange Rust Engine (`stock-exchange-rust`)

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2021-orange.svg?style=for-the-badge&logo=rust" alt="Rust 2021"/>
  <img src="https://img.shields.io/badge/SQLite-0.7-blue.svg?style=for-the-badge&logo=sqlite" alt="SQLite"/>
  <img src="https://img.shields.io/badge/Upstash_Redis-Serverless-red.svg?style=for-the-badge&logo=redis" alt="Upstash Redis"/>
  <img src="https://img.shields.io/badge/Axum-0.7-purple.svg?style=for-the-badge&logo=express" alt="Axum"/>
  <img src="https://img.shields.io/badge/Currency-INR_%E2%82%B9-brightgreen.svg?style=for-the-badge" alt="Currency INR"/>
</p>

---

### **I have tried my best to write code wherever I can write myself for self learning. AI was only used to automate mundance tasks.**

This is a stock exchange written from scratch in Rust that has all the core features of a real stock exchange.

---

## 🏛️ Codebase Modules & Architecture

Here is a detailed explanation of the domain modules we have built:

### 1. 🪙 Fixed-Point Price (`src/domain/price.rs`)
- Represents stock prices using integers (`paisa: u64`) to prevent floating-point calculation bugs.
- Provides integer constructors (`from_paisa`, `from_rupees_paisa`) and custom `Display` formatting (`₹150.25`).

### 2. 📜 Orders & Shared Ownership (`src/domain/order.rs`)
- Supports **Limit Orders** and **Market Orders** for both **Buy (Bid)** and **Sell (Ask)** sides.
- Uses `Arc<Mutex<OrderInner>>` thread-safe pointers so `OrderBook` and user `Portfolio` share the exact same order reference in memory.
- When an order gets matched and filled in the order book, the user's open orders view updates automatically without data duplication.

### 3. 📗 Order Book & Matching Engine (`src/domain/orderbook.rs`)
- Uses price-priority `BTreeMap` queues (`bids` and `asks`) to match orders.
- Executes limit orders and multi-level market orders.
- Automatically updates the Last Traded Price (LTP) whenever a match happens.
- Generates `Trade` execution records when buyer and seller orders meet.

### 4. 🏢 Market Order Router (`src/domain/market.rs`)
- Acts as the central exchange router (e.g. `AYUSHSE`, `NSE`).
- Maps stock ticker symbols (`TCS`, `RELIANCE`) to their respective `OrderBook` instances using a `HashMap`.
- Routes incoming buy and sell orders to the correct stock order book.

### 5. 👤 User, Portfolio & Risk Validation (`src/domain/user.rs`, `src/domain/portfolio.rs`, `src/domain/holding.rs`)
- Stores user demat accounts (`acc_no`), cash balances (`balance_paisa`), open orders, and stock holdings (`Holding`).
- Performs **Pre-Trade Risk Checks**:
  - Rejects buy orders if the user does not have enough cash.
  - Rejects sell orders if the user does not own enough shares.
- Settles trades anonymously (`apply_buy_trade`, `apply_sell_trade`) by updating cash, adding holdings, and cleaning up filled open orders.

### 6. 🤝 Trade Settlement Records (`src/domain/trade.rs`)
- Records executed trades containing `symbol`, `price`, `quantity`, `buyer_acc_no`, `seller_acc_no`, and `timestamp`.

---

## 🛠️ Tech Used

- 🦀 **Language**: Rust (2021 Edition)
- ⚡ **Async Runtime**: Tokio (for upcoming server phase)
- 🌐 **Web Framework**: Axum (for upcoming HTTP API)
- 💾 **Database**: SQLite (via `sqlx` for persistence)
- 📡 **Event Streaming**: Upstash Redis (for real-time trade events)


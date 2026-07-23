# 📈 Stock Exchange Rust Engine (`stock-exchange-rust`)

<p center>
  <img src="https://img.shields.io/badge/Rust-2021-orange.svg?style=for-the-badge&logo=rust" alt="Rust 2021"/>
  <img src="https://img.shields.io/badge/SQLite-0.7-blue.svg?style=for-the-badge&logo=sqlite" alt="SQLite"/>
  <img src="https://img.shields.io/badge/Upstash_Redis-Serverless-red.svg?style=for-the-badge&logo=redis" alt="Upstash Redis"/>
  <img src="https://img.shields.io/badge/Axum-0.7-purple.svg?style=for-the-badge&logo=express" alt="Axum"/>
  <img src="https://img.shields.io/badge/Currency-INR_%E2%82%B9-brightgreen.svg?style=for-the-badge" alt="Currency INR"/>
</p>

---

## 🚀 Overview

### **Except for this README file, I have not used AI anywhere in the logic. I did use AI to make a roadmap for this project.**

A high-performance, deterministic **Stock Exchange & Matching Engine** built from scratch in Rust.

Engineered with zero floating-point arithmetic errors using fixed-point integer representation (`rupees` & `paisa` up to 2 decimal places: `₹0.00`). Built incrementally with modular object-oriented domain modeling, CLI interaction checkpoints, and eventual integration with SQLite persistence and Upstash Redis event broadcasting.

---

## 🏛️ Key Domain Objects

- 🪙 **`Price`**: Fixed-point price representation storing `rupees` (`i64`) and `paisa` (`u8`, `0..=99`).
- 📜 **`Order`**: Limit order representation supporting `Bid` and `Ask` sides with FIFO priority.
- 📖 **`Spread`**: Bid/Ask spread object calculating market liquidity and top-of-book levels.
- 📗 **`OrderBook`**: BTreeMap-based matching engine managing price levels and executing matching rules.
- 🤝 **`Trade`**: Settlement record emitted upon order match execution.

---

## 🛠️ Tech Stack

- **Language**: Rust (2021 Edition)
- **Currency Standard**: Indian Rupee (`₹`, 2 decimal pips)
- **Async Runtime**: Tokio
- **Web Framework**: Axum
- **Database**: SQLite (via `sqlx`)
- **Event Streaming / PubSub**: Upstash Redis
- **Config**: `dotenvy`

---

## 📋 Development Plan & Verification

Execution proceeds in progressive, verifiable checkpoints:
1. **Core Domain Objects**: Fixed-point `Price`, `Order`, `Spread`, `OrderBook`.
2. **CLI Sandbox**: Interactive CLI to test order submission, matching, and spread calculations in-memory.
3. **Persistence Layer**: SQLite schema & `sqlx` repository integration.
4. **Event Streaming**: Upstash Redis pub-sub integration for trades and book updates.
5. **REST API Layer**: Axum HTTP endpoints (`POST /orders`, `GET /orderbook`, `GET /trades`).

---

## 📜 Principles & Guidelines

This codebase strictly follows software engineering guidelines focused on simplicity, empirical verification, zero magic, and zero floating-point imprecision. See [ANDREJ_KARPATHY_GUIDELINES.md](file:///Users/ayush/dev/Rust/stock-exchange-rust/ANDREJ_KARPATHY_GUIDELINES.md) and [TODO.md](file:///Users/ayush/dev/Rust/stock-exchange-rust/TODO.md) for progress tracking.

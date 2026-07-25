use std::time::Instant;
use stock_exchange_rust::domain::{
    holding::Holding,
    market::Market,
    order::{BidOrAsk, Order},
    portfolio::Portfolio,
    price::Price,
    user::User,
};

fn main() {
    let mut ayushse_market = Market::new("AYUSHSE".to_string());

    // Register stocks on Exchange
    ayushse_market.add_stock("TCS".to_string(), Price::from_rupees_paisa(2245, 0));
    ayushse_market.add_stock("RELIANCE".to_string(), Price::from_rupees_paisa(2500, 0));

    // Create User 1 (Seller) with 50 TCS shares
    let seller_user = User::new("usr_1".to_string(), "Alice (Seller)".to_string());
    let mut seller_portfolio = Portfolio::new(seller_user, "1208160012345678".to_string(), 100_000_00); // ₹100,000
    seller_portfolio.holdings.insert(
        "TCS".to_string(),
        vec![Holding {
            symbol: "TCS".to_string(),
            quantity: 50,
            buy_price: Price::from_rupees_paisa(2200, 0),
            bought_at: std::time::SystemTime::now(),
        }],
    );

    // Create User 2 (Buyer) with ₹500,000 cash
    let buyer_user = User::new("usr_2".to_string(), "Bob (Buyer)".to_string());
    let mut buyer_portfolio = Portfolio::new(buyer_user, "1208160087654321".to_string(), 500_000_00); // ₹500,000

    println!("Initial Buyer Cash: ₹{:.2}", buyer_portfolio.balance_paisa as f64 / 100.0);
    println!("Initial Seller TCS Shares: {}", seller_portfolio.total_shares("TCS"));

    // 1. Seller dispatches Limit Sell Order of 50 TCS @ ₹2250.00
    let seller_order = Order::new(
        "TCS".to_string(),
        Some(Price::from_rupees_paisa(2250, 0)),
        50,
        BidOrAsk::Ask,
        seller_portfolio.acc_no.clone(),
    );

    let start_limit = Instant::now();
    let _ = seller_portfolio.dispatch_limit_order(seller_order, &mut ayushse_market);
    let limit_latency = start_limit.elapsed();

    println!("\n⚡ Limit Order Internal Latency: {:?} ({} nanoseconds / {:.3} µs)", 
        limit_latency, 
        limit_latency.as_nanos(), 
        limit_latency.as_nanos() as f64 / 1000.0
    );
    println!("{}", ayushse_market);

    // 2. Buyer dispatches Market Buy Order of 30 TCS Shares
    let buyer_order = Order::new(
        "TCS".to_string(),
        None,
        30,
        BidOrAsk::Bid,
        buyer_portfolio.acc_no.clone(),
    );
    
    let start_market = Instant::now();
    let market_res = buyer_portfolio.dispatch_market_order(buyer_order, &mut ayushse_market);
    let market_latency = start_market.elapsed();

    println!("\n⚡ Market Order Matching Latency: {:?} ({} nanoseconds / {:.3} µs)", 
        market_latency, 
        market_latency.as_nanos(), 
        market_latency.as_nanos() as f64 / 1000.0
    );

    if let Ok((trades, _msg)) = market_res {
        for trade in trades {
            println!("🎉 Trade Executed! {} shares of {} @ {}", trade.quantity, trade.symbol, trade.price);
            buyer_portfolio.apply_buy_trade(&trade.symbol, trade.quantity, trade.price);
            seller_portfolio.apply_sell_trade(&trade.symbol, trade.quantity, trade.price);
        }
    }

    println!("\n--- Post-Trade Settlement ---");
    println!("Buyer Cash Balance: ₹{:.2}", buyer_portfolio.balance_paisa as f64 / 100.0);
    println!("Buyer TCS Holdings: {} shares", buyer_portfolio.total_shares("TCS"));
    println!("Seller Cash Balance: ₹{:.2}", seller_portfolio.balance_paisa as f64 / 100.0);
    println!("Seller Remaining TCS Shares: {}", seller_portfolio.total_shares("TCS"));
    println!("\nUpdated Market OrderBook:");
    println!("{}", ayushse_market);
}

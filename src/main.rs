use stock_exchange_rust::domain::{
    market::Market,
    order::{BidOrAsk, Order},
    price::Price,
};

fn main() {
    let mut ayushse_market = Market::new("AYUSHSE".to_string());

    // Register stocks on NSE
    ayushse_market.add_stock("TCS".to_string(), Price::from_rupees_paisa(2245, 0));
    ayushse_market.add_stock("RELIANCE".to_string(), Price::from_rupees_paisa(2500, 0));

    // Place Limit Orders via Market Router
    let _ = ayushse_market.place_limit_order(Order::new(
        "TCS".to_string(),
        Some(Price::from_rupees_paisa(2250, 0)),
        50,
        BidOrAsk::Ask,
    ));

    let _ = ayushse_market.place_limit_order(Order::new(
        "TCS".to_string(),
        Some(Price::from_rupees_paisa(2240, 50)),
        100,
        BidOrAsk::Bid,
    ));

    let _ = ayushse_market.place_limit_order(Order::new(
        "RELIANCE".to_string(),
        Some(Price::from_rupees_paisa(2505, 0)),
        20,
        BidOrAsk::Ask,
    ));

    println!("{}", ayushse_market);

    // Place Market Order on TCS via Market Router
    let _ = ayushse_market.place_market_order(Order::new(
        "TCS".to_string(),
        None,
        30,
        BidOrAsk::Bid,
    ));

    println!("\n--- After Market Buy of 30 TCS Shares ---");
    println!("{}", ayushse_market);
}

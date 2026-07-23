use stock_exchange_rust::domain::{
    order::{BidOrAsk, Order},
    orderbook::OrderBook,
    price::Price,
};

fn main() {
    let mut book = OrderBook::new("TCS".to_string(), Price::from_rupees_paisa(2245, 0));

    // Create a limit sell order (Ask)
    let ask_order = Order::new(
        "TCS".to_string(),
        Some(Price::from_rupees_paisa(2250, 0)),
        50,
        BidOrAsk::Ask,
    );

    // Create a limit buy order (Bid)
    let bid_order = Order::new(
        "TCS".to_string(),
        Some(Price::from_rupees_paisa(2240, 50)),
        100,
        BidOrAsk::Bid,
    );

    println!("Initial Orders:");
    println!("  {}", ask_order);
    println!("  {}", bid_order);

    let _ = book.add_limit_order(ask_order);
    let _ = book.add_limit_order(bid_order);

    println!("\nFormatted OrderBook Output:");
    println!("{}", book);
    
    let _ = book.add_market_order(Order::new("TCS".to_string(), None, 30, BidOrAsk::Bid));
    
    println!("\nFormatted OrderBook Output:");
    println!("{}", book);
    
}

use stock_exchange_rust::domain::{orderbook::OrderBook, price::Price};

fn main() {
    println!("Hello, world!");
    let apple_order_book = OrderBook::new("TCS".to_string(), Price::from_paisa(224500));
    println!("{:?}", apple_order_book);
}

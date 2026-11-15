use exchange::{
    core::types::{OrderType, Side, Symbol},
    matching::order_book::OrderBook,
};

fn main() {
    let appl_symbol = Symbol::from("APPL");
    let nvda_symbol: Symbol = "NVDA".into();

    let appl_view = appl_symbol.view();
    let nvda_view = nvda_symbol.view();
    println!("{1}: {0:?}", appl_symbol, appl_view);
    println!("{1}: {0:?}", nvda_symbol, nvda_view);

    //let mut order_book_appl = OrderBook::<1_000_000, 10_000, 262_144>::new(appl_symbol);
    let mut order_book_nvda = OrderBook::<1_000_000, 10_000, 262_144>::new(nvda_symbol);
    // let events = order_book_appl.add_order(
    //     1,
    //     Side::Sell,
    //     1_252_500,
    //     30,
    //     OrderType::Limit,
    //     100,
    //     1001,
    //     0,
    //     0,
    // );
    // for event in events {
    //     println!("{}", event)
    // }
    // let events = order_book_appl.add_order(
    //     2,
    //     Side::Sell,
    //     1_252_000,
    //     90,
    //     OrderType::Limit,
    //     100,
    //     1002,
    //     0,
    //     0,
    // );
    // for event in events {
    //     println!("{}", event)
    // }
    // // let events = order_book_appl.cancel_order(1, 101);
    // // for event in events {
    // //     println!("{}", event)
    // // }
    // let events = order_book_appl.add_order(
    //     3,
    //     Side::Buy,
    //     1_252_500,
    //     120,
    //     OrderType::Market,
    //     102,
    //     1003,
    //     0,
    //     0,
    // );
    // for event in events {
    //     println!("{}", event)
    // }

    for i in 5..15 {
        let events = order_book_nvda.add_order(
            i,
            Side::Sell,
            998_500 - i as i64 * 100,
            450,
            OrderType::Limit,
            101 + i,
            1000 + i as u32,
            0,
            0,
        );
        for event in events {
            println!("{}", event)
        }
    }
    let events = order_book_nvda.add_order(
        1,
        Side::Buy,
        997_500,
        4500,
        OrderType::Iceberg,
        105,
        1004,
        0,
        450,
    );
    for event in events {
        println!("{}", event)
    }
}

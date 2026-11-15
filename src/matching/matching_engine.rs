use std::collections::BTreeMap;

use crate::core::types::{
    OrderId, OrderType, ParticipantId, Price, Quantity, Side, Symbol, Timestamp,
};
use crate::matching::events::Event;
use crate::matching::order_book::OrderBook;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestAction {
    Add,
    Cancel,
    Modify,
}

#[derive(Clone, Copy, Debug)]
pub struct InboundOrder {
    pub action: RequestAction,
    pub symbol: Symbol,
    pub order_id: OrderId,
    pub side: Side,
    pub price: Price,
    pub qty: Quantity,
    pub order_type: OrderType,
    pub timestamp: Timestamp,
    pub participant_id: ParticipantId,
    pub trigger_price: Price,
    pub display_qty: Quantity,
}

impl Default for InboundOrder {
    fn default() -> Self {
        Self {
            action: RequestAction::Add,
            symbol: Symbol::new(),
            order_id: 0,
            side: Side::Buy,
            price: 0,
            qty: 0,
            order_type: OrderType::Limit,
            timestamp: 0,
            participant_id: 0,
            trigger_price: 0,
            display_qty: 0,
        }
    }
}

pub struct MatchingEngine<
    const ORDER_CAPACITY: usize = 1_000_000,
    const LEVEL_CAPACITY: usize = 10_000,
    const EVENT_CAPACITY: usize = 262_144,
> {
    books: BTreeMap<Symbol, Box<OrderBook<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY>>>,
}

impl<const ORDER_CAPACITY: usize, const LEVEL_CAPACITY: usize, const EVENT_CAPACITY: usize> Default
    for MatchingEngine<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY>
{
    fn default() -> Self {
        Self {
            books: BTreeMap::new(),
        }
    }
}

impl<const ORDER_CAPACITY: usize, const LEVEL_CAPACITY: usize, const EVENT_CAPACITY: usize>
    MatchingEngine<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY>
{
    pub fn prepare_symbols(&mut self, symbols: &[Symbol]) {
        for &symbol in symbols {
            self.book_for(symbol);
        }
    }

    pub fn process_order(&mut self, order: &InboundOrder) -> &[Event] {
        let book = self.book_for(order.symbol);
        match order.action {
            RequestAction::Add => book.add_order(
                order.order_id,
                order.side,
                order.price,
                order.qty,
                order.order_type,
                order.timestamp,
                order.participant_id,
                order.trigger_price,
                order.display_qty,
            ),
            RequestAction::Cancel => book.cancel_order(order.order_id, order.timestamp),
            RequestAction::Modify => {
                book.modify_order(order.order_id, order.qty, order.price, order.timestamp)
            }
        }
    }

    pub fn find_book(
        &self,
        symbol: &Symbol,
    ) -> Option<&OrderBook<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY>> {
        self.books.get(symbol).map(|book| &**book)
    }

    fn book_for(
        &mut self,
        symbol: Symbol,
    ) -> &mut OrderBook<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY> {
        self.books
            .entry(symbol)
            .or_insert_with(|| Box::new(OrderBook::new(symbol)))
    }
}

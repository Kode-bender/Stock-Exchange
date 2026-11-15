use crate::core::types::{
    MatchId, OrderId, OrderType, Price, Quantity, SequenceNumber, Side, Symbol, Timestamp,
    PRICE_SCALE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ReasonCode {
    None = 0,
    ZeroQuantity,
    NegativePrice,
    BookEmpty,
    DuplicateOrderId,
    OrderNotFound,
    OrderNotActive,
    FokInsufficientLiquidity,
    PostOnlyWouldCross,
    InvalidModification,
    InvalidIcebergDisplay,
    OrderPoolExhausted,
}

impl std::fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            ReasonCode::None => "NONE",
            ReasonCode::ZeroQuantity => "ZERO QUANTITY",
            ReasonCode::NegativePrice => "NEGATIVE PRICE",
            ReasonCode::BookEmpty => "BOOK EMPTY",
            ReasonCode::OrderPoolExhausted => "ORDER POOL EXHAUSTED",
            ReasonCode::OrderNotActive => "ORDER NOT ACTIVE",
            ReasonCode::DuplicateOrderId => "DUPLICATE ORDER ID",
            ReasonCode::InvalidIcebergDisplay => "INVALID ICEBERG DISPLAY",
            ReasonCode::FokInsufficientLiquidity => "FOK INSUFFICIENT LIQUIDITY",
            ReasonCode::PostOnlyWouldCross => "POSTONLY WOULD CROSS",
            ReasonCode::InvalidModification => "INVALID MODIFICATION",
            ReasonCode::OrderNotFound => "ORDER NOT FOUND",
        };

        write!(f, "{out}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderAccepted {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: Side,
    pub price: Price,
    pub qty: Quantity,
    pub order_type: OrderType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderRested {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: Side,
    pub price: Price,
    pub qty: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderRejected {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub reason_code: ReasonCode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderReduced {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub new_qty: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderPartiallyFilled {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub filled_qty: Quantity,
    pub remaining_qty: Quantity,
    pub price: Price,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderFilled {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub filled_qty: Quantity,
    pub price: Price,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderCanceled {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
    pub canceled_qty: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trade {
    pub sequence_number: SequenceNumber,
    pub timestamp: Timestamp,
    pub match_id: MatchId,
    pub symbol: Symbol,
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub price: Price,
    pub qty: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    OrderAccepted(OrderAccepted),
    OrderRested(OrderRested),
    OrderRejected(OrderRejected),
    OrderReduced(OrderReduced),
    OrderPartiallyFilled(OrderPartiallyFilled),
    OrderFilled(OrderFilled),
    OrderCanceled(OrderCanceled),
    Trade(Trade),
}

use std::fmt;
impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::OrderAccepted(e) => {
                writeln!(f, "Order Accepted:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(f, "  {:<15} {}", "Symbol:", e.symbol.view())?;
                writeln!(f, "  {:<15} {}", "Side:", e.side)?;
                writeln!(
                    f,
                    "  {:<15} {:.2}",
                    "Price:",
                    e.price as f64 / PRICE_SCALE as f64
                )?;
                writeln!(f, "  {:<15} {}", "Qty:", e.qty)?;
                writeln!(f, "  {:<15} {}", "Type:", e.order_type)
            }
            Event::OrderRested(e) => {
                writeln!(f, "Order Rested:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(f, "  {:<15} {}", "Symbol:", e.symbol.view())?;
                writeln!(f, "  {:<15} {}", "Side:", e.side)?;
                writeln!(
                    f,
                    "  {:<15} {:.2}",
                    "Price:",
                    e.price as f64 / PRICE_SCALE as f64
                )?;
                writeln!(f, "  {:<15} {}", "Qty:", e.qty)
            }
            Event::OrderRejected(e) => {
                writeln!(f, "Order Rejected:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(f, "  {:<15} {}", "Reason:", e.reason_code)
            }
            Event::OrderReduced(e) => {
                writeln!(f, "Order Reduced:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(f, "  {:<15} {}", "New Qty:", e.new_qty)
            }
            Event::OrderPartiallyFilled(e) => {
                writeln!(f, "Order Partially Filled:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(
                    f,
                    "  {:<15} {:.2}",
                    "Price:",
                    e.price as f64 / PRICE_SCALE as f64
                )?;
                writeln!(f, "  {:<15} {}", "Filled Qty:", e.filled_qty)?;
                writeln!(f, "  {:<15} {}", "Remaining Qty:", e.remaining_qty)
            }
            Event::OrderFilled(e) => {
                writeln!(f, "Order Filled:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(
                    f,
                    "  {:<15} {:.2}",
                    "Price:",
                    e.price as f64 / PRICE_SCALE as f64
                )?;
                writeln!(f, "  {:<15} {}", "Filled Qty:", e.filled_qty)
            }
            Event::OrderCanceled(e) => {
                writeln!(f, "Order Canceled:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Order ID:", e.order_id)?;
                writeln!(f, "  {:<15} {}", "Canceled Qty:", e.canceled_qty)
            }
            Event::Trade(e) => {
                writeln!(f, "Trade:")?;
                writeln!(f, "  {:<15} {}", "Sequence:", e.sequence_number)?;
                writeln!(f, "  {:<15} {}", "Timestamp:", e.timestamp)?;
                writeln!(f, "  {:<15} {}", "Match ID:", e.match_id)?;
                writeln!(f, "  {:<15} {}", "Symbol:", e.symbol.view())?;
                writeln!(f, "  {:<15} {}", "Buy Order ID:", e.buy_order_id)?;
                writeln!(f, "  {:<15} {}", "Sell Order ID:", e.sell_order_id)?;
                writeln!(
                    f,
                    "  {:<15} {:.2}",
                    "Price:",
                    e.price as f64 / PRICE_SCALE as f64
                )?;
                writeln!(f, "  {:<15} {}", "Qty:", e.qty)
            }
        }
    }
}

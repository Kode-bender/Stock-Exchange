use crate::core::types::{
    OrderId, OrderStatus, OrderType, ParticipantId, Price, Quantity, Side, Timestamp,
};

pub const NIL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Order {
    // Hot fields: everything the matching loop touches fits in the first cache line.
    pub(super) id: OrderId,
    pub(super) price: Price,
    pub(super) qty: Quantity,
    pub(super) display_qty: Quantity,
    pub(super) hidden_qty: Quantity,
    pub(super) peak_qty: Quantity,
    pub(super) prev: u32,
    pub(super) next: u32,
    pub(super) parent_level: u32,
    pub(super) side: Side,
    pub(super) order_type: OrderType,
    pub(super) status: OrderStatus,
    // Cold fields.
    pub(super) original_qty: Quantity,
    pub(super) participant_id: ParticipantId,
    pub(super) timestamp: Timestamp,
    pub(super) trigger_price: Price,
}

impl Order {
    pub fn new() -> Self {
        const {
            assert!(
                size_of::<Order>() <= 128,
                "Order should stay compact enough for cache-friendly access.",
            );
        }
        Self {
            id: 0,
            price: 0,
            qty: 0,
            display_qty: 0,
            hidden_qty: 0,
            peak_qty: 0,
            prev: NIL,
            next: NIL,
            parent_level: NIL,
            side: Side::Buy,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            original_qty: 0,
            participant_id: 0,
            timestamp: 0,
            trigger_price: 0,
        }
    }
    pub(super) fn is_stop_order(&self) -> bool {
        self.order_type == OrderType::Stop || self.order_type == OrderType::StopLimit
    }
}

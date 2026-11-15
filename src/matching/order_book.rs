use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};

use crate::core::memory_pool::MemoryPool;
use crate::core::types::{
    MatchId, OrderId, OrderStatus, OrderType, ParticipantId, Price, Quantity, SequenceNumber, Side,
    Symbol, Timestamp,
};
use crate::matching::events::{
    Event, OrderAccepted, OrderCanceled, OrderFilled, OrderPartiallyFilled, OrderReduced,
    OrderRejected, OrderRested, ReasonCode, Trade,
};
use crate::matching::level::Level;
use crate::matching::order::{Order, NIL};

#[derive(Clone, Copy, Debug)]
pub struct OrderRecord {
    pub order_handle: u32,
    pub last_status: OrderStatus,
}

impl Default for OrderRecord {
    fn default() -> Self {
        Self {
            order_handle: NIL,
            last_status: OrderStatus::New,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PriceLevelView {
    pub price: Price,
    pub qty: Quantity,
}

type StopMap = BTreeMap<(Price, u64), u32>;
/// Hasher for order_handle ids: eight consecutive ids share a cache line, while the
/// block number is multiplicatively mixed so strided ids do not collide.
#[derive(Default)]
pub struct OrderIdHasher(u64);

impl Hasher for OrderIdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    fn write_u64(&mut self, id: u64) {
        let block = (id >> 3).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = (((block >> 20) << 3) | (id & 7)) | (block & 0xFE00_0000_0000_0000);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type OrderMap = HashMap<OrderId, OrderRecord, BuildHasherDefault<OrderIdHasher>>;

pub struct OrderBook<
    const ORDER_CAPACITY: usize = 1_000_000,
    const LEVEL_CAPACITY: usize = 10_000,
    const EVENT_CAPACITY: usize = 262_144,
> {
    symbol: Symbol,

    bids: BTreeMap<Price, u32>,
    asks: BTreeMap<Price, u32>,
    stop_buys: StopMap,
    stop_sells: StopMap,
    stop_sequence: u64,
    orders: OrderMap,

    best_bid: u32,
    best_ask: u32,

    order_pool: MemoryPool<Order, ORDER_CAPACITY>,
    level_pool: MemoryPool<Level, LEVEL_CAPACITY>,

    events: Vec<Event>,

    sequence: SequenceNumber,
    next_match_id: MatchId,
    last_trade_price: Price,
}

impl<const ORDER_CAPACITY: usize, const LEVEL_CAPACITY: usize, const EVENT_CAPACITY: usize>
    OrderBook<ORDER_CAPACITY, LEVEL_CAPACITY, EVENT_CAPACITY>
{
    pub fn new(symbol: Symbol) -> Self {
        let mut orders = OrderMap::default();
        orders.reserve(ORDER_CAPACITY);

        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            stop_buys: StopMap::new(),
            stop_sells: StopMap::new(),
            stop_sequence: 0,
            orders,
            best_bid: NIL,
            best_ask: NIL,
            order_pool: MemoryPool::new(),
            level_pool: MemoryPool::new(),
            events: Vec::with_capacity(EVENT_CAPACITY),
            sequence: 0,
            next_match_id: 1,
            last_trade_price: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_order(
        &mut self,
        order_id: OrderId,
        side: Side,
        price: Price,
        qty: Quantity,
        order_type: OrderType,
        timestamp: Timestamp,
        participant_id: ParticipantId,
        trigger_price: Price,
        display_qty: Quantity,
    ) -> &[Event] {
        self.events.clear();
        if let Some(reason) = self.validate_new_order(
            order_id,
            side,
            price,
            qty,
            order_type,
            trigger_price,
            display_qty,
        ) {
            self.emit_rejected(timestamp, order_id, reason);
            return &self.events;
        }

        let peak_qty = match order_type {
            OrderType::Iceberg => display_qty,
            _ => 0,
        };
        let Some(order_handle) = self.order_pool.allocate(Order {
            id: order_id,
            side,
            price,
            qty,
            original_qty: qty,
            display_qty: qty,
            hidden_qty: 0,
            timestamp,
            order_type,
            status: OrderStatus::New,
            participant_id,
            trigger_price,
            peak_qty,
            prev: NIL,
            next: NIL,
            parent_level: NIL,
        }) else {
            self.emit_rejected(timestamp, order_id, ReasonCode::OrderPoolExhausted);
            return &self.events;
        };

        self.prepare_visible_slice(order_handle);
        self.orders.insert(
            order_id,
            OrderRecord {
                order_handle,
                last_status: OrderStatus::New,
            },
        );

        self.execute_inbound_order(order_handle, timestamp, true);
        &self.events
    }

    pub fn cancel_order(&mut self, order_id: OrderId, timestamp: Timestamp) -> &[Event] {
        self.events.clear();

        let Some(order_handle) = self.find_active_order(order_id) else {
            let reason = self.lookup_reject_reason(order_id);
            self.emit_rejected(timestamp, order_id, reason);
            return &self.events;
        };

        let canceled_qty = self.order_pool[order_handle].qty;
        self.order_pool[order_handle].status = OrderStatus::Canceled;
        self.emit_canceled(timestamp, order_id, canceled_qty);
        self.retire_order(order_handle, OrderStatus::Canceled, true);
        &self.events
    }

    pub fn modify_order(
        &mut self,
        order_id: OrderId,
        new_qty: Quantity,
        new_price: Price,
        timestamp: Timestamp,
    ) -> &[Event] {
        self.events.clear();

        let Some(order_handle) = self.find_active_order(order_id) else {
            let reason = self.lookup_reject_reason(order_id);
            self.emit_rejected(timestamp, order_id, reason);
            return &self.events;
        };

        let order = self.order_pool[order_handle];
        if order.is_stop_order() {
            self.emit_rejected(timestamp, order_id, ReasonCode::InvalidModification);
            return &self.events;
        }

        if new_qty == 0 {
            self.emit_rejected(timestamp, order_id, ReasonCode::ZeroQuantity);
            return &self.events;
        }

        if new_price < 0 {
            self.emit_rejected(timestamp, order_id, ReasonCode::NegativePrice);
            return &self.events;
        }

        if new_price == order.price && new_qty < order.qty {
            self.reduce_order_quantity(order_handle, new_qty, timestamp);
            return &self.events;
        }

        if new_price == order.price && new_qty == order.qty {
            return &self.events;
        }

        self.emit_canceled(timestamp, order_id, order.qty);
        self.retire_order(order_handle, OrderStatus::Canceled, true);

        let Some(replacement_handle) = self.order_pool.allocate(Order {
            id: order_id,
            side: order.side,
            price: new_price,
            qty: new_qty,
            original_qty: new_qty,
            display_qty: new_qty,
            hidden_qty: 0,
            timestamp,
            order_type: order.order_type,
            status: OrderStatus::New,
            participant_id: order.participant_id,
            trigger_price: order.trigger_price,
            peak_qty: order.peak_qty,
            prev: NIL,
            next: NIL,
            parent_level: NIL,
        }) else {
            self.emit_rejected(timestamp, order_id, ReasonCode::OrderPoolExhausted);
            return &self.events;
        };

        self.prepare_visible_slice(replacement_handle);
        self.orders.insert(
            order_id,
            OrderRecord {
                order_handle: replacement_handle,
                last_status: OrderStatus::New,
            },
        );

        self.execute_inbound_order(replacement_handle, timestamp, true);
        &self.events
    }

    fn opposite_best_level(&self, side: Side) -> u32 {
        match side {
            Side::Buy => self.best_ask,
            Side::Sell => self.best_bid,
        }
    }

    fn allows_resting(order_type: OrderType) -> bool {
        matches!(
            order_type,
            OrderType::Limit
                | OrderType::Gtc
                | OrderType::Iceberg
                | OrderType::PostOnly
                | OrderType::StopLimit
        )
    }

    fn would_cross(&self, side: Side, price: Price) -> bool {
        let best_opposite = self.opposite_best_level(side);
        if best_opposite == NIL {
            return false;
        }
        let best_price = self.level_pool[best_opposite].price;
        match side {
            Side::Buy => best_price <= price,
            Side::Sell => best_price >= price,
        }
    }

    fn available_liquidity(&self, side: Side, limit_price: Price) -> Quantity {
        let mut available: u64 = 0;

        if side == Side::Buy {
            for (&price, &level_handle) in &self.asks {
                if price > limit_price {
                    break;
                }
                available += u64::from(self.level_pool[level_handle].total_qty);
            }
        } else {
            for (&price, &level_handle) in self.bids.iter().rev() {
                if price < limit_price {
                    break;
                }
                available += u64::from(self.level_pool[level_handle].total_qty);
            }
        }

        available.min(u64::from(Quantity::MAX)) as Quantity
    }

    fn find_active_order(&self, order_id: OrderId) -> Option<u32> {
        self.orders
            .get(&order_id)
            .map(|record| record.order_handle)
            .filter(|&order_handle| order_handle != NIL)
    }

    fn lookup_reject_reason(&self, order_id: OrderId) -> ReasonCode {
        match self.orders.contains_key(&order_id) {
            true => ReasonCode::OrderNotActive,
            false => ReasonCode::OrderNotFound,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_new_order(
        &self,
        order_id: OrderId,
        side: Side,
        price: Price,
        qty: Quantity,
        order_type: OrderType,
        trigger_price: Price,
        display_qty: Quantity,
    ) -> Option<ReasonCode> {
        if qty == 0 {
            return Some(ReasonCode::ZeroQuantity);
        }

        if self.orders.contains_key(&order_id) {
            return Some(ReasonCode::DuplicateOrderId);
        }

        let requires_limit_price = matches!(
            order_type,
            OrderType::Limit
                | OrderType::Ioc
                | OrderType::Fok
                | OrderType::Gtc
                | OrderType::StopLimit
                | OrderType::Iceberg
                | OrderType::PostOnly
        );

        if requires_limit_price && price <= 0 {
            return Some(ReasonCode::NegativePrice);
        }

        if matches!(order_type, OrderType::Stop | OrderType::StopLimit) && trigger_price <= 0 {
            return Some(ReasonCode::NegativePrice);
        }

        if order_type == OrderType::Iceberg && (display_qty == 0 || display_qty > qty) {
            return Some(ReasonCode::InvalidIcebergDisplay);
        }

        if order_type == OrderType::Market && self.opposite_best_level(side) == NIL {
            return Some(ReasonCode::BookEmpty);
        }

        if order_type == OrderType::PostOnly && self.would_cross(side, price) {
            return Some(ReasonCode::PostOnlyWouldCross);
        }

        if order_type == OrderType::Fok && self.available_liquidity(side, price) < qty {
            return Some(ReasonCode::FokInsufficientLiquidity);
        }

        None
    }

    fn replenish_iceberg(&mut self, order_handle: u32, event_timestamp: Timestamp) {
        let level_handle = self.order_pool[order_handle].parent_level;
        if level_handle == NIL {
            return;
        }

        self.level_pool[level_handle].remove_order(&mut self.order_pool, order_handle);
        {
            let entry = &mut self.order_pool[order_handle];
            entry.timestamp = event_timestamp;
            let replenished = entry.hidden_qty.min(entry.peak_qty);
            entry.hidden_qty -= replenished;
            entry.display_qty = replenished;
        }
        self.level_pool[level_handle].add_order(level_handle, &mut self.order_pool, order_handle);
        self.emit_rested(event_timestamp, order_handle);
    }

    fn match_fifo_at_level(
        &mut self,
        aggressor_handle: u32,
        level_handle: u32,
        event_timestamp: Timestamp,
        last_fill_price: &mut Price,
    ) {
        while self.order_pool[aggressor_handle].qty > 0 && !self.level_pool[level_handle].is_empty()
        {
            let passive_handle = self.level_pool[level_handle].front();

            let (display_qty, hidden_qty) = {
                let entry = &self.order_pool[passive_handle];
                (entry.display_qty, entry.hidden_qty)
            };

            if display_qty == 0 {
                if hidden_qty > 0 {
                    self.replenish_iceberg(passive_handle, event_timestamp);
                    continue;
                }
                break;
            }

            let trade_qty = self.order_pool[aggressor_handle].qty.min(display_qty);
            self.execute_trade_slice(
                aggressor_handle,
                passive_handle,
                trade_qty,
                event_timestamp,
                last_fill_price,
            );
        }
    }

    fn refresh_best_level(&mut self, side: Side) {
        if side == Side::Buy {
            self.best_bid = self
                .bids
                .last_key_value()
                .map_or(NIL, |(_, &level_handle)| level_handle);
        } else {
            self.best_ask = self
                .asks
                .first_key_value()
                .map_or(NIL, |(_, &level_handle)| level_handle);
        }
    }

    fn insert_order(&mut self, order_handle: u32) {
        let (side, price) = {
            let entry = &self.order_pool[order_handle];
            (entry.side, entry.price)
        };

        let level_handle = match side {
            Side::Buy => Self::find_or_create_level(&mut self.bids, &mut self.level_pool, price),
            Side::Sell => Self::find_or_create_level(&mut self.asks, &mut self.level_pool, price),
        };
        let Some(level_handle) = level_handle else {
            std::process::abort();
        };

        self.level_pool[level_handle].add_order(level_handle, &mut self.order_pool, order_handle);

        if side == Side::Buy {
            if self.best_bid == NIL || price > self.level_pool[self.best_bid].price {
                self.best_bid = level_handle;
            }
        } else if self.best_ask == NIL || price < self.level_pool[self.best_ask].price {
            self.best_ask = level_handle;
        }
    }

    fn find_or_create_level(
        levels: &mut BTreeMap<Price, u32>,
        level_pool: &mut MemoryPool<Level, LEVEL_CAPACITY>,
        price: Price,
    ) -> Option<u32> {
        match levels.entry(price) {
            Entry::Occupied(entry) => Some(*entry.get()),
            Entry::Vacant(entry) => {
                let level_handle = level_pool.allocate(Level {
                    price,
                    total_qty: 0,
                    order_count: 0,
                    head: NIL,
                    tail: NIL,
                })?;
                entry.insert(level_handle);
                Some(level_handle)
            }
        }
    }

    fn retire_order(
        &mut self,
        order_handle: u32,
        terminal_status: OrderStatus,
        refresh_best: bool,
    ) {
        let (id, side, parent_level, is_stop) = {
            let entry = &self.order_pool[order_handle];
            (
                entry.id,
                entry.side,
                entry.parent_level,
                entry.is_stop_order(),
            )
        };

        let mut removed_best_level = false;

        if parent_level != NIL {
            self.level_pool[parent_level].remove_order(&mut self.order_pool, order_handle);
            if self.level_pool[parent_level].is_empty() {
                removed_best_level = match side {
                    Side::Buy => self.best_bid == parent_level,
                    Side::Sell => self.best_ask == parent_level,
                };
                let price = self.level_pool[parent_level].price;
                self.deactivate_level(side, price, parent_level);
            }
        } else if is_stop {
            self.erase_stop_reference(order_handle);
        }

        self.orders.insert(
            id,
            OrderRecord {
                order_handle: NIL,
                last_status: terminal_status,
            },
        );

        self.order_pool.deallocate(order_handle);
        if refresh_best && removed_best_level {
            self.refresh_best_level(side);
        }
    }

    fn deactivate_level(&mut self, side: Side, price: Price, level_handle: u32) {
        if side == Side::Buy {
            self.bids.remove(&price);
        } else {
            self.asks.remove(&price);
        }
        self.level_pool.deallocate(level_handle);
    }

    fn execute_trade_slice(
        &mut self,
        aggressor_handle: u32,
        passive_handle: u32,
        trade_qty: Quantity,
        event_timestamp: Timestamp,
        last_fill_price: &mut Price,
    ) {
        let trade_price = self.order_pool[passive_handle].price;
        *last_fill_price = trade_price;
        self.last_trade_price = trade_price;

        self.order_pool[aggressor_handle].qty -= trade_qty;
        let (
            passive_handle_level,
            passive_handle_qty,
            passive_handle_display_qty,
            passive_handle_hidden_qty,
        ) = {
            let entry = &mut self.order_pool[passive_handle];
            entry.qty -= trade_qty;
            entry.display_qty -= trade_qty;
            (
                entry.parent_level,
                entry.qty,
                entry.display_qty,
                entry.hidden_qty,
            )
        };
        self.level_pool[passive_handle_level].total_qty -= trade_qty;

        self.emit_trade(
            event_timestamp,
            aggressor_handle,
            passive_handle,
            trade_price,
            trade_qty,
        );
        self.emit_passive_handle_fill_state(
            event_timestamp,
            passive_handle,
            trade_qty,
            trade_price,
        );

        if passive_handle_qty == 0 {
            self.retire_order(passive_handle, OrderStatus::Filled, false);
        } else if passive_handle_display_qty == 0 && passive_handle_hidden_qty > 0 {
            self.replenish_iceberg(passive_handle, event_timestamp);
        } else {
            self.order_pool[passive_handle].status = OrderStatus::PartiallyFilled;
        }
    }

    fn is_marketable(aggressor: &Order, resting_price: Price) -> bool {
        if aggressor.order_type == OrderType::Market {
            return true;
        }

        match aggressor.side {
            Side::Buy => aggressor.price >= resting_price,
            Side::Sell => aggressor.price <= resting_price,
        }
    }

    fn trigger_stop_orders(&mut self, event_timestamp: Timestamp) {
        loop {
            let mut triggered_any = false;

            while let Some(entry) = self.stop_buys.first_entry() {
                if entry.key().0 > self.last_trade_price {
                    break;
                }
                let order_handle = entry.remove();
                self.activate_stop_order(order_handle, event_timestamp);
                triggered_any = true;
            }

            loop {
                let key = self
                    .stop_sells
                    .range((self.last_trade_price, 0)..)
                    .next()
                    .map(|(&key, _)| key);
                let Some(key) = key else {
                    break;
                };
                let Some(order_handle) = self.stop_sells.remove(&key) else {
                    break;
                };
                self.activate_stop_order(order_handle, event_timestamp);
                triggered_any = true;
            }

            if !triggered_any {
                break;
            }
        }
    }

    fn activate_stop_order(&mut self, order_handle: u32, event_timestamp: Timestamp) {
        {
            let entry = &mut self.order_pool[order_handle];
            entry.timestamp = event_timestamp;
            if entry.order_type == OrderType::Stop {
                entry.order_type = OrderType::Market;
                entry.price = 0;
            } else {
                entry.order_type = OrderType::Limit;
            }
        }

        self.execute_inbound_order(order_handle, event_timestamp, false);
    }

    fn match_against_levels(
        &mut self,
        aggressor_handle: u32,
        resting_side: Side,
        event_timestamp: Timestamp,
        last_fill_price: &mut Price,
    ) {
        while self.order_pool[aggressor_handle].qty > 0 {
            let best = match resting_side {
                Side::Sell => self.asks.first_key_value(),
                Side::Buy => self.bids.last_key_value(),
            };
            let Some((_, &level_handle)) = best else {
                break;
            };

            if !Self::is_marketable(
                &self.order_pool[aggressor_handle],
                self.level_pool[level_handle].price,
            ) {
                break;
            }

            while self.order_pool[aggressor_handle].qty > 0
                && !self.level_pool[level_handle].is_empty()
            {
                self.match_fifo_at_level(
                    aggressor_handle,
                    level_handle,
                    event_timestamp,
                    last_fill_price,
                );
            }
        }

        self.refresh_best_level(resting_side);
        if *last_fill_price != 0 {
            self.trigger_stop_orders(event_timestamp);
        }
    }

    fn match_order(
        &mut self,
        aggressor_handle: u32,
        event_timestamp: Timestamp,
        last_fill_price: &mut Price,
    ) {
        let side = self.order_pool[aggressor_handle].side;
        let best_opposite = self.opposite_best_level(side);
        if best_opposite == NIL
            || !Self::is_marketable(
                &self.order_pool[aggressor_handle],
                self.level_pool[best_opposite].price,
            )
        {
            return;
        }

        if side == Side::Buy {
            self.match_against_levels(
                aggressor_handle,
                Side::Sell,
                event_timestamp,
                last_fill_price,
            );
        } else {
            self.match_against_levels(
                aggressor_handle,
                Side::Buy,
                event_timestamp,
                last_fill_price,
            );
        }
    }

    fn execute_inbound_order(
        &mut self,
        order_handle: u32,
        event_timestamp: Timestamp,
        emit_accept_event: bool,
    ) {
        if emit_accept_event {
            self.order_pool[order_handle].status = OrderStatus::Accepted;
            self.emit_accepted(event_timestamp, order_handle);
        }

        if self.order_pool[order_handle].is_stop_order() {
            self.park_stop_order(order_handle);
            return;
        }

        let original_qty = self.order_pool[order_handle].qty;
        let mut last_fill_price: Price = 0;
        self.match_order(order_handle, event_timestamp, &mut last_fill_price);

        let (remaining_qty, order_id, order_type) = {
            let entry = &self.order_pool[order_handle];
            (entry.qty, entry.id, entry.order_type)
        };

        let total_filled = original_qty - remaining_qty;
        if total_filled > 0 {
            self.emit_aggressor_handle_fill_state(
                event_timestamp,
                order_handle,
                total_filled,
                last_fill_price,
            );
        }

        if remaining_qty == 0 {
            self.retire_order(order_handle, OrderStatus::Filled, true);
            return;
        }

        if Self::allows_resting(order_type) {
            self.prepare_visible_slice(order_handle);
            self.insert_order(order_handle);
            self.emit_rested(event_timestamp, order_handle);
            self.order_pool[order_handle].status = match total_filled > 0 {
                true => OrderStatus::PartiallyFilled,
                false => OrderStatus::Accepted,
            };
            return;
        }

        self.emit_canceled(event_timestamp, order_id, remaining_qty);
        self.retire_order(order_handle, OrderStatus::Canceled, true);
    }

    fn prepare_visible_slice(&mut self, order_handle: u32) {
        let entry = &mut self.order_pool[order_handle];
        if entry.order_type != OrderType::Iceberg {
            entry.display_qty = entry.qty;
            entry.hidden_qty = 0;
            return;
        }

        entry.display_qty = entry.qty.min(entry.peak_qty);
        entry.hidden_qty = entry.qty - entry.display_qty;
    }

    fn erase_stop_reference(&mut self, order_handle: u32) {
        let (side, trigger_price) = {
            let entry = &self.order_pool[order_handle];
            (entry.side, entry.trigger_price)
        };
        let stops = match side {
            Side::Buy => &mut self.stop_buys,
            Side::Sell => &mut self.stop_sells,
        };

        let key = stops
            .range((trigger_price, 0)..=(trigger_price, u64::MAX))
            .find(|&(_, &candidate)| candidate == order_handle)
            .map(|(&key, _)| key);
        if let Some(key) = key {
            stops.remove(&key);
        }
    }

    fn park_stop_order(&mut self, order_handle: u32) {
        let (side, trigger_price) = {
            let entry = &self.order_pool[order_handle];
            (entry.side, entry.trigger_price)
        };
        let key = (trigger_price, self.stop_sequence);
        self.stop_sequence += 1;

        if side == Side::Buy {
            self.stop_buys.insert(key, order_handle);
        } else {
            self.stop_sells.insert(key, order_handle);
        }
    }

    fn reduce_order_quantity(
        &mut self,
        order_handle: u32,
        new_qty: Quantity,
        event_timestamp: Timestamp,
    ) {
        let (id, parent_level, old_display_qty, new_display_qty, level_reduction) = {
            let entry = &mut self.order_pool[order_handle];
            let old_display_qty = entry.display_qty;
            let reduction = entry.qty - new_qty;
            entry.qty = new_qty;

            let level_reduction;
            if entry.order_type == OrderType::Iceberg {
                let mut remaining_reduction = reduction;
                let hidden_reduction = remaining_reduction.min(entry.hidden_qty);
                entry.hidden_qty -= hidden_reduction;
                remaining_reduction -= hidden_reduction;

                level_reduction = remaining_reduction;
                if remaining_reduction > 0 {
                    entry.display_qty -= remaining_reduction;
                }
            } else {
                entry.display_qty = new_qty;
                level_reduction = reduction;
            }

            (
                entry.id,
                entry.parent_level,
                old_display_qty,
                entry.display_qty,
                level_reduction,
            )
        };

        if parent_level != NIL {
            self.level_pool[parent_level].total_qty -= level_reduction;
        }

        if parent_level != NIL && new_display_qty != old_display_qty {
            self.emit_reduced(event_timestamp, id, new_display_qty);
        }
    }

    fn push_event(&mut self, event: Event) {
        if self.events.len() >= EVENT_CAPACITY {
            std::process::abort();
        }
        self.events.push(event);
    }

    fn next_sequence(&mut self) -> SequenceNumber {
        self.sequence += 1;
        self.sequence
    }

    fn emit_reduced(&mut self, timestamp: Timestamp, order_id: OrderId, new_qty: Quantity) {
        let sequence_number = self.next_sequence();
        self.push_event(Event::OrderReduced(OrderReduced {
            sequence_number,
            timestamp,
            order_id,
            new_qty,
        }));
    }

    fn emit_rested(&mut self, timestamp: Timestamp, order_handle: u32) {
        let sequence_number = self.next_sequence();
        let entry = &self.order_pool[order_handle];
        let event = Event::OrderRested(OrderRested {
            sequence_number,
            timestamp,
            order_id: entry.id,
            symbol: self.symbol,
            side: entry.side,
            price: entry.price,
            qty: entry.display_qty,
        });
        self.push_event(event);
    }

    fn emit_trade(
        &mut self,
        timestamp: Timestamp,
        aggressor_handle: u32,
        passive_handle: u32,
        price: Price,
        qty: Quantity,
    ) {
        let (aggressor_handle_id, aggressor_handle_side) = {
            let entry = &self.order_pool[aggressor_handle];
            (entry.id, entry.side)
        };
        let passive_handle_id = self.order_pool[passive_handle].id;
        let (buy_order_id, sell_order_id) = match aggressor_handle_side {
            Side::Buy => (aggressor_handle_id, passive_handle_id),
            Side::Sell => (passive_handle_id, aggressor_handle_id),
        };

        let sequence_number = self.next_sequence();
        let match_id = self.next_match_id;
        self.next_match_id += 1;

        self.push_event(Event::Trade(Trade {
            sequence_number,
            timestamp,
            match_id,
            symbol: self.symbol,
            buy_order_id,
            sell_order_id,
            price,
            qty,
        }));
    }

    fn emit_passive_handle_fill_state(
        &mut self,
        timestamp: Timestamp,
        passive_handle: u32,
        filled_qty: Quantity,
        price: Price,
    ) {
        let (order_id, remaining_qty) = {
            let entry = &self.order_pool[passive_handle];
            (entry.id, entry.qty)
        };
        let sequence_number = self.next_sequence();

        if remaining_qty == 0 {
            self.push_event(Event::OrderFilled(OrderFilled {
                sequence_number,
                timestamp,
                order_id,
                filled_qty,
                price,
            }));
            return;
        }

        self.push_event(Event::OrderPartiallyFilled(OrderPartiallyFilled {
            sequence_number,
            timestamp,
            order_id,
            filled_qty,
            remaining_qty,
            price,
        }));
    }

    fn emit_canceled(&mut self, timestamp: Timestamp, order_id: OrderId, canceled_qty: Quantity) {
        let sequence_number = self.next_sequence();
        self.push_event(Event::OrderCanceled(OrderCanceled {
            sequence_number,
            timestamp,
            order_id,
            canceled_qty,
        }));
    }

    fn emit_accepted(&mut self, timestamp: Timestamp, order_handle: u32) {
        let sequence_number = self.next_sequence();
        let entry = &self.order_pool[order_handle];
        let event = Event::OrderAccepted(OrderAccepted {
            sequence_number,
            timestamp,
            order_id: entry.id,
            symbol: self.symbol,
            side: entry.side,
            price: match entry.order_type {
                OrderType::Stop => entry.trigger_price,
                _ => entry.price,
            },
            qty: entry.qty,
            order_type: entry.order_type,
        });
        self.push_event(event);
    }

    fn emit_aggressor_handle_fill_state(
        &mut self,
        timestamp: Timestamp,
        aggressor_handle: u32,
        total_filled: Quantity,
        last_fill_price: Price,
    ) {
        let (order_id, remaining_qty) = {
            let entry = &self.order_pool[aggressor_handle];
            (entry.id, entry.qty)
        };
        let sequence_number = self.next_sequence();

        if remaining_qty == 0 {
            self.push_event(Event::OrderFilled(OrderFilled {
                sequence_number,
                timestamp,
                order_id,
                filled_qty: total_filled,
                price: last_fill_price,
            }));
            return;
        }

        self.push_event(Event::OrderPartiallyFilled(OrderPartiallyFilled {
            sequence_number,
            timestamp,
            order_id,
            filled_qty: total_filled,
            remaining_qty,
            price: last_fill_price,
        }));
    }

    fn emit_rejected(&mut self, timestamp: Timestamp, order_id: OrderId, reason: ReasonCode) {
        let sequence_number = self.next_sequence();
        self.push_event(Event::OrderRejected(OrderRejected {
            sequence_number,
            timestamp,
            order_id,
            reason_code: reason,
        }));
    }
}

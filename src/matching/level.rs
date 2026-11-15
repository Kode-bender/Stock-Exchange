use std::ops::IndexMut;

use crate::core::types::{Price, Quantity};
use crate::matching::order::{Order, NIL};

#[derive(Clone, Copy, Debug)]
#[repr(C, align(32))]
pub struct Level {
    pub price: Price,
    pub total_qty: Quantity,
    pub order_count: u32,
    pub head: u32,
    pub tail: u32,
}

impl Level {
    pub fn new() -> Self {
        Self {
            price: 0,
            total_qty: 0,
            order_count: 0,
            head: NIL,
            tail: NIL,
        }
    }
    pub fn add_order(
        &mut self,
        level_index: u32,
        orders: &mut impl IndexMut<u32, Output = Order>,
        order: u32,
    ) {
        let tail = self.tail;
        let display_qty = {
            let entry = &mut orders[order];
            entry.prev = tail;
            entry.next = NIL;
            entry.parent_level = level_index;
            entry.display_qty
        };

        if tail != NIL {
            orders[tail].next = order;
        } else {
            self.head = order;
        }

        self.tail = order;
        self.total_qty += display_qty;
        self.order_count += 1;
    }

    pub fn remove_order(&mut self, orders: &mut impl IndexMut<u32, Output = Order>, order: u32) {
        let (prev, next, display_qty) = {
            let entry = &orders[order];
            (entry.prev, entry.next, entry.display_qty)
        };

        if prev != NIL {
            orders[prev].next = next;
        } else {
            self.head = next;
        }

        if next != NIL {
            orders[next].prev = prev;
        } else {
            self.tail = prev;
        }

        self.total_qty -= display_qty;
        self.order_count -= 1;

        let entry = &mut orders[order];
        entry.prev = NIL;
        entry.next = NIL;
        entry.parent_level = NIL;
    }

    pub fn front(&self) -> u32 {
        self.head
    }

    pub fn is_empty(&self) -> bool {
        self.head == NIL
    }
}

use crate::rust_decimal::prelude::FromPrimitive;
use crate::rust_decimal::prelude::ToPrimitive;
use crate::rust_decimal::prelude::Zero;
use rust_decimal::Decimal;
use skiplist::SkipList;
use std::ops::Mul;
use std::vec::Vec;

use common::bitmap::bitmap;

#[macro_use]
extern crate lazy_static;
extern crate rust_decimal;

const MAX_u64: u64 = 0xffffffff; // 2^64
const MAX_len: usize = 0x100000; // 64
const NEW_bit: u64 = 0x1; // 1
const MAX_ORDER_NUM: usize = 4096;
const MAX_BITMAP: usize = 4096 / 8;

lazy_static! {
    static ref DECIMAL_1E8: rust_decimal::Decimal = rust_decimal::Decimal::new(10000000, 0);
}

#[derive(Copy, Clone)]
enum order_op {
    Limit,
    Market,
    Cancel,
}

#[derive(Copy, Clone)]
enum order_side {
    BUY,
    SELL,
}

#[derive(Copy, Clone)]
enum order_status {
    ALL = 0,
    PART = 1,
    DONE = 2,
}

#[derive(Copy, Clone)]
pub struct order {
    id: u64,          // order id
    qty: Decimal,     // order quantity
    op: order_op,     // order opreation
    side: order_side, // order side
    price: Decimal,   // order price
    status: i8,       // order status
    pre: usize,
    next: usize,
}

impl order {
    //{{{
    fn check(&self) -> bool {
        if self.id < 0 || self.price.is_zero() || self.qty.is_zero() {
            return false;
        }
        return true;
    }
} //}}}

impl Default for order {
    //{{{
    fn default() -> order {
        order {
            pre: 0,
            next: 0,
            id: 0u64,
            qty: Decimal::new(0, 0),
            op: order_op::Limit,
            side: order_side::BUY,
            price: Decimal::new(0, 0),
            status: 0i8,
        }
    }
} //}}}

#[derive(Copy, Clone)]
pub struct trade_record {
    trade_id: i64,
    qty: Decimal,
}

impl Default for trade_record {
    fn default() -> trade_record {
        trade_record {
            trade_id: -1,
            qty: Decimal::default(),
        }
    }
}

pub struct order_book {
    buy_leader: price_node,
    sell_leader: price_node,
    buy_price_leader: Decimal,
    sell_price_leader: Decimal,
    order_bitmap: bitmap, // bitmap index of order
    orders: Vec<order>,
    price_node: Vec<price_node>, // bitmax index of price_node
    price_node_bitmap: bitmap,
    buy_price_skiplist: SkipList<usize>, // price_node of buy skiplist index
    sell_price_skiplist: SkipList<usize>, // price_node of sell skiplist index
    decimal_1e8: Decimal,
}

impl order_book {
    fn entry(&mut self, ord: order) -> Option<trade_record> {
        {
            {
                {
                    match ord.op {
                        order_op::Limit => self.limit_match_order(ord),
                        //order_op::Market => self.market_match_order(ord),
                        //order_op::Cancel => self.cancel_order(ord),
                        default => None,
                    }
                }
            }
        }
    }

    fn limit_match_order(&mut self, ord: order) -> Option<trade_record> {
        // match the order
        match ord.side {
            order_side::BUY => {
                if ord.price < self.sell_price_leader {
                    // insert this order to orders list
                    let slot = self.order_bitmap.find_unset();
                    self.orders[slot] = ord;

                    let key = ord.price * self.decimal_1e8;
                    let v = self.buy_price_skiplist.get(key.to_usize().unwrap());
                    let price_slot = match v {
                        None => -1i32,
                        Some(s) => *s as i32,
                    };
                    if price_slot == -1 {
                        // new price node
                        let new_price_node = price_node {
                            curr: slot as i32,
                            pre: -1i32,
                            next: -1i32,
                            qty: ord.qty,
                            price: ord.price,
                            first: slot,
                            last: slot,
                            num: 1i32,
                            used: true,
                        };
                        let slot = self.price_node_bitmap.find_unset();
                        self.price_node[slot] = new_price_node;
                        self.buy_price_skiplist
                            .insert(slot, key.to_usize().unwrap());
                        return None;
                    // insert price node
                    } else {
                        //
                        let last = self.price_node[price_slot as usize].last;
                        self.orders[last].next = slot;
                        self.orders[slot].pre = last;
                        self.price_node[price_slot as usize].num += 1;
                        self.price_node[price_slot as usize].qty += ord.qty;
                        self.price_node[price_slot as usize].last = slot;
                        return None;
                    }
                } else {
                    // start matching
                    //let node = self.buy_price_skiplist
                }
            }
            order_side::SELL => {}
        }
        None
    }

    fn market_match_order(&mut self, ord: order) -> (trade_record, bool) {
        {
            {
                {
                    match ord.side {
                        order_side::BUY => {}
                        order_side::SELL => {}
                    }
                    (trade_record::default(), false)
                }
            }
        }
    }

    fn cancel_order(&mut self, ord: order) -> (trade_record, bool) {
        {
            {
                {
                    (trade_record::default(), false)
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct price_node {
    curr: i32,      // curr node slot
    pre: i32,       // pre node slot
    next: i32,      // next node slot
    qty: Decimal,   // curr node order qty
    price: Decimal, // curr node price
    first: usize,   // first order slot
    last: usize,    // last order slot
    num: i32,
    used: bool,
}

// struct bitmap {
//     //{{{
//     vector: Vec<u64>,
// } //}}}

// impl Default for bitmap {
//     //{{{
//     #[inline]
//     fn default() -> bitmap {
//         bitmap { vector: vec![0] }
//     }
// } //}}}

// impl bitmap {
//     //{{{
//     fn find_unset(&mut self) -> usize {
//         if self.vector.len() == 0 {
//             self.vector.push(NEW_bit);
//             return 0;
//         }

//         for i in 0..self.vector.len() {
//             if self.vector[i] == MAX_u64 {
//                 continue;
//             }

//             for j in 0..MAX_len {
//                 if (self.vector[0 as usize] & (1 << j)) == 0 {
//                     self.vector[i as usize] |= 1 << j;
//                     return (i * MAX_len + j);
//                 }
//             }
//         }
//         self.vector.push(NEW_bit);

//         self.vector.len() * MAX_len + 1
//     }

//     fn clear(&mut self, slot: usize) {
//         if slot + 1 > (self.vector.len() * MAX_len) {
//             return;
//         }
//         let i = (slot / MAX_len) as usize;
//         let j = (slot % MAX_len) as usize;

//         self.vector[i] ^= (1 << j);
//     }
// } //}}}

macro_rules! println_bit {
    ($p:expr) => {
        println!("{:#018b}", $p)
    };
}

#[test]
fn bitmap() {
    let mut bm = bitmap::default();
    println!("{}", &mut bm.find_unset());
    println!("{}", &mut bm.find_unset());
    println_bit!(bm.vector[0]);
    bm.clear(0);
    println_bit!(bm.vector[0]);
    println!("{}", &mut bm.find_unset());
    println_bit!(bm.vector[0]);
}

#![feature(map_first_last)]
use chrono::offset::LocalResult;
use chrono::prelude::*;
use common::bitmap::BitMap;
use crossbeam_channel::unbounded;
use libc::fsync;
use order::proto::{OrderInfo, OrderOp, OrderSide, OrderStatus};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::*;
use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::thread;

#[macro_use]
extern crate smart_default;

#[derive(Copy, Clone, Default, Debug, Serialize, Deserialize)]
struct PriceNode {
    qty: Decimal,      // curr node order qty
    price: Decimal,    // curr node price
    order_slot: usize, // curr node order number
    last_slot: usize,  // last order slot
}

#[derive(Copy, Clone, Debug, PartialEq, SmartDefault, Serialize, Deserialize)]
pub enum Signal {
    #[default]
    Closed,
    CancelAllOrder,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBook {
    market: String,        // curr orderbook market ID
    bid_leader: PriceNode, // the best buy price_node
    ask_leader: PriceNode, // the best sell price_node

    orders: Vec<OrderInfo>, // store order array
    order_bitmap: BitMap,   // bitmap index of order

    bid_price_index: BTreeMap<Decimal, PriceNode>, // price_node of buy skiplist index
    ask_price_index: BTreeMap<Decimal, PriceNode>, // price_node of sell skiplist index

                                                   // #[serde(skip_serializing)]
                                                   //    order_chan: Receiver<Result<OrderInfo>>,
}

pub enum Msg {
    SimpleOrder(OrderInfo),           // new order
    CancelOrder((u64, u64, Decimal)), // cancel order operation
    CancelAllOrder,                   // cancel all order
    Snapshot,                         // start snapshot signal
}

impl OrderBook {
    pub fn new(max_order_num: usize, market: String) -> OrderBook {
        //{{{
        let orders: Vec<OrderInfo> = vec![OrderInfo::default(); max_order_num];
        OrderBook {
            market: market,
            bid_leader: Default::default(),
            ask_leader: Default::default(),
            orders: orders,
            order_bitmap: BitMap::new(max_order_num),
            bid_price_index: BTreeMap::new(),
            ask_price_index: BTreeMap::new(),
        }
    } //}}}

    pub fn run(self, recv: crossbeam_channel::Receiver<Msg>) {
        //{{{
        thread::spawn(move || loop {
            match recv.recv() {
                Ok(msg) => match msg {
                    Msg::SimpleOrder(o) => {
                        println!("{:?}", o);
                    }

                    Msg::CancelOrder((order_id, uid, price)) => {
                        println!("{}->{}->{}", order_id, uid, price);
                    }

                    Msg::Snapshot => self.snapshot(),

                    Msg::CancelAllOrder => {}
                },
                Err(err) => println!("{:?}", err),
            }
        })
        .join()
        .unwrap();
        return;
    } //}}}

    // orderbook match entry
    pub fn match_entry(&mut self, order: &mut OrderInfo) {
        //{{{
        match order.op {
            OrderOp::Limit => self.limit_match(order),

            OrderOp::Market => self.market_match(order),

            OrderOp::Cancel => {
                self.cancel(order);
            }
        }
    } //}}}

    /// limit price match
    /// If there are remainning parts after the order is matched. match engine will insert this part into the order book
    fn limit_match(&mut self, taker: &mut OrderInfo) {
        //{{{
        assert_eq!(taker.op, OrderOp::Limit);

        match taker.side {
            OrderSide::Ask => {
                //{{{
                // there is no suitable bid order
                if self.bid_leader.qty.is_zero()
                    || self.bid_leader.price.is_zero()
                    || self.bid_leader.price < taker.price
                {
                    self.insert_order(taker);
                    return;
                }

                self.bid_leader.qty -= taker.remain_qty;

                if !self.bid_leader.qty.is_zero() && self.bid_leader.qty.is_sign_positive() {
                    // After match. bid leader still remain qty
                    let mut maker_slot: usize;
                    loop {
                        if taker.remain_qty.is_zero() {
                            break;
                        }
                        maker_slot = self.bid_leader.order_slot;
                        if self.orders[maker_slot].remain_qty > taker.remain_qty {
                            self.orders[maker_slot].trade(taker);
                            break;
                        } else {
                            let record = self.orders[maker_slot].trade(taker).unwrap();
                            self.order_bitmap.clear(&maker_slot);

                            let next = self.orders[maker_slot].logic.next_slot;
                            self.bid_leader.order_slot = next;
                            self.orders[next].logic.pre_slot = 0;
                        }
                    }

                    // replace price node index
                    self.bid_price_index
                        .insert(self.bid_leader.price, self.bid_leader.clone());
                } else {
                    let mut remove_node_key: Decimal = dec!(0);

                    loop {
                        //{{{
                        if !remove_node_key.is_zero() {
                            self.bid_price_index.remove(&remove_node_key);
                            remove_node_key = dec!(0);
                        }

                        if taker.remain_qty.is_zero() {
                            break;
                        }
                        match self.bid_price_index.last_entry() {
                            Some(mut entry) => {
                                let price = *entry.key();
                                if price < taker.price {
                                    if taker.remain_qty.is_sign_positive() {
                                        self.insert_order(taker);
                                        return;
                                    }
                                }

                                let node = entry.get_mut();
                                node.qty -= taker.remain_qty;

                                if node.qty.is_zero() || !node.qty.is_sign_positive() {
                                    remove_node_key = node.price;
                                }

                                let mut order_slot: usize;

                                loop {
                                    if taker.remain_qty.is_zero() {
                                        break;
                                    }

                                    order_slot = node.order_slot;
                                    if order_slot == 0 {
                                        break;
                                    }

                                    assert_eq!(self.orders[order_slot].logic.used, true);

                                    if self.orders[order_slot].remain_qty > taker.remain_qty {
                                        self.orders[order_slot].trade(taker);
                                        break;
                                    } else {
                                        self.orders[order_slot].logic.used = false;
                                        self.order_bitmap.clear(&order_slot);

                                        self.orders[order_slot].trade(taker);
                                        let next = self.orders[order_slot].logic.next_slot;

                                        if next != 0 || order_slot == next {
                                            node.order_slot = next;
                                            self.orders[next].logic.pre_slot = 0;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                            None => {
                                if !taker.remain_qty.is_zero()
                                    && taker.remain_qty.is_sign_positive()
                                {
                                    self.insert_order(taker);
                                }
                                return;
                            }
                        }
                    } //}}}
                    match self.bid_price_index.last_key_value() {
                        Some((_, price_node)) => self.bid_leader = *price_node,
                        None => self.bid_leader = PriceNode::default(),
                    }
                }
            } //}}}
            OrderSide::Bid => {
                //{{{
                if self.ask_leader.qty.is_zero()
                    || self.ask_leader.price.is_zero()
                    || self.ask_leader.price > taker.price
                {
                    self.insert_order(taker);
                    return;
                }

                self.ask_leader.qty -= taker.remain_qty;

                if !self.ask_leader.qty.is_zero() && self.ask_leader.qty.is_sign_positive() {
                    // After match . ask leader still remain qty
                    let mut maker_slot: usize;

                    loop {
                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        maker_slot = self.ask_leader.order_slot;
                        if self.orders[maker_slot].remain_qty > taker.remain_qty {
                            self.orders[maker_slot].trade(taker);
                            break;
                        } else {
                            self.orders[maker_slot].trade(taker);
                            self.order_bitmap.clear(&maker_slot);

                            let next = self.orders[maker_slot].logic.next_slot;
                            assert!(next != 0);

                            self.ask_leader.order_slot = next;
                            self.orders[next].logic.pre_slot = 0;
                        }
                    }
                    self.ask_price_index
                        .insert(self.bid_leader.price, self.bid_leader.clone());
                } else {
                    let mut remove_node_key: Decimal = dec!(0);
                    loop {
                        if !remove_node_key.is_zero() {
                            self.ask_price_index.remove(&remove_node_key);
                        }

                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        match self.ask_price_index.first_entry() {
                            Some(mut entry) => {
                                let price = *entry.key();
                                let node = entry.get_mut();

                                if price > taker.price {
                                    //{{{
                                    if taker.remain_qty.is_sign_positive() {
                                        self.insert_order(taker);
                                        break;
                                    }
                                } //}}}

                                node.qty -= taker.remain_qty;

                                if node.qty.is_zero() || !node.qty.is_sign_positive() {
                                    remove_node_key = node.price;
                                }
                                let mut order_slot: usize;
                                loop {
                                    if taker.remain_qty.is_zero() {
                                        break;
                                    }

                                    order_slot = node.order_slot;
                                    if order_slot == 0 {
                                        break;
                                    }

                                    assert_eq!(self.orders[order_slot].logic.used, true);

                                    if self.orders[order_slot].remain_qty > taker.remain_qty {
                                        self.orders[order_slot].trade(taker);
                                    } else {
                                        self.orders[order_slot].logic.used = false;
                                        self.order_bitmap.clear(&order_slot);

                                        self.orders[order_slot].trade(taker);
                                        let next = self.orders[order_slot].logic.next_slot;

                                        if next != 0 || next == order_slot {
                                            node.order_slot = next;
                                            self.orders[next].logic.pre_slot = 0;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                            None => {
                                if !taker.remain_qty.is_zero()
                                    && taker.remain_qty.is_sign_positive()
                                {
                                    self.insert_order(taker);
                                }
                            }
                        }
                    }
                }
            }
        } //}}}
    } //}}}

    /// market price match
    /// When the order is market type they will not be write into the order book.
    /// If there are remainning parts after the order is matched. match engine will reject this parts and gennerate a trade reocrd for this parts
    fn market_match(&mut self, taker: &mut OrderInfo) {
        assert_eq!(taker.op, OrderOp::Market);

        match taker.side {
            OrderSide::Ask => {
                //{{{
                if self.bid_leader.qty.is_zero() || self.bid_leader.price.is_zero() {
                    // gen new reject trade_record
                    // TODO:
                    return;
                }

                self.bid_leader.qty -= taker.remain_qty;
                if !self.bid_leader.qty.is_zero() || self.bid_leader.qty.is_sign_positive() {
                    let mut maker_slot: usize;
                    loop {
                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        maker_slot = self.bid_leader.order_slot;

                        if self.orders[maker_slot].remain_qty > taker.remain_qty {
                            self.orders[maker_slot].trade(taker);
                            break;
                        } else {
                            self.order_bitmap.clear(&maker_slot);

                            let next = self.orders[maker_slot].logic.next_slot;
                            self.bid_leader.order_slot = next;
                            self.orders[next].logic.pre_slot = 0;
                        }
                    }

                    // update the price node in the index
                    self.bid_price_index
                        .insert(self.bid_leader.price, self.bid_leader.clone());
                } else {
                    let mut remove_node_key = dec!(0);
                    loop {
                        if !remove_node_key.is_zero() {
                            self.bid_price_index.remove(&remove_node_key);
                            remove_node_key = dec!(0);
                        }

                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        match self.bid_price_index.last_entry() {
                            Some(mut entry) => {
                                let price = *entry.key();
                                let node = entry.get_mut();

                                node.qty -= taker.remain_qty;

                                if node.qty.is_zero() || !node.qty.is_sign_positive() {
                                    remove_node_key = price;
                                }

                                let mut maker_slot: usize;

                                loop {
                                    if taker.remain_qty.is_zero() {
                                        break;
                                    }

                                    maker_slot = node.order_slot;

                                    if maker_slot == 0 {
                                        break;
                                    }

                                    assert_eq!(self.orders[maker_slot].logic.used, true);

                                    if self.orders[maker_slot].remain_qty > taker.remain_qty {
                                        self.orders[maker_slot].trade(taker);
                                        break;
                                    } else {
                                        self.orders[maker_slot].logic.used = true;
                                        self.order_bitmap.clear(&maker_slot);

                                        self.orders[maker_slot].trade(taker);

                                        let next = self.orders[maker_slot].logic.next_slot;
                                        if next != 0 || maker_slot == next {
                                            node.order_slot = next;
                                            self.orders[next].logic.pre_slot = 0;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                match self.bid_price_index.last_key_value() {
                                    Some((_, price_node)) => self.bid_leader = *price_node,
                                    None => self.bid_leader = PriceNode::default(),
                                }
                            }
                            None => {
                                if !taker.remain_qty.is_zero() {
                                    // TODO:
                                    //  reject taker remain qty
                                }
                            }
                        }
                    }
                }
            } //}}}
            OrderSide::Bid => {
                //{{{
                if self.ask_leader.qty.is_zero() || self.ask_leader.price.is_zero() {
                    // TODO: reject this market order. because there is no suitable ask order.
                    return;
                }

                self.ask_leader.qty = taker.remain_qty / self.ask_leader.price;

                if !self.ask_leader.qty.is_zero() && self.ask_leader.qty.is_sign_positive() {
                    let mut maker_slot: usize;

                    loop {
                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        maker_slot = self.ask_leader.order_slot;

                        if self.orders[maker_slot].remain_qty
                            > taker.remain_qty / self.ask_leader.price
                        {
                            self.orders[maker_slot].trade(taker);
                            break;
                        } else {
                            self.orders[maker_slot].trade(taker);
                            self.order_bitmap.clear(&maker_slot);

                            let next = self.orders[maker_slot].logic.next_slot;
                            assert!(next != 0);

                            self.ask_leader.order_slot = next;
                            self.orders[next].logic.pre_slot = 0;
                        }
                    }
                    self.ask_price_index
                        .insert(self.bid_leader.price, self.bid_leader.clone());
                } else {
                    let mut remove_node_key: Decimal = dec!(0);

                    loop {
                        if !remove_node_key.is_zero() {
                            self.ask_price_index.remove(&remove_node_key);
                        }

                        if taker.remain_qty.is_zero() {
                            break;
                        }

                        match self.ask_price_index.first_entry() {
                            Some(mut entry) => {
                                let price = *entry.key();
                                let price_node = entry.get_mut();

                                price_node.qty -= taker.remain_qty / price;

                                if price_node.qty.is_zero() || !price_node.qty.is_sign_positive() {
                                    remove_node_key = price;
                                }

                                let mut maker_slot: usize;

                                loop {
                                    if taker.remain_qty.is_zero() {
                                        break;
                                    }

                                    maker_slot = price_node.order_slot;

                                    if maker_slot == 0 {
                                        break;
                                    }

                                    assert_eq!(self.orders[maker_slot].logic.used, true);

                                    if self.orders[maker_slot].remain_qty > taker.remain_qty / price
                                    {
                                        self.orders[maker_slot].trade(taker);
                                    } else {
                                        self.orders[maker_slot].logic.used = false;
                                        self.order_bitmap.clear(&maker_slot);

                                        self.orders[maker_slot].trade(taker);

                                        let next = self.orders[maker_slot].logic.next_slot;

                                        if next != 0 || next == maker_slot {
                                            price_node.order_slot = next;
                                            self.orders[next].logic.pre_slot = 0;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                            None => {
                                if !taker.remain_qty.is_zero() {
                                    // TODO: reject remain part
                                    return;
                                }
                            }
                        }
                    }
                }
            } //}}}
        }
    }

    // cancel order
    fn cancel(&mut self, order: &mut OrderInfo) -> Option<OrderInfo> {
        //{{{
        assert!(order.op == OrderOp::Cancel);

        match order.side {
            OrderSide::Ask => match self.ask_price_index.get_mut(&order.price) {
                //{{{
                Some(mut price_node) => {
                    let mut order_slot = price_node.order_slot;
                    loop {
                        if order_slot == 0 {
                            return None;
                        }

                        if self.orders[order_slot].id == order.id {
                            self.order_bitmap.clear(&order_slot);
                            self.orders[order_slot].logic.used = false;

                            // fix the price node
                            let order = self.orders[order_slot];
                            price_node.qty -= order.remain_qty;

                            // order.logic.used =  false;
                            if order.logic.pre_slot == 0 && order.logic.next_slot == 0 {
                                // remove this price node
                                self.ask_price_index.remove(&order.price);
                                if self.ask_leader.price == order.price {
                                    // update ask leader info
                                    match self.ask_price_index.first_key_value() {
                                        Some((_, price_node)) => {
                                            self.ask_leader = *price_node;
                                        }
                                        None => {
                                            self.ask_leader = PriceNode::default();
                                        }
                                    }
                                }
                                return None;
                            }

                            if order.logic.pre_slot == 0 {
                                let next_slot = order.logic.next_slot;
                                price_node.order_slot = next_slot;
                                self.orders[next_slot].logic.pre_slot = 0usize;
                                return None;
                            }

                            //
                            self.orders[order.logic.pre_slot].logic.next_slot =
                                order.logic.next_slot;
                            if order.logic.next_slot != 0 {
                                self.orders[order.logic.next_slot].logic.pre_slot =
                                    order.logic.pre_slot;
                            }
                            return Some(self.orders[order_slot].clone());
                        }
                        order_slot = self.orders[order_slot].logic.next_slot;
                    }
                }
                None => {
                    return None;
                }
            }, //}}}
            OrderSide::Bid => match self.bid_price_index.get_mut(&order.price) {
                //{{{
                Some(mut price_node) => {
                    //{{{
                    let mut order_slot = price_node.order_slot;
                    loop {
                        if order_slot == 0 {
                            return None;
                        }
                        if self.orders[order_slot].id == order.id {
                            self.order_bitmap.clear(&order_slot);
                            self.orders[order_slot].logic.used = false;

                            // fix the price node
                            let order = self.orders[order_slot];
                            price_node.qty -= order.remain_qty;

                            // order.logic.used =  false;
                            if order.logic.pre_slot == 0 && order.logic.next_slot == 0 {
                                // remove this price node
                                self.bid_price_index.remove(&order.price);
                                // update bid leader price node
                                if self.bid_leader.price == order.price {
                                    match self.bid_price_index.last_key_value() {
                                        Some((_, price_node)) => {
                                            self.bid_leader = *price_node;
                                        }
                                        None => {
                                            self.bid_leader = PriceNode::default();
                                        }
                                    }
                                }
                                return None;
                            }

                            if order.logic.pre_slot == 0 {
                                let next_slot = order.logic.next_slot;
                                price_node.order_slot = next_slot;
                                self.orders[next_slot].logic.pre_slot = 0usize;
                                return None;
                            }

                            //
                            self.orders[order.logic.pre_slot].logic.next_slot =
                                order.logic.next_slot;
                            if order.logic.next_slot != 0 {
                                self.orders[order.logic.next_slot].logic.pre_slot =
                                    order.logic.pre_slot;
                            }
                            return Some(self.orders[order_slot].clone());
                        }
                        order_slot = self.orders[order_slot].logic.next_slot;
                    }
                } //}}}
                None => {
                    return None;
                }
            }, //}}}
        }
    } //}}}

    // There is no suitable price order, Insert this order into orderbook
    fn insert_order(&mut self, order: &mut OrderInfo) {
        //{{{
        assert!(order.op == OrderOp::Limit);

        let slot = self.order_bitmap.find_unset();
        let price = order.price;
        match order.side {
            OrderSide::Ask => {
                //{{{
                match self.ask_price_index.get_mut(&price) {
                    Some(mut price_node) => {
                        // price node already exist
                        price_node.qty += order.remain_qty;
                        let last_slot = price_node.last_slot;
                        order.logic.curr_slot = slot;
                        order.logic.pre_slot = last_slot;
                        order.logic.next_slot = 0usize;

                        order.logic.used = true;

                        self.orders[last_slot].logic.next_slot = slot;
                        price_node.last_slot = slot;
                        if self.ask_leader.price == order.price {
                            self.ask_leader.last_slot = slot;
                            self.ask_leader.qty += order.remain_qty;
                        }
                    }
                    None => {
                        // price node does not exist
                        let new_node = PriceNode {
                            qty: order.remain_qty,
                            price: order.price,
                            order_slot: slot,
                            last_slot: slot,
                        };
                        order.logic.curr_slot = slot;
                        order.logic.pre_slot = 0usize;
                        order.logic.next_slot = 0usize;

                        order.logic.used = true;
                        if self.ask_leader.price > order.price || self.ask_leader.price.is_zero() {
                            self.ask_leader.price = order.price;
                            self.ask_leader.qty = order.remain_qty;
                            self.ask_leader.order_slot = slot;
                            self.ask_leader.last_slot = slot;
                        }

                        self.ask_price_index.insert(order.price, new_node);
                    }
                }
            } //}}}
            OrderSide::Bid => {
                //{{{
                match self.bid_price_index.get_mut(&price) {
                    Some(mut price_node) => {
                        // price node already exist
                        price_node.qty += order.remain_qty;

                        let last_slot = price_node.last_slot;

                        order.logic.curr_slot = slot;
                        order.logic.pre_slot = last_slot;
                        order.logic.next_slot = 0usize;

                        order.logic.used = true;
                        self.orders[last_slot].logic.next_slot = slot;
                        price_node.last_slot = slot;
                        if order.price == self.bid_leader.price {
                            self.bid_leader.qty += order.remain_qty;
                            self.bid_leader.last_slot = slot;
                        }
                    }
                    None => {
                        // price node does not existc
                        let new_node = PriceNode {
                            qty: order.remain_qty,
                            price: order.price,
                            order_slot: slot,
                            last_slot: slot,
                        };
                        order.logic.curr_slot = slot;
                        order.logic.pre_slot = 0usize;
                        order.logic.next_slot = 0usize;

                        order.logic.used = true;
                        if self.bid_leader.price < order.price {
                            self.bid_leader.price = order.price;
                            self.bid_leader.qty = order.remain_qty;
                            self.bid_leader.order_slot = slot;
                            self.bid_leader.last_slot = slot;
                        }
                        self.bid_price_index.insert(order.price, new_node);
                    }
                }
            } //}}}
        }
        self.orders[slot] = *order;
    } //}}}

    fn snapshot(&self) {
        //{{{
        let json = serde_json::to_string(self).unwrap();
        let dump_file_name = Utc::now().format("%Y-%m-%d_").to_string() + &self.market + ".d";
        println!("{}", dump_file_name);
        let mut file = File::create("batch/".to_owned() + &dump_file_name).unwrap();

        file.write_all(json.as_bytes()).unwrap();
        unsafe {
            fsync(file.as_raw_fd());
        }
    } //}}}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_book_init_insert_test() {
        //{{{
        let mut orderbook = OrderBook::new(100, "BTC/USDT".to_owned());
        let mut test_order = OrderInfo::default();
        test_order.price = dec!(1.2);
        test_order.id = 1;
        test_order.raw_qty = dec!(100);
        test_order.remain_qty = dec!(100);
        orderbook.insert_order(&mut test_order);
        assert_eq!(orderbook.bid_leader.qty, dec!(100));
        test_order.id = 2;
        orderbook.insert_order(&mut test_order);
        assert_eq!(orderbook.bid_leader.qty, dec!(200));
        test_order.id = 3;
        orderbook.insert_order(&mut test_order);
        assert_eq!(orderbook.bid_leader.qty, dec!(300));
        assert_eq!(orderbook.orders[1].logic.curr_slot, 1);
        assert_eq!(orderbook.orders[1].logic.pre_slot, 0);
        assert_eq!(orderbook.orders[1].logic.next_slot, 2);

        assert_eq!(orderbook.orders[2].logic.curr_slot, 2);
        assert_eq!(orderbook.orders[2].logic.pre_slot, 1);
        assert_eq!(orderbook.orders[2].logic.next_slot, 3);

        assert_eq!(orderbook.orders[3].logic.curr_slot, 3);
        assert_eq!(orderbook.orders[3].logic.pre_slot, 2);
        assert_eq!(orderbook.orders[3].logic.next_slot, 0);

        for (k, v) in orderbook.bid_price_index.iter() {
            println!("{}-> {:?}", k, v);
        }
        let price_node = orderbook.bid_price_index.get(&test_order.price).unwrap();
        println!("{:?}", price_node);
    } //}}}

    #[test]
    fn order_book_cancel_test() {
        //{{{
        let mut orderbook = OrderBook::new(100, "BTC/USDT".to_owned());
        let mut test_order = OrderInfo::default();
        test_order.id = 1;
        test_order.price = dec!(1.23);
        test_order.raw_qty = dec!(100);
        test_order.remain_qty = dec!(100);
        orderbook.insert_order(&mut test_order);
        test_order.id = 2;
        orderbook.insert_order(&mut test_order);
        test_order.id = 3;
        orderbook.insert_order(&mut test_order);

        test_order.id = 2;
        test_order.op = OrderOp::Cancel;
        orderbook.cancel(&mut test_order);
        let price_node = orderbook.bid_price_index.get(&test_order.price).unwrap();

        assert_eq!(price_node.qty, dec!(200));
        assert_eq!(price_node.order_slot, 1usize);
        assert_eq!(price_node.last_slot, 3usize);
        assert_eq!(orderbook.orders[2].logic.used, false);

        test_order.op = OrderOp::Limit;
        test_order.side = OrderSide::Ask;
        test_order.id = 4;
        test_order.price = dec!(1.25);
        orderbook.insert_order(&mut test_order);

        test_order.id = 5;
        orderbook.insert_order(&mut test_order);
        test_order.id = 6;
        orderbook.insert_order(&mut test_order);
        test_order.id = 5;
        test_order.op = OrderOp::Cancel;
        orderbook.cancel(&mut test_order);

        assert_eq!(orderbook.orders[2].logic.curr_slot, 2);
        assert_eq!(orderbook.orders[2].logic.pre_slot, 0);
        assert_eq!(orderbook.orders[2].logic.next_slot, 5);

        assert_eq!(orderbook.orders[4].logic.curr_slot, 4);
        assert_eq!(orderbook.orders[4].logic.pre_slot, 2);
        assert_eq!(orderbook.orders[4].logic.next_slot, 5);

        assert_eq!(orderbook.orders[5].logic.curr_slot, 5);
        assert_eq!(orderbook.orders[5].logic.pre_slot, 2);
        assert_eq!(orderbook.orders[5].logic.next_slot, 0);
        let price_node = orderbook.ask_price_index.get(&test_order.price).unwrap();
        assert_eq!(price_node.qty, dec!(200));
        assert_eq!(price_node.order_slot, 2);
        assert_eq!(price_node.last_slot, 5);
    } //}}}

    #[test]
    fn order_bool_limit_match_test_bid_taker() {
        //{{{
        let mut orderbook = OrderBook::new(100, "BTC/USDT".to_owned());
        let mut test_order = OrderInfo::default();
        test_order.id = 1;
        test_order.price = dec!(1.23);
        test_order.raw_qty = dec!(100);
        test_order.remain_qty = dec!(100);
        test_order.uid = 10001;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.bid_price_index.get(&dec!(1.23)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.23));
        assert_eq!(node.order_slot, 1usize);
        assert_eq!(node.last_slot, 1usize);

        test_order.price = dec!(1.24);
        test_order.uid = 10002;
        test_order.id = 2;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.bid_price_index.get(&dec!(1.24)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.24));
        assert_eq!(node.order_slot, 2usize);
        assert_eq!(node.last_slot, 2usize);

        test_order.price = dec!(1.25);
        test_order.uid = 10003;
        test_order.id = 3;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.bid_price_index.get(&dec!(1.25)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.25));
        assert_eq!(node.order_slot, 3usize);
        assert_eq!(node.last_slot, 3usize);

        test_order.price = dec!(1.23);
        test_order.raw_qty = dec!(250);
        test_order.remain_qty = dec!(250);
        test_order.uid = 10005;
        test_order.side = OrderSide::Ask;
        orderbook.match_entry(&mut test_order.clone());

        match orderbook.bid_price_index.get(&dec!(1.25)) {
            Some(_) => panic!("error"),
            None => (),
        }
        match orderbook.bid_price_index.get(&dec!(1.24)) {
            Some(_) => panic!("error"),
            None => (),
        }
        let node = orderbook.bid_price_index.get(&dec!(1.23)).unwrap();
        assert_eq!(node.qty, dec!(50));
        assert_eq!(node.price, dec!(1.23));
        assert_eq!(node.order_slot, 1usize);
        assert_eq!(node.last_slot, 1usize);

        assert_eq!(orderbook.orders[1].logic.used, true);
        assert_eq!(orderbook.orders[2].logic.used, false);
        assert_eq!(orderbook.orders[3].logic.used, false);
    } //}}}

    #[test]
    fn order_bool_limit_match_test_ask_taker() {
        //{{{
        //{{{

        let mut orderbook = OrderBook::new(100, "BTC/USDT".to_owned());
        let mut test_order = OrderInfo::default();
        test_order.side = OrderSide::Ask;
        test_order.id = 1;
        test_order.price = dec!(1.23);
        test_order.raw_qty = dec!(100);
        test_order.remain_qty = dec!(100);
        test_order.uid = 10001;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.ask_price_index.get(&dec!(1.23)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.23));
        assert_eq!(node.order_slot, 1usize);
        assert_eq!(node.last_slot, 1usize);

        test_order.price = dec!(1.24);
        test_order.uid = 10002;
        test_order.id = 2;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.ask_price_index.get(&dec!(1.24)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.24));
        assert_eq!(node.order_slot, 2usize);
        assert_eq!(node.last_slot, 2usize);

        test_order.price = dec!(1.25);
        test_order.uid = 10003;
        test_order.id = 3;
        orderbook.insert_order(&mut test_order.clone());
        let node = orderbook.ask_price_index.get(&dec!(1.25)).unwrap();
        assert_eq!(node.qty, dec!(100));
        assert_eq!(node.price, dec!(1.25));
        assert_eq!(node.order_slot, 3usize);
        assert_eq!(node.last_slot, 3usize);

        test_order.price = dec!(1.25);
        test_order.raw_qty = dec!(250);
        test_order.remain_qty = dec!(250);
        test_order.uid = 10005;
        test_order.side = OrderSide::Bid;
        orderbook.match_entry(&mut test_order.clone());

        match orderbook.bid_price_index.get(&dec!(1.24)) {
            Some(_) => panic!("error"),
            None => (),
        }
        match orderbook.bid_price_index.get(&dec!(1.23)) {
            Some(_) => panic!("error"),
            None => (),
        }
        let node = orderbook.ask_price_index.get(&dec!(1.25)).unwrap();

        assert_eq!(node.qty, dec!(50));
        assert_eq!(node.price, dec!(1.25));
        assert_eq!(node.order_slot, 3usize);
        assert_eq!(node.last_slot, 3usize);

        assert_eq!(orderbook.orders[1].logic.used, false);
        assert_eq!(orderbook.orders[2].logic.used, false);
        assert_eq!(orderbook.orders[3].logic.used, true);
    } //}}}}}}

    #[test]
    fn snapshot_test() {
        let mut orderbook = OrderBook::new(2, "BTC_USDT".to_owned());
        let mut test_order = OrderInfo::default();
        test_order.id = 1;
        test_order.price = dec!(1.23);
        test_order.raw_qty = dec!(100);
        test_order.remain_qty = dec!(100);
        test_order.uid = 10001;
        orderbook.insert_order(&mut test_order.clone());
        orderbook.snapshot();
    }
}

fn main() {}

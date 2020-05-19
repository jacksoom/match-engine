use rust_decimal::Decimal;
use rust_decimal_macros::*;
use std::fmt;
use std::time::Instant;
use serde::{Deserialize, Serialize};


#[derive(Copy, Clone, Debug, PartialEq, SmartDefault, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum OrderOp {
    #[default]
    Limit, // limit order type
    Market, // market order type
    Cancel, // cancel order type
}

#[derive(Copy, Clone, Debug, PartialEq, SmartDefault, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum OrderSide {
    #[default]
    Bid, // bid
    Ask, // ask
}

#[derive(Copy, Clone, Debug, PartialEq, SmartDefault, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum OrderStatus {
    #[default]
    PaddingTrade, // padding trade
    AllTrade,   // all trade
    PartTrade,  // part trade
    AllCancel,  // all cancel
    PartCancel, // part cancel
    AutoCancel, // auto cancel, market order untrade part
}

#[derive(Copy, Clone, Debug, PartialEq, SmartDefault, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum TradeType {
    #[default]
    SimpleTrade, // simple trade trade record type
    CancelTrade, // cancel order trade record type
}

#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OrderInfo {
    //{{{
    pub id: u64,                  // order id
    pub uid: u64,                 // order user id
    pub op: OrderOp,              // order opreation
    pub side: OrderSide,          // order side
    pub price: Decimal,           // order price
    pub avg_trade_price: Decimal, // order tarde average price
    pub raw_qty: Decimal,         // order quantity
    pub remain_qty: Decimal,
    pub trade_qty: Decimal,      // order traded quantity
    pub trade_oppo_qty: Decimal, // order traded oppo quantity
    pub status: OrderStatus,     // current order status
    pub taker_fee_rate: Decimal, // order taker fee rate
    pub maker_fee_rate: Decimal, // order maker fee rate
    pub fee: Decimal,
    pub logic: OrderLogic, // order logic info
} //}}}

impl fmt::Display for OrderInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "id:{}\nuid:{}\nop:{:?}\nside:{:?}\nprice:{}\navg_trade_price::{}\nraw_qty::{}\nremain_qty:{}\ntrade_qty:{}\ntrade_oppo_qty:{}\nstatus:{:?}\ntaker_fee:{}\nmaker_fee:{}\nfee:{}\ncurr_slot:{}\npre_slot:{}\nnext_slot:{}\nused:{}\n", self.id, self.uid, self.op, self.side, self.price, self.avg_trade_price, self.raw_qty, self.remain_qty, self.trade_qty, self.trade_oppo_qty, self.status, self.taker_fee_rate,  self.maker_fee_rate, self.fee, self.logic.curr_slot, self.logic.pre_slot, self.logic.next_slot, self.logic.used)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_print() {
        let order = OrderInfo::default();
        println!("{:?}", order);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OrderLogic {
    pub curr_slot: usize, // current order slot
    pub pre_slot: usize,  // pre order slot, 0 / unuse
    pub next_slot: usize, // next order slot, 0 unuse
    pub used: bool,
}

#[allow(dead_code)]
enum TradeError {
    OrderQtyIllegal,
    OrderPriceIllegal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TradeRecord {
    //{{{
    trade_id: u64, // unique trade record id

    bid_order_id: u64,       // bid order id
    bid_uid: u64,            // user of the bid order
    bid_type: OrderOp,       // bid order type
    bid_raw_qty: Decimal,    // bid order raw quantity
    bid_remain_qty: Decimal, // bid order remain quantity
    bid_raw_price: Decimal,  // bid order raw quantity
    bid_avg_price: Decimal,  // bid order trade avg price
    bid_fee: Decimal,

    ask_order_id: u64,       // ask order id
    ask_uid: u64,            // user of the ask order
    ask_type: OrderOp,       // ask order type
    ask_raw_qty: Decimal,    // ask order raw quantity
    ask_remain_qty: Decimal, // ask order remain quantity
    ask_raw_price: Decimal,  // ask order raw price
    ask_avg_price: Decimal,  // ask order trade avg price
    ask_fee: Decimal,

    trade_qty: Decimal,          // trade qty
    trade_price: Decimal,        // trade price
    trade_oppo_qty: Decimal,     // trade_oppo_qty = trade_qty * trade_price
    trade_unfreeze_qty: Decimal, // taker order should be unfreeze qty
    time_stamp: u64,             // trade timestap
    trade_type: TradeType,
} //}}}

impl Default for TradeRecord {
    //{{{
    fn default() -> TradeRecord {
        let zero = dec!(0);
        TradeRecord {
            trade_id: 0u64,
            bid_order_id: 0u64,
            bid_uid: 0u64,
            bid_raw_qty: zero,
            bid_remain_qty: zero,
            bid_type: OrderOp::Limit,
            bid_raw_price: zero,
            bid_avg_price: zero,
            bid_fee: zero,
            ask_order_id: 0u64,
            ask_uid: 0u64,
            ask_type: OrderOp::Limit,
            ask_raw_qty: zero,
            ask_remain_qty: zero,
            ask_raw_price: zero,
            ask_avg_price: zero,
            ask_fee: zero,
            trade_qty: zero,
            trade_price: zero,
            trade_oppo_qty: zero,
            trade_unfreeze_qty: zero,
            time_stamp: 0u64,
            trade_type: TradeType::SimpleTrade,
        }
    }
} //}}}

pub(crate) fn gen_trade_id() -> u64 {
    // TODO:
    1u64
}

impl OrderInfo {
    #[inline]
    pub fn new(
        id: u64,
        uid: u64,
        side: OrderSide,
        qty: Decimal,
        price: Decimal,
        (taker_fee, maker_fee): (Decimal, Decimal),
    ) -> OrderInfo {
        OrderInfo {
            //{{{
            id: id,
            uid: uid,
            op: OrderOp::Limit,
            side: side,
            price: price,
            avg_trade_price: dec!(0),
            raw_qty: qty,
            trade_qty: dec!(0),
            remain_qty: qty,
            trade_oppo_qty: dec!(0),
            status: OrderStatus::PaddingTrade,
            taker_fee_rate: taker_fee,
            maker_fee_rate: maker_fee,
            fee: dec!(0),
            logic: OrderLogic {
                curr_slot: 0,
                pre_slot: 0,
                next_slot: 0,
                used: true,
            },
        } //}}}
    }

    // gennerate new unique trade record id
    pub fn trade(&mut self, taker: &mut OrderInfo) -> Option<TradeRecord> {
        //{{{
        // ensure the taker order is limit type
        assert_eq!(self.op, OrderOp::Limit);

        match taker.op {
            OrderOp::Limit => {
                //{{{
                // colc trade qty
                let trade_qty = if self.remain_qty > taker.remain_qty {
                    taker.remain_qty
                } else {
                    self.remain_qty
                };

                assert!(trade_qty > dec!(0));
                self.trade_qty = self.trade_qty + trade_qty;
                self.remain_qty = self.remain_qty - trade_qty;

                taker.trade_qty = taker.trade_qty + trade_qty;
                taker.remain_qty = taker.remain_qty - trade_qty;

                let oppo_qty = trade_qty * self.price;

                match self.side {
                    OrderSide::Ask => {
                        self.fee = self.fee + oppo_qty * self.maker_fee_rate;
                        taker.fee = taker.fee + trade_qty * taker.taker_fee_rate;
                    }
                    OrderSide::Bid => {
                        self.fee = self.fee + trade_qty * self.maker_fee_rate;
                        taker.fee = taker.fee + oppo_qty * taker.taker_fee_rate;
                    }
                }

                self.trade_oppo_qty = self.trade_oppo_qty + oppo_qty;

                taker.trade_oppo_qty = taker.trade_oppo_qty + oppo_qty;

                self.avg_trade_price = self.trade_oppo_qty / self.trade_qty;

                taker.avg_trade_price = taker.trade_oppo_qty / taker.trade_qty;

                // colc self and taker order status.
                self.status = if self.raw_qty == self.trade_qty {
                    self.logic.used = false;
                    OrderStatus::AllTrade
                } else {
                    OrderStatus::PartTrade
                };

                taker.status = if taker.raw_qty == taker.trade_qty {
                    taker.logic.used = false;
                    OrderStatus::AllTrade
                } else {
                    OrderStatus::PartTrade
                };

                let trade_id = gen_trade_id();
                let bid_order = if self.side == OrderSide::Ask {
                    *taker
                } else {
                    *self
                };

                let ask_order = if self.side == OrderSide::Ask {
                    *self
                } else {
                    *taker
                };
                let trade_unfreeze_qty =
                    if taker.side == OrderSide::Bid && taker.op == OrderOp::Limit {
                        trade_qty * (taker.price - self.price)
                    } else {
                        dec!(0)
                    };

                Some(TradeRecord {
                    trade_id: trade_id,
                    bid_order_id: bid_order.id,
                    bid_uid: bid_order.uid,
                    bid_type: OrderOp::Limit,
                    bid_raw_qty: bid_order.raw_qty,
                    bid_remain_qty: bid_order.remain_qty,
                    bid_raw_price: bid_order.price,
                    bid_avg_price: bid_order.avg_trade_price,
                    bid_fee: bid_order.fee,

                    ask_order_id: ask_order.id,
                    ask_uid: ask_order.uid,
                    ask_type: OrderOp::Limit,
                    ask_raw_qty: ask_order.raw_qty,
                    ask_remain_qty: ask_order.remain_qty,
                    ask_raw_price: ask_order.price,
                    ask_avg_price: ask_order.avg_trade_price,
                    ask_fee: ask_order.fee,

                    time_stamp: Instant::now().elapsed().as_secs(),

                    trade_qty: trade_qty,
                    trade_price: self.price,
                    trade_oppo_qty: oppo_qty,
                    trade_unfreeze_qty: trade_unfreeze_qty,

                    trade_type: TradeType::SimpleTrade,
                })
            } //}}}

            OrderOp::Market => {
                match taker.side {
                    OrderSide::Ask => {
                        //{{{
                        // colc trade qty
                        let trade_qty = if self.remain_qty > taker.remain_qty {
                            taker.remain_qty
                        } else {
                            self.remain_qty
                        };

                        self.trade_qty = self.trade_qty + trade_qty;
                        self.remain_qty = self.remain_qty - trade_qty;
                        self.fee = self.fee + trade_qty * self.maker_fee_rate;

                        taker.trade_qty = taker.trade_qty + trade_qty;
                        taker.remain_qty = taker.remain_qty - trade_qty;

                        let oppo_qty = trade_qty * self.price;

                        self.trade_oppo_qty = self.trade_oppo_qty + oppo_qty;

                        taker.trade_oppo_qty = taker.trade_oppo_qty + oppo_qty;

                        taker.fee = taker.fee + oppo_qty * taker.taker_fee_rate;

                        self.avg_trade_price = self.trade_qty / self.trade_oppo_qty;

                        taker.avg_trade_price = taker.trade_qty / taker.trade_oppo_qty;

                        // colc self and taker order status.
                        self.status = if self.raw_qty == self.trade_qty {
                            OrderStatus::AllTrade
                        } else {
                            OrderStatus::PartTrade
                        };

                        taker.status = if taker.raw_qty == taker.trade_qty {
                            OrderStatus::AllTrade
                        } else {
                            OrderStatus::PartTrade
                        };

                        let trade_id = gen_trade_id();
                        let bid_order = if self.side == OrderSide::Ask {
                            *taker
                        } else {
                            *self
                        };

                        let ask_order = if self.side == OrderSide::Ask {
                            *self
                        } else {
                            *taker
                        };

                        Some(TradeRecord {
                            trade_id: trade_id,
                            bid_order_id: bid_order.id,
                            bid_uid: bid_order.uid,
                            bid_type: OrderOp::Limit,
                            bid_raw_qty: bid_order.raw_qty,
                            bid_remain_qty: bid_order.remain_qty,
                            bid_raw_price: bid_order.price,
                            bid_avg_price: bid_order.avg_trade_price,
                            bid_fee: bid_order.fee,

                            ask_order_id: ask_order.id,
                            ask_uid: ask_order.uid,
                            ask_type: OrderOp::Limit,
                            ask_raw_qty: ask_order.raw_qty,
                            ask_remain_qty: ask_order.remain_qty,
                            ask_raw_price: ask_order.price,
                            ask_avg_price: ask_order.avg_trade_price,
                            ask_fee: ask_order.fee,

                            time_stamp: Instant::now().elapsed().as_secs(),

                            trade_qty: trade_qty,
                            trade_price: self.price,
                            trade_oppo_qty: oppo_qty,
                            trade_unfreeze_qty: dec!(0),
                            trade_type: TradeType::SimpleTrade,
                        });
                    } //}}}
                    OrderSide::Bid => {
                        //{{{
                        let trade_qty = if self.remain_qty * self.price > taker.remain_qty {
                            taker.remain_qty / self.price
                        } else {
                            self.remain_qty
                        };

                        let trade_oppo_qty = trade_qty * self.price;

                        self.trade_qty = self.trade_qty + trade_qty;
                        self.remain_qty = self.remain_qty - trade_qty;

                        taker.trade_qty = taker.trade_qty + trade_oppo_qty;
                        taker.remain_qty = taker.remain_qty - trade_oppo_qty;

                        self.trade_oppo_qty = self.trade_oppo_qty + trade_oppo_qty;
                        taker.trade_oppo_qty = taker.trade_oppo_qty + trade_qty;

                        self.avg_trade_price = self.trade_qty / self.trade_oppo_qty;
                        taker.avg_trade_price = taker.trade_oppo_qty / taker.trade_qty;

                        self.fee = self.fee + trade_oppo_qty * self.maker_fee_rate;
                        taker.fee = taker.fee + trade_qty * taker.taker_fee_rate;

                        self.status = if self.raw_qty == self.trade_qty {
                            OrderStatus::AllTrade
                        } else {
                            OrderStatus::PartTrade
                        };

                        taker.status = if taker.raw_qty == self.trade_qty {
                            OrderStatus::AllTrade
                        } else {
                            OrderStatus::PartTrade
                        };

                        let trade_id = gen_trade_id();
                        let bid_order = if self.side == OrderSide::Ask {
                            *taker
                        } else {
                            *self
                        };

                        let ask_order = if self.side == OrderSide::Ask {
                            *self
                        } else {
                            *taker
                        };

                        Some(TradeRecord {
                            trade_id: trade_id,
                            bid_order_id: bid_order.id,
                            bid_uid: bid_order.uid,
                            bid_type: OrderOp::Limit,
                            bid_raw_qty: bid_order.raw_qty,
                            bid_remain_qty: bid_order.remain_qty,
                            bid_raw_price: bid_order.price,
                            bid_avg_price: bid_order.avg_trade_price,
                            bid_fee: bid_order.fee,

                            ask_order_id: ask_order.id,
                            ask_uid: ask_order.uid,
                            ask_type: OrderOp::Limit,
                            ask_raw_qty: ask_order.raw_qty,
                            ask_remain_qty: ask_order.remain_qty,
                            ask_raw_price: ask_order.price,
                            ask_avg_price: ask_order.avg_trade_price,
                            ask_fee: ask_order.fee,
                            time_stamp: Instant::now().elapsed().as_secs(),

                            trade_qty: trade_qty,
                            trade_price: self.price,
                            trade_oppo_qty: trade_oppo_qty,
                            trade_unfreeze_qty: dec!(0),
                            trade_type: TradeType::SimpleTrade,
                        });
                    } //}}}
                }
                None
            }

            _ => {
                println! {"unsupport order operation"}
                None
            }
        }
    } //}}}

    pub fn cancel(&mut self) -> Option<TradeRecord> {
        //{{{
        if !self.logic.used || self.remain_qty == dec!(0) {
            return None;
        }

        let trade_id = gen_trade_id();
        self.logic.used = false;

        match self.side {
            OrderSide::Ask => Some(TradeRecord {
                trade_id: trade_id,
                ask_order_id: self.id,
                ask_uid: self.uid,
                ask_type: self.op,
                ask_raw_qty: self.raw_qty,
                ask_remain_qty: self.remain_qty,
                ask_raw_price: self.price,
                ask_avg_price: self.avg_trade_price,
                ask_fee: self.fee,
                trade_type: TradeType::CancelTrade,
                ..Default::default()
            }),
            OrderSide::Bid => Some(TradeRecord {
                bid_order_id: self.id,
                bid_uid: self.uid,
                bid_type: self.op,
                bid_raw_qty: self.raw_qty,
                bid_remain_qty: self.remain_qty,
                bid_raw_price: self.price,
                bid_avg_price: self.avg_trade_price,
                bid_fee: self.fee,
                trade_type: TradeType::CancelTrade,
                ..Default::default()
            }),
        }
    } //}}}
}

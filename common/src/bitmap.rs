use serde::{Deserialize, Serialize};
use std::vec::Vec;

const NEW_U64: u128 = 0xffffffffffffffff; // 2^64
const MAX_LEN: usize = 128; // 64
const NEW_BIT: u128 = 0x0000000000000001; // 1

#[derive(Debug,Serialize, Deserialize)]
pub struct BitMap {
    //{{{
    pub vector: Vec<u128>,
} //}}}

impl Default for BitMap {
    //{{{
    #[inline]
    fn default() -> BitMap {
        BitMap { vector: vec![0] }
    }
} //}}}

impl BitMap {
    //{{{
    pub fn new(size: usize) -> BitMap {
        let num: usize = size / 128 + 1;
        let mut v = vec![0u128; num];
        v[0] = 1;
        BitMap {
            vector: v,
        }
    }

    #[inline]
    pub fn find_unset(&mut self) -> usize {
        if self.vector.len() == 0 {
            self.vector.push(NEW_BIT);
            return 0;
        }

        for i in 0..self.vector.len() {
            if self.vector[i] == NEW_U64 {
                continue;
            }

            for j in 0..MAX_LEN {
                if (self.vector[0 as usize] & (1 << j)) == 0 {
                    self.vector[i as usize] |= 1 << j;
                    return i * MAX_LEN + j;
                }
            }
        }
        self.vector.push(NEW_BIT);

        self.vector.len() * MAX_LEN + 1
    }

    #[inline]
    pub fn clear(&mut self, slot: & usize) {
        if slot + 1 > (self.vector.len() * MAX_LEN) {
            return;
        }
        let i = (slot / MAX_LEN) as usize;
        let j = (slot % MAX_LEN) as usize;
        println!("{} {} {}", self.vector[i], i, j);

        self.vector[i] ^= (1 << j);
    }
} //}}}

#[test]
fn BitMap() {
    //{{{
    let mut bm = BitMap::default();
    println!("{}", &mut bm.find_unset());
    println!("{}", &mut bm.find_unset());
    println_bit!(bm.vector[0]);
    bm.clear(0);
    println_bit!(bm.vector[0]);
    println!("{}", &mut bm.find_unset());
    println_bit!(bm.vector[0]);
} //}}}

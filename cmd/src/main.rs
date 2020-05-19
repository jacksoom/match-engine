#[macro_use]
extern crate log;
extern crate chrono;
use env_logger;
use log::{info, trace, warn};
use nix::unistd::{fork, ForkResult};
use rocksdb::{Options, DB};
use std::{thread, time};
use chrono::prelude::*;
use chrono::Local;

macro_rules! create_function {
    ($func_name:ident, $x:expr) => {
        fn $func_name() {
            println!("function {:?} is called, {}", stringify!($func_name), $x);
        }
    };
}

fn main() {
    env_logger::init();
    info!("hello world!");
    create_function!(f1, 1234);
    f1();
    println!("--------------------------------");

    thread::spawn(move || {
        println!("new thread;");
        let mut num = 19941010usize;
        match fork() {
            Ok(ForkResult::Child) => {
                println!("i am a child process");
                println!("-->1: {}", num);
                thread::sleep(time::Duration::from_millis(5000));
                println!("-->2: {}", num);
                num = 1995;
            }
            Err(_) => println!("Fork failed"),
            _ => return,
        }
        num = 1880;
        println!("i am spawn thread {}", num);
    })
    .join()
    .unwrap();

    println!("continue running main thread");

    thread::sleep(time::Duration::from_millis(5000));
    rocksdb();
}

fn rocksdb() {
    // NB: db is automatically closed at end of lifetime
    let path = format!("/Users/jacksoom/snapshot_{}", Local::now().format("%Y%m%d"));
    {
        let db = DB::open_default(path).unwrap();
        db.put(b"my key", b"my value").unwrap();
        match db.get(b"my key") {
            Ok(Some(value)) => println!("retrieved value {}", String::from_utf8(value).unwrap()),
            Ok(None) => println!("value not found"),
            Err(e) => println!("operational problem encountered: {}", e),
        }
        // db.delete(b"my key").unwrap();
    }
    // let _ = DB::destroy(&Options::default(), path);
}

extern crate byteorder;
extern crate hex;
extern crate nanocurrency_types;
extern crate serde;
extern crate serde_json;

use byteorder::{ByteOrder, BigEndian};
use nanocurrency_types::{work_threshold, work_value, BlockInner, Network};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process;

fn main() {
    let mut args = env::args();
    args.next();
    let blocks_inner_file = args
        .next()
        .expect("Expected blocks inner file as first argument");
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let work_file = args
        .next()
        .expect("Expected blocks work file as first argument");
    let work_file = BufReader::new(File::open(work_file).expect("Failed to open blocks work file"));
    let blocks_inner = serde_json::Deserializer::from_reader(blocks_inner_file).into_iter();
    let work_values = work_file.lines();
    let mut i = 0;
    for (block_inner, work) in blocks_inner.zip(work_values) {
        i += 1;
        let block_inner: BlockInner = block_inner.expect("Failed to read block inner from file");
        let work = work.expect("Failed to read block work from file");
        let work = hex::decode(work).expect("Failed to decode work as hex");
        if work.len() != 8 {
            panic!(
                "Work value {} has incorrect length {} (should be 8 bytes long)",
                i,
                work.len(),
            );
        }
        let work = BigEndian::read_u64(&work);
        if work_value(block_inner.root_bytes(), work) < work_threshold(Network::Live) {
            println!();
            eprintln!("Invalid work for block {}", i);
            process::exit(1);
        }
        if i % 1000 == 0 {
            print!("\r{}", i);
            io::stdout().flush().expect("Failed to flush stdout");
        }
    }
    println!("\r{}", i);
}

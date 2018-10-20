extern crate byteorder;
extern crate hex;
extern crate lmdb_zero as lmdb;
extern crate nanocurrency_types;
extern crate serde;
extern crate serde_json;

use lmdb::LmdbResultExt;
use nanocurrency_types::BlockInner;
use std::env;
use std::fs::File;
use std::io::{self, BufReader, Write};

fn main() {
    let mut args = env::args();
    args.next();
    let blocks_inner_file = args
        .next()
        .expect("Expected blocks inner file as first argument");
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let blocks_inner = serde_json::Deserializer::from_reader(blocks_inner_file).into_iter();
    let env = unsafe {
        let mut builder = lmdb::EnvBuilder::new().unwrap();
        builder.set_maxdbs(64).unwrap();
        builder
            .open(
                &args.next().expect("Expected DB path as second argument"),
                lmdb::open::NOSUBDIR | lmdb::open::NOTLS,
                0o600,
            ).unwrap()
    };
    let accounts_v0_db =
        lmdb::Database::open(&env, Some("accounts"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let txn = lmdb::ReadTransaction::new(&env).unwrap();
    let access = txn.access();
    let mut forks = 0;
    let mut blocks = 0;
    for block_inner in blocks_inner {
        let block_inner: BlockInner = block_inner.expect("Failed to read block inner from file");
        if let BlockInner::State {
            account, previous, ..
        } = block_inner
        {
            let maybe_acct = access
                .get::<_, [u8]>(&accounts_v0_db, &account.0)
                .to_opt()
                .expect("Failed to read from accounts table");
            if let Some(acct) = maybe_acct {
                // Check head block
                if &acct[..32] != &previous.0 {
                    forks += 1;
                }
            } else {
                if previous.0.iter().any(|&b| b != 0) {
                    forks += 1;
                }
            }
        } else {
            panic!("Non-state block in block inner file: {:?}", block_inner);
        }
        blocks += 1;
        if blocks % 100 == 0 {
            print!("\rBlocks: {}, Forks: {}", blocks, forks);
            io::stdout().flush().expect("Failed to flush stdout");
        }
    }
    println!("\rBlocks: {}, Forks: {}", blocks, forks);
}

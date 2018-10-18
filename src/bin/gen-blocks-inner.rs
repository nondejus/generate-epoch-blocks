extern crate atty;
extern crate byteorder;
extern crate hex;
extern crate lmdb_zero as lmdb;
extern crate nanocurrency_types;
extern crate serde;
extern crate serde_json;

use byteorder::{BigEndian, ByteOrder};
use lmdb::LmdbResultExt;
use nanocurrency_types::{Account, BlockHash, BlockInner};
use std::collections::HashSet;
use std::env;
use std::io::{self, Write};
use std::process;

fn main() {
    if atty::is(atty::Stream::Stdout) {
        eprintln!("Stdout is a terminal. This output is very large.");
        eprintln!("Please pipe this somewhere.");
        process::exit(1);
    }
    let mut args = env::args();
    args.next();
    let env = unsafe {
        let mut builder = lmdb::EnvBuilder::new().unwrap();
        builder.set_maxdbs(64).unwrap();
        builder
            .open(
                &args.next().expect("Expected path as arg"),
                lmdb::open::NOSUBDIR | lmdb::open::NOTLS,
                0o600,
            ).unwrap()
    };
    let open_db =
        lmdb::Database::open(&env, Some("open"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let change_db =
        lmdb::Database::open(&env, Some("change"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let state_db =
        lmdb::Database::open(&env, Some("state"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let accounts_v0_db =
        lmdb::Database::open(&env, Some("accounts"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let accounts_v1_db = lmdb::Database::open(
        &env,
        Some("accounts_v1"),
        &lmdb::DatabaseOptions::defaults(),
    ).unwrap();
    let pending_v0_db =
        lmdb::Database::open(&env, Some("pending"), &lmdb::DatabaseOptions::defaults()).unwrap();
    let mut epoch_link = [0u8; 32];
    for (i, o) in b"epoch v1 block".iter().zip(epoch_link.iter_mut()) {
        *o = *i;
    }
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut output_block = |block_inner| {
        serde_json::to_writer(&mut stdout, &block_inner)
            .expect("Failed to serialize block into stdout");
        stdout
            .write_all(b"\n")
            .expect("Failed to write newline to stdout");
    };
    let txn = lmdb::ReadTransaction::new(&env).unwrap();
    let access = txn.access();
    let mut accounts_it = txn.cursor(&accounts_v0_db).unwrap();
    let mut current_kv = accounts_it.first::<[u8], [u8]>(&access).to_opt().unwrap();
    while let Some((account_slice, account_info)) = current_kv {
        let mut account = [0u8; 32];
        account.clone_from_slice(account_slice);
        let mut head_block = [0u8; 32];
        head_block.clone_from_slice(&account_info[..32]);
        let rep_block = &account_info[32..64];
        let mut representative = None;
        if let Some(open_block) = access.get::<_, [u8]>(&open_db, rep_block).to_opt().unwrap() {
            let mut rep_bytes = [0u8; 32];
            rep_bytes.copy_from_slice(&open_block[32..64]);
            representative = Some(rep_bytes);
        } else if let Some(change_block) = access
            .get::<_, [u8]>(&change_db, rep_block)
            .to_opt()
            .unwrap()
        {
            let mut rep_bytes = [0u8; 32];
            rep_bytes.copy_from_slice(&change_block[32..64]);
            representative = Some(rep_bytes);
        } else if let Some(state_block) = access
            .get::<_, [u8]>(&state_db, rep_block)
            .to_opt()
            .unwrap()
        {
            let mut rep_bytes = [0u8; 32];
            rep_bytes.copy_from_slice(&state_block[64..96]);
            representative = Some(rep_bytes);
        }
        let representative = representative.expect("Representative block doesn't exist");
        let balance = &account_info[96..112];
        output_block(BlockInner::State {
            account: Account(account),
            previous: BlockHash(head_block),
            representative: Account(representative),
            balance: BigEndian::read_u128(balance),
            link: epoch_link,
        });
        current_kv = accounts_it.next::<[u8], [u8]>(&access).to_opt().unwrap();
    }
    let mut pending_it = txn.cursor(&pending_v0_db).unwrap();
    current_kv = pending_it.first::<[u8], [u8]>(&access).to_opt().unwrap();
    let mut seen_destinations = HashSet::new();
    while let Some((pending_key, _pending_info)) = current_kv {
        let destination = &pending_key[..32];
        if destination != &[0u8; 32] && seen_destinations.insert(destination) {
            let v0_acct_exists = access
                .get::<_, [u8]>(&accounts_v0_db, destination)
                .to_opt()
                .unwrap()
                .is_some();
            let v1_acct_exists = access
                .get::<_, [u8]>(&accounts_v1_db, destination)
                .to_opt()
                .unwrap()
                .is_some();
            if !v0_acct_exists && !v1_acct_exists {
                let mut destination_bytes = [0u8; 32];
                destination_bytes.copy_from_slice(destination);
                output_block(BlockInner::State {
                    account: Account(destination_bytes),
                    previous: BlockHash([0u8; 32]),
                    representative: Account([0u8; 32]),
                    balance: 0,
                    link: epoch_link,
                });
            }
        }
        current_kv = pending_it.next::<[u8], [u8]>(&access).to_opt().unwrap();
    }
}

extern crate atty;
extern crate blake2;
extern crate ed25519_dalek;
extern crate hex;
extern crate nanocurrency_types;
extern crate serde;
extern crate serde_json;

use blake2::Blake2b;
use ed25519_dalek::{Keypair, PublicKey, SecretKey};
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
    if atty::is(atty::Stream::Stderr) && atty::is(atty::Stream::Stdin) {
        eprint!("Please enter the signing private key: ");
    }
    let mut skey_str = String::new();
    io::stdin()
        .read_line(&mut skey_str)
        .expect("Failed to read private key from stdin");
    if skey_str.ends_with('\n') {
        skey_str.pop();
    }
    if skey_str.ends_with('\r') {
        skey_str.pop();
    }
    let skey = hex::decode(&skey_str).expect("Private key was not valid hex");
    let skey = SecretKey::from_bytes(&skey).expect("Private key was not 32 bytes long");
    let keypair = Keypair {
        public: PublicKey::from_secret::<Blake2b>(&skey),
        secret: skey,
    };
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut epoch_link = [0u8; 32];
    for (i, o) in b"epoch v1 block".iter().zip(epoch_link.iter_mut()) {
        *o = *i;
    }
    for block_inner in serde_json::Deserializer::from_reader(blocks_inner_file).into_iter() {
        let block_inner = block_inner.expect("Failed to read block inner from file");
        match block_inner {
            BlockInner::State { ref link, .. } if link == &epoch_link => {}
            _ => {
                panic!("Block link is not epoch link");
            }
        }
        let hash = block_inner.get_hash();
        let signature = keypair.sign::<Blake2b>(&hash.0);
        writeln!(
            stdout,
            "{}",
            &hex::encode_upper(&signature.to_bytes() as &[u8]),
        ).expect("Failed to write signature to stdout");
    }
}

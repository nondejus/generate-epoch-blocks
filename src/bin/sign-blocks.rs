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
use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

struct SignBlocksVisitor<'a, S: SerializeSeq + 'a>(Keypair, [u8; 32], &'a mut S);

impl<'a, S: SerializeSeq + 'a> Visitor<'a> for SignBlocksVisitor<'a, S> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an array of block inners")
    }

    fn visit_seq<A: SeqAccess<'a>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(block_inner) = seq.next_element::<BlockInner>()? {
            match block_inner {
                BlockInner::State { ref link, .. } if link == &self.1 => {}
                _ => {
                    return Err(serde::de::Error::custom("Block link is not epoch link"));
                }
            }
            let hash = block_inner.get_hash();
            let signature = self.0.sign::<Blake2b>(&hash.0);
            self.2
                .serialize_element(&hex::encode(&signature.to_bytes() as &[u8]))
                .map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}

fn main() {
    if atty::is(atty::Stream::Stdout) {
        eprintln!("Stdout is a terminal. This output is very large and does not have newlines.");
        eprintln!("Please pipe this somewhere.");
        process::exit(1);
    }
    let mut args = env::args();
    args.next();
    let blocks_inner_file = args
        .next()
        .expect("Expected blocks inner file as first argument");
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let mut blocks_inner_deser = serde_json::Deserializer::from_reader(blocks_inner_file);
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
    let stdout = stdout.lock();
    let mut output_ser = serde_json::Serializer::new(stdout);
    let mut output_seq = output_ser
        .serialize_seq(None)
        .expect("Failed to start serializing blocks output sequence");
    let mut epoch_link = [0u8; 32];
    for (i, o) in b"epoch v1 block".iter().zip(epoch_link.iter_mut()) {
        *o = *i;
    }
    {
        let visitor = SignBlocksVisitor(keypair, epoch_link, &mut output_seq);
        blocks_inner_deser
            .deserialize_seq(visitor)
            .expect("Failed to start deserializing blocks inner sequence");
    }
    output_seq
        .end()
        .expect("Failed to finish writing blocks into stdout");
}

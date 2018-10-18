extern crate atty;
extern crate hex;
extern crate nanocurrency_types;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use nanocurrency_types::BlockInner;
use serde::de::{Deserializer, SeqAccess, Visitor};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Write};

struct GenWorkVisitor<'a, W: Write + 'a>(&'a reqwest::Client, &'a str, usize, &'a mut W);

impl<'a, W: Write + 'a> Visitor<'a> for GenWorkVisitor<'a, W> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an array of block inners")
    }

    fn visit_seq<A: SeqAccess<'a>>(self, mut seq: A) -> Result<(), A::Error> {
        for _ in 0..self.2 {
            seq.next_element::<serde::de::IgnoredAny>()?;
        }
        while let Some(block_inner) = seq.next_element::<BlockInner>()? {
            #[derive(Serialize)]
            struct WorkGenerateReq<'a> {
                action: &'a str,
                hash: &'a str,
            }
            #[derive(Deserialize)]
            struct WorkGenerateRes {
                #[serde(default)]
                error: Option<String>,
                work: String,
            }
            let root = block_inner.root_bytes();
            let root_string = hex::encode_upper(root);
            let req = WorkGenerateReq {
                action: "work_generate",
                hash: &root_string,
            };
            let mut res = self
                .0
                .post(self.1)
                .json(&req)
                .send()
                .expect("Failed to send work_generate request to RPC")
                .json::<WorkGenerateRes>()
                .expect("Failed to parse RPC work_generate response");
            if let Some(error) = res.error {
                panic!("RPC work_generate returned error: {}", error);
            }
            writeln!(self.3, "{}", res.work).expect("Failed to write to stdout");
        }
        Ok(())
    }
}

fn main() {
    let mut args = env::args();
    args.next();
    let blocks_inner_file = args
        .next()
        .expect("Expected blocks inner file as first argument");
    let rpc_url = args.next().expect("Expected RPC URL as second argument");
    let skip_blocks = args
        .next()
        .unwrap_or_else(|| "0".into())
        .parse()
        .expect("Failed to parse third argument as number of blocks to skip");
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let mut blocks_inner_deser = serde_json::Deserializer::from_reader(blocks_inner_file);
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let req_client = reqwest::Client::new();
    {
        let visitor = GenWorkVisitor(&req_client, &rpc_url, skip_blocks, &mut stdout);
        blocks_inner_deser
            .deserialize_seq(visitor)
            .expect("Failed to start deserializing blocks inner sequence");
    }
}

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
use serde::ser::{SerializeSeq, Serializer};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

struct GenWorkVisitor<'a, S: SerializeSeq + 'a>(&'a reqwest::Client, &'a str, &'a mut S);

impl<'a, S: SerializeSeq + 'a> Visitor<'a> for GenWorkVisitor<'a, S> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an array of block inners")
    }

    fn visit_seq<A: SeqAccess<'a>>(self, mut seq: A) -> Result<(), A::Error> {
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
            let mut res = self.0
                .post(self.1)
                .json(&req)
                .send()
                .expect("Failed to send work_generate request to RPC")
                .json::<WorkGenerateRes>()
                    .expect("Failed to parse RPC work_generate response");
            if let Some(error) = res.error {
                panic!("RPC work_generate returned error: {}", error);
            }
            self.2
                .serialize_element(&res.work)
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
    let rpc_url = args
        .next()
        .expect("Expected RPC URL as second argument");
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let mut blocks_inner_deser = serde_json::Deserializer::from_reader(blocks_inner_file);
    let stdout = io::stdout();
    let stdout = stdout.lock();
    let mut output_ser = serde_json::Serializer::new(stdout);
    let mut output_seq = output_ser
        .serialize_seq(None)
        .expect("Failed to start serializing blocks output sequence");
    let req_client = reqwest::Client::new();
    {
        let visitor = GenWorkVisitor(&req_client, &rpc_url, &mut output_seq);
        blocks_inner_deser
            .deserialize_seq(visitor)
            .expect("Failed to start deserializing blocks inner sequence");
    }
    output_seq
        .end()
        .expect("Failed to finish writing blocks into stdout");
}

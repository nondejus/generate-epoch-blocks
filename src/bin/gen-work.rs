extern crate atty;
extern crate hex;
extern crate nanocurrency_types;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use nanocurrency_types::{work_threshold, work_value, BlockInner, Network};
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
            let res = loop {
                let mut sent_req = self.0.post(self.1).json(&req).send();
                let mut res = match sent_req {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!("Failed to send work_generate request to RPC: {}", err);
                        continue;
                    }
                };
                let res = match res.json::<WorkGenerateRes>() {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!("Failed to parse work_generate response: {}", err);
                        continue;
                    }
                };
                if let Some(error) = res.error {
                    eprintln!("RPC work_generate returned error: {}", error);
                    continue;
                }
                let work = match u64::from_str_radix(&res.work, 16) {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!(
                            "Failed to parse work_generate response work value as hex: {}",
                            err,
                        );
                        continue;
                    }
                };
                if work_value(root, work) < work_threshold(Network::Live) {
                    eprintln!(
                        "work_generate response doesn't meet threshold: root {} work {}",
                        root_string, res.work,
                    );
                    continue;
                }
                break res;
            };
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

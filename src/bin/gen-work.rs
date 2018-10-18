extern crate atty;
extern crate hex;
extern crate nanocurrency_types;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use nanocurrency_types::{work_threshold, work_value, BlockInner, Network};
use std::env;
use std::fs::File;
use std::io::{self, BufReader, Write};

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
    let mut blocks_inner = serde_json::Deserializer::from_reader(blocks_inner_file).into_iter();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let req_client = reqwest::Client::new();
    for _ in 0..skip_blocks {
        blocks_inner
            .next()
            .expect("Tried to skip more blocks inner than file has")
            .expect("Failed to read block inner from file");
    }
    for block_inner in blocks_inner {
        let block_inner: BlockInner = block_inner.expect("Failed to read block inner from file");
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
            let mut sent_req = req_client.post(&rpc_url).json(&req).send();
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
        writeln!(stdout, "{}", res.work).expect("Failed to write to stdout");
    }
}

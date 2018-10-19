extern crate hex;
extern crate nanocurrency_types;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio;

use nanocurrency_types::{work_threshold, work_value, BlockInner, Network};
use std::env;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::prelude::*;
use tokio::timer::Delay;

fn main() {
    let mut args = env::args();
    args.next();
    let blocks_inner_file = args
        .next()
        .expect("Expected blocks inner file as first argument");
    let rpc_url = Arc::new(args.next().expect("Expected RPC URL as second argument"));
    let skip_blocks = args
        .next()
        .unwrap_or_else(|| "0".into())
        .parse()
        .expect("Failed to parse third argument as number of blocks to skip");
    let parallel_requests = env::var("PARALLEL_REQUESTS")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .expect("Failed to parse PARALLEL_REQUESTS");
    let rpc_key = env::var("RPC_KEY").ok();
    let blocks_inner_file =
        BufReader::new(File::open(blocks_inner_file).expect("Failed to open blocks inner file"));
    let mut blocks_inner = serde_json::Deserializer::from_reader(blocks_inner_file).into_iter();
    let req_client = Arc::new(reqwest::async::Client::new());
    for _ in 0..skip_blocks {
        blocks_inner
            .next()
            .expect("Tried to skip more blocks inner than file has")
            .expect("Failed to read block inner from file");
    }
    let fut = stream::iter_result(blocks_inner.into_iter())
        .map_err(|x| panic!("Failed to read blocks inner from file: {}", x))
        .map(move |block_inner: BlockInner| {
            #[derive(Serialize)]
            struct WorkGenerateReq {
                action: &'static str,
                hash: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                key: Option<String>,
            }
            #[derive(Deserialize)]
            struct WorkGenerateRes {
                #[serde(default)]
                error: Option<String>,
                #[serde(default)]
                status: Option<String>,
                work: Option<String>,
            }
            let root: [u8; 32] = block_inner.into_root().into();
            let root_string = hex::encode_upper(root);
            let req = WorkGenerateReq {
                action: "work_generate",
                hash: root_string.clone(),
                key: rpc_key.clone(),
            };
            let req_client = req_client.clone();
            let rpc_url = rpc_url.clone();
            future::loop_fn((req, root, root_string), move |(req, root, root_string)| {
                req_client
                    .post(&*rpc_url)
                    .json(&req)
                    .send()
                    .and_then(|mut res| res.json::<WorkGenerateRes>())
                    .then(move |res| {
                        let res = match res {
                            Ok(x) => x,
                            Err(err) => {
                                eprintln!("Failed to call work_generate RPC: {}", err);
                                return Err((root, root_string));
                            }
                        };
                        let work = match res.work {
                            Some(work) => work,
                            None => {
                                if let Some(error) = res.error.or(res.status) {
                                    eprintln!("RPC work_generate returned error: {}", error);
                                    return Err((root, root_string));
                                }
                                eprintln!("RPC work_generate response didn't include `work`, `status`, or `error`");
                                return Err((root, root_string));
                            }
                        };
                        let work = match u64::from_str_radix(&work, 16) {
                            Ok(x) => x,
                            Err(err) => {
                                eprintln!(
                                    "Failed to parse work_generate response work value as hex: {}",
                                    err,
                                );
                                return Err((root, root_string));
                            }
                        };
                        if work_value(&root, work) < work_threshold(Network::Live) {
                            eprintln!(
                                "work_generate response doesn't meet threshold: root {} work {}",
                                &root_string, work,
                            );
                            return Err((root, root_string));
                        }
                        Ok(work)
                    }).then(move |x| match x {
                        Ok(res) => future::Either::A(future::ok(future::Loop::Break(res))),
                        Err((root, root_string)) => future::Either::B(
                            Delay::new(Instant::now() + Duration::from_secs(5))
                                .map_err(|e| panic!("Tokio timer error: {}", e))
                                .map(move |_| future::Loop::Continue((req, root, root_string))),
                        ),
                    })
            })
        }).buffered(parallel_requests)
        .for_each(|work| {
            writeln!(io::stdout(), "{}", work).expect("Failed to write to stdout");
            future::ok(())
        });
    tokio::run(fut);
}

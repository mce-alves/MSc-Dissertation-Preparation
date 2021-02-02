// mca @ 49828

use std::process;
use std::env;

#[path = "common/common_state.rs"] mod common;
#[path = "common/message.rs"] mod message;

#[path = "tests/multipaxos_tests.rs"] mod mptests;
#[path = "multipaxos/peer.rs"] mod multipaxos;

#[path = "tests/raft_tests.rs"] mod rtests;
#[path = "raft/peer.rs"] mod raft;

fn main() {
    let args:Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: cargo run <algorithm>");
        println!("Where <algorithm> can be one of the following: [multipaxospaxos, raft]");
        println!("");
        process::exit(0);
    }

    let algorithm = String::from(&args[1]);

    match algorithm.to_lowercase().as_str() {
        "multipaxos" => {
            mptests::test_nprocesses_multiple_proposals();
        },
        "raft" => {
            rtests::test_nprocesses_multiple_proposals();
        }
        _ => println!("Invalid algorithm.")
    }

}


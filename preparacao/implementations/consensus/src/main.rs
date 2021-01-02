// mca @ 49828

use std::process;
use std::env;

#[path = "tests/paxos_tests.rs"] mod ptests;
#[path = "paxos/messages.rs"] mod pmessage;
#[path = "paxos/agent.rs"] mod agent;

#[path = "tests/raft_tests.rs"] mod rtests;
#[path = "raft/messages.rs"] mod rmessage;
#[path = "raft/peer.rs"] mod raft;

fn main() {
    let args:Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: cargo run <algorithm>");
        println!("Where algorithm can be one of [paxos, raft]");
        println!("");
        process::exit(0);
    }

    let algorithm = String::from(&args[1]);

    match algorithm.to_lowercase().as_str() {
        "paxos" => {
            ptests::test_nprocesses_concurrent_proposals();
        },
        "raft" => {
            rtests::test_nprocesses_multiple_proposals();
        }
        _ => println!("Invalid algorithm.")
    }

}


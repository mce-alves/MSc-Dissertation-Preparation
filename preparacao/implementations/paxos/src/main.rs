// mca @ 49828

use std::process;
use std::env;

#[path = "tests/paxos_tests.rs"] mod tests;
#[path = "communication/message.rs"] mod message;
#[path = "paxos/agent.rs"] mod agent;

fn main() {
    let args:Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("Usage: cargo run <algorithm> <test>");
        println!("Where algorithm can be one of [paxos]");
        println!("Where test can be one of [single, concurrent] for one or concurrent proposals respectively");
        println!("");
        process::exit(0);
    }

    let algorithm = String::from(&args[1]);
    let test      = String::from(&args[2]);

    match algorithm.to_lowercase().as_str() {
        "paxos" => {
            match test.to_lowercase().as_str() {
                "single" => tests::test_nprocesses_single_proposal(),
                "concurrent" => tests::test_nprocesses_concurrent_proposals(),
                _ => println!("Invalid test.")
            } 
        },
        _ => println!("Invalid algorithm.")
    }

}


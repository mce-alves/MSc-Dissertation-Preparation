// mca @ 49828

use std::sync::mpsc;
use rand::Rng;
use std::process;
use std::io;

#[path = "communication/message.rs"] mod message;
#[path = "paxos/agent.rs"] mod agent;

static NUM_PROCESSES:i32   = 5;
static MAJORITY_QUORUM:i32 = 3;


fn main() {
    let mut channels:Vec<(mpsc::Sender<message::Message>, mpsc::Receiver<message::Message>)> = Vec::new();
    let mut agents = Vec::new();
    let mut membership:Vec<mpsc::Sender<message::Message>> = Vec::new();

    // create communication channels
    for _ in 0..NUM_PROCESSES {
        let (tx, rx) = mpsc::channel::<message::Message>();
        channels.push((tx.clone(), rx));
        membership.push(tx);
    }

    // create nodes
    for i in 0..NUM_PROCESSES {
        let (tx, rx) = channels.remove(0);
        // start one thread to act as each node
        agents.push(std::thread::spawn( {
            let mut node = agent::Agent::new(i, MAJORITY_QUORUM, tx, rx, membership.clone());
            move || {
                node.run();
            }
        }))
    }

    // handle console input (commands to start proposals)
    loop {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let split_input:Vec<&str> = input.split_whitespace().collect();
                if split_input.len() > 0 {
                    if split_input[0].to_string() == "begin" {
                        // choose a random node and send a BEGIN message to that node
                        let mut rng = rand::thread_rng();
                        let roll = rng.gen_range(0, NUM_PROCESSES);
                        membership[roll as usize].send(message::Message{
                            msg_type: message::MessageType::BEGIN,
                            prepare: None,
                            promise: None,
                            propose: None,
                            accepted: None,
                            rejected: None
                        }).unwrap();
                    }
                    else if split_input[0].to_string() == "exit" {
                        process::exit(0);
                    }
                    else {
                        println!("Invalid command.");
                    }
                }
                else {
                    println!("Invalid command.");
                }
            }
            Err(error) => println!("error: {}", error),
        }
    }

}


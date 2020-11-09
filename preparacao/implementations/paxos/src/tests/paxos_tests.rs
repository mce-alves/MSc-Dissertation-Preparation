// mca @ 49828

use std::sync::mpsc;
use std::thread::JoinHandle;
use rand::Rng;
use std::process;
use std::io;

use crate::message::*;
use crate::agent;

static NUM_PROCESSES:i32   = 5;
static MAJORITY_QUORUM:i32 = 3;

// create channels that agents will use to communicate, and return them
fn create_channels_membership() -> (Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)>,Vec<mpsc::Sender<Message>>) {
    let mut channels:Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)> = Vec::new();
    let mut membership:Vec<mpsc::Sender<Message>> = Vec::new();

    for _ in 0..NUM_PROCESSES {
        let (tx, rx) = mpsc::channel::<Message>(); // send to TX, read from RX
        channels.push((tx.clone(), rx));
        membership.push(tx);
    }

    return (channels, membership);
}

// create the agents that will be involved in the protocol
fn create_agents(mut channels:Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)>, membership:&Vec<mpsc::Sender<Message>>) -> Vec<JoinHandle<()>> {
    let mut agents = Vec::new();
    for i in 0..NUM_PROCESSES {
        let (tx, rx) = channels.remove(0);
        // start one thread to act as each agent
        agents.push(std::thread::spawn( {
            let mut node = agent::Agent::new(i, MAJORITY_QUORUM, tx, rx, membership.clone());
            move || {
                node.run();
            }
        }))
    }
    return agents;
}

// executes a test for NUM_PROCESSES with a single proposal
pub fn test_nprocesses_single_proposal() {
    let (channels, membership) = create_channels_membership();

    let agents = create_agents(channels, &membership);

    // handle console input (commands to start proposals)
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let split_input:Vec<&str> = input.split_whitespace().collect();
                if split_input.len() > 0 {
                    if split_input[0].to_string() == "begin" {
                        // choose a random node and send a BEGIN message to that node
                        let mut rng = rand::thread_rng();
                        let roll = rng.gen_range(0, NUM_PROCESSES);
                        membership[roll as usize].send(Message{
                            msg_type: MessageType::BEGIN,
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

    for ag in agents {
        let _ = ag.join();
    }

}

// executes a test for NUM_PROCESSES and concurrent proposals
pub fn test_nprocesses_concurrent_proposals() {
    // TODO
}
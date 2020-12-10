// mca @ 49828

use std::sync::mpsc;
use std::thread::JoinHandle;
use rand::Rng;

use crate::pmessage::*;
use crate::agent;

static NUM_PROCESSES:i32   = 500;
static MAJORITY_QUORUM:i32 = 251;

// create channels that agents will use to communicate, and return them
pub fn create_channels_membership() -> (Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)>,Vec<mpsc::Sender<Message>>) {
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
pub fn create_agents(mut channels:Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)>, membership:&Vec<mpsc::Sender<Message>>) -> Vec<JoinHandle<()>> {
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

    // choose a random node to be the proposer, and send a BEGIN message to that node
    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(0, NUM_PROCESSES);
    membership[roll as usize].send(Message{
        msg_type: MessageType::BEGIN,
        prepare: None,
        promise: None,
        propose: None,
        accepted: None,
        rejected: None,
        consensus: None
    }).unwrap();

    for ag in agents {
        let _ = ag.join();
    }

}

// executes a test for NUM_PROCESSES and 10 concurrent proposals (requires NUM_PROCESSES >= 10)
pub fn test_nprocesses_concurrent_proposals() {
    if NUM_PROCESSES < 10 {
        println!("This test requires more than 10 agents (processes).");
        return;
    }

    let (channels, membership) = create_channels_membership();
    let agents = create_agents(channels, &membership);

    for i in 0..10 {
        membership[i].send(Message{
            msg_type: MessageType::BEGIN,
            prepare: None,
            promise: None,
            propose: None,
            accepted: None,
            rejected: None,
            consensus: None
        }).unwrap();
    }

    for ag in agents {
        let _ = ag.join();
    }
}
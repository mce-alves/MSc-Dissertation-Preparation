// mca @ 49828

use std::sync::mpsc;
use std::thread::JoinHandle;
use rand::Rng;
use std::{thread, time};

use crate::message::*;
use crate::multipaxos;

static NUM_PROCESSES:i32 = 5;

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

// create the peers that will be involved in the protocol
pub fn create_peers(mut channels:Vec<(mpsc::Sender<Message>, mpsc::Receiver<Message>)>, membership:&Vec<mpsc::Sender<Message>>) -> Vec<JoinHandle<()>> {
    let mut agents = Vec::new();
    for i in 0..NUM_PROCESSES {
        let (tx, rx) = channels.remove(0);
        // start one thread to act as each peer
        agents.push(std::thread::spawn( {
            let mut node = multipaxos::Peer::new(i, rx, tx, membership.clone());
            move || {
                node.run();
            }
        }))
    }
    return agents;
}

// executes a test for NUM_PROCESSES with a single proposal
pub fn test_nprocesses_multiple_proposals() {
    let (channels, membership) = create_channels_membership();

    let agents = create_peers(channels, &membership);

    thread::sleep(time::Duration::from_secs(10));
    let mut rng = rand::thread_rng();
    for i in 0..10 {
        let roll = rng.gen_range(0, NUM_PROCESSES);
        membership[roll as usize].send(Message::REQOP(RequestOperation {
            operation: String::from(format!("SET X = {}",i))
        })).unwrap();
        thread::sleep(time::Duration::from_secs(1));
    }

    for ag in agents {
        let _ = ag.join();
    }

}
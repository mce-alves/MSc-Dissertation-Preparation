// mca @ 49828

use std::sync::mpsc;
use crate::message::*;

mod acceptor;
mod learner;
mod proposer;

// A node will be a process
// A process can act as a proposer, acceptor or learner

// Keeps track of the membership

// Receives the messages, and according to type of message acts as proposer, acceptor, etc. (calls their functions)


pub struct Node {
    pid: i32,                               // identifier of the node
    quorum_amount: i32,                     // needed responses to achieve majority quorum
    proposer: proposer::Proposer,           // proposer component/role of the node
    acceptor: acceptor::Acceptor,           // acceptor component/role of the node
    membership: Vec<mpsc::Sender<Message>>, // membership (known correct processes)
    tx: mpsc::Sender<Message>,              // this node's TX
    rx: mpsc::Receiver<Message>             // this node's RX
}

impl Node {

    pub fn new(t_pid:i32, t_quorum_amount:i32, t_tx:mpsc::Sender<Message>, t_rx:mpsc::Receiver<Message>, t_membership:Vec<mpsc::Sender<Message>>) -> Node {
        Node {
            pid: t_pid,
            quorum_amount: t_quorum_amount,
            membership: t_membership.clone(),
            tx: t_tx.clone(),
            rx: t_rx,
            proposer: proposer::Proposer::new(t_pid, t_quorum_amount, t_tx.clone(), t_membership.clone()),
            acceptor: acceptor::Acceptor::new(t_pid, t_tx, t_membership)
        }
    }

    pub fn run(&mut self) {
        for msg in self.rx.iter() {
            match msg.msg_type {
                MessageType::PREPARE => self.acceptor.rcv_prepare(msg),
                MessageType::PROMISE => self.proposer.rcv_promise(msg),
                MessageType::PROPOSE => self.acceptor.rcv_propose(msg),
                MessageType::ACCEPTED => self.proposer.rcv_accept(msg),
                MessageType::REJECTED => self.proposer.rcv_reject(msg)
            }
        }
    }


}
// mca @ 49828

use std::sync::mpsc;
use crate::pmessage::*;

mod acceptor;
mod learner;
mod proposer;

// A agent will be a process
// A process can act as a proposer, acceptor or learner

// Keeps track of the membership

// Receives the messages, and according to type of message acts as proposer, acceptor, etc. (calls their functions)


pub struct Agent {
    proposer: proposer::Proposer,           // proposer component/role of the agent
    acceptor: acceptor::Acceptor,           // acceptor component/role of the agent
    learner: learner::Learner,              // learner component/role of the agent
    rx: mpsc::Receiver<Message>             // this agent's RX
}

impl Agent {

    pub fn new(t_pid:i32, t_quorum_amount:i32, t_tx:mpsc::Sender<Message>, t_rx:mpsc::Receiver<Message>, t_membership:Vec<mpsc::Sender<Message>>) -> Agent {
        Agent {
            rx: t_rx,
            proposer: proposer::Proposer::new(t_pid, t_quorum_amount, t_tx.clone(), t_membership.clone()),
            acceptor: acceptor::Acceptor::new(t_pid, t_tx),
            learner: learner::Learner::new(t_pid, t_membership)
        }
    }

    pub fn run(&mut self) {
        for msg in self.rx.iter() {
            match msg.msg_type {
                MessageType::PREPARE => self.acceptor.rcv_prepare(msg),
                MessageType::PROMISE => self.proposer.rcv_promise(msg),
                MessageType::PROPOSE => self.acceptor.rcv_propose(msg),
                MessageType::ACCEPTED => {
                    self.proposer.rcv_accept(msg.clone());
                    self.learner.rcv_accept(msg);
                },
                MessageType::REJECTED => self.proposer.rcv_reject(msg),
                MessageType::BEGIN => {
                    self.proposer.snd_prepare();
                    self.learner.set_distinguished_status(true);
                }
            }
        }
    }


}
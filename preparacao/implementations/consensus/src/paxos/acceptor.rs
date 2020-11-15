// mca @ 49828

use std::sync::mpsc;
use crate::pmessage::*;

enum AcceptorState {
    IDLE,
    PROMISED,
    ACCEPTED
}

pub struct Acceptor {
    pid:i32,                                    // id of the acceptor
    state: AcceptorState,                       // state / phase of the acceptor 
    max_id: f32,                                // highest ID seen so far
    tx:mpsc::Sender<Message>,                   // this proposers TX
    accepted_val:Option<i32>,                   // already accepted value
    accepted_id:Option<f32>                     // id of the propose of the accepted value
}


impl Acceptor {

    pub fn new(t_pid:i32, t_tx:mpsc::Sender<Message>) -> Acceptor {
        Acceptor {
            pid: t_pid,
            state: AcceptorState::IDLE,
            max_id: -1.0,
            tx: t_tx,
            accepted_val: None,
            accepted_id: None
        }
    }

    // send PROMISE message to target
    pub fn snd_promise(&mut self, target:mpsc::Sender<Message>, target_pid:i32) -> () {
        // can send promise in any state, as long as max_id > -1 (has received some prepare)
        if self.max_id > -1.0 {
            println!("Acceptor {} sending promise with id={} to Proposer {}.", self.pid, self.max_id, target_pid);
            target.send(self.create_promise_msg()).unwrap();
            match self.state {
                AcceptorState::IDLE => {
                    // update state to promised if currently in IDLE state
                    self.state = AcceptorState::PROMISED;
                },
                _ => {}
            }
        }
        else {
            println!("Acceptor {} cannot send promise for max_id {}.", self.pid, self.max_id);
        }
    }

    // send ACCEPTED message to target
    pub fn snd_accept(&mut self, target:mpsc::Sender<Message>, target_pid:i32) -> () {
        match self.state {
            AcceptorState::ACCEPTED => println!("Acceptor {} has already accepted a proposal.", self.pid),
            AcceptorState::PROMISED => {
                match (self.accepted_id, self.accepted_val) {
                    (Some(id), Some(val)) => {
                        println!("Acceptor {} sending accepted with id={}, val={} to Proposer {}.", self.pid, id, val,target_pid);
                        target.send(self.create_accept_msg(id, val)).unwrap();
                        self.state = AcceptorState::ACCEPTED; // update state to ACCEPTED
                    },
                    _ => println!("Acceptor {} cannot send ACCEPTED because it is missing id or val.", self.pid)
                }
            },
            AcceptorState::IDLE => println!("Acceptor {} cannot accept a proposal while in IDLE state.", self.pid)
        }
    }

    // send REJECTED message to target
    pub fn snd_reject(&self, rejected_id:f32, target:mpsc::Sender<Message>) -> () {
        target.send(self.create_reject_msg(rejected_id)).unwrap();
    }

    // process a received PREPARE message
    pub fn rcv_prepare(&mut self, msg:Message) -> () {
        match (msg.msg_type, msg.prepare) {
            (MessageType::PREPARE, Some(prepare)) => {
                println!("Acceptor {} received prepare from Proposer {} with id={}.", self.pid, prepare.sender_pid, prepare.id);
                match self.state {
                    AcceptorState::IDLE => {
                        // idle means no prepare has been received yet, so we can promise any received ID
                        self.max_id = prepare.id;
                        self.snd_promise(prepare.sender, prepare.sender_pid); // also updates state to PROMISED
                    },
                    AcceptorState::PROMISED => {
                        // in promised state, we only send promise if ID is the highest we have seen so far
                        if prepare.id > self.max_id {
                            // can make promise
                            self.max_id = prepare.id;
                            self.snd_promise(prepare.sender, prepare.sender_pid);
                        }
                        else {
                            // reject the prepare message
                            self.snd_reject(prepare.id, prepare.sender);
                        }
                    },
                    AcceptorState::ACCEPTED => {
                        // accepted state means a proposal has already been accepted
                        // send a promise (which will include the accepted_id and accepted_val) so that the following proposals will use that same value in order to ensure P2b
                        // Property 2b : if a proposal with value V is chosen, then every higher-numbered proposal issued by any proposer has value V
                        self.snd_promise(prepare.sender, prepare.sender_pid);
                    }
                }
            },
            _ => println!("Acceptor {} received invalid prepare message.", self.pid)
        }
    }

    // process a received PROPOSE message
    pub fn rcv_propose(&mut self, msg:Message) -> () {
        match self.state {
            AcceptorState::PROMISED => {
                match (msg.msg_type, msg.propose) {
                    (MessageType::PROPOSE, Some(proposal)) => {
                        println!("Acceptor {} received proposal from Proposer {} with id={}, val={}.", self.pid, proposal.sender_pid, proposal.id, proposal.value);
                        // accept the proposal if it contains the highest ID for which we received a prepare
                        // P1a : an acceptor can accept a proposal with id N if it has not responded to a prepare request having number greater than N
                        if proposal.id >= self.max_id {
                            // SAFETY PROPERTY : only a value that has been proposed may be chosen
                            self.accepted_id = Some(proposal.id);
                            self.accepted_val = Some(proposal.value);
                            self.max_id = proposal.id;
                            self.snd_accept(proposal.sender, proposal.sender_pid); // also updates state to ACCEPTED
                            // since this updates the state to accepted, we guarantee another safety property
                            // SAFETY PROPERTY : only a single value is chosen
                        }
                        else {
                            // reject the proposal
                            self.snd_reject(proposal.id, proposal.sender);
                        }
                    },
                    _ => println!("Acceptor {} received invalid PROPOSE message.", self.pid)
                }
            },
            _ => println!("Acceptor {} cannot process PROPOSE message in IDLE or ACCEPTED state.", self.pid),
        }
    }

    // create promise message
    fn create_promise_msg(&self) -> Message {
        Message {
            msg_type: MessageType::PROMISE,
            prepare: None,
            promise: Some(Promise {
                accepted_id: self.accepted_id,
                accepted_value: self.accepted_val,
                id: self.max_id,
                sender: self.tx.clone(),
                sender_pid: self.pid
            }),
            propose: None,
            accepted: None,
            rejected: None,
            consensus: None
        }
    }

    // create accept message
    fn create_accept_msg(&self, accepted_id:f32, accepted_val:i32) -> Message {
        Message {
            msg_type: MessageType::ACCEPTED,
            prepare: None,
            promise: None,
            propose: None,
            accepted: Some(Accepted {
                id: accepted_id,
                sender_pid: self.pid,
                sender: self.tx.clone(),
                value: accepted_val
            }),
            rejected: None,
            consensus: None
        }
    }

    // create reject message
    fn create_reject_msg(&self, rejected_id:f32) -> Message {
        Message {
            msg_type: MessageType::REJECTED,
            prepare: None,
            promise: None,
            propose: None,
            accepted: None,
            consensus: None,
            rejected: Some(Rejected {
                id: rejected_id,
                max_id: self.max_id,
                sender_pid: self.pid
            })
        }
    }

}
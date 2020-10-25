// mca @ 49828

use std::sync::mpsc;
use std::sync::Arc;
use crate::message::*;

enum AcceptorState {
    IDLE,
    PROMISED,
    ACCEPTED
}

struct Acceptor {
    pid:i32,                                    // id of the acceptor
    state: AcceptorState,                       // state / phase of the acceptor 
    max_id: i32,                                // highest ID seen so far
    tx:mpsc::Sender<Message>,                   // this proposers TX
    membership:Vec<mpsc::Sender<Message>>,      // membership (known correct processes)
    accepted_val:Option<i32>,                   // already accepted value
    accepted_id:Option<i32>                     // id of the propose of the accepted value
}


impl Acceptor {

    fn new(t_pid:i32, t_tx:mpsc::Sender<Message>, mship:Vec<mpsc::Sender<Message>>) -> Acceptor {
        Acceptor {
            pid: t_pid,
            state: AcceptorState::IDLE,
            max_id: -1,
            tx: t_tx,
            membership: mship,
            accepted_val: None,
            accepted_id: None
        }
    }

    // send PROMISE message to target
    fn snd_promise(&mut self, target:mpsc::Sender<Message>) -> () {
        // can send promise in any state, as long as max_id > -1 (as received some prepare)
        if self.max_id > -1 {
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
    fn snd_accept(&mut self, target:mpsc::Sender<Message>) -> () {
        match self.state {
            AcceptorState::ACCEPTED => println!("Acceptor {} has already accepted a proposal.", self.pid),
            AcceptorState::PROMISED => {
                match (self.accepted_id, self.accepted_val) {
                    (Some(id), Some(val)) => {
                        target.send(self.create_accept_msg(id, val)).unwrap();
                        // update state to ACCEPTED
                        self.state = AcceptorState::ACCEPTED;
                    },
                    _ => println!("Acceptor {} cannot send ACCEPTED because it is missing id or val.", self.pid)
                }
            },
            AcceptorState::IDLE => println!("Acceptor {} cannot accept a proposal while in IDLE state.", self.pid)
        }
    }

    // send REJECTED message to target
    fn snd_reject(&mut self, rejected_id:i32, target:mpsc::Sender<Message>) -> () {
        target.send(self.create_reject_msg(rejected_id)).unwrap();
    }

    // process a received PREPARE message
    fn rcv_prepare(&mut self, msg:Message) -> () {
        match (msg.msg_type, msg.prepare) {
            (MessageType::PREPARE, Some(prepare)) => {
                match self.state {
                    AcceptorState::IDLE => {
                        // idle means no prepare has been received yet, so we can promise any received ID
                        self.max_id = prepare.id;
                        self.snd_promise(prepare.sender.clone()); // also updates state to PROMISED
                    },
                    AcceptorState::PROMISED => {
                        // in promised state, we only send promise if ID is the highest we have seen so far
                        if prepare.id > self.max_id {
                            // can make promise
                            self.max_id = prepare.id;
                            self.snd_promise(prepare.sender.clone());
                        }
                        else {
                            // reject the prepare message
                            self.snd_reject(prepare.id, prepare.sender.clone());
                        }
                    },
                    AcceptorState::ACCEPTED => {
                        // accepted state means a proposal has already been accepted
                        // send a promise (which will include the accepted_id and accepted_val)
                        self.snd_promise(prepare.sender.clone());
                    }
                }
            },
            _ => println!("Acceptor {} received invalid prepare message.", self.pid)
        }
    }

    // process a received PROPOSE message
    fn rcv_propose(&mut self, msg:Message) -> () {
        match self.state {
            AcceptorState::PROMISED => {
                match (msg.msg_type, msg.propose) {
                    (MessageType::PROPOSE, Some(proposal)) => {
                        // accept the proposal if it is using the highest ID for which we received a prepare
                        if proposal.id == self.max_id {
                            self.accepted_id = Some(proposal.id);
                            self.accepted_val = Some(proposal.value);
                            self.snd_accept(proposal.sender.clone()); // also updates state to ACCEPTED
                        }
                        else {
                            // reject the proposal
                            self.snd_reject(proposal.id, proposal.sender.clone());
                        }
                    },
                    _ => println!("Acceptor {} received invalid PROPOSE message.", self.pid)
                }
            },
            _ => println!("Acceptor {} cannot process PROPOSE message in IDLE or ACCEPTED state.", self.pid),
        }
    }

    // create promise message
    fn create_promise_msg(&mut self) -> Message {
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
            rejected: None
        }
    }

    // create accept message
    fn create_accept_msg(&mut self, accepted_id:i32, accepted_val:i32) -> Message {
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
            rejected: None
        }
    }

    // create reject message
    fn create_reject_msg(&mut self, rejected_id:i32) -> Message {
        Message {
            msg_type: MessageType::REJECTED,
            prepare: None,
            promise: None,
            propose: None,
            accepted: None,
            rejected: Some(Rejected {
                id: rejected_id,
                max_id: self.max_id
            })
        }
    }

}
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

    // post condition for snd_promise
    fn post_snd_promise(&mut self) {
        match self.state {
            AcceptorState::IDLE => {
                // update state to promised if currently in IDLE state
                self.state = AcceptorState::PROMISED;
            },
            _ => {}
        }
    }

    // send PROMISE message to target
    pub fn snd_promise(&mut self, target:mpsc::Sender<Message>, target_pid:i32) -> () {
        println!("Acceptor {} sending promise with id={} to Proposer {}.", self.pid, self.max_id, target_pid);
        target.send(self.create_promise_msg()).unwrap();
        self.post_snd_promise();
    }


    // pre condition for snd_accept
    fn pre_snd_accept(&self) -> bool {
        match self.state {
            AcceptorState::ACCEPTED => {
                println!("Acceptor {} has already accepted a proposal.", self.pid);
                false
            },
            AcceptorState::PROMISED => {
                true
            },
            AcceptorState::IDLE => {
                println!("Acceptor {} cannot accept a proposal while in IDLE state.", self.pid);
                false
            }
        }
    }


    // post condition for snd_accept
    fn post_snd_accept(&mut self) {
        self.state = AcceptorState::ACCEPTED; // update state to ACCEPTED
    }

    // send ACCEPTED message to target
    pub fn snd_accept(&mut self, target:mpsc::Sender<Message>, target_pid:i32) -> () {
        if self.pre_snd_accept() {
            match (self.accepted_id, self.accepted_val) {
                (Some(id), Some(val)) => {
                    println!("Acceptor {} sending accepted with id={}, val={} to Proposer {}.", self.pid, id, val,target_pid);
                    target.send(self.create_accept_msg(id, val)).unwrap();
                    self.post_snd_accept();
                },
                _ => println!("Acceptor {} cannot send ACCEPTED because it is missing id or val.", self.pid)
            }
        }
    }

    // send REJECTED message to target
    pub fn snd_reject(&self, rejected_id:f32, target:mpsc::Sender<Message>) -> () {
        target.send(self.create_reject_msg(rejected_id)).unwrap();
    }

    // process a received PREPARE message
    pub fn rcv_prepare(&mut self, prepare:Prepare) -> () {
        println!("Acceptor {} received prepare from Proposer {} with id={}.", self.pid, prepare.sender_pid, prepare.id);
        match self.state {
            AcceptorState::IDLE => {
                // idle means no prepare has been received yet, so we can promise any received ID
                self.max_id = prepare.id;
                self.snd_promise(prepare.sender, prepare.sender_pid); // also updates state to PROMISED
            },
            _ => {
                // in promised state, we only send promise if ID is the highest we have seen so far
                // in accepted state the behaviour is the same, but it means a proposal has already been accepted
                // send a promise (which will include the accepted_id and accepted_val if in accepted state)
                // so that the following proposals will use that same value in order to ensure P2b
                // Property 2b : if a proposal with value V is chosen, then every higher-numbered proposal issued by any proposer has value V
                if prepare.id > self.max_id {
                    // can make promise
                    self.max_id = prepare.id;
                    self.snd_promise(prepare.sender, prepare.sender_pid);
                }
                else {
                    // reject the prepare message
                    self.snd_reject(prepare.id, prepare.sender);
                }
            }
        }
    }


    // pre condition for rcv_propose
    fn pre_rcv_propose(&self) -> bool {
        match self.state {
            AcceptorState::PROMISED => {
                true
            },
            _ => {
                println!("Acceptor {} cannot process PROPOSE message in IDLE or ACCEPTED state.", self.pid);
                false
            }
        }
    }

    // process a received PROPOSE message
    pub fn rcv_propose(&mut self, proposal:Propose) -> () {
        if self.pre_rcv_propose() {
            println!("Acceptor {} received proposal from Proposer {} with id={}, val={}.", self.pid, proposal.sender_pid, proposal.id, proposal.value);
            // accept the proposal if it contains the highest ID for which we received a prepare
            // P1a : an acceptor can accept a proposal with id N if it has not responded to a prepare request having number greater than N
            if proposal.id >= self.max_id {
                self.accept_proposal(proposal);
            }
            else {
                // reject the proposal
                self.snd_reject(proposal.id, proposal.sender);
            }
        }
    }

    // pre condition for accept_proposal
    fn pre_accept_proposal(&self, proposal_id:f32) -> bool {
        proposal_id >= self.max_id
    }

    // post condition for accept_proposal
    fn post_accept_proposal(&mut self, proposal_id:f32, proposal_value:i32) {
        self.accepted_id = Some(proposal_id);
        self.accepted_val = Some(proposal_value);
        self.max_id = proposal_id;
    }

    // accepts a proposal
    fn accept_proposal(&mut self, proposal:Propose) {
        if self.pre_accept_proposal(proposal.id) {
            // SAFETY PROPERTY : only a value that has been proposed may be chosen
            self.post_accept_proposal(proposal.id, proposal.value);
            self.snd_accept(proposal.sender, proposal.sender_pid); // also updates state to ACCEPTED
            // since this updates the state to accepted, we guarantee another safety property
            // SAFETY PROPERTY : only a single value is chosen
        }
    }


    // create promise message
    fn create_promise_msg(&self) -> Message {
        return Message::PROMISE(Promise {
            accepted_id: self.accepted_id,
            accepted_value: self.accepted_val,
            id: self.max_id,
            sender: self.tx.clone(),
            sender_pid: self.pid
        });
    }

    // create accept message
    fn create_accept_msg(&self, accepted_id:f32, accepted_val:i32) -> Message {
        return Message::ACCEPTED(Accepted {
            id: accepted_id,
            sender_pid: self.pid,
            sender: self.tx.clone(),
            value: accepted_val
        });
    }

    // create reject message
    fn create_reject_msg(&self, rejected_id:f32) -> Message {
        return Message::REJECTED(Rejected {
            id: rejected_id,
            max_id: self.max_id,
            sender_pid: self.pid
        });
    }

}
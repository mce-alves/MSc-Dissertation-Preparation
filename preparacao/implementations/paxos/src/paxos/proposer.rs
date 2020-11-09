// mca @ 49828

use std::sync::mpsc;
use crate::message::*;

enum ProposerState {
    IDLE,
    PREPARED,
    PROPOSED,
    ACCEPTED
}

pub struct Proposer {
    pid:i32,                               // id of the proposer
    state: ProposerState,                  // state / phase of the proposer 
    id: f32,                               // id that will be associated with a prepare/propose request
    quorum_amount: i32,                    // needed responses to achieve majority quorum
    tx: mpsc::Sender<Message>,             // this proposers TX
    rcvd_promises: Vec<Promise>,           // vec of PID of processes that sent a PROMISE
    rcvd_accepts: Vec<Accepted>,           // vec of PID of processes that sent an ACCEPT
    membership: Vec<mpsc::Sender<Message>> // membership (known correct processes)
}


impl Proposer {

    pub fn new(t_pid:i32, t_quorum_amount:i32, t_tx:mpsc::Sender<Message>, t_membership:Vec<mpsc::Sender<Message>>) -> Proposer {
        Proposer {
            pid: t_pid,
            state: ProposerState::IDLE,
            id: (t_pid as f32)/1000.0,
            quorum_amount: t_quorum_amount,
            tx: t_tx,
            rcvd_promises: vec!(),
            rcvd_accepts: vec!(),
            membership: t_membership
        }
    }

    // send a PREPARE message
    pub fn snd_prepare(&mut self) -> () {
        match self.state {
            ProposerState::IDLE => {
                // generate new high id
                self.id = self.id + 1.0;
                let msg = self.create_prepare_msg();
                // broadcast the PREPARE message
                println!("Proposer {} broadcasting PREPARE with id={}.", self.pid, self.id);
                broadcast((self.membership).clone(), msg);
                // update state
                self.state = ProposerState::PREPARED;
            },
            _ => {
                println!("Proposer {} cannot PREPARE since it is not in IDLE state.", self.pid);
            }
        }
    }

    // send a PROPOSE message
    pub fn snd_propose(&mut self, val:i32) -> () {
        match self.state {
            ProposerState::PREPARED => {
                let msg = self.create_propose_msg(val);
                // broadcast the PROPOSE message
                println!("Proposer {} broadcasting PROPOSE with id={}, val={}.", self.pid, self.id, val);
                broadcast(self.membership.clone(), msg);
                // update state
                self.state = ProposerState::PROPOSED;
            },
            _ => println!("Proposer {} cannot send PROPOSE because it is not in PREPARED state.", self.pid)
        }
    }

    // process a received PROMISE message
    pub fn rcv_promise(&mut self, msg:Message) -> () {
        match self.state {
            ProposerState::PREPARED => {
                match (msg.msg_type, msg.promise) {
                    (MessageType::PROMISE, Some(promise)) => {
                        println!("Proposer {} received promise from Acceptor {} for id={}.", self.pid, promise.sender_pid, promise.id);

                        let already_received = self.check_received_promises(promise.sender_pid);
                        if !already_received {
                            // add to received promises if not previously received
                            self.rcvd_promises.push(promise);
                        }
                        if self.rcvd_promises.len() as i32 >= self.quorum_amount {
                            // proposer has received majority quorum of promises, therefore it can propose a value
                            let propose_val = self.check_promises_contain_value(); // check if promises contain accepted value, and return the value that should be proposed
                            self.snd_propose(propose_val);
                        }
                    },
                    _ => println!("Proposer {} received invalid PROMISE.", self.pid)
                }
            },
            _ => println!("Proposer {} cannot receive PROMISE since it is not in PREPARED state.", self.pid)
        }
    }

    // process a received ACCEPTED message
    pub fn rcv_accept(&mut self, msg:Message) -> () {
        match self.state {
            ProposerState::PROPOSED => {
                match (msg.msg_type, msg.accepted) {
                    (MessageType::ACCEPTED, Some(accepted)) => {
                        println!("Proposer {} received accepted from Acceptor {} for id={}, val={}.", self.pid, accepted.sender_pid, accepted.id, accepted.value);
                        let already_received = self.check_received_accepts(accepted.sender_pid);
                        let v = accepted.value;

                        if !already_received {
                            // add to the vec of received accepted messages
                            self.rcvd_accepts.push(accepted);
                        }
                        if self.rcvd_accepts.len() as i32 >= self.quorum_amount {
                            // we received ACCEPT from majority, so consensus as been reached
                            for m in self.rcvd_accepts.as_slice() {
                                assert!(m.value == v); // all accept messages need to accept the same value!
                            }
                            println!("Proposer {} has reached consensus on value {}.", self.pid, v);
                            self.state = ProposerState::ACCEPTED; // update state
                            self.clear_data(); // clear protocol data
                        }
                    },
                    _ => println!("Proposer {} received invalid ACCEPTED message.", self.pid)
                }
            },
            _ => println!("Proposer {} cannot receive ACCEPTED since it is not in PROPOSED state.", self.pid)
        }
    }

    // process a received REJECTED message (for a PREPARE)
    pub fn rcv_reject(&mut self, msg:Message) -> () {
        match self.state { 
            ProposerState::PREPARED => {
                match (msg.msg_type, msg.rejected) {
                    (MessageType::REJECTED, Some(rejected)) => {
                        println!("Proposer {} received rejected from Acceptor {} for id={}.", self.pid, rejected.sender_pid, rejected.id);
                        // if we receive a rejected message, do nothing
                    },
                    _ => println!("Proposer {} received invalid REJECTED message.", self.pid)
                }
            },
            _ => println!("Proposer {} cannot receive REJECTED since it is not in PREPARED state.", self.pid)
        }
    }

    // checks if the proposer already received an accept message from this same acceptor (using it's pid)
    fn check_received_accepts(&mut self, sender_pid:i32) -> bool {
        for m in self.rcvd_accepts.as_slice() {
            if m.sender_pid == sender_pid {
                // already received accept from this process
                return true;
            }
        }
        return false;
    }

    // checks if the proposer already received a promise from this same acceptor (using it's pid)
    fn check_received_promises(&mut self, sender_pid:i32) -> bool {
        for m in self.rcvd_promises.as_slice() {
            if m.sender_pid == sender_pid {
                // already received promise from this process
                return true;
            }
        }
        return false;
    }

    // checks if any of the promises received contain an already accepted value (to ensure the P2c invariant)
    // if they do, returns that value (which will be proposed), else returns this proposer's pid (which will be proposed)
    fn check_promises_contain_value(&mut self) -> i32 {
        let mut highest_id = -1.0;
        let mut propose_val = self.pid;
        for m in self.rcvd_promises.as_slice() {
            // check if the promise said that the acceptor had already accepted another value
            // Property 2c : for any V and N, if a proposal with value V and id N is issued, then there is a set of majority acceptors where (a) none accepted a proposal numbered less than N, or (b) V is the value of the highest-numbered proposal among all proposals accepted by the majority
            match (m.accepted_value, m.accepted_id) {
                (Some(av), Some(ai)) => {
                    if ai > highest_id {
                        highest_id = ai;
                        propose_val = av;
                        while highest_id > self.id {
                            self.id += 1.0;
                        }
                    }
                },
                _ => {}
            }
        }
        return propose_val;
    }

    // clear data used in the protocol
    fn clear_data(&mut self) -> () {
        self.rcvd_promises.clear();
        self.rcvd_accepts.clear();
    }

    // create a prepare message
    fn create_prepare_msg(&mut self) -> Message {
        Message{
            msg_type: MessageType::PREPARE,
            prepare: Some(Prepare{
                id: self.id,
                sender_pid: self.pid,
                sender: self.tx.clone(),
            }),
            promise: None,
            propose: None,
            accepted: None,
            rejected: None
        }
    }

    // create a propose message with value <v>
    fn create_propose_msg(&mut self, v:i32) -> Message {
        Message{
            msg_type: MessageType::PROPOSE,
            prepare: None,
            promise: None,
            propose: Some(Propose{
                id: self.id,
                sender_pid: self.pid,
                sender: self.tx.clone(),
                value: v
            }),
            accepted: None,
            rejected: None
        }
    }

}
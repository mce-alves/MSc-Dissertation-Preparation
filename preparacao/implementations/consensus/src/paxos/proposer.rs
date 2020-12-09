// mca @ 49828

use std::sync::mpsc;
use crate::pmessage::*;

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
    propose_val: i32,                      // value that will be proposed
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
            propose_val: t_pid,
            quorum_amount: t_quorum_amount,
            tx: t_tx,
            rcvd_promises: vec!(),
            rcvd_accepts: vec!(),
            membership: t_membership
        }
    }


    // pre condition for snd_prepare
    fn pre_prepare(&mut self) -> bool {
        match self.state {
            ProposerState::IDLE => {
                // generate new high id
                self.id = self.id + 1.0;
                true
            },
            _ => false
        }
    }

    // post condition for snd_prepare
    fn post_prepare(&mut self) {
        // update state
        self.state = ProposerState::PREPARED;
    }

    // send a PREPARE message
    pub fn snd_prepare(&mut self) -> () {
        if self.pre_prepare() {
            let msg = self.create_prepare_msg();
            // broadcast the PREPARE message
            println!("Proposer {} broadcasting PREPARE with id={}.", self.pid, self.id);
            broadcast(&self.membership, msg);
            self.post_prepare();
        }
        else {
            println!("Proposer {} : pre-condition failed for snd_prepare.", self.pid);
        }
    }

    // pre condition for snd_propose
    fn pre_propose(&mut self) -> bool {
        match self.state {
            ProposerState::PREPARED => {
                if self.rcvd_promises.len() as i32 >= self.quorum_amount {
                    true // have received majority of accepts
                }
                else {
                    false // no majority
                }
            },
            _ => false // invalid state
        }
    }

    // post condition for snd_propose
    fn post_propose(&mut self) {
        // update state
        self.state = ProposerState::PROPOSED;
    }

    // send a PROPOSE message
    pub fn snd_propose(&mut self) -> () {
        if self.pre_propose() {
            let msg = self.create_propose_msg();
            // check if received promises contain an already accepted value
            self.check_promises_contain_value();
            // broadcast the PROPOSE message
            println!("Proposer {} broadcasting PROPOSE with id={}, val={}.", self.pid, self.id, self.propose_val);
            broadcast(&self.membership, msg);
            self.post_propose();
        }
        else {
            println!("Proposer {} : pre-condition failed for snd_propose.", self.pid)
        }
    }

    // informs the learner that this proposer has gotten consensus on a value
    pub fn inform_learners(&self, id:f32, value:i32) {
        match self.state {
            ProposerState::ACCEPTED => broadcast(&self.membership, self.create_consensus_msg(id, value)),
            _ => println!("Proposer {} cannot inform learners because it is not in ACCEPTED state.", self.pid)
        }
    }


    // pre condition for rcv_promise
    fn pre_rcv_promise(&self, msg:&Message) -> bool {
        match self.state {
            ProposerState::PREPARED => {
                match(&msg.msg_type, &msg.promise) {
                    (MessageType::PROMISE, Some(promise)) => {
                        self.check_received_promises(promise.sender_pid) == false // valid pre-condition if we haven't received a promise from this peer
                    },
                    _ => false // invalid message format
                }
            },
            _ => false // invalid state
        }
    }

    // post condition for rcv_promise
    fn post_rcv_promise(&mut self, promise:Promise) {
        // add to received promise
        self.rcvd_promises.push(promise);
    }

    // process a received PROMISE message
    pub fn rcv_promise(&mut self, msg:Message) -> () {
        if self.pre_rcv_promise(&msg) {
            match (msg.msg_type, msg.promise) {
                (MessageType::PROMISE, Some(promise)) => {
                    println!("Proposer {} received promise from Acceptor {} for id={}.", self.pid, promise.sender_pid, promise.id);

                    self.post_rcv_promise(promise);
                    
                    if self.rcvd_promises.len() as i32 >= self.quorum_amount {
                        // proposer has received majority quorum of promises, therefore it can propose a value
                        self.snd_propose();
                    }
                },
                _ => println!("Proposer {} received invalid PROMISE.", self.pid)
            }
        }
        else {
            println!("Proposer {} cannot receive PROMISE since it is not in PREPARED state.", self.pid)
        }
    }


    // pre condition for rcv_accept
    fn pre_rcv_accept(&self, msg:&Message) -> bool {
        match self.state {
            ProposerState::PROPOSED => {
                match(&msg.msg_type, &msg.accepted) {
                    (MessageType::ACCEPTED, Some(accept)) => {
                        self.check_received_accepts(accept.sender_pid) == false // valid pre-condition if we haven't received an accepted from this peer
                    },
                    _ => false // invalid message format
                }
            },
            _ => false // invalid state
        }
    }

    // post condition for rcv_accept
    fn post_rcv_accept(&mut self, accept:Accepted) {
        // add to received accept
        self.rcvd_accepts.push(accept);
    }

    // process a received ACCEPTED message
    pub fn rcv_accept(&mut self, msg:Message) -> () {
        if self.pre_rcv_accept(&msg) {
            match (msg.msg_type, msg.accepted) {
                (MessageType::ACCEPTED, Some(accepted)) => {
                    self.post_rcv_accept(accepted.clone());
                    if self.rcvd_accepts.len() as i32 >= self.quorum_amount {
                        // we have majority of ACCEPTS so we should have consensus
                        self.check_consensus(accepted.id, accepted.value);
                    }
                },
                _ => println!("Proposer {} received invalid ACCEPTED message.", self.pid)
            }
        }
    }


    // pre condition for check_consensus
    fn pre_check_consensus(&self) -> bool {
        self.rcvd_accepts.len() as i32 >= self.quorum_amount
    }

    // post condition for check_consensus
    fn post_check_consensus(&mut self, id:f32, v:i32) {
        self.state = ProposerState::ACCEPTED; // update state
        self.inform_learners(id, v);
        self.clear_data(); // clear protocol data
    }

    // check if we have achieved consensus
    fn check_consensus(&mut self, id:f32, v:i32) {
        if self.pre_check_consensus() {
            // we received ACCEPT from majority, so consensus as been reached
            for m in self.rcvd_accepts.as_slice() {
                assert!(m.value == v); // all accept messages need to accept the same value!
            }
            println!("Proposer {} has reached consensus on value {}.", self.pid, v);
            self.post_check_consensus(id, v);
        }
    }

    // process a received REJECTED message (for a PREPARE)
    pub fn rcv_reject(&self, msg:Message) -> () {
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
    fn check_received_accepts(&self, sender_pid:i32) -> bool {
        for m in self.rcvd_accepts.as_slice() {
            if m.sender_pid == sender_pid {
                // already received accept from this process
                return true;
            }
        }
        return false;
    }

    // checks if the proposer already received a promise from this same acceptor (using it's pid)
    fn check_received_promises(&self, sender_pid:i32) -> bool {
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
    fn check_promises_contain_value(&mut self) {
        let mut highest_id = -1.0;
        for m in self.rcvd_promises.as_slice() {
            // check if the promise said that the acceptor had already accepted another value
            // Property 2c : for any V and N, if a proposal with value V and id N is issued, then there is a set of majority acceptors where (a) none accepted a proposal numbered less than N, or (b) V is the value of the highest-numbered proposal among all proposals accepted by the majority
            match (m.accepted_value, m.accepted_id) {
                (Some(av), Some(ai)) => {
                    if ai > highest_id {
                        highest_id = ai;
                        self.propose_val = av;
                        while highest_id > self.id {
                            self.id += 1.0;
                        }
                    }
                },
                _ => {}
            }
        }
    }

    // clear data used in the protocol
    fn clear_data(&mut self) -> () {
        self.rcvd_promises.clear();
        self.rcvd_accepts.clear();
    }

    // create a prepare message
    fn create_prepare_msg(&self) -> Message {
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
            rejected: None,
            consensus: None
        }
    }

    // create a propose message with value <v>
    fn create_propose_msg(&self) -> Message {
        Message{
            msg_type: MessageType::PROPOSE,
            prepare: None,
            promise: None,
            propose: Some(Propose{
                id: self.id,
                sender_pid: self.pid,
                sender: self.tx.clone(),
                value: self.propose_val
            }),
            accepted: None,
            rejected: None,
            consensus: None
        }
    }

    // create a consensus message with id <id> and value <v>
    fn create_consensus_msg(&self, t_id:f32, v:i32) -> Message {
        Message{
            msg_type: MessageType::CONSENSUS,
            prepare: None,
            promise: None,
            propose: None,
            accepted: None,
            rejected: None,
            consensus: Some(Consensus{
                id: t_id,
                value: v
            })
        }
    }

}
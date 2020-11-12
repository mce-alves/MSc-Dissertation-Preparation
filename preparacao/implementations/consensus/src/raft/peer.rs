// mca @ 49828

// Based on the article: "In Search of an Understandable Consensus Algorithm (Extended Version)", Ongaro & Ousterhout

// TODO : re-read page 4 to re-check the implementations

/*
    Invariants:
    - I1 -> whenever a server receives a message with an higher term, it updates it's term
    - I2 -> whenever a server discovers that it had a lower term (regardless of it's role), it becomes a FOLLOWER
*/

use std::cmp;
use crate::rmessage;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

static ELECTION_TIMEOUT:u64  = 3; // if no message is received during this amount of time, begin an election
static HEARTBEAT_TIMEOUT:u64 = 1; // if leader hasn't sent a message in this amount of time, leader sends a heartbeat

#[derive(Clone)]
pub enum Role {
    CANDIDATE, // means this server is a candidate for leader
    LEADER,    // means this server is the leader
    FOLLOWER   // means this server is a follower
}

#[derive(Clone)]
pub struct Entry {
    command : String, // command to be replicated
    term : i32        // term when entry was received by leader
}

pub struct Peer {
    pid : i32,                   // this server's identifier
    current_term : i32,          // latest term that this server as seen (initialized at 0)
    voted_for : Option<i32>,     // pid of the candidate that received vote in current term (or none)
    vote_count: i32,             // number of accepted votes the candidate has received
    log : Vec<Entry>,            // each entry is an operation for the state machine, and term when entry was received by leader
    role: Role,                  // this server's role
    commit_index : usize,        // index of highest log entry known to be committed (initialized at 0)
    last_applied : usize,        // index of highest log entry applied to the state machine (initialized at 0)
    next_index : Vec<usize>,     // for each server, index of the next log entry to send to that server (init 0)
    match_index : Vec<usize>,    // for each server, index of highest log entry known to be replicated on that server (init 1)
    election_timeout: Option<SystemTime>,  // the system time when an election timeout should occur
    heartbeat_timeout: Option<SystemTime>, // the system time when the leader should send a heartbeat
    rx: mpsc::Receiver<rmessage::Message>,           // this peer's RX
    tx: mpsc::Sender<rmessage::Message>,             // this peer's TX
    membership: Vec<mpsc::Sender<rmessage::Message>> // membership (known correct processes)
}

impl Peer {

    pub fn new(t_pid:i32, t_rx:mpsc::Receiver<rmessage::Message>, t_tx:mpsc::Sender<rmessage::Message>, t_membership:Vec<mpsc::Sender<rmessage::Message>>) -> Peer {
        Peer {
            pid: t_pid,
            role: Role::FOLLOWER,
            current_term: 0,
            voted_for: None,
            vote_count: 0,
            log: vec!(),
            commit_index: 0,
            last_applied: 0,
            next_index: vec!(),
            match_index: vec!(),
            election_timeout: SystemTime::now().checked_add(Duration::from_secs(ELECTION_TIMEOUT)),
            heartbeat_timeout: SystemTime::now().checked_add(Duration::from_secs(HEARTBEAT_TIMEOUT)),
            rx: t_rx,
            tx: t_tx,
            membership:t_membership
        }
    }

    pub fn run(&mut self) {
        loop {
            // role dependant operations
            match self.role {
                Role::LEADER => {
                    if self.timed_out(self.heartbeat_timeout) {
                        self.send_entries();
                    }
                },
                _ => {
                    if self.timed_out(self.election_timeout) {
                        self.begin_election();
                    }
                }
            }
            // role independant operations
            self.handle_messages();
        }
    }

    /*
    RPC initiated by the leader, to replicate log entries and to act as a heartbeat
      leader_term         -> leader's term
      leader_pid          -> pid of the leader
      prev_log_index      -> index of log entry immediately preceding new ones
      prev_log_term       -> term of prev_log_index entry
      entries             -> log entries to store (empty for heartbeat)
      leader_commit_index -> leader's commit_index
    Returns (current_term, conflictIndex, conflictTerm, success)
    */
    pub fn process_append_entries_request(&mut self, leader_term:i32, leader_pid:i32, prev_log_index:usize, prev_log_term:i32, entries:&Vec<Entry>, leader_commit_index:usize) -> (i32, bool) {
        if leader_term < self.current_term { return (self.current_term, false) } // "old" message
        self.update_term(leader_term); // I1, I2
        self.update_election_timeout();
        if prev_log_index >= self.log.len() {
            // we are missing entries, and the conflict happened at our log length index
            return (self.current_term, false);
        }

        if (self.log[prev_log_index].term != leader_term) && (prev_log_index >= 0) { // we have conflicting entries
            // remove the conflicting entry and all that follow it
            let mut i = prev_log_index;
            while i < self.log.len() {
                self.log.remove(i);
            }

            return (self.current_term, false);
        }

        // append new entries
        self.log.append(&mut entries.clone());
        // update state to latest leader commit
        if leader_commit_index > self.commit_index {
            self.commit_index = cmp::min(leader_commit_index, self.log.len()-1);
        }

        // apply newly committed operations
        if self.commit_index > self.last_applied {
            for i in self.last_applied+1..=self.commit_index {
                self.apply_operation(self.log[i].command.clone());
                self.last_applied = i;
            }
         }

        return (self.current_term, true); // appendEntries was successful
    }

    // process a response from an append_entries request
    pub fn process_append_entries_response(&mut self, msg:rmessage::ResponseAppend) {
        match self.role {
            Role::LEADER => {
                if msg.follower_term > self.current_term {
                    // we should step down, since there is a follower with an higher term than ours
                    self.update_term(msg.follower_term);
                    return;
                }
                if !msg.success {
                    self.next_index[msg.follower_pid as usize] -= 1; // decrease entry to be sent
                    // TODO : immediately re-send?
                }
                self.match_index[msg.follower_pid as usize] = msg.match_index;
                self.next_index[msg.follower_pid as usize]  = msg.match_index + 1;

                // see if we can commit any new entries
                for x in self.commit_index..self.log.len() {
                    let mut num_replicas = 0;
                    for peer_match in &self.match_index { // count the number of replicas that contain the log entry
                        if *peer_match >= x {
                            num_replicas += 1;
                        }
                    }
                    if num_replicas > (self.membership.len()/2) {
                        // a majority of replicas contain the log entry
                        self.commit_index += 1;
                    }
                    else {
                        break; // ensure that there are no holes
                    }
                }
            },
            _ => println!("Peer {} cannot process response to append entries because it is not the leader.", self.pid)
        }
    }

    /*
    RPC initiated by candidates during elections
        candidate_term -> candidate's term
        candidate_pid  -> pid of the candidate requesting the vote
        last_log_index -> index of the candidate's last log entry
        last_log_term  -> term of the candidate's last log entry
    Returns (current_term, voteGranted)
    */
    pub fn request_vote(&mut self, candidate_term:i32, candidate_pid:i32, last_log_index:usize, last_log_term:i32) -> (i32, bool) {
        if candidate_term < self.current_term { return (self.current_term, false) } // do not vote for out of date candidates
        self.update_term(candidate_term); // I1, I2
        let self_last_log_index = self.log.len()-1;
        let mut self_last_log_term = -1;
        match self.voted_for {
            None  => {
                if self.log.len() != 0 {
                    self_last_log_term = self.log[self_last_log_index].term; // check how up to date our log is
                }
                if last_log_index < self.log.len() { return (self.current_term, false) } // we have entries that the candidate does not have
            },
            Some(c_id) => {
                if c_id != candidate_pid { return (self.current_term, false) } // do not vote for several different candidates
            }
        }
        if last_log_term < self_last_log_term { return (self.current_term, false) } // reject leaders with older logs

        if (last_log_term == self_last_log_term) && (last_log_index < self_last_log_index) {
            return (self.current_term, false); // reject leaders with short logs
        }

        // if all previous checks pass, then vote for the candidate that requested the vote
        self.voted_for = Some(candidate_pid);
        self.update_election_timeout();
        return (self.current_term, true);
    }

    // process a response from a request_vote
    pub fn process_req_vote_response(&mut self, msg:rmessage::ResponseVote) {
        match self.role {
            Role::CANDIDATE => {
                self.update_election_timeout();
                if msg.follower_term > self.current_term {
                    // the "supposed" follower has a higher term, so we need to step down
                    self.update_term(msg.follower_term);
                    return;
                }
                if msg.vote_granted {
                    self.vote_count += 1;
                }
                if self.vote_count > (self.membership.len()/2) as i32 {
                    self.role = Role::LEADER;
                    self.update_election_timeout();
                    // TODO : reset the next index and match index
                }
            },
            _ => println!("Peer {} received vote response but he is not a candidate.", self.pid)
        }
    }

    // if <term> is greater than current_term, then set current_term to be equal to <term> and step down (convert to follower)
    fn update_term(&mut self, term:i32) {
        if term > self.current_term { 
            self.current_term = term;   // I1
            self.role = Role::FOLLOWER; // I2
            self.voted_for = None;
            self.vote_count = 0;
            self.update_election_timeout();
        }
    }

    // update the election timeout
    fn update_election_timeout(&mut self) {
        self.election_timeout = SystemTime::now().checked_add(Duration::from_secs(3));
    }

    // when the server gets an election timeout and assumes there is no viable leader, it begins an election
    fn begin_election(&mut self) {
        self.current_term += 1;
        self.role = Role::CANDIDATE;
        self.voted_for = Some(self.pid);
        self.update_election_timeout();
        
        let msg = self.create_request_vote_msg();
        rmessage::broadcast(&self.membership, msg);
    }

    // send new log entries (or heartbeat if there are no new entries)
    fn send_entries(&mut self) {
        match self.role {
            Role::LEADER => {
                for i in 0..self.membership.len() {
                    if self.next_index[i] > self.log.len() { // just to be safe
                        self.next_index[i] = self.log.len();
                    }
                    let mut entries = vec!();
                    for j in self.next_index[i]..self.log.len() { // get new log entries that need to be sent
                        entries.insert(self.log.len(), self.log[j].clone());
                    }
                    let prev_log_index = self.next_index[i] - 1; // prepare the previous log index
                    let mut prev_log_term = -1;
                    if prev_log_index >= 0 {
                        prev_log_term = self.log[prev_log_index].term; // get the previous log term if it exists
                    }

                    let msg = self.create_append_entries_req_msg(entries.clone(), prev_log_index, prev_log_term);
                    self.membership[i].send(msg).unwrap(); // send the request to the peer
                }
            },
            _ => println!("Peer {} cannot send entries because it is not the leader", self.pid)
        }
    }

    // apply an operation
    fn apply_operation(&mut self, op:String) {
        println!("Peer {} applied operation: {}", self.pid, op);
    }

    // check if there is a timeout (comparing current time to <t>)
    fn timed_out(&self, t:Option<SystemTime>) -> bool {
        match t {
            Some(time) => {
                return SystemTime::now() >= time
            },
            None => return true
        }
    }


    // handle message receival
    fn handle_messages(&mut self) {
        let msg = self.rx.recv_timeout(Duration::from_millis(100));
        match msg {
            Ok(m) => {
                match m.msg_type {
                    rmessage::MessageType::REQAPPEND => {
                        match m.request_append {
                            Some(req) => self.process_append_entries_request(req.leader_term, req.leader_pid, req.prev_log_index, req.prev_log_term, &req.entries, req.leader_commit_index),
                            None => println!("Peer {} received invalid append entries request.", self.pid)
                        }
                    },
                    rmessage::MessageType::RESAPPEND => {
                        match m.response_append {
                            Some(req) => self.process_append_entries_response(req),
                            None => println!("Peer {} received invalid append entries response.", self.pid)
                        }
                    },
                    rmessage::MessageType::REQVOTE => {
                        match m.request_vote {
                            Some(req) => self.request_vote(req.candidate_term, req.candidate_pid, req.last_log_index, req.last_log_term),
                            None => println!("Peer {} received invalid vote request.", self.pid)
                        }
                    },
                    rmessage::MessageType::RESVOTE => {
                        match m.response_vote {
                            Some(req) => self.process_req_vote_response(req),
                            None => println!("Peer {} received invalid vote reponse.", self.pid)
                        }
                    },
                    rmessage::MessageType::REQOP => {
                        // TODO : not yet implemented
                    }
                }
            },
            Err(_) => () // no message was received before the timeout
        }
    }


    ////////// Functions for creating messages

    // TODO : re-checks fields (receive the necessary values as arguments)
    fn create_request_vote_msg(&self) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::REQVOTE,
            request_append: None,
            request_operation: None,
            response_vote: None,
            response_append: None,
            request_vote: Some(rmessage::RequestVote {
                candidate_pid: self.pid,
                candidate_term: self.current_term,
                last_log_index: self.log.len(),
                last_log_term: self.log[self.log.len()-1].term,
                sender: self.tx.clone()
            })
        }
    }

    // TODO : re-check fields (receive the necessary values as arguments)
    fn create_append_entries_req_msg(&self, new_entries:Vec<Entry>, pli:usize, plt:i32) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::REQAPPEND,
            request_append: Some(rmessage::RequestAppend {
                leader_term:self.current_term,
                leader_pid:self.pid,
                prev_log_index:pli,
                prev_log_term:plt,
                entries:new_entries,
                leader_commit_index:self.commit_index,
                sender:self.tx.clone()
            }),
            request_operation: None,
            response_vote: None,
            response_append: None,
            request_vote: None
        }
    }

    // TODO : re-check fields (receive the necessary values as arguments)
    fn create_response_vote_msg(&self, c_pid:i32, vote:bool) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::RESVOTE,
            request_append: None,
            request_operation: None,
            response_vote: Some(rmessage::ResponseVote {
                candidate_pid:c_pid,
                follower_term:self.current_term,
                follower_pid:self.pid,
                vote_granted:vote,
                sender:self.tx.clone()
            }),
            response_append: None,
            request_vote: None
        }
    }

    // TODO : re-check fields (receive the necessary values as arguments)
    fn create_response_append_msg(&self, l_pid:i32, res:bool, m_index:usize, ci:i32) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::RESAPPEND,
            request_append: None,
            request_operation: None,
            response_vote: None,
            response_append: Some(rmessage::ResponseAppend {
                leader_pid:l_pid,
                follower_term:self.current_term,
                follower_pid:self.pid,
                success:res,
                match_index:m_index,
                sender:self.tx.clone()
            }),
            request_vote: None
        }
    }

    // TODO : re-check fields (receive the necessary values as arguments)
    fn create_request_operation_msg(&self, op:String) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::REQOP,
            request_append: None,
            request_operation: Some(rmessage::RequestOperation {
                operation:op,
                sender:self.tx.clone()
            }),
            response_vote: None,
            response_append: None,
            request_vote: None
        }
    }
    
}
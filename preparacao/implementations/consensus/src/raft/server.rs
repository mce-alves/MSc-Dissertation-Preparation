// mca @ 49828

// Based on the article: "In Search of an Understandable Consensus Algorithm (Extended Version)", Ongaro & Ousterhout

// TODO : re-read page 4

/*
    Invariants:
    - I1 -> whenever a server receives a message with an higher term, it updates it's term
    - I2 -> whenever a server discovers that it had a lower term (regardless of it's role), it becomes a FOLLOWER
*/

use std::cmp;

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

pub struct Server {
    // State that should be persistent
    pid : i32,                 // this server's identifier
    current_term : i32,        // latest term that this server as seen (initialized at 0)
    voted_for : Option<i32>,   // pid of the candidate that received vote in current term (or none)
    log : Vec<Entry>,          // log entries; each entry is an operation for the state machine, and term when entry was received by leader (first index is 1)
    // State that should be volatile
    role: Role,                // this server's role
    commit_index : i32,        // index of highest log entry known to be committed (initialized at 0)
    last_applied : i32,        // index of highest log entry applied to the state machine (initialized at 0)
    // State on leaders (reinitialized after election)
    next_index : Vec<i32>,     // for each server, index of the next log entry to send to that server (initialized to leader last log index +1 )
    match_index : Vec<i32>     // for each server, index of highest log entry known to be replicated on that server (initialized to 0)
}

impl Server {

    pub fn new(t_pid:i32) -> Server {
        Server {
            pid: t_pid,
            role: Role::FOLLOWER,
            current_term: 0,
            voted_for: None,
            log: vec!(),
            commit_index: 0,
            last_applied: 0,
            next_index: vec!(),
            match_index: vec!()
        }
    }

    // executes while this server is in FOLLOWER state
    pub fn follower() {}

    // executes while this server is in CANDIDATE state
    pub fn candidate() {}

    // executes while this server is in LEADER state
    pub fn leader(&mut self) {
        
    }

    /*
    RPC initiated by the leader, to replicate log entries and to act as a heartbeat
      leader_term         -> leader's term
      leader_pid          -> pid of the leader
      prev_log_index      -> index of log entry immediately preceding new ones
      prev_log_term       -> term of prev_log_index entry
      entries             -> log entries to store (empty for heartbeat)
      leader_commit_index -> leader's commit_index  
    */
    pub fn process_append_entries_request(&mut self, leader_term:i32, leader_pid:i32, prev_log_index:i32, prev_log_term:i32, entries:&Vec<Entry>, leader_commit_index:i32) -> (i32, bool, i32) {
        if leader_term < self.current_term { return (self.current_term, false, 0) }        // "old" message
        self.update_term(leader_term);                                                     // I1, I2
        if (self.log.len() as i32) < prev_log_index { return (self.current_term, false, 0) }  // missing entries
        if self.log[prev_log_index as usize].term != leader_term {                         // conflicting entries
            // remove the conflicting log entry, and all that follow it
            let mut i = self.log.len();
            while (i as i32) >= prev_log_index {
                self.log.remove(i);
                i = self.log.len();
            }
        }
        // append new entries
        self.log.append(&mut entries.clone());
        // update state to latest commit
        if leader_commit_index > self.commit_index {
            self.commit_index = cmp::min(leader_commit_index, (self.log.len()-1) as i32); // TODO : check if -1 is correct
        }

        return (self.current_term, true, self.log.len() as i32); // TODO : correct return for the term?
    }

    // process the response from an append entries request
    pub fn process_append_entries_response(&mut self, follower_term:i32, follower_pid:i32, success:bool, match_index:i32) {
        self.update_term(follower_term);
        match self.role {
            Role::LEADER => {
                if success {
                    self.match_index[follower_pid as usize] = match_index;
                    self.next_index[follower_pid as usize] = match_index + 1;
                }
                else {
                    self.next_index[follower_pid as usize] = cmp::max(0, self.next_index[follower_pid as usize] - 1);
                }
            },
            _ => println!("Server {} cannot process append entries response since it is not the leader.", self.pid)
        }
    }

    /*
    RPC initiated by candidates during elections
        candidate_term -> candidate's term
        candidate_pid  -> pid of the candidate requesting the vote
        last_log_index -> index of the candidate's last log entry
        last_log_term  -> term of the candidate's last log entry
    */
    pub fn request_vote(&mut self, candidate_term:i32, candidate_pid:i32, last_log_index:i32, last_log_term:i32) -> (i32, bool) {
        if candidate_term < self.current_term { return (self.current_term, false) }
        self.update_term(candidate_term); // I1, I2
        match self.voted_for {
            None  => {
                if last_log_index < self.log.len() as i32 { return (self.current_term, false) } // we have entries that the candidate does not have
            },
            Some(c_id) => {
                if c_id != candidate_pid { return (self.current_term, false) }
            }
        }
        if last_log_index >= self.log.len() as i32 { // TODO : is this the only necessary condition?
            return (self.current_term, true);
        }
        else {
            return (self.current_term, false);
        }
    }

    // if <term> is greater than current_term, then set current_term to be equal to <term>
    fn update_term(&mut self, term:i32) {
        if term > self.current_term { 
            self.current_term = term;   // I1
            self.role = Role::FOLLOWER; // I2
        }
    }

    // when the server gets an election timeout and assumes there is no viable leader, it begins an election
    fn begin_election(&mut self) {
        self.current_term += 1;
        self.role = Role::CANDIDATE;
        self.voted_for = Some(self.pid);
        // TODO : send request_vote to all other servers
        // maybe use futures? https://docs.rs/futures/0.3.8/futures/

        // raft uses randomized election timeouts between 150ms and 300ms
    }
    
}
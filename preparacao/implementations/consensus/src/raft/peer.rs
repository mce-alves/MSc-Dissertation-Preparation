// mca @ 49828

// Written according to the TLA specification found in https://github.com/ongardie/raft.tla/blob/master/raft.tla
// with some changes to handle sending multiple entries at the same time

use std::cmp;
use crate::rmessage;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};
use std::{thread, time};
use rand::Rng;

static ELECTION_TIMEOUT:u64  = 3; // if no message is received during this amount of time, begin an election
static HEARTBEAT_TIMEOUT:u64 = 1; // if leader hasn't sent a message in this amount of time, leader sends a heartbeat

#[derive(Clone)]
pub enum Role {
    CANDIDATE, // means this server is a candidate for leader
    LEADER,    // means this server is the leader
    FOLLOWER   // means this server is a follower
}

#[derive(Clone, Debug)]
pub struct Entry {
    operation : String, // operation to be replicated
    term : i32          // term when entry was received by leader
}

pub struct Peer {
    pid : i32,                   // this server's identifier
    current_term : i32,          // latest term that this server as seen (initialized at 0)
    voted_for : Option<i32>,     // pid of the candidate that received vote in current term (or none)
    votes_rcvd: Vec<i32>,        // pid of the peers that responded to this candidate's vote request
    granted_votes_rcvd: Vec<i32>,// pid of the peers that voted for this candidate's vote request
    current_leader: Option<i32>, // pid of who this peer THINKS is the current leader (might not be)
    log : Vec<Entry>,            // each entry is an operation for the state machine, and term when entry was received by leader
    role: Role,                  // this server's role
    commit_index : i32,          // index of highest log entry known to be committed (initialized at -1)
    last_applied : i32,          // index of highest log entry applied to the state machine (initialized at -1)
    next_index : Vec<i32>,       // for each server, index of the next log entry to send to that server (init 0)
    match_index : Vec<i32>,      // for each server, index of highest log entry known to be replicated on that server (init -1)
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
            current_term: 1,
            voted_for: None,
            votes_rcvd: vec!(),
            granted_votes_rcvd: vec!(),
            current_leader: None,
            log: vec!(),
            commit_index: -1,
            last_applied: -1,
            next_index: vec![0; t_membership.len()],
            match_index: vec![-1; t_membership.len()],
            election_timeout: SystemTime::now().checked_add(Duration::from_secs(ELECTION_TIMEOUT+(t_pid as u64))),
            heartbeat_timeout: SystemTime::now().checked_add(Duration::from_secs(HEARTBEAT_TIMEOUT)),
            rx: t_rx,
            tx: t_tx,
            membership:t_membership
        }
    }

    pub fn run(&mut self) {
        let mut failed = 0;
        let mut rng = rand::thread_rng();
        loop {
            // role dependant operations
            match self.role {
                Role::LEADER => {
                    if self.timed_out(self.heartbeat_timeout) {
                        self.send_entries();
                    }
                    let r = rng.gen_range(1, 25);
                    if r == 1 && failed < 2 {
                        // random-ish chance of peer failing for a duration of 15 seconds
                        thread::sleep(time::Duration::new(15, 0));
                        failed += 1;
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

    // Loops through the membership and sends an append entries request to each peer
    fn send_entries(&mut self) -> () {
        match self.role {
            Role::LEADER => {
                self.update_heartbeat_timeout();
                println!("Peer {} is sending entries.", self.pid);
                for i in 0..self.membership.len() {
                    // send an append entries request to each peer in the membership, except itself
                    if i as i32 != self.pid {
                        self.append_entries(i);
                    }
                }
            },
            _ => println!("Peer {} cannot send entries because it is not the leader", self.pid)
        }
    }

    // handle message receival
    fn handle_messages(&mut self) -> () {
        let msg = self.rx.recv_timeout(Duration::from_millis(100));
        match msg {
            Ok(m) => {
                match m.msg_type {
                    rmessage::MessageType::REQAPPEND => {
                        match m.request_append {
                            Some(req) => {
                                let (_, res) = self.handle_append_entries_request(req.clone());
                                if res {
                                    let msg = self.create_response_append_msg(self.pid, res, (self.log.len() as i32)-1);
                                    rmessage::send_msg(&req.sender, msg);
                                }
                                else {
                                    let msg = self.create_response_append_msg(self.pid, res, -1);
                                    rmessage::send_msg(&req.sender, msg);
                                }
                            },
                            None => println!("Peer {} received invalid append entries request.", self.pid)
                        }
                    },
                    rmessage::MessageType::RESAPPEND => {
                        match m.response_append {
                            Some(req) => self.handle_append_entries_response(req),
                            None => println!("Peer {} received invalid append entries response.", self.pid)
                        }
                    },
                    rmessage::MessageType::REQVOTE => {
                        match m.request_vote {
                            Some(req) => {
                                let (_, vote) = self.handle_vote_request(req.clone());
                                rmessage::send_msg(&req.sender, self.create_response_vote_msg(vote))
                            },
                            None => println!("Peer {} received invalid vote request.", self.pid)
                        }
                    },
                    rmessage::MessageType::RESVOTE => {
                        match m.response_vote {
                            Some(req) => self.handle_vote_response(req),
                            None => println!("Peer {} received invalid vote reponse.", self.pid)
                        }
                    },
                    rmessage::MessageType::REQOP => {
                        match m.request_operation {
                            Some(req) => self.receive_client_request(req),
                            None => println!("Peer {} received invalid client request.", self.pid)
                        }
                    }
                }
            },
            Err(_) => () // no message was received before the timeout, which is fine!
        }
    }



    /* Functions for main RAFT operations */

    // When the server gets an election timeout it assumes there is no viable leader, so it begins an election
    fn begin_election(&mut self) {
        println!("Peer {} has started an election.", self.pid);
        self.current_term += 1;
        self.role = Role::CANDIDATE;
        self.voted_for = Some(self.pid);
        self.granted_votes_rcvd = vec!();
        self.votes_rcvd = vec!();
        self.update_election_timeout();
        
        for i in 0..self.membership.len() {
            self.request_vote(i);
        }
    }

    // Sends a request vote request to peer at <index> in membership
    fn request_vote(&self, index:usize) -> () {
        match self.role {
            Role::CANDIDATE => {
                println!("Peer {} is sending a vote request to peer {}.", self.pid, index);
                // Only send message if we haven't received a vote response from that peer
                if !self.has_response(index as i32) {
                    rmessage::send_msg(&self.membership[index], self.create_request_vote_msg())
                }
            },
            _ => println!("Peer {} cannot request a vote since it is not a candidate.", self.pid)
        }
    }

    // Sends an append entries request to peer at index <peer> in membership
    fn append_entries(&mut self, peer:usize) -> () {
        match self.role {
            Role::LEADER => {
                println!("Peer {} is sending append_entries to {}.", self.pid, peer);
                // Get the previous log index and term for the peer
                let prev_log_index = self.next_index[peer] - 1;
                let mut prev_log_term = 0;
                if prev_log_index >= 0 && self.log.len() as i32 > prev_log_index {
                    prev_log_term = self.log[prev_log_index as usize].term;
                }
                // Get all new entries that need to be sent to the peer
                let mut entries = vec!();
                for i in self.next_index[peer] as usize..self.log.len() {
                    entries.insert(entries.len(), self.log[i].clone());
                }
                // Create and send the append entries request message
                let msg = self.create_append_entries_req_msg(entries, prev_log_index, prev_log_term);
                println!("Peer {} is sending the msg {:?} to peer {}.", self.pid, msg.clone(), peer);
                rmessage::send_msg(&self.membership[peer], msg);
            },
            _ => println!("Peer {} cannot send an append entries request since it is not a leader.", self.pid)
        }
    }

    // Assume role of leader
    fn become_leader(&mut self) -> () {
        match self.role {
            Role::CANDIDATE => {
                if !(self.granted_votes_rcvd.len() > (self.membership.len() / 2)) {
                    // we did not receive a majority of votes, so we cannot become the leader
                    return;
                }
                println!("Peer {} received {} votes (majority).", self.pid, self.granted_votes_rcvd.len());
                // we can become the leader
                self.role = Role::LEADER;
                self.current_leader = Some(self.pid);
                // set next index to be the same as our next index
                self.next_index = vec![self.log.len() as i32; self.membership.len()];
                // reset the match index to 0 (as far as we know, no logs in other peers match ours)
                self.match_index = vec![0; self.membership.len()];
                println!("Peer {} is now the leader.", self.pid);
            },
            _ => println!("Peer {} cannot become leader since it is not a candidate.", self.pid)
        }
    }

    // Checks if the leader can update it's commit_index, and if successful applies the newly committed operations
    fn update_commit_index(&mut self) -> () {
        match self.role {
            Role::LEADER => {
                for index in (self.commit_index + 1) as usize..self.log.len() {
                    // count the number of replicas that contain the log entry at index <index>
                    let mut num_replicas = 0;
                    for peer_match in &self.match_index {
                        if *peer_match >= index as i32 {
                            num_replicas += 1;
                        }
                    }
                    if num_replicas > (self.membership.len() / 2) {
                        // a majority of replicas contain the log entry
                        self.commit_index += 1;
                        println!("Peer {} (leader) updated commit index to {}.", self.pid, self.commit_index);
                    }
                    else {
                        break; // ensure that there are no holes
                    }
                }
            },
            _ => println!("Peer {} cannot update commit index since it is not a leader.", self.pid)
        }
    }

    // apply an operation
    fn apply_operation(&mut self, op:String) {
        println!("Peer {} applied operation: {}", self.pid, op);
        println!("Peer {}'s log is currently: {:?}.", self.pid, self.log);
    }

    /* End of functions for main RAFT operations */


    /* Functions to handle received messages */

    // Receive a request from a client to add an operation to the log
    fn receive_client_request(&mut self, msg:rmessage::RequestOperation) -> () {
        match self.role {
            Role::LEADER => {
                println!("Peer {} received client request.", self.pid);
                // insert the new operation at the end of the log
                self.log.insert(self.log.len(), Entry{operation:msg.operation, term:self.current_term});
            },
            _ => {
                // Try to redirect to leader
                match self.current_leader {
                    Some(id) => {
                        rmessage::send_msg(&self.membership[id as usize], self.create_reqop_msg_wrapper(msg));
                    },
                    None => println!("Peer {} cannot handle client request because it is not the leader, and does not know who the leader might be.", self.pid)
                }
            }
        }
    }

    // Receive and process a vote request from a candidate peer
    fn handle_vote_request(&mut self, msg:rmessage::RequestVote) -> (i32, bool) {
        println!("Peer {} received a vote request from {}.", self.pid, msg.candidate_pid);
        if msg.candidate_term < self.current_term {
            // do not vote for out of date candidates
            return (self.current_term, false);
        }
        self.update_term(msg.candidate_term);
        let mut self_last_log_term = 0;
        match self.voted_for {
            None => {
                if self.log.len() != 0 {
                    // check how up to date our log is
                    self_last_log_term = self.log[self.log.len() - 1].term; 
                }
            },
            Some(c_id) => {
                if c_id != msg.candidate_pid {
                    // do not vote for several different candidates
                    return (self.current_term, false);
                }
            }
        }
        if msg.last_log_term < self_last_log_term {
            // candidate has out of date logs
            return (self.current_term, false);
        }
        if msg.last_log_term == self_last_log_term {
            if self_last_log_term > 0 {
                if msg.last_log_index < (self.log.len() - 1) as i32 {
                    // candidate has short logs
                    return (self.current_term, false);
                }
            }
            
        }
        // if all previous checks pass, then vote for the candidate that requested the vote
        self.voted_for = Some(msg.candidate_pid);
        println!("Peer {} has voted for peer {}.", self.pid, msg.candidate_pid);
        self.update_election_timeout();
        // as far as we know, the candidate will become the leader
        self.current_leader = Some(msg.candidate_pid);
        return (self.current_term, true);
    }

    // Receive and process a vote response from a peer
    fn handle_vote_response(&mut self, msg:rmessage::ResponseVote) -> () {
        match self.role {
            Role::CANDIDATE => {
                println!("Peer {} received vote response from {}.", self.pid, msg.follower_pid);
                self.update_term(msg.follower_term);
                // check if we have already received a response from that same peer
                if !self.has_response(msg.follower_pid) {
                    self.votes_rcvd.insert(self.votes_rcvd.len(), msg.follower_pid);
                    
                    if msg.vote_granted {
                        self.granted_votes_rcvd.insert(self.granted_votes_rcvd.len(), msg.follower_pid);
                    }

                    self.become_leader(); // only succeeds if we have majority of granted votes
                }
            },
            _ => println!("Peer {} cannot receive a vote since it is not a candidate.", self.pid)
        }
    }

    // Receive and process an append entries request from a peer
    fn handle_append_entries_request(&mut self, msg:rmessage::RequestAppend) -> (i32, bool) {
        println!("Peer {} received append_entries request from {}.", self.pid, msg.leader_pid);
        if msg.leader_term < self.current_term {
            // "old" message
            return (self.current_term, false);
        }
        self.update_term(msg.leader_term);
        self.update_election_timeout();
        // as far as we know, we received a request from a valid leader
        self.current_leader = Some(msg.leader_pid);
        if msg.prev_log_index >= 0 { 
            if msg.prev_log_index > self.log.len() as i32 {
                // we are missing entries
                return (self.current_term, false);
            }
    
            // check if we have conflicting entries
            if self.log[msg.prev_log_index as usize].term != msg.leader_term {
                // remove the conflicting entry and all that follow it
                let i = msg.prev_log_index as usize;
                while i < self.log.len() {
                    self.log.remove(i);
                }
                return (self.current_term, false);
            }
            // check if we have entries that the leader doesn't know we have (entries after the prev_log_index)
            // if we do, remove them (we can still append the new entries afterwards)
            // Note : this can happen for example when a response to an append_entries is lost in the network, so
            //        the leader will resend entries that we had already added to our log
            while msg.prev_log_index < (self.log.len() as i32) - 1 {
                self.log.remove((msg.prev_log_index + 1) as usize); // remove every entry after the prev_log_index
            }
        }
        else {
            // if it is equal to 0, means that we are receiving the first entry
            // make sure that our log is empty
            while self.log.len() > 0 {
                self.log.remove(self.log.len() - 1);
            }
        }
        // if all checks pass, we can append the new entries to our log
        self.log.append(&mut msg.entries.clone());
        // update state to latest leader commit
        if msg.leader_commit_index > self.commit_index {
            self.commit_index = cmp::min(msg.leader_commit_index, self.log.len() as i32);
        }
        // apply newly commited operations, if there are any
        self.apply_new_commits();

        return (self.current_term, true);
    }

    // Receive and process a response to an append entries from a peer
    fn handle_append_entries_response(&mut self, msg:rmessage::ResponseAppend) -> () {
        match self.role {
            Role::LEADER => {
                println!("Peer {} received append_entries response.", self.pid);
                if msg.follower_term > self.current_term {
                    // we should step down, since there is a follower with an higher term than ours
                    self.update_term(msg.follower_term);
                    return;
                }
                if !msg.success {
                    // decrease entry to be sent
                    self.next_index[msg.follower_pid as usize] = cmp::max(self.next_index[msg.follower_pid as usize] - 1, 0);
                }
                self.match_index[msg.follower_pid as usize] = msg.match_index as i32;
                self.next_index[msg.follower_pid as usize]  = msg.match_index as i32 + 1;

                // see if we can commit any new entries
                self.update_commit_index();
                self.apply_new_commits(); // only succeeds if there are new commits
            },
            _ => println!("Peer {} cannot process response to append entries since it is not a leader.", self.pid)
        }
    }

    /* End of functions to handle received messages */


    /* Auxiliary functions */

    // Checks if we already received a response from a peer with pid = <pid>
    fn has_response(&self, pid:i32) -> bool {
        return self.votes_rcvd.contains(&pid);
    }

    // if <term> is greater than current_term, then set current_term to be equal to <term> and step down (convert to follower)
    fn update_term(&mut self, term:i32) {
        if term > self.current_term { 
            self.current_term = term;   // I1
            self.role = Role::FOLLOWER; // I2
            self.voted_for = None;
            self.update_election_timeout();
        }
    }

    // update the election timeout
    fn update_election_timeout(&mut self) {
        self.election_timeout = SystemTime::now().checked_add(Duration::from_secs(ELECTION_TIMEOUT + (self.pid as u64)));
    }

    // update the heartbeat timeout
    fn update_heartbeat_timeout(&mut self) {
        self.heartbeat_timeout = SystemTime::now().checked_add(Duration::from_secs(HEARTBEAT_TIMEOUT));
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

    // apply newly committed operations
    fn apply_new_commits(&mut self) -> () {
        // if there are non-applied commits, apply them
        if self.commit_index > self.last_applied {
            for i in self.last_applied+1..=self.commit_index {
                self.apply_operation(self.log[i as usize].operation.clone());
                self.last_applied = i;
            }
        }
    }

    /* End of auxiliary functions */


    /* Functions for creating messages */

    fn create_request_vote_msg(&self) -> rmessage::Message {
        let mut last_log_term = 0;
        if self.log.len() > 0 {
            last_log_term = self.log[self.log.len()-1].term
        }
        rmessage::Message {
            msg_type: rmessage::MessageType::REQVOTE,
            request_append: None,
            request_operation: None,
            response_vote: None,
            response_append: None,
            request_vote: Some(rmessage::RequestVote {
                candidate_pid: self.pid,
                candidate_term: self.current_term,
                last_log_index: self.log.len() as i32,
                last_log_term: last_log_term,
                sender: self.tx.clone()
            })
        }
    }

    fn create_append_entries_req_msg(&self, new_entries:Vec<Entry>, pli:i32, plt:i32) -> rmessage::Message {
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

    fn create_response_vote_msg(&self, vote:bool) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::RESVOTE,
            request_append: None,
            request_operation: None,
            response_vote: Some(rmessage::ResponseVote {
                follower_term:self.current_term,
                follower_pid:self.pid,
                vote_granted:vote,
                sender:self.tx.clone()
            }),
            response_append: None,
            request_vote: None
        }
    }

    fn create_response_append_msg(&self, l_pid:i32, res:bool, m_index:i32) -> rmessage::Message {
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

    fn create_reqop_msg_wrapper(&self, reqop:rmessage::RequestOperation) -> rmessage::Message {
        rmessage::Message {
            msg_type: rmessage::MessageType::REQOP,
            request_append: None,
            request_operation: Some(reqop),
            response_vote: None,
            response_append: None,
            request_vote: None
        }
    }

    /* End of functions for creating messages */

}
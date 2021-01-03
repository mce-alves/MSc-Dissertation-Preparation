// mca @ 49828

// Written according to the TLA specification found in https://github.com/ongardie/raft.tla/blob/master/raft.tla
// with some changes to handle sending multiple entries at the same time

use std::cmp;
use crate::rmessage;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};
use std::{thread, time};
use rand::Rng;
use std::time::Instant;

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
    membership: Vec<mpsc::Sender<rmessage::Message>>,// membership (known correct processes)
    start_time: Instant                              // when this peer started executing
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
            membership:t_membership,
            start_time: Instant::now()
        }
    }

    pub fn run(&mut self) {
        loop {
            // role dependant operations
            match self.role {
                Role::LEADER => {
                    if self.timed_out(self.heartbeat_timeout) {
                        self.send_entries();
                        let r = rand::thread_rng().gen_range(1, 10);
                        if r == 1 {
                            // 1 in 10 chance of peer failing for 5 seconds
                            thread::sleep(time::Duration::new(5, 0));
                            // do not receive any message from when the peer failed
                            loop {
                                let msg = self.rx.recv_timeout(Duration::from_millis(10));
                                match msg {
                                    Ok(_) => {},
                                    Err(_) => break
                                }
                            }
                        }
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


    // pre conditions for send_entries
    fn pre_send_entries(&self) -> bool {
        match self.role {
            Role::LEADER => true,
            _ => {
                println!("Peer {} cannot send entries because it is not the leader", self.pid);
                false
            }
        }
    }

    // post conditions for send_entries
    fn post_append_entries(&mut self) {
        self.update_heartbeat_timeout();
    }

    // Loops through the membership and sends an append entries request to each peer
    fn send_entries(&mut self) -> () {
        if self.pre_send_entries() {
            println!("Peer {} is sending entries.", self.pid);
            self.post_append_entries();
            for i in 0..self.membership.len() {
                // send an append entries request to each peer in the membership, except itself
                if i as i32 != self.pid {
                    self.append_entries(i);
                }
            }
        }

    }

    // handle message receival
    fn handle_messages(&mut self) -> () {
        let msg = self.rx.recv_timeout(Duration::from_millis(100));
        match msg {
            Ok(m) => {
                match m {
                    rmessage::Message::REQAPPEND(req) => {
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
                    rmessage::Message::RESAPPEND(res) => {
                        self.handle_append_entries_response(res);
                    },
                    rmessage::Message::REQVOTE(req) => {
                        let (_, vote) = self.handle_vote_request(req.clone());
                        rmessage::send_msg(&req.sender, self.create_response_vote_msg(vote));
                    },
                    rmessage::Message::RESVOTE(res) => {
                        self.handle_vote_response(res);
                    },
                    rmessage::Message::REQOP(req) => {
                        self.receive_client_request(req);
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

    // pre conditions for request_vote
    fn pre_request_vote(&self, index:usize) -> bool {
        match self.role {
            Role::CANDIDATE => !self.has_response(index as i32),
            _ => {
                println!("Peer {} cannot request a vote since it is not a candidate.", self.pid);
                false
            }
        }
    }

    // Sends a request vote request to peer at <index> in membership
    fn request_vote(&self, index:usize) -> () {
        if self.pre_request_vote(index) {
            println!("Peer {} is sending a vote request to peer {}.", self.pid, index);
            rmessage::send_msg(&self.membership[index], self.create_request_vote_msg());
        }
    }

    // pre condition for append entries
    fn pre_append_entries(&self) -> bool {
        match self.role {
            Role::LEADER => true,
            _ => {
                println!("Peer {} cannot send an append entries request since it is not a leader.", self.pid);
                false
            }
        }
    }

    // Sends an append entries request to peer at index <peer> in membership
    fn append_entries(&mut self, peer:usize) -> () {
        if self.pre_append_entries() {
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
        }
    }

    // pre conditions for become_leader
    fn pre_become_leader(&self) -> bool {
        match self.role {
            Role::CANDIDATE => (self.granted_votes_rcvd.len() > (self.membership.len() / 2)),
            _ => {
                println!("Peer {} cannot become leader since it is not a candidate.", self.pid);
                false
            }
        }
    }

    // post conditions for become_leader
    fn post_become_leader(&mut self) {
        self.role = Role::LEADER;
        self.current_leader = Some(self.pid);
        // set next index to be the same as our next index
        self.next_index = vec![self.log.len() as i32; self.membership.len()];
        // reset the match index to 0 (as far as we know, no logs in other peers match ours)
        self.match_index = vec![0; self.membership.len()];
        println!("Peer {} is now the leader.", self.pid);
    }

    // Assume role of leader
    fn become_leader(&mut self) -> () {
        if self.pre_become_leader() {
            println!("Peer {} received {} votes (majority).", self.pid, self.granted_votes_rcvd.len());
            // we can become the leader
            self.post_become_leader();
            println!("Peer {} is now the leader.", self.pid);
        }
    }

    // pre condition for update_commit_index
    fn pre_update_commit_index(&self) -> bool {
        match self.role {
            Role::LEADER => true,
            _ => {
                println!("Peer {} cannot update commit index since it is not a leader.", self.pid);
                false
            }
        }
    }

    // Checks if the leader can update it's commit_index, and if successful applies the newly committed operations
    fn update_commit_index(&mut self) -> () {
        if self.pre_update_commit_index() {
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
                    if self.log[index].term == self.current_term {
                        // a leader can only commit entries from it's own term
                        // (uncommitted entries from previous terms will get committed when the leader
                        // commits one entry from his term)
                        self.commit_index = index as i32;
                        let time_elapsed = self.start_time.elapsed();
                        println!("Peer {} (leader) updated commit index to {} after {} ms.", self.pid, self.commit_index, time_elapsed.as_millis());
                    }
                }
                else {
                    break; // ensure that there are no holes
                }
            }
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

    // check if a RequestVote is valid
    fn check_valid_vote_request(&self, msg:&rmessage::RequestVote) -> bool {
        if msg.candidate_term < self.current_term {
            // candidate is out of date
            return false;
        }
        else {
            let mut self_last_log_term = 0;
            match self.voted_for {
                None => {
                    // we haven't voted for anyone yet
                    if self.log.len() != 0 {
                        // check how up to date our log is
                        self_last_log_term = self.log[self.log.len() - 1].term; 
                    }
                },
                Some(c_id) => {
                    if c_id != msg.candidate_pid {
                        // do not vote for several different candidates
                        return false;
                    }
                }
            }
            if msg.last_log_term < self_last_log_term {
                // candidate has out of date logs
                return false;
            }
            if msg.last_log_term == self_last_log_term {
                if self_last_log_term > 0 {
                    if msg.last_log_index < (self.log.len() - 1) as i32 {
                        // candidate has short logs
                        return false;
                    }
                }
                
            }
            return true;
        }
    }

    // post condition for handle_vote_request
    fn post_handle_vote_request(&mut self, msg:&rmessage::RequestVote) {
        self.voted_for = Some(msg.candidate_pid);
        println!("Peer {} has voted for peer {}.", self.pid, msg.candidate_pid);
        self.update_election_timeout();
        // as far as we know, the candidate will become the leader
        self.current_leader = Some(msg.candidate_pid);
    }

    // Receive and process a vote request from a candidate peer
    fn handle_vote_request(&mut self, msg:rmessage::RequestVote) -> (i32, bool) {
        println!("Peer {} received a vote request from {}.", self.pid, msg.candidate_pid);
        self.update_term(msg.candidate_term);
        if self.check_valid_vote_request(&msg) {
            // if all previous checks pass, then vote for the candidate that requested the vote
            self.post_handle_vote_request(&msg);
            return (self.current_term, true);
        }
        else {
            return (self.current_term, false);
        }
    }


    // pre conditions for handle_vote_response
    fn pre_handle_vote_response(&self, msg:&rmessage::ResponseVote) -> bool {
        match self.role {
            Role::CANDIDATE => !self.has_response(msg.follower_pid), // only passes if we haven't received a response from this candidate yet
            _ => {
                println!("Peer {} cannot receive a vote since it is not a candidate.", self.pid);
                false
            }
        }
    }

    // post condition for handle_vote_response
    fn post_handle_vote_response(&mut self, msg:&rmessage::ResponseVote) {
        self.votes_rcvd.insert(self.votes_rcvd.len(), msg.follower_pid);
                    
        if msg.vote_granted {
            self.granted_votes_rcvd.insert(self.granted_votes_rcvd.len(), msg.follower_pid);
        }

        self.become_leader(); // only succeeds if we have majority of granted votes
    }

    // Receive and process a vote response from a peer
    fn handle_vote_response(&mut self, msg:rmessage::ResponseVote) -> () {
        if self.pre_handle_vote_response(&msg) {
            self.update_term(msg.follower_term);
            self.post_handle_vote_response(&msg);
        }
    }


    // check and handle log conflicts in a RequestAppend message
    // returns TRUE if there was a conflict, FALSE otherwise
    fn handle_log_conflicts(&mut self, msg:&rmessage::RequestAppend) -> bool {
        if msg.prev_log_index >= 0 { 
            if msg.prev_log_index >= self.log.len() as i32 {
                // we are missing entries
                return true;
            }
    
            // check if we have conflicting entries
            if self.log[msg.prev_log_index as usize].term != msg.prev_log_term {
                // remove the conflicting entry and all that follow it
                let i = msg.prev_log_index as usize;
                while i < self.log.len() {
                    self.log.remove(i);
                }
                return true;
            }
            // check if we have entries that the leader doesn't know we have (entries after the prev_log_index)
            // if we do, remove them (we can still append the new entries afterwards)
            // Note : this can happen for example when a response to an append_entries is lost in the network, so
            //        the leader will resend entries that we had already added to our log
            while msg.prev_log_index < (self.log.len() as i32) - 1 {
                self.log.remove((msg.prev_log_index + 1) as usize); // remove every entry after the prev_log_index
            }
            return false;
        }
        else {
            // if it is less than 0, means that we are receiving the first entry
            // make sure that our log is empty
            while self.log.len() > 0 {
                self.log.remove(self.log.len() - 1);
            }
            return false;
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
        if !self.handle_log_conflicts(&msg) {
            // if there were no conflicts, we can append the new entries to our log
            self.log.append(&mut msg.entries.clone());
            // update state to latest leader commit
            if msg.leader_commit_index > self.commit_index {
                self.commit_index = cmp::min(msg.leader_commit_index, (self.log.len() - 1) as i32);
            }
            // apply newly commited operations, if there are any
            self.apply_new_commits();

            return (self.current_term, true);
        }
        else {
            return (self.current_term, false);
        }
    }

    // pre conditions for handle_append_entries_response
    fn pre_handle_append_entries_response(&mut self, msg:&rmessage::ResponseAppend) -> bool {
        match self.role {
            Role::LEADER => {
                if msg.follower_term > self.current_term {
                    // we should step down, since there is a follower with an higher term than ours
                    self.update_term(msg.follower_term);
                    return false;
                }
                return true;
            }
            _ => {
                println!("Peer {} cannot process response to append entries since it is not a leader.", self.pid);
                return false;
            }
        }
    }

    // post conditions for handle_append_entries_response
    fn post_handle_append_entries_response(&mut self, msg:&rmessage::ResponseAppend) {
        if !msg.success {
            // decrease entry to be sent
            self.next_index[msg.follower_pid as usize] = cmp::max(self.next_index[msg.follower_pid as usize] - 1, 0);
        }
        else {
            self.next_index[msg.follower_pid as usize]  = msg.match_index as i32 + 1;
        }
        self.match_index[msg.follower_pid as usize] = msg.match_index as i32;
        // self.next_index[msg.follower_pid as usize]  = msg.match_index as i32 + 1;   do this??
    }

    // Receive and process a response to an append entries from a peer
    fn handle_append_entries_response(&mut self, msg:rmessage::ResponseAppend) -> () {
        if self.pre_handle_append_entries_response(&msg) {
            println!("Peer {} received append_entries response.", self.pid);
            self.post_handle_append_entries_response(&msg);
            
            // see if we can commit any new entries
            self.update_commit_index();
            self.apply_new_commits(); // only succeeds if there are new commits
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
        return rmessage::Message::REQVOTE(rmessage::RequestVote {
            candidate_pid: self.pid,
            candidate_term: self.current_term,
            last_log_index: self.log.len() as i32,
            last_log_term: last_log_term,
            sender: self.tx.clone()
        });
    }

    fn create_append_entries_req_msg(&self, new_entries:Vec<Entry>, pli:i32, plt:i32) -> rmessage::Message {
        return rmessage::Message::REQAPPEND(rmessage::RequestAppend {
            leader_term:self.current_term,
            leader_pid:self.pid,
            prev_log_index:pli,
            prev_log_term:plt,
            entries:new_entries,
            leader_commit_index:self.commit_index,
            sender:self.tx.clone()
        });
    }

    fn create_response_vote_msg(&self, vote:bool) -> rmessage::Message {
        return rmessage::Message::RESVOTE(rmessage::ResponseVote {
            follower_term:self.current_term,
            follower_pid:self.pid,
            vote_granted:vote,
            sender:self.tx.clone()
        });
    }

    fn create_response_append_msg(&self, l_pid:i32, res:bool, m_index:i32) -> rmessage::Message {
        return rmessage::Message ::RESAPPEND(rmessage::ResponseAppend {
            leader_pid:l_pid,
            follower_term:self.current_term,
            follower_pid:self.pid,
            success:res,
            match_index:m_index,
            sender:self.tx.clone()
        });
    }

    fn create_reqop_msg_wrapper(&self, reqop:rmessage::RequestOperation) -> rmessage::Message {
        return rmessage::Message::REQOP(reqop);
    }

    /* End of functions for creating messages */

}
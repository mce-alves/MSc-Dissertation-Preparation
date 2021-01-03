// mca @ 49828

// Common state and functions between raft and multipaxos, implemented according to the article "Paxos VS Raft: Have we reached consensus on distributed consensus"

use std::cmp;
use crate::message;
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

#[derive(Clone, Debug)]
pub struct Entry {
    pub operation : String, // operation to be replicated
    pub term : i32          // term when entry was received by leader
}


pub struct CommonState {
	pub pid : i32,                   // this server's identifier
    pub current_term : i32,          // latest term that this server as seen (initialized at 0)
    pub votes_rcvd: Vec<i32>,        // pid of the peers that responded to this candidate's vote request
    pub granted_votes_rcvd: Vec<i32>,// pid of the peers that voted for this candidate's vote request
    pub current_leader: Option<i32>, // pid of who this peer THINKS is the current leader (might not be)
    pub log : Vec<Entry>,            // each entry is an operation for the state machine, and term when entry was received by leader
    pub role: Role,                  // this server's role
    pub commit_index : i32,          // index of highest log entry known to be committed (initialized at -1)
    pub last_applied : i32,          // index of highest log entry applied to the state machine (initialized at -1)
    pub next_index : Vec<i32>,       // for each server, index of the next log entry to send to that server (init 0)
    pub match_index : Vec<i32>,      // for each server, index of highest log entry known to be replicated on that server (init -1)
    pub election_timeout: Option<SystemTime>,  // the system time when an election timeout should occur
    pub heartbeat_timeout: Option<SystemTime>, // the system time when the leader should send a heartbeat
    pub rx: mpsc::Receiver<message::Message>,           // this peer's RX
    pub tx: mpsc::Sender<message::Message>,             // this peer's TX
    pub membership: Vec<mpsc::Sender<message::Message>> // membership (known correct processes)
}


impl CommonState {


	pub fn new(t_pid:i32, t_rx:mpsc::Receiver<message::Message>, t_tx:mpsc::Sender<message::Message>, t_membership:Vec<mpsc::Sender<message::Message>>) -> CommonState {
        CommonState {
            pid: t_pid,
            role: Role::FOLLOWER,
            current_term: 1,
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
    pub fn send_entries(&mut self) -> () {
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

    // Common operations when peers want to begin an election
    pub fn become_candidate(&mut self, msg:message::Message) {
        self.role = Role::CANDIDATE;
        self.granted_votes_rcvd = vec!();
        self.votes_rcvd = vec!();
        self.update_election_timeout();

        for i in 0..self.membership.len() {
            self.request_vote(i, msg.clone());
        }
    }

    // pre conditions for request_vote
    pub fn pre_request_vote(&self, index:usize) -> bool {
        match self.role {
            Role::CANDIDATE => !self.has_response(index as i32),
            _ => {
                println!("Peer {} cannot request a vote since it is not a candidate.", self.pid);
                false
            }
        }
    }

    // Sends a request vote request to peer at <index> in membership
    pub fn request_vote(&self, index:usize, msg:message::Message) -> () {
        if self.pre_request_vote(index) {
            println!("Peer {} is sending a vote request to peer {}.", self.pid, index);
            message::send_msg(&self.membership[index], msg);
        }
    }

    // Get all entries that need to be sent to a peer
    pub fn get_entries_for_peer(&self, peer:usize) -> Vec<Entry> {
        let mut entries = vec!();
        for i in self.next_index[peer] as usize..self.log.len() {
            entries.insert(entries.len(), self.log[i].clone());
        }
        return entries;
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
    pub fn append_entries(&mut self, peer:usize) -> () {
        if self.pre_append_entries() {
            println!("Peer {} is sending append_entries to {}.", self.pid, peer);
            // Get the previous log index and term for the peer
            let prev_log_index = self.next_index[peer] - 1;
            let mut prev_log_term = 0;
            if prev_log_index >= 0 && self.log.len() as i32 > prev_log_index {
                prev_log_term = self.log[prev_log_index as usize].term;
            }
            // Get all new entries that need to be sent to the peer
            let entries = self.get_entries_for_peer(peer);
            // Create and send the append entries request message
            let msg = self.create_append_entries_req_msg(entries, prev_log_index, prev_log_term);
            println!("Peer {} is sending the msg {:?} to peer {}.", self.pid, msg.clone(), peer);
            message::send_msg(&self.membership[peer], msg);
        }
    }

    // pre conditions for become_leader
    pub fn pre_become_leader(&self) -> bool {
        match self.role {
            Role::CANDIDATE => (self.granted_votes_rcvd.len() > (self.membership.len() / 2)),
            _ => {
                println!("Peer {} cannot become leader since it is not a candidate.", self.pid);
                false
            }
        }
    }

    // post conditions for become_leader
    pub fn post_become_leader(&mut self) {
        self.role = Role::LEADER;
        self.current_leader = Some(self.pid);
        // set next index to be the same as our next index
        self.next_index = vec![self.log.len() as i32; self.membership.len()];
        // reset the match index to 0 (as far as we know, no logs in other peers match ours)
        self.match_index = vec![0; self.membership.len()];
        println!("Peer {} is now the leader.", self.pid);
    }

    // pre condition for update_commit_index
    pub fn pre_update_commit_index(&self) -> bool {
        match self.role {
            Role::LEADER => true,
            _ => {
                println!("Peer {} cannot update commit index since it is not a leader.", self.pid);
                false
            }
        }
    }

    // apply an operation
    pub fn apply_operation(&mut self, op:String) {
        println!("Peer {} applied operation: {}", self.pid, op);
        println!("Peer {}'s log is currently: {:?}.", self.pid, self.log);
    }

    // Receive a request from a client to add an operation to the log
    pub fn receive_client_request(&mut self, msg:message::RequestOperation) -> () {
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
                        message::send_msg(&self.membership[id as usize], self.create_reqop_msg_wrapper(msg));
                    },
                    None => println!("Peer {} cannot handle client request because it is not the leader, and does not know who the leader might be.", self.pid)
                }
            }
        }
    }

    // check and handle log conflicts in a RequestAppend message
    // returns TRUE if there was a conflict, FALSE otherwise
    pub fn handle_log_conflicts(&mut self, msg:&message::RequestAppend) -> bool {
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
    pub fn handle_append_entries_request(&mut self, msg:message::RequestAppend) -> (i32, bool) {
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
    pub fn pre_handle_append_entries_response(&mut self, msg:&message::ResponseAppend) -> bool {
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
    pub fn post_handle_append_entries_response(&mut self, msg:&message::ResponseAppend) {
        if !msg.success {
            // decrease entry to be sent
            self.next_index[msg.follower_pid as usize] = cmp::max(self.next_index[msg.follower_pid as usize] - 1, 0);
        }
        else {
            self.next_index[msg.follower_pid as usize]  = msg.match_index as i32 + 1;
        }
        self.match_index[msg.follower_pid as usize] = msg.match_index as i32;
    }

    /* Auxiliary functions */

    // Checks if we already received a response from a peer with pid = <pid>
    pub fn has_response(&self, pid:i32) -> bool {
        return self.votes_rcvd.contains(&pid);
    }

    // update the election timeout
    pub fn update_election_timeout(&mut self) {
        self.election_timeout = SystemTime::now().checked_add(Duration::from_secs(ELECTION_TIMEOUT + (self.pid as u64)));
    }

    // update the heartbeat timeout
    pub fn update_heartbeat_timeout(&mut self) {
        self.heartbeat_timeout = SystemTime::now().checked_add(Duration::from_secs(HEARTBEAT_TIMEOUT));
    }

    // check if there is a timeout (comparing current time to <t>)
    pub fn check_timed_out(&self, t:Option<SystemTime>) -> bool {
        match t {
            Some(time) => {
                return SystemTime::now() >= time
            },
            None => return true
        }
    }

    // apply newly committed operations
    pub fn apply_new_commits(&mut self) -> () {
        // if there are non-applied commits, apply them
        if self.commit_index > self.last_applied {
            for i in self.last_applied+1..=self.commit_index {
                self.apply_operation(self.log[i as usize].operation.clone());
                self.last_applied = i;
            }
        }
    }

    // if <term> is greater than current_term, then set current_term to be equal to <term> and step down (convert to follower)
    pub fn update_term(&mut self, term:i32) {
        if term > self.current_term { 
            self.current_term = term;
            self.role = Role::FOLLOWER;
            //self.voted_for = None;            // TODO: CAREFUL IN RAFT
            self.update_election_timeout();
        }
    }

    /* End of auxiliary functions */

    /* Common functions for creating messages */

    pub fn create_reqop_msg_wrapper(&self, reqop:message::RequestOperation) -> message::Message {
        return message::Message::REQOP(reqop);
    }

    pub fn create_response_append_msg(&self, l_pid:i32, res:bool, m_index:i32) -> message::Message {
        return message::Message ::RESAPPEND(message::ResponseAppend {
            leader_pid:l_pid,
            follower_term:self.current_term,
            follower_pid:self.pid,
            success:res,
            match_index:m_index,
            sender:self.tx.clone()
        });
    }

    pub fn create_append_entries_req_msg(&self, new_entries:Vec<Entry>, pli:i32, plt:i32) -> message::Message {
        return message::Message::REQAPPEND(message::RequestAppend {
            leader_term:self.current_term,
            leader_pid:self.pid,
            prev_log_index:pli,
            prev_log_term:plt,
            entries:new_entries,
            leader_commit_index:self.commit_index,
            sender:self.tx.clone()
        });
    }



}
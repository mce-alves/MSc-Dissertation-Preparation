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


    // Send a request vote message to a peer
    pub fn request_vote(&self, index:usize, msg:message::Message) -> () {
    	match self.role {
            Role::CANDIDATE => {
                println!("Peer {} is sending a vote request to peer {}.", self.pid, index);
                // Only send message if we haven't received a vote response from that peer
                if !self.has_response(index as i32) {
                    message::send_msg(&self.membership[index], msg);
                }
            },
            _ => println!("Peer {} cannot request a vote since it is not a candidate.", self.pid)
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


    // Check if we have received a majority of votes
    pub fn check_vote_majority(&self) -> bool {
    	if !(self.granted_votes_rcvd.len() > (self.membership.len() / 2)) {
            // we did not receive a majority of votes, so we cannot become the leader
            return false;
        }
        println!("Peer {} received {} votes (majority).", self.pid, self.granted_votes_rcvd.len());
        return true;
    }

    // Change state to leader
    pub fn set_role_leader(&mut self) {
    	self.role = Role::LEADER;
        self.current_leader = Some(self.pid);
    }

    // Checks if the leader can update it's commit_index, and if successful applies the newly committed operations
    pub fn update_commit_index(&mut self) -> () {
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

    // checks if a message is out of date
    pub fn is_out_of_date(&self, msg_term:i32) -> bool {
        return msg_term < self.current_term;
    }

    // Update state when voting for a candidate
    pub fn prepare_vote(&mut self, candidate_pid:i32) -> (i32, bool) {
    	println!("Peer {} has voted for peer {}.", self.pid, candidate_pid);
        self.update_election_timeout();
        // as far as we know, the candidate will become the leader
        self.current_leader = Some(candidate_pid);
        return (self.current_term, true);
    }


    // Checks if there are log conflicts (removing the conflicting entries, if they exist)
    pub fn check_handle_log_conflicts(&mut self, prev_log_index:i32, leader_term:i32) -> bool {
        if self.log[prev_log_index as usize].term != leader_term {
            // remove the conflicting entry and all that follow it
            let i = prev_log_index as usize;
            while i < self.log.len() {
                self.log.remove(i);
            }
            return true; // conflicts 
        }
        return false; // no conflicts
    }


    // Decrease next_index upon receiving a failed append_entries response
    pub fn decrease_next_index(&mut self, follower_pid:i32) {
    	self.next_index[follower_pid as usize] = cmp::max(self.next_index[follower_pid as usize] - 1, 0);
    }


    // update state to latest leader commit
    pub fn update_commit_state(&mut self, leader_commit_index:i32) {
        if leader_commit_index > self.commit_index {
            self.commit_index = cmp::min(leader_commit_index, self.log.len() as i32);
        }
    }


    // Checks if we already received a response from a peer with pid = <pid>
    pub fn has_response(&self, pid:i32) -> bool {
        return self.votes_rcvd.contains(&pid);
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









// mca @ 49828

// Implemented according to the article "Paxos VS Raft: Have we reached consensus on distributed consensus"
// with some changes to handle sending multiple entries at the same time

use crate::message;
use crate::common::*;
use std::sync::mpsc;
use std::time::{Duration};

pub struct Peer {
    common: CommonState,    // common state between raft and multipaxos       
    voted_for : Option<i32> // pid of the candidate that received vote in current term (or none)
}

impl Peer {

    pub fn new(t_pid:i32, t_rx:mpsc::Receiver<message::Message>, t_tx:mpsc::Sender<message::Message>, t_membership:Vec<mpsc::Sender<message::Message>>) -> Peer {
        Peer {
            common: CommonState::new(t_pid, t_rx, t_tx, t_membership),
            voted_for: None
        }
    }

    pub fn run(&mut self) {
        loop {
            // role dependant operations
            match self.common.role {
                Role::LEADER => {
                    if self.common.check_timed_out(self.common.heartbeat_timeout) {
                        self.common.send_entries();
                    }
                },
                _ => {
                    if self.common.check_timed_out(self.common.election_timeout) {
                        self.begin_election();
                    }
                }
            }
            self.handle_messages();
        }
    }



    /* Functions for main RAFT operations */

    // When the server gets an election timeout it assumes there is no viable leader, so it begins an election
    fn begin_election(&mut self) {
        println!("Peer {} has started an election.", self.common.pid);
        self.common.current_term += 1;
        self.voted_for = Some(self.common.pid);
        self.common.become_candidate(self.create_request_vote_msg());
    }


    // Assume role of leader
    fn become_leader(&mut self) -> () {
        if self.common.pre_become_leader() {
            println!("Peer {} received {} votes (majority).", self.common.pid, self.common.granted_votes_rcvd.len());
            // we can become the leader
            self.common.post_become_leader();
            println!("Peer {} is now the leader.", self.common.pid);
        }
    }

    // Checks if the leader can update it's commit_index, and if successful applies the newly committed operations
    fn update_commit_index(&mut self) -> () {
        if self.common.pre_update_commit_index() {
            for index in (self.common.commit_index + 1) as usize..self.common.log.len() {
                // count the number of replicas that contain the log entry at index <index>
                let mut num_replicas = 0;
                for peer_match in &self.common.match_index {
                    if *peer_match >= index as i32 {
                        num_replicas += 1;
                    }
                }
                if num_replicas > (self.common.membership.len() / 2) {
                    // a majority of replicas contain the log entry
                    if self.common.log[index].term == self.common.current_term {
                        // a leader can only commit entries from it's own term
                        // (uncommitted entries from previous terms will get committed when the leader
                        // commits one entry from his term)
                        self.common.commit_index = index as i32;
                        println!("Peer {} (leader) updated commit index to {}.", self.common.pid, self.common.commit_index);
                    }
                }
                else {
                    break; // ensure that there are no holes
                }
            }
        }
    }

    

    /* End of functions for main RAFT operations */


    /* Functions to handle received messages */

    

    // check if a RequestVote is valid
    fn check_valid_vote_request(&self, msg:&message::RRequestVote) -> bool {
        if msg.candidate_term < self.common.current_term {
            // candidate is out of date
            return false;
        }
        else {
            let mut self_last_log_term = 0;
            match self.voted_for {
                None => {
                    // we haven't voted for anyone yet
                    if self.common.log.len() != 0 {
                        // check how up to date our log is
                        self_last_log_term = self.common.log[self.common.log.len() - 1].term; 
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
                    if msg.last_log_index < (self.common.log.len() - 1) as i32 {
                        // candidate has short logs
                        return false;
                    }
                }
                
            }
            return true;
        }
    }

    // post condition for handle_vote_request
    fn post_handle_vote_request(&mut self, msg:&message::RRequestVote) {
        self.voted_for = Some(msg.candidate_pid);
        println!("Peer {} has voted for peer {}.", self.common.pid, msg.candidate_pid);
        self.common.update_election_timeout();
        // as far as we know, the candidate will become the leader
        self.common.current_leader = Some(msg.candidate_pid);
    }

    // Receive and process a vote request from a candidate peer
    fn handle_vote_request(&mut self, msg:message::RRequestVote) -> (i32, bool) {
        println!("Peer {} received a vote request from {}.", self.common.pid, msg.candidate_pid);
        self.update_term(msg.candidate_term);
        if self.check_valid_vote_request(&msg) {
            // if all previous checks pass, then vote for the candidate that requested the vote
            self.post_handle_vote_request(&msg);
            return (self.common.current_term, true);
        }
        else {
            return (self.common.current_term, false);
        }
    }


    // pre conditions for handle_vote_response
    fn pre_handle_vote_response(&self, msg:&message::RResponseVote) -> bool {
        match self.common.role {
            Role::CANDIDATE => !self.common.has_response(msg.follower_pid), // only passes if we haven't received a response from this candidate yet
            _ => {
                println!("Peer {} cannot receive a vote since it is not a candidate.", self.common.pid);
                false
            }
        }
    }

    // post condition for handle_vote_response
    fn post_handle_vote_response(&mut self, msg:&message::RResponseVote) {
        self.common.votes_rcvd.insert(self.common.votes_rcvd.len(), msg.follower_pid);
                    
        if msg.vote_granted {
            self.common.granted_votes_rcvd.insert(self.common.granted_votes_rcvd.len(), msg.follower_pid);
        }

        self.become_leader(); // only succeeds if we have majority of granted votes
    }

    // Receive and process a vote response from a peer
    fn handle_vote_response(&mut self, msg:message::RResponseVote) -> () {
        if self.pre_handle_vote_response(&msg) {
            self.post_handle_vote_response(&msg);
            self.update_term(msg.follower_term);
        }
    }

    // Receive and process a response to an append entries from a peer
    fn handle_append_entries_response(&mut self, msg:message::ResponseAppend) -> () {
        if self.common.pre_handle_append_entries_response(&msg) {
            println!("Peer {} received append_entries response.", self.common.pid);
            self.common.post_handle_append_entries_response(&msg);
            
            // see if we can commit any new entries
            self.update_commit_index();
            self.common.apply_new_commits(); // only succeeds if there are new commits
        }
    }

    /* End of functions to handle received messages */


    // if <term> is greater than current_term, then set current_term to be equal to <term> and step down (convert to follower)
    fn update_term(&mut self, term:i32) {
        if term > self.common.current_term { 
            self.common.current_term = term;   // I1
            self.common.role = Role::FOLLOWER; // I2
            self.voted_for = None;
            self.common.update_election_timeout();
        }
    }

    /* Functions for creating messages */

    fn create_request_vote_msg(&self) -> message::Message {
        let mut last_log_term = 0;
        if self.common.log.len() > 0 {
            last_log_term = self.common.log[self.common.log.len()-1].term
        }
        return message::Message::RREQVOTE(message::RRequestVote {
            candidate_pid: self.common.pid,
            candidate_term: self.common.current_term,
            last_log_index: self.common.log.len() as i32,
            last_log_term: last_log_term,
            sender: self.common.tx.clone()
        });
    }

    fn create_response_vote_msg(&self, vote:bool) -> message::Message {
        return message::Message::RRESVOTE(message::RResponseVote {
            follower_term:self.common.current_term,
            follower_pid:self.common.pid,
            vote_granted:vote,
            sender:self.common.tx.clone()
        });
    }

    /* End of functions for creating messages */

    /* Handle Message Receival */

    fn handle_messages(&mut self) -> () {
        let msg = self.common.rx.recv_timeout(Duration::from_millis(100));
        match msg {
            Ok(m) => {
                match m {
                    message::Message::REQAPPEND(req) => {
                        let (_, res) = self.common.handle_append_entries_request(req.clone());
                        if res {
                            let msg = self.common.create_response_append_msg(self.common.pid, res, (self.common.log.len() as i32)-1);
                            message::send_msg(&req.sender, msg);
                        }
                        else {
                            let msg = self.common.create_response_append_msg(self.common.pid, res, -1);
                            message::send_msg(&req.sender, msg);
                        }
                    },
                    message::Message::RESAPPEND(res) => {
                        self.handle_append_entries_response(res);
                    },
                    message::Message::RREQVOTE(req) => {
                        let (_, vote) = self.handle_vote_request(req.clone());
                        message::send_msg(&req.sender, self.create_response_vote_msg(vote));
                    },
                    message::Message::RRESVOTE(res) => {
                        self.handle_vote_response(res);
                    },
                    message::Message::REQOP(req) => {
                        self.common.receive_client_request(req);
                    },
                    _ => println!("Received invalid message")
                }
            },
            Err(_) => () // no message was received before the timeout, which is fine!
        }
    }

}
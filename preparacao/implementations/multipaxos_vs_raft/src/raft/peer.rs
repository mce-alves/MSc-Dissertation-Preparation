// mca @ 49828

// Written according to the TLA specification found in https://github.com/ongardie/raft.tla/blob/master/raft.tla
// with some changes to handle sending multiple entries at the same time (according to the original RAFT article)

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
                        self.send_entries();
                    }
                },
                _ => {
                    if self.common.check_timed_out(self.common.election_timeout) {
                        self.begin_election();
                    }
                }
            }
            // TODO : adicionar hipotese de o lider falhar (deixar de responder)
            // role independant operations
            self.handle_messages();
        }
    }

    // Loops through the membership and sends an append entries request to each peer
    fn send_entries(&mut self) -> () {
        match self.common.role {
            Role::LEADER => {
                self.common.update_heartbeat_timeout();
                println!("Peer {} is sending entries.", self.common.pid);
                for i in 0..self.common.membership.len() {
                    // send an append entries request to each peer in the membership, except itself
                    if i as i32 != self.common.pid {
                        self.append_entries(i);
                    }
                }
            },
            _ => println!("Peer {} cannot send entries because it is not the leader", self.common.pid)
        }
    }

    /* Functions for main RAFT operations */

    // When the server gets an election timeout it assumes there is no viable leader, so it begins an election
    fn begin_election(&mut self) {
        println!("Peer {} has started an election.", self.common.pid);
        self.common.current_term += 1;
        self.common.become_candidate(self.create_request_vote_msg());
    }

    // Sends an append entries request to peer at index <peer> in membership
    fn append_entries(&mut self, peer:usize) -> () {
        match self.common.role {
            Role::LEADER => {
                println!("Peer {} is sending append_entries to {}.", self.common.pid, peer);
                // Get the previous log index and term for the peer
                let prev_log_index = self.common.next_index[peer] - 1;
                let mut prev_log_term = 0;
                if prev_log_index >= 0 && self.common.log.len() as i32 > prev_log_index {
                    prev_log_term = self.common.log[prev_log_index as usize].term;
                }
                // Get all new entries that need to be sent to the peer
                let entries = self.common.get_entries_for_peer(peer);
                // Create and send the append entries request message
                let msg = self.common.create_append_entries_req_msg(entries, prev_log_index, prev_log_term);
                println!("Peer {} is sending the msg {:?} to peer {}.", self.common.pid, msg.clone(), peer);
                message::send_msg(&self.common.membership[peer], msg);
            },
            _ => println!("Peer {} cannot send an append entries request since it is not a leader.", self.common.pid)
        }
    }

    // Assume role of leader
    fn become_leader(&mut self) -> () {
        match self.common.role {
            Role::CANDIDATE => {
                if !self.common.check_vote_majority() {
                    return; // we do not have majority of votes yet
                }
                // we can become the leader
                self.common.set_role_leader();
                // set next index to be the same as our next index
                self.common.next_index = vec![self.common.log.len() as i32; self.common.membership.len()];
                // reset the match index to 0 (as far as we know, no logs in other peers match ours)
                self.common.match_index = vec![0; self.common.membership.len()];
                println!("Peer {} is now the leader.", self.common.pid);
            },
            _ => println!("Peer {} cannot become leader since it is not a candidate.", self.common.pid)
        }
    }

    /* End of functions for main RAFT operations */


    /* Functions to handle received messages */

    // Receive a request from a client to add an operation to the log
    fn receive_client_request(&mut self, msg:message::RequestOperation) -> () {
        self.common.receive_client_request(msg);
    }

    // Receive and process a vote request from a candidate peer
    fn handle_vote_request(&mut self, msg:message::RRequestVote) -> (i32, bool) {
        println!("Peer {} received a vote request from {}.", self.common.pid, msg.candidate_pid);
        if self.common.is_out_of_date(msg.candidate_term) {
            // do not vote for out of date candidates
            return (self.common.current_term, false);
        }
        if msg.candidate_term > self.common.current_term {
            self.voted_for = None;
        }
        self.common.update_term(msg.candidate_term);
        let mut self_last_log_term = 0;
        match self.voted_for {
            None => {
                if self.common.log.len() != 0 {
                    // check how up to date our log is
                    self_last_log_term = self.common.log[self.common.log.len() - 1].term; 
                }
            },
            Some(c_id) => {
                if c_id != msg.candidate_pid {
                    // do not vote for several different candidates
                    return (self.common.current_term, false);
                }
            }
        }
        if msg.last_log_term < self_last_log_term {
            // candidate has out of date logs
            return (self.common.current_term, false);
        }
        if msg.last_log_term == self_last_log_term {
            if self_last_log_term > 0 {
                if msg.last_log_index < (self.common.log.len() - 1) as i32 {
                    // candidate has short logs
                    return (self.common.current_term, false);
                }
            }
            
        }
        // if all previous checks pass, then vote for the candidate that requested the vote
        self.voted_for = Some(msg.candidate_pid);
        return self.common.prepare_vote(msg.candidate_pid);
    }

    // Receive and process a vote response from a peer
    fn handle_vote_response(&mut self, msg:message::RResponseVote) -> () {
        match self.common.role {
            Role::CANDIDATE => {
                println!("Peer {} received vote response from {}.", self.common.pid, msg.follower_pid);
                self.common.update_term(msg.follower_term);
                // check if we have already received a response from that same peer
                if !self.common.has_response(msg.follower_pid) {
                    self.common.votes_rcvd.insert(self.common.votes_rcvd.len(), msg.follower_pid);
                    if msg.vote_granted {
                        self.common.granted_votes_rcvd.insert(self.common.granted_votes_rcvd.len(), msg.follower_pid);
                    }
                    self.become_leader(); // only succeeds if we have majority of granted votes
                }
            },
            _ => println!("Peer {} cannot receive a vote since it is not a candidate.", self.common.pid)
        }
    }

    // Receive and process an append entries request from a peer
    fn handle_append_entries_request(&mut self, msg:message::RequestAppend) -> (i32, bool) {
        println!("Peer {} received append_entries request from {}.", self.common.pid, msg.leader_pid);
        if self.common.is_out_of_date(msg.leader_term) {
            // "old" message
            return (self.common.current_term, false);
        }
        if msg.leader_term > self.common.current_term {
            self.voted_for = None;
        }
        self.common.update_term(msg.leader_term);
        self.common.update_election_timeout();
        // as far as we know, we received a request from a valid leader
        self.common.current_leader = Some(msg.leader_pid);
        if msg.prev_log_index >= 0 { 
            if msg.prev_log_index > self.common.log.len() as i32 {
                // we are missing entries
                return (self.common.current_term, false);
            }
    
            // check if we have conflicting entries
            if self.common.check_handle_log_conflicts(msg.prev_log_index, msg.leader_term) {
                return (self.common.current_term, false);
            }

            // check if we have entries that the leader doesn't know we have (entries after the prev_log_index)
            // if we do, remove them (we can still append the new entries afterwards)
            // Note : this can happen for example when a response to an append_entries is lost in the network, so
            //        the leader will resend entries that we had already added to our log
            while msg.prev_log_index < (self.common.log.len() as i32) - 1 {
                self.common.log.remove((msg.prev_log_index + 1) as usize); // remove every entry after the prev_log_index
            }
        }
        else {
            // if it is equal to 0, means that we are receiving the first entry
            // make sure that our log is empty
            while self.common.log.len() > 0 {
                self.common.log.remove(self.common.log.len() - 1);
            }
        }
        // if all checks pass, we can append the new entries to our log
        self.common.log.append(&mut msg.entries.clone());
        // update state to latest leader commit
        self.common.update_commit_state(msg.leader_commit_index);
        // apply newly commited operations, if there are any
        self.common.apply_new_commits();

        return (self.common.current_term, true);
    }

    // Receive and process a response to an append entries from a peer
    fn handle_append_entries_response(&mut self, msg:message::ResponseAppend) -> () {
        match self.common.role {
            Role::LEADER => {
                println!("Peer {} received append_entries response.", self.common.pid);
                if msg.follower_term > self.common.current_term {
                    // we should step down, since there is a follower with an higher term than ours
                    self.voted_for = None;
                    self.common.update_term(msg.follower_term);
                    return;
                }
                if !msg.success {
                    // decrease entry to be sent
                    self.common.decrease_next_index(msg.follower_pid);
                }
                self.common.match_index[msg.follower_pid as usize] = msg.match_index as i32;
                self.common.next_index[msg.follower_pid as usize]  = msg.match_index as i32 + 1;

                // see if we can commit any new entries
                self.common.update_commit_index();
                self.common.apply_new_commits(); // only succeeds if there are new commits
            },
            _ => println!("Peer {} cannot process response to append entries since it is not a leader.", self.common.pid)
        }
    }

    /* End of functions to handle received messages */



    /* Functions for creating messages */

    fn create_request_vote_msg(&self) -> message::Message {
        let mut last_log_term = 0;
        if self.common.log.len() > 0 {
            last_log_term = self.common.log[self.common.log.len()-1].term
        }
        message::Message {
            msg_type: message::MessageType::REQVOTE,
            request_append: None,
            request_operation: None,
            raft_response_vote: None,
            response_append: None,
            raft_request_vote: Some(message::RRequestVote {
                candidate_pid: self.common.pid,
                candidate_term: self.common.current_term,
                last_log_index: self.common.log.len() as i32,
                last_log_term: last_log_term,
                sender: self.common.tx.clone()
            }),
            paxos_response_vote: None,
            paxos_request_vote: None
        }
    }

    fn create_response_vote_msg(&self, vote:bool) -> message::Message {
        message::Message {
            msg_type: message::MessageType::RESVOTE,
            request_append: None,
            request_operation: None,
            raft_response_vote: Some(message::RResponseVote {
                follower_term:self.common.current_term,
                follower_pid:self.common.pid,
                vote_granted:vote,
                sender:self.common.tx.clone()
            }),
            response_append: None,
            raft_request_vote: None,
            paxos_response_vote: None,
            paxos_request_vote: None
        }
    }

    /* End of functions for creating messages */


    /* Handle Message Receival */

    fn handle_messages(&mut self) -> () {
        let msg = self.common.rx.recv_timeout(Duration::from_millis(100));
        match msg {
            Ok(m) => {
                match m.msg_type {
                    message::MessageType::REQAPPEND => {
                        match m.request_append {
                            Some(req) => {
                                let (_, res) = self.handle_append_entries_request(req.clone());
                                if res {
                                    let msg = self.common.create_response_append_msg(self.common.pid, res, (self.common.log.len() as i32)-1);
                                    message::send_msg(&req.sender, msg);
                                }
                                else {
                                    let msg = self.common.create_response_append_msg(self.common.pid, res, -1);
                                    message::send_msg(&req.sender, msg);
                                }
                            },
                            None => println!("Peer {} received invalid append entries request.", self.common.pid)
                        }
                    },
                    message::MessageType::RESAPPEND => {
                        match m.response_append {
                            Some(req) => self.handle_append_entries_response(req),
                            None => println!("Peer {} received invalid append entries response.", self.common.pid)
                        }
                    },
                    message::MessageType::REQVOTE => {
                        match m.raft_request_vote {
                            Some(req) => {
                                let (_, vote) = self.handle_vote_request(req.clone());
                                message::send_msg(&req.sender, self.create_response_vote_msg(vote))
                            },
                            None => println!("Peer {} received invalid vote request.", self.common.pid)
                        }
                    },
                    message::MessageType::RESVOTE => {
                        match m.raft_response_vote {
                            Some(req) => self.handle_vote_response(req),
                            None => println!("Peer {} received invalid vote reponse.", self.common.pid)
                        }
                    },
                    message::MessageType::REQOP => {
                        match m.request_operation {
                            Some(req) => self.receive_client_request(req),
                            None => println!("Peer {} received invalid client request.", self.common.pid)
                        }
                    }
                }
            },
            Err(_) => () // no message was received before the timeout, which is fine!
        }
    }

}
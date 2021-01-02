// mca @ 49828

// Written according to the article "Paxos VS Raft: Have we reached consensus on distributed consensus"

use crate::message;
use crate::common::*;
use std::sync::mpsc;
use std::time::{Duration};

pub struct Peer {
    common: CommonState, // common state between multipaxos and raft
    entries: Vec<Entry>  // received alongside request_vote responses votes from each peer
}

impl Peer {

    pub fn new(t_pid:i32, t_rx:mpsc::Receiver<message::Message>, t_tx:mpsc::Sender<message::Message>, t_membership:Vec<mpsc::Sender<message::Message>>) -> Peer {
        Peer {
            common: CommonState::new(t_pid, t_rx, t_tx, t_membership),
            entries: vec!()
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



    /* Functions for main MultiPaxos operations */

    // When the server gets an election timeout it assumes there is no viable leader, so it begins an election
    fn begin_election(&mut self) {
        println!("Peer {} has started an election.", self.common.pid);
        // increment current_term to next t, such that <t mod n = s>
        while (self.common.current_term % (self.common.membership.len() as i32)) != self.common.pid {
            self.common.current_term += 1;
        }
        // copy any log entries after commit_index to entries[]
        self.entries = vec!(); // ensure there are no old values from a previous election
        for i in (self.common.commit_index+1) as usize..self.common.log.len() {
            self.entries.push(self.common.log[i].clone());
        }

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
                    return; // we do not have majority of votes
                }
                // we can become the leader
                self.common.set_role_leader();
                // set next index to be the one immediately after our commit index
                self.common.next_index = vec![self.common.commit_index + 1; self.common.membership.len()];
                // reset the match index to 0 (as far as we know, no logs in other peers match ours)
                self.common.match_index = vec![0; self.common.membership.len()];
                // add entries received in the request_vote responses to our log, using our current_term
                for e in self.entries.as_slice() {
                    let mut entry = e.clone();
                    entry.term = self.common.current_term;
                    self.common.log.push(entry);
                }
                println!("Peer {} is now the leader.", self.common.pid);
            },
            _ => println!("Peer {} cannot become leader since it is not a candidate.", self.common.pid)
        }
    }

    /* End of functions for main RAFT operations */


    /* Functions to handle received messages */

    // Receive and process a vote request from a candidate peer
    fn handle_vote_request(&mut self, msg:message::MPRequestVote) -> (i32, bool, Vec<Entry>) {
        println!("Peer {} received a vote request from {}.", self.common.pid, msg.candidate_pid);
        if msg.candidate_term < self.common.current_term {
            // do not vote for out of date candidates
            return (self.common.current_term, false, vec!());
        }
        self.common.update_term(msg.candidate_term);
        
        // send log entries after the candidate's commit_index
        let mut tmp_entries = vec!();
        for i in (msg.leader_commit+1) as usize..self.common.log.len() {
            tmp_entries.push(self.common.log[i].clone());
        }

        // vote for the candidate that requested the vote
        println!("Peer {} has voted for peer {}.", self.common.pid, msg.candidate_pid);
        self.common.update_election_timeout();
        // as far as we know, the candidate will become the leader
        self.common.current_leader = Some(msg.candidate_pid);
        return (self.common.current_term, true, tmp_entries);
    }

    // Receive and process a vote response from a peer
    fn handle_vote_response(&mut self, msg:message::MPResponseVote) -> () {
        match self.common.role {
            Role::CANDIDATE => {
                println!("Peer {} received vote response from {}.", self.common.pid, msg.follower_pid);
                self.common.update_term(msg.follower_term);
                // check if we have already received a response from that same peer
                if !self.common.has_response(msg.follower_pid) {
                    self.common.votes_rcvd.insert(self.common.votes_rcvd.len(), msg.follower_pid); // store received vote
                    if msg.vote_granted {
                        // store received granted vote
                        self.common.granted_votes_rcvd.insert(self.common.granted_votes_rcvd.len(), msg.follower_pid);
                        // add log entries received (are logs after our commit_index) to entries[]
                        for i in 0..msg.entries.len() {
                            if i > self.entries.len() {
                                // we haven't received any entry for this index, so add this one
                                self.entries.push(msg.entries[i].clone());
                            }
                            else {
                                // we have already seen another entry for this index
                                // so keep the one with the highest term
                                if msg.entries[i].term > self.entries[i].term {
                                    self.entries[i] = msg.entries[i].clone();
                                }
                            }
                        }
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
        return message::Message::MPREQVOTE(message::MPRequestVote {
            candidate_pid: self.common.pid,
            candidate_term: self.common.current_term,
            leader_commit: self.common.commit_index,
            sender: self.common.tx.clone()
        });
    }

    fn create_response_vote_msg(&self, vote:bool, t_entries:Vec<Entry>) -> message::Message {
        return message::Message::MPRESVOTE(message::MPResponseVote {
            follower_term:self.common.current_term,
            follower_pid:self.common.pid,
            vote_granted:vote,
            entries:t_entries,
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
                    message::Message::RESAPPEND(res) => {
                        self.handle_append_entries_response(res);
                    },
                    message::Message::MPREQVOTE(req) => {
                        let (_, vote, entries) = self.handle_vote_request(req.clone());
                        message::send_msg(&req.sender, self.create_response_vote_msg(vote, entries));
                    },
                    message::Message::MPRESVOTE(res) => {
                        self.handle_vote_response(res);
                    },
                    message::Message::REQOP(req) => {
                        self.common.receive_client_request(req);
                    },
                    _ => println!("Received invalid message")
                }
            },
            Err(_) => () // no message was received before the set timeout, which is fine!
        }
    }

}
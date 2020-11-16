// mca @ 49828

use std::sync::mpsc;
use rand::Rng;
use std::thread;
use std::time;

use crate::raft::*;

static CHANCE_OF_FAILURE:i32 = 5; // chance of a message not being sent


#[derive(Clone, Debug)]
pub enum MessageType {
    REQVOTE,   // request to execute request_vote operation
    RESVOTE,   // result of executing request_vote operation
    REQAPPEND, // request to execute append_entries operation
    RESAPPEND, // result of executing append_entries operation
    REQOP      // request of to execute an operation by a client
}

#[derive(Clone, Debug)]
pub struct Message {
    pub msg_type: MessageType,                       // Type of the message content. If type is req_vote, then request_vote should be SOME and the others None, etc.
    pub request_vote:  Option<RequestVote>,          // RequestVote  message struct or none
    pub response_vote:  Option<ResponseVote>,        // RequestVote  message struct or none
    pub request_append:  Option<RequestAppend>,      // RequestVote  message struct or none
    pub response_append:  Option<ResponseAppend>,    // RequestVote  message struct or none
    pub request_operation:  Option<RequestOperation> // RequestVote  message struct or none
}

#[derive(Clone, Debug)]
pub struct RequestVote {
    pub candidate_term:i32,           // candidate's term
    pub candidate_pid:i32,            // pid of the candidate requesting the vote
    pub last_log_index:i32,           // index of the candidate's last log entry
    pub last_log_term:i32,            // term of the candidate's last log entry
    pub sender:mpsc::Sender<Message>  // where to send the response to this message
}

#[derive(Clone, Debug)]
pub struct ResponseVote {
    pub follower_term:i32,            // follower's term
    pub follower_pid:i32,             // pid of the follower sending the response
    pub vote_granted:bool,            // true if the follower voted for the candidate, false otherwise
    pub sender:mpsc::Sender<Message>  // where to send the response to this message
}

#[derive(Clone, Debug)]
pub struct RequestAppend {
    pub leader_term:i32,                  // leader's term
    pub leader_pid:i32,                   // pid of the leader
    pub prev_log_index:i32,               // index of log entry immediately preceding new ones
    pub prev_log_term:i32,                // term of prev_log_index entry
    pub entries:Vec<Entry>,               // log entries to store (empty for heartbeat)
    pub leader_commit_index:i32,          // leader's commit_index
    pub sender:mpsc::Sender<Message>      // where to send the response to this message
}

#[derive(Clone, Debug)]
pub struct ResponseAppend {
    pub leader_pid:i32,               // pid of the leader that requested the vote
    pub follower_term:i32,            // follower's term
    pub follower_pid:i32,             // pid of the follower sending the response
    pub success:bool,                 // true if the follower appended the entries to it's log, false otherwise
    pub match_index:i32,              // latest index of follower
    pub sender:mpsc::Sender<Message>  // where to send the response to this message
}

#[derive(Clone, Debug)]
pub struct RequestOperation {
    pub operation:String  // the operation to execute
}

/// Functions

// Send a message to all processes in the membership
pub fn broadcast(membership:&Vec<mpsc::Sender<Message>>, msg:Message) -> () {
    for member in membership {
        send_msg(member, msg.clone());
    }
}

// Send a message to a process with a chance for the message to get lost
pub fn send_msg(destination:&mpsc::Sender<Message>, msg:Message) -> () {
    let mut rng = rand::thread_rng();

    let delay = rng.gen_range(1, 50);
    thread::sleep(time::Duration::new(0, delay)); // add a delay to make it easier to test concurrent proposals
    
    let roll = rng.gen_range(1, 100);
    if roll > CHANCE_OF_FAILURE { // chance for the message to be "lost in the network"
        destination.send(msg).unwrap();
    }
}
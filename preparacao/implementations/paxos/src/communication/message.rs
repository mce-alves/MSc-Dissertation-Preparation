// mca @ 49828

use std::sync::mpsc;
use rand::Rng;

static CHANCE_OF_FAILURE:i32 = 5; // chance of a message not being sent


#[derive(Clone)]
pub enum MessageType {
    BEGIN,
    PREPARE,
    PROMISE,
    PROPOSE,
    ACCEPTED,
    REJECTED
}

#[derive(Clone)]
pub struct Message {
    pub msg_type: MessageType,      // Type of the message content. If type is prepare, then prepare should be SOME and other None, etc.
    pub prepare:  Option<Prepare>,  // Prepare  message struct or none
    pub promise:  Option<Promise>,  // Promise  message struct or none
    pub propose:  Option<Propose>,  // Propose  message struct or none
    pub accepted: Option<Accepted>, // Accepted message struct or none
    pub rejected: Option<Rejected>  // Rejected message struct or none
}

#[derive(Clone)]
pub struct Prepare {
    pub id:i32,                       // ID that sender wants to use in a future proposal
    pub sender_pid:i32,               // identifier of the sender process
    pub sender:mpsc::Sender<Message>  // where to send the response to this message
}

#[derive(Clone)]
pub struct Promise {
    pub id:i32,                      // minimum ID the sender promises to accept
    pub sender_pid:i32,              // identifier of the sender process
    pub accepted_id:Option<i32>,     // if already accepted a proposal, then Some(id) else None
    pub accepted_value:Option<i32>,  // if already accepted a proposal, then Some(value) else None
    pub sender:mpsc::Sender<Message> // where to send the response to this message
}

#[derive(Clone)]
pub struct Propose {
    pub id:i32,                      // ID associated with the proposal
    pub sender_pid:i32,              // identifier of the sender process
    pub value:i32,                   // proposed value
    pub sender:mpsc::Sender<Message> // where to send the response to this message
}

#[derive(Clone)]
pub struct Accepted {
    pub id:i32,                      // ID of the accepted proposal
    pub sender_pid:i32,              // identifier of the sender process
    pub value:i32,                   // accepted value
    pub sender:mpsc::Sender<Message> // where to send the response to this message
}

#[derive(Clone)]
pub struct Rejected {
    pub id:i32,        // ID that was rejected
    pub max_id:i32,    // highest ID that the acceptor had seen so far
    pub sender_pid:i32 // identifier of the sender process
}

// Send a message to all processes in the membership
pub fn broadcast(membership:Vec<mpsc::Sender<Message>>, msg:Message) -> () {
    for member in membership {
        send_msg(member, msg.clone());
    }
}

// Send a message to a process with a chance for the message to get lost
pub fn send_msg(destination:mpsc::Sender<Message>, msg:Message) -> () {
    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(1, 100);
    if roll > CHANCE_OF_FAILURE {
        destination.send(msg).unwrap();
    }
}


// mca @ 49828

use std::sync::mpsc;
use rand::Rng;

static CHANCE_OF_FAILURE:i32 = 5;

#[derive(Clone)]
pub enum MessageType {
    PREPARE,
    PROMISE,
    PROPOSE,
    ACCEPTED,
    REJECTED
}

#[derive(Clone)]
pub struct Message {
    pub msg_type: MessageType,
    pub prepare:  Option<Prepare>,
    pub promise:  Option<Promise>,
    pub propose:  Option<Propose>,
    pub accepted: Option<Accepted>,
    pub rejected: Option<Rejected>
}

#[derive(Clone)]
pub struct Prepare {
    pub id:i32,
    pub sender_pid:i32,
    pub sender:mpsc::Sender<Message>
}

#[derive(Clone)]
pub struct Promise {
    pub id:i32,
    pub sender_pid:i32,
    pub accepted_id:Option<i32>,
    pub accepted_value:Option<i32>,
    pub sender:mpsc::Sender<Message>
}

#[derive(Clone)]
pub struct Propose {
    pub id:i32,
    pub sender_pid:i32,
    pub value:i32,
    pub sender:mpsc::Sender<Message>
}

#[derive(Clone)]
pub struct Accepted {
    pub id:i32,
    pub sender_pid:i32,
    pub value:i32,
    pub sender:mpsc::Sender<Message>
}

#[derive(Clone)]
pub struct Rejected {
    pub id:i32,
    pub max_id:i32
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


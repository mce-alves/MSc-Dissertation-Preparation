// mca @ 49828

use std::sync::mpsc;
use crate::pmessage::*;

pub struct Learner {
    pid: i32,                              // id of the learner
    last_accepted_id: f32,                 // id of the last learned (accepted proposal) value
    current_value: i32,                    // current value stored by the learner
    is_distinguished: bool,                // marks if this is the distinguished learner
    membership: Vec<mpsc::Sender<Message>> // membership (known correct processes)
}

impl Learner {

    pub fn new(t_pid:i32, t_membership:Vec<mpsc::Sender<Message>>) -> Learner {
        Learner {
            pid: t_pid,
            last_accepted_id: -1.0,
            current_value: -1,
            is_distinguished: false,
            membership: t_membership
        }
    }

    // process an incoming ACCEPTED message
    pub fn rcv_accept(&mut self, acc_msg:Consensus) -> () {
        if acc_msg.id > self.last_accepted_id {
            self.last_accepted_id = acc_msg.id;
            self.current_value = acc_msg.value;
            println!("Learner {} received ACCEPTED for with id={};val={}.", self.pid, acc_msg.id, acc_msg.value);
            if self.is_distinguished {
                self.propagate_accepted_msg(acc_msg);
            }
        }
    }

    // propagate an ACCEPTED message to all the other learners in the membership
    pub fn propagate_accepted_msg(&self, msg:Consensus) -> () {
        if self.is_distinguished {
            println!("Learner {} (distinguished) is propagating ACCEPTED message to other learners.", self.pid);
            broadcast(&self.membership, Message::CONSENSUS(msg));
        }
        else {
            println!("Learner {} cannot propagate ACCEPTED message because it is not distinguished.", self.pid);
        }
    }

    // updates this learner's distinguished status
    pub fn set_distinguished_status(&mut self, status:bool) -> () {
        self.is_distinguished = status;
    }

}

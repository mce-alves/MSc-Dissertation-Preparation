// mca @ 49828

/*
    Notes:
    * Messages between threads : https://doc.rust-lang.org/book/ch16-02-message-passing.html
    * Simple Paxos explanation : https://www.cs.rutgers.edu/~pxk/417/notes/paxos.html
    * Better Paxos explanation : https://paxos.systems/index.html
*/

use std::sync::mpsc;
use std::thread;

#[path = "communication/message.rs"] mod message;
#[path = "paxos/node.rs"] mod node;

struct Message {
    msg_type:String,
    msg_id:i32,
    msg_content:i32
}

fn thread_communication_test() -> () {
    let (tx, rx) = mpsc::channel::<Message>();

    let process1 = thread::spawn({
        let tx1 = mpsc::Sender::clone(&tx);
        move || {
            for i in 0..10 {
                tx1.send(Message{msg_type:"PREPARE".to_string(), msg_id:i, msg_content:i}).unwrap();
            }
        }
    });

    let process2 = thread::spawn({
        let tx1 = mpsc::Sender::clone(&tx);
        move || {
            for i in 0..10 {
                tx1.send(Message{msg_type:"PROPOSE".to_string(), msg_id:i, msg_content:i}).unwrap();
            }
        }
    });

    for rcvd in rx {
        println!("Got Message: msg_type={}, msg_id={}, msg_content={}", rcvd.msg_type, rcvd.msg_id, rcvd.msg_content);
    }

    let _ = process1.join();
    let _ = process2.join();
}

fn main() {
    thread_communication_test();
}


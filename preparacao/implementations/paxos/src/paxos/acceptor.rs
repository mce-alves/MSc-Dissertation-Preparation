// mca @ 49828

use std::sync::mpsc;
use std::sync::Arc;
use crate::message::*;

enum AcceptorState {
    IDLE,
    PROMISED,
    ACCEPTED
}

struct Acceptor {
    state: AcceptorState,                       // state / phase of the acceptor 
    max_id: i32,                                // highest ID seen so far
    this_tx:Arc<mpsc::Sender<Message>>,         // this proposers TX
    membership:Arc<Vec<mpsc::Sender<Message>>>, // membership (known correct processes)
    accepted:Vec<i32>                           // vector of all accepted values
}


impl Acceptor {

    fn new(tx:Arc<mpsc::Sender<Message>>, mship:Arc<Vec<mpsc::Sender<Message>>>) -> Acceptor {
        Acceptor {
            state: AcceptorState::IDLE,
            max_id: -1,
            this_tx: tx,
            membership: mship,
            accepted: Vec::new()
        }
    }

    fn snd_promise(&mut self) -> () {
        
    }

    fn snd_accept(&mut self) -> () {
        
    }

    fn rcv_prepare(&mut self) -> () {
        /* 
            if (ID <= max_id)
                do not respond (or respond with a "fail" message)
            else
                max_id = ID     // save highest ID we've seen so far
                if (proposal_accepted == true) // was a proposal already accepted?
                    respond: PROMISE(ID, accepted_ID, accepted_VALUE)
                else
                    respond: PROMISE(ID)
        */
    }

    fn rcv_propose(&mut self) -> () {
        /*
            if (ID == max_id) // is the ID the largest I have seen so far?
                proposal_accepted = true     // note that we accepted a proposal
                accepted_ID = ID             // save the accepted proposal number
                accepted_VALUE = VALUE       // save the accepted proposal data
                respond: ACCEPTED(ID, VALUE) to the proposer and all learners
            else
                do not respond (or respond with a "fail" message)
        */
    }

}
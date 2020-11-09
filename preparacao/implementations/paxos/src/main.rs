// mca @ 49828

#[path = "tests/paxos_tests.rs"] mod tests;
#[path = "communication/message.rs"] mod message;
#[path = "paxos/agent.rs"] mod agent;

fn main() {
    tests::test_nprocesses_single_proposal();
}


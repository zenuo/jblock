//! Emit a Java reproducer to stdout.
//!
//! Usage: cargo run --example gen_java -- <scenario> [count]
//!
//! Scenarios: lock-contention | deadlock | thread-pool-exhaustion
//!
//! Example:
//!   cargo run --example gen_java -- deadlock 3 > DeadlockCycle.java
//!   javac DeadlockCycle.java && java DeadlockCycle &
//!   jstack <pid>

use std::process::exit;

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().unwrap_or_default();
    let count: usize = args.next().and_then(|c| c.parse().ok()).unwrap_or(3);

    match jblock::parse_scenario(&scenario) {
        Some(scenario) => print!("{}", jblock::generate_java_source(scenario, count)),
        None => {
            eprintln!(
                "unknown scenario: {scenario:?}\nusage: gen_java <lock-contention|deadlock|thread-pool-exhaustion|sync-io-hotspot|dangerous-hot-lock|connection-pool-starve|future-latch-deadlock|logging-appender-contention> [count]"
            );
            exit(2);
        }
    }
}

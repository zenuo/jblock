//! feat-007 tests: Java code generation for lock-contention and deadlock cycles.
//! Included from `codegen.rs` as a child module (`#[path]`), so scenario items
//! are reachable via `super::`.

use super::{generate, parse_scenario, Scenario};

#[test]
fn parses_scenario_aliases() {
    assert_eq!(parse_scenario("lock-contention"), Some(Scenario::LockContention));
    assert_eq!(parse_scenario("Contention"), Some(Scenario::LockContention));
    assert_eq!(parse_scenario("waiting_threads"), Some(Scenario::LockContention));
    assert_eq!(parse_scenario("deadlock"), Some(Scenario::Deadlock));
    assert_eq!(parse_scenario("deadlock-cycle"), Some(Scenario::Deadlock));
    assert_eq!(
        parse_scenario("thread-pool-exhaustion"),
        Some(Scenario::ThreadPoolExhaustion)
    );
    assert_eq!(parse_scenario("pool"), Some(Scenario::ThreadPoolExhaustion));
    assert_eq!(
        parse_scenario("sync-io-hotspot"),
        Some(Scenario::SyncIoHotspot)
    );
    assert_eq!(parse_scenario("rpc-hotspot"), Some(Scenario::SyncIoHotspot));
    assert_eq!(
        parse_scenario("dangerous-hot-lock"),
        Some(Scenario::DangerousHotLock)
    );
    assert_eq!(parse_scenario("blocking-owner"), Some(Scenario::DangerousHotLock));
    assert_eq!(parse_scenario("nonsense"), None);
}

#[test]
fn lock_contention_has_expected_structure() {
    let code = generate(Scenario::LockContention, 4);
    assert!(code.contains("public class LockContention"));
    assert!(code.contains("private static final Object LOCK"));
    assert!(code.contains("synchronized (LOCK)"));
    // Requested worker count is embedded.
    assert!(code.contains("final int workers = 4;"));
    // Threads are named so a dump groups them clearly.
    assert!(code.contains("\"worker-\" + i"));
}

#[test]
fn deadlock_forms_a_cycle() {
    let code = generate(Scenario::Deadlock, 3);
    assert!(code.contains("public class DeadlockCycle"));
    assert!(code.contains("final int n = 3;"));
    // Circular wait: lock i then lock (i+1) % n.
    assert!(code.contains("locks[(i + 1) % n]"));
    assert!(code.contains("synchronized (first)"));
    assert!(code.contains("synchronized (second)"));
}

#[test]
fn thread_pool_exhaustion_uses_executor() {
    let code = generate(Scenario::ThreadPoolExhaustion, 4);
    assert!(code.contains("public class ThreadPoolExhaustion"));
    assert!(code.contains("Executors.newFixedThreadPool"));
    assert!(code.contains("final int workers = 4;"));
    assert!(code.contains("synchronized (LOCK)"));
    assert!(code.contains("pool.submit"));
}

#[test]
fn sync_io_hotspot_blocks_on_socket_read() {
    let code = generate(Scenario::SyncIoHotspot, 4);
    assert!(code.contains("public class SyncIoHotspot"));
    assert!(code.contains("ServerSocket"));
    assert!(code.contains("final int clients = 4;"));
    assert!(code.contains("rpc-client-"));
    assert!(code.contains("getInputStream()"));
    assert!(code.contains("in.read()"));
}

#[test]
fn dangerous_hot_lock_owner_sleeps_while_holding() {
    let code = generate(Scenario::DangerousHotLock, 4);
    assert!(code.contains("public class DangerousHotLock"));
    assert!(code.contains("\"lock-owner\""));
    assert!(code.contains("waiter-"));
    assert!(code.contains("final int waiters = 3;"));
    assert!(code.contains("Thread.sleep(Long.MAX_VALUE)"));
    assert!(code.contains("synchronized (LOCK)"));
}

#[test]
fn count_is_clamped_to_sane_range() {
    // Below minimum is bumped to 2.
    let low = generate(Scenario::Deadlock, 0);
    assert!(low.contains("final int n = 2;"));
    // Above maximum is capped at 64.
    let high = generate(Scenario::LockContention, 9999);
    assert!(high.contains("final int workers = 64;"));
}

#[test]
fn class_name_matches_source() {
    assert_eq!(Scenario::LockContention.class_name(), "LockContention");
    assert_eq!(Scenario::Deadlock.class_name(), "DeadlockCycle");
    assert_eq!(
        Scenario::ThreadPoolExhaustion.class_name(),
        "ThreadPoolExhaustion"
    );
    assert_eq!(Scenario::SyncIoHotspot.class_name(), "SyncIoHotspot");
    assert_eq!(Scenario::DangerousHotLock.class_name(), "DangerousHotLock");
    assert!(generate(Scenario::Deadlock, 2).contains("class DeadlockCycle"));
}

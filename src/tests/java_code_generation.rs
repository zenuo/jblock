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
    assert!(generate(Scenario::Deadlock, 2).contains("class DeadlockCycle"));
}

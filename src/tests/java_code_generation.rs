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
    assert_eq!(
        parse_scenario("connection-pool-starve"),
        Some(Scenario::ConnectionPoolStarve)
    );
    assert_eq!(parse_scenario("hikari-starve"), Some(Scenario::ConnectionPoolStarve));
    assert_eq!(
        parse_scenario("future-latch-deadlock"),
        Some(Scenario::FutureLatchDeadlock)
    );
    assert_eq!(parse_scenario("future-get"), Some(Scenario::FutureLatchDeadlock));
    assert_eq!(
        parse_scenario("logging-appender-contention"),
        Some(Scenario::LoggingAppenderContention)
    );
    assert_eq!(
        parse_scenario("logback-contention"),
        Some(Scenario::LoggingAppenderContention)
    );
    assert_eq!(parse_scenario("busy-wait-spin"), Some(Scenario::BusyWaitSpin));
    assert_eq!(parse_scenario("cpu-spin"), Some(Scenario::BusyWaitSpin));
    assert_eq!(
        parse_scenario("condition-starvation"),
        Some(Scenario::ConditionStarvation)
    );
    assert_eq!(parse_scenario("park-starvation"), Some(Scenario::ConditionStarvation));
    assert_eq!(parse_scenario("lock-order-risk"), Some(Scenario::LockOrderRisk));
    assert_eq!(
        parse_scenario("inconsistent-lock-order"),
        Some(Scenario::LockOrderRisk)
    );
    assert_eq!(parse_scenario("finalizer-pressure"), Some(Scenario::FinalizerPressure));
    assert_eq!(parse_scenario("reference-handler"), Some(Scenario::FinalizerPressure));
    assert_eq!(parse_scenario("sleep-as-scheduler"), Some(Scenario::SleepAsScheduler));
    assert_eq!(parse_scenario("thread-sleep-scheduler"), Some(Scenario::SleepAsScheduler));
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
fn connection_pool_starve_blocks_on_borrow() {
    let code = generate(Scenario::ConnectionPoolStarve, 4);
    assert!(code.contains("public class ConnectionPoolStarve"));
    assert!(code.contains("HikariDataSource"));
    assert!(code.contains("getConnection()"));
    assert!(code.contains("borrowObject()"));
    assert!(code.contains("db-borrower-"));
    assert!(code.contains("pool-holder"));
    assert!(code.contains("final int waiters = 3;"));
}

#[test]
fn future_latch_deadlock_forms_wait_tree() {
    let code = generate(Scenario::FutureLatchDeadlock, 3);
    assert!(code.contains("public class FutureLatchDeadlock"));
    assert!(code.contains("CompletableFuture"));
    assert!(code.contains("CountDownLatch"));
    assert!(code.contains("future-waiter-"));
    assert!(code.contains("latch-waiter-"));
    assert!(code.contains("final int futureWaiters = 3;"));
    assert!(code.contains(".get()"));
    assert!(code.contains(".await()"));
}

#[test]
fn logging_appender_contention_holds_appender_lock() {
    let code = generate(Scenario::LoggingAppenderContention, 4);
    assert!(code.contains("public class LoggingAppenderContention"));
    assert!(code.contains("OutputStreamAppender"));
    assert!(code.contains("doAppend"));
    assert!(code.contains("log-holder"));
    assert!(code.contains("log-writer-"));
    assert!(code.contains("final int waiters = 3;"));
    assert!(code.contains("synchronized void append"));
}

#[test]
fn busy_wait_spin_uses_tight_loop() {
    let code = generate(Scenario::BusyWaitSpin, 4);
    assert!(code.contains("public class BusyWaitSpin"));
    assert!(code.contains("spinUntilReady"));
    assert!(code.contains("spin-worker-"));
    assert!(code.contains("final int workers = 4;"));
    assert!(code.contains("while (!ready)"));
    assert!(code.contains("sink++"));
}

#[test]
fn condition_starvation_awaits_without_signal() {
    let code = generate(Scenario::ConditionStarvation, 4);
    assert!(code.contains("public class ConditionStarvation"));
    assert!(code.contains("ReentrantLock"));
    assert!(code.contains("Condition"));
    assert!(code.contains("COND.await()"));
    assert!(code.contains("cond-waiter-"));
    assert!(code.contains("final int waiters = 4;"));
    // No runtime signal call — only await (comments may mention signal).
    assert!(!code.contains("COND.signal"));
    assert!(!code.contains(".signal("));
    assert!(!code.contains(".signalAll("));
}

#[test]
fn lock_order_risk_uses_opposite_orders() {
    let code = generate(Scenario::LockOrderRisk, 2);
    assert!(code.contains("public class LockOrderRisk"));
    assert!(code.contains("LOCK_A"));
    assert!(code.contains("LOCK_B"));
    assert!(code.contains("order-ab"));
    assert!(code.contains("order-ba"));
    assert!(code.contains("synchronized (LOCK_A)"));
    assert!(code.contains("synchronized (LOCK_B)"));
}

#[test]
fn finalizer_pressure_blocks_in_finalize() {
    let code = generate(Scenario::FinalizerPressure, 3);
    assert!(code.contains("public class FinalizerPressure"));
    assert!(code.contains("HeavyFinalizer"));
    assert!(code.contains("finalize()"));
    assert!(code.contains("app-lock-holder"));
    assert!(code.contains("app-waiter-"));
    assert!(code.contains("System.gc()"));
    assert!(code.contains("final int waiters = 2;"));
    assert!(code.contains("synchronized (LOCK)"));
}

#[test]
fn sleep_as_scheduler_uses_sleep_loop() {
    let code = generate(Scenario::SleepAsScheduler, 4);
    assert!(code.contains("public class SleepAsScheduler"));
    assert!(code.contains("scheduleNextTick"));
    assert!(code.contains("Thread.sleep"));
    assert!(code.contains("sleep-scheduler-"));
    assert!(code.contains("final int workers = 4;"));
    assert!(code.contains("while (true)"));
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
    assert_eq!(
        Scenario::ConnectionPoolStarve.class_name(),
        "ConnectionPoolStarve"
    );
    assert_eq!(
        Scenario::FutureLatchDeadlock.class_name(),
        "FutureLatchDeadlock"
    );
    assert_eq!(
        Scenario::LoggingAppenderContention.class_name(),
        "LoggingAppenderContention"
    );
    assert_eq!(Scenario::BusyWaitSpin.class_name(), "BusyWaitSpin");
    assert_eq!(
        Scenario::ConditionStarvation.class_name(),
        "ConditionStarvation"
    );
    assert_eq!(Scenario::LockOrderRisk.class_name(), "LockOrderRisk");
    assert_eq!(Scenario::FinalizerPressure.class_name(), "FinalizerPressure");
    assert_eq!(Scenario::SleepAsScheduler.class_name(), "SleepAsScheduler");
    assert!(generate(Scenario::Deadlock, 2).contains("class DeadlockCycle"));
}

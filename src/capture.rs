//! Host-only helper: compile generated Java, run it, capture a jstack dump.
//!
//! Used by pattern tests (feat-027+) so each scenario can prove both the
//! reproducer source and a real JVM thread dump.
//!
//! feat-049 also captures `jcmd Thread.dump_to_file -format=json` for JDK 21+
//! virtual-thread scenarios.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Location of JDK tools. Prefers explicit `JAVA_HOME`, else PATH.
fn tool(name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let candidate = Path::new(&home).join("bin").join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

/// True when `javac`, `java`, and `jstack` are runnable.
pub fn jdk_tools_available() -> bool {
    for name in ["javac", "java", "jstack"] {
        let status = Command::new(tool(name))
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // `jstack -version` may exit non-zero on some JDKs; existence via `-h`/`-version` still spawns.
        if status.is_err() {
            // Fall back to `command -v`.
            let ok = Command::new("sh")
                .args(["-c", &format!("command -v {name}")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                return false;
            }
        }
    }
    true
}

/// True when JDK tools include `jcmd` and `java -version` reports major >= 21.
pub fn jdk21_tools_available() -> bool {
    if !jdk_tools_available() {
        return false;
    }
    let jcmd_ok = Command::new("sh")
        .args(["-c", "command -v jcmd"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !jcmd_ok {
        // Also try JAVA_HOME/bin/jcmd.
        if !tool("jcmd").exists() && tool("jcmd") == PathBuf::from("jcmd") {
            return false;
        }
    }
    let out = Command::new(tool("java"))
        .args(["-XshowSettings:properties", "-version"])
        .output();
    let Ok(out) = out else {
        return false;
    };
    // Properties land on stderr for -version.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("java.version = ") {
            let major = rest
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            return major >= 21;
        }
        // Older "java version \"21.x\"" form on some builds.
        if let Some(idx) = line.find("version \"") {
            let after = &line[idx + "version \"".len()..];
            let major = after
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            if major >= 21 {
                return true;
            }
        }
    }
    false
}

fn wait_until_attachable(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    let needle = format!("{pid} ");
    while start.elapsed() < timeout {
        if let Ok(out) = Command::new(tool("jcmd")).arg("-l").output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if text.lines().any(|l| l.starts_with(&needle)) {
                // Brief settle so the attach socket is ready.
                thread::sleep(Duration::from_millis(400));
                return true;
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

/// Compile `ClassName.java` containing `source`, run it, wait, jstack, then kill.
///
/// Returns the jstack text. The process is always terminated.
pub fn compile_run_jstack(
    source: &str,
    class_name: &str,
    warmup: Duration,
) -> io::Result<String> {
    let dir = tempfile_dir()?;
    let java_path = dir.join(format!("{class_name}.java"));
    fs::write(&java_path, source)?;

    let javac = Command::new(tool("javac"))
        .current_dir(&dir)
        .arg(java_path.file_name().unwrap())
        .output()?;
    if !javac.status.success() {
        return Err(io::Error::other(format!(
            "javac failed:\n{}",
            String::from_utf8_lossy(&javac.stderr)
        )));
    }

    let mut child = Command::new(tool("java"))
        .current_dir(&dir)
        .args(["-cp", ".", class_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    thread::sleep(warmup);

    let pid = child.id().to_string();
    let jstack = Command::new(tool("jstack"))
        .arg(&pid)
        .output();

    let _ = child.kill();
    let _ = child.wait();

    let jstack = jstack?;
    if !jstack.status.success() {
        return Err(io::Error::other(format!(
            "jstack failed for pid {pid}:\n{}",
            String::from_utf8_lossy(&jstack.stderr)
        )));
    }

    let dump = String::from_utf8_lossy(&jstack.stdout).into_owned();
    if !dump.contains("Full thread dump") && !dump.contains("\n\"") {
        return Err(io::Error::other(
            "jstack output did not look like a thread dump",
        ));
    }
    Ok(dump)
}

/// Compile/run `source`, then `jcmd Thread.dump_to_file -format=json` (feat-049).
///
/// Requires JDK 21+. Waits until the JVM is listed by `jcmd -l` before attaching.
pub fn compile_run_dump_to_file_json(
    source: &str,
    class_name: &str,
    warmup: Duration,
) -> io::Result<String> {
    let dir = tempfile_dir()?;
    let java_path = dir.join(format!("{class_name}.java"));
    fs::write(&java_path, source)?;

    let javac = Command::new(tool("javac"))
        .current_dir(&dir)
        .arg(java_path.file_name().unwrap())
        .output()?;
    if !javac.status.success() {
        return Err(io::Error::other(format!(
            "javac failed:\n{}",
            String::from_utf8_lossy(&javac.stderr)
        )));
    }

    let mut child = Command::new(tool("java"))
        .current_dir(&dir)
        .args(["-cp", ".", class_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    thread::sleep(warmup);
    let pid = child.id();
    if !wait_until_attachable(pid, Duration::from_secs(20)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other(format!(
            "JVM pid {pid} never became attachable via jcmd -l"
        )));
    }

    let out_path = dir.join("thread_dump.json");
    let jcmd = Command::new(tool("jcmd"))
        .arg(pid.to_string())
        .args([
            "Thread.dump_to_file",
            "-format=json",
            out_path.to_str().unwrap_or("thread_dump.json"),
        ])
        .output();

    let _ = child.kill();
    let _ = child.wait();

    let jcmd = jcmd?;
    if !jcmd.status.success() {
        return Err(io::Error::other(format!(
            "jcmd dump_to_file failed for pid {pid}:\n{}",
            String::from_utf8_lossy(&jcmd.stderr)
        )));
    }
    if !out_path.exists() {
        return Err(io::Error::other(
            "jcmd reported success but dump file is missing",
        ));
    }
    let dump = fs::read_to_string(&out_path)?;
    if !dump.contains("\"threadDump\"") {
        return Err(io::Error::other(
            "dump_to_file JSON missing threadDump object",
        ));
    }
    Ok(dump)
}

fn tempfile_dir() -> io::Result<PathBuf> {
    let base = std::env::temp_dir().join(format!(
        "jblock-capture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&base)?;
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{generate, Scenario};
    use crate::parser::{analyze, PatternKind};
    use std::time::Duration;

    #[test]
    fn live_capture_thread_pool_exhaustion_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::ThreadPoolExhaustion, 4);
        let dump = compile_run_jstack(
            &source,
            "ThreadPoolExhaustion",
            Duration::from_millis(800),
        )
        .expect("compile/run/jstack");

        // Optionally refresh the offline fixture when explicitly requested.
        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/thread_pool_exhaustion_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ThreadPoolExhaustion),
            "expected thread-pool-exhaustion in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(40).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ThreadPoolExhaustion)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("pool-")),
            "expected pool-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_sync_io_hotspot_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::SyncIoHotspot, 4);
        let dump = compile_run_jstack(&source, "SyncIoHotspot", Duration::from_millis(1000))
            .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/sync_io_hotspot_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::SyncIoHotspot),
            "expected sync-io-hotspot in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(60).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::SyncIoHotspot)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("rpc-client-")),
            "expected rpc-client-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_dangerous_hot_lock_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::DangerousHotLock, 4);
        let dump =
            compile_run_jstack(&source, "DangerousHotLock", Duration::from_millis(900))
                .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/dangerous_hot_lock_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::DangerousHotLockOwner),
            "expected dangerous-hot-lock-owner in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(60).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::DangerousHotLockOwner)
            .unwrap();
        assert!(
            hit.thread_names.iter().any(|n| n == "lock-owner"),
            "names={:?}",
            hit.thread_names
        );
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("waiter-")),
            "names={:?}",
            hit.thread_names
        );
        assert!(hit.detail.contains("sleep") || hit.detail.contains("Thread.sleep"));
    }

    #[test]
    fn live_capture_connection_pool_starve_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::ConnectionPoolStarve, 4);
        let dump = compile_run_jstack(
            &source,
            "ConnectionPoolStarve",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/connection_pool_starve_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ConnectionPoolBorrow),
            "expected connection-pool-borrow in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(80).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ConnectionPoolBorrow)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("db-borrower-")),
            "expected db-borrower-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_future_latch_deadlock_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::FutureLatchDeadlock, 3);
        let dump = compile_run_jstack(
            &source,
            "FutureLatchDeadlock",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/future_latch_deadlock_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FutureLatchWaitTree),
            "expected future-latch-wait-tree in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FutureLatchWaitTree)
            .unwrap();
        assert!(hit.thread_names.len() >= 2, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("future-waiter-")),
            "expected future-waiter-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_logging_appender_contention_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::LoggingAppenderContention, 4);
        let dump = compile_run_jstack(
            &source,
            "LoggingAppenderContention",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/logging_appender_contention_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::LoggingAppenderContention),
            "expected logging-appender-contention in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::LoggingAppenderContention)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n == "log-holder"),
            "expected log-holder, got {:?}",
            hit.thread_names
        );
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("log-writer-")),
            "expected log-writer-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_busy_wait_spin_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::BusyWaitSpin, 4);
        let dump = compile_run_jstack(&source, "BusyWaitSpin", Duration::from_millis(900))
            .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/busy_wait_spin_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::BusyWaitSpinHotspot),
            "expected busy-wait-spin-hotspot in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::BusyWaitSpinHotspot)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("spin-worker-")),
            "expected spin-worker-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_condition_starvation_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::ConditionStarvation, 4);
        let dump = compile_run_jstack(
            &source,
            "ConditionStarvation",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/condition_starvation_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ConditionParkStarvation),
            "expected condition-park-starvation in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ConditionParkStarvation)
            .unwrap();
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("cond-waiter-")),
            "expected cond-waiter-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_lock_order_risk_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::LockOrderRisk, 2);
        let dump =
            compile_run_jstack(&source, "LockOrderRisk", Duration::from_millis(1200))
                .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/lock_order_risk_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::LockOrderInconsistency),
            "expected lock-order-inconsistency in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::LockOrderInconsistency)
            .unwrap();
        assert!(
            hit.thread_names.iter().any(|n| n == "order-ab"),
            "names={:?}",
            hit.thread_names
        );
        assert!(
            hit.thread_names.iter().any(|n| n == "order-ba"),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_finalizer_pressure_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::FinalizerPressure, 3);
        let dump = compile_run_jstack(
            &source,
            "FinalizerPressure",
            Duration::from_millis(2200),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/finalizer_pressure_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FinalizerPressure),
            "expected finalizer-pressure in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(120).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FinalizerPressure)
            .unwrap();
        assert!(
            hit.thread_names.iter().any(|n| n == "Finalizer"),
            "expected Finalizer thread, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_sleep_as_scheduler_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::SleepAsScheduler, 4);
        let dump = compile_run_jstack(
            &source,
            "SleepAsScheduler",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/sleep_as_scheduler_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::SleepAsScheduler),
            "expected sleep-as-scheduler in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(120).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::SleepAsScheduler)
            .unwrap();
        assert!(
            hit.thread_names
                .iter()
                .filter(|n| n.starts_with("sleep-scheduler-"))
                .count()
                >= 3,
            "expected sleep-scheduler-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_framework_pool_saturation_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::FrameworkPoolSaturation, 4);
        let dump = compile_run_jstack(
            &source,
            "FrameworkPoolSaturation",
            Duration::from_millis(1000),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/framework_pool_saturation_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FrameworkPoolSaturation),
            "expected framework-pool-saturation in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(120).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FrameworkPoolSaturation)
            .unwrap();
        assert!(
            hit.thread_names
                .iter()
                .filter(|n| n.starts_with("http-nio-8080-exec-"))
                .count()
                >= 3,
            "expected http-nio-8080-exec-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_dns_resolution_stall_detects_pattern() {
        if !jdk_tools_available() {
            eprintln!("skip live capture: JDK tools not available");
            return;
        }
        let source = generate(Scenario::DnsResolutionStall, 4);
        let dump = compile_run_jstack(
            &source,
            "DnsResolutionStall",
            Duration::from_millis(1500),
        )
        .expect("compile/run/jstack");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/patterns/dns_resolution_stall_jstack.txt");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::DnsResolutionStall),
            "expected dns-resolution-stall in patterns, got {:?}; dump head:\n{}",
            a.patterns.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            dump.lines().take(140).collect::<Vec<_>>().join("\n")
        );
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::DnsResolutionStall)
            .unwrap();
        assert!(
            hit.thread_names
                .iter()
                .filter(|n| n.starts_with("dns-resolver-"))
                .count()
                >= 3,
            "expected dns-resolver-* threads, got {:?}",
            hit.thread_names
        );
    }

    #[test]
    fn live_capture_virtual_thread_block_dump_to_file() {
        if !jdk21_tools_available() {
            eprintln!("skip live VT capture: JDK 21+ / jcmd not available");
            return;
        }
        let source = generate(Scenario::VirtualThreadBlock, 3);
        let dump = compile_run_dump_to_file_json(
            &source,
            "VirtualThreadBlock",
            Duration::from_millis(1200),
        )
        .expect("compile/run/dump_to_file json");

        if std::env::var_os("JBLOCK_UPDATE_FIXTURES").is_some() {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/virtual-threads/dump_to_file.json");
            if let Some(parent) = fixture.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&fixture, &dump);
        }

        let a = analyze(&dump);
        assert_eq!(a.format, crate::parser::DumpFormat::ThreadDumpJson);
        let virtuals: Vec<_> = a
            .threads
            .iter()
            .filter(|t| t.kind == crate::parser::ThreadKind::Virtual)
            .collect();
        assert!(
            virtuals.len() >= 3,
            "expected >=3 virtual threads, got {}; dump head:\n{}",
            virtuals.len(),
            dump.chars().take(800).collect::<String>()
        );
        assert!(
            virtuals.iter().any(|t| t.name.starts_with("vt-waiter-")),
            "expected vt-waiter-* names, got {:?}",
            virtuals.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }
}

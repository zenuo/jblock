//! Host-only helper: compile generated Java, run it, capture a jstack dump.
//!
//! Used by pattern tests (feat-027+) so each scenario can prove both the
//! reproducer source and a real JVM thread dump.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

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
        .arg(class_name)
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
}

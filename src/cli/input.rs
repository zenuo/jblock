//! Resolve dump text from files, stdin, or clipboard (feat-056 / feat-066).

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command;

use super::CliOptions;

/// Host clipboard tools, tried in order until one succeeds.
///
/// macOS `pbpaste`, Wayland `wl-paste`, X11 `xclip`/`xsel`, then Windows
/// PowerShell `Get-Clipboard -Raw` (feat-066; needed by the Windows CLI artifact).
pub(crate) const CLIPBOARD_TOOLS: &[(&str, &[&str])] = &[
    ("pbpaste", &[]),
    ("wl-paste", &["--no-newline"]),
    ("xclip", &["-selection", "clipboard", "-o"]),
    ("xsel", &["--clipboard", "--output"]),
    (
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Clipboard -Raw",
        ],
    ),
    (
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Clipboard -Raw",
        ],
    ),
];

/// One dump payload with a human-readable source label.
#[derive(Debug, Clone)]
pub struct InputSource {
    pub label: String,
    pub text: String,
}

#[derive(Debug)]
pub enum InputError {
    Usage(String),
    Io(String),
    Clipboard(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputError::Usage(m) | InputError::Io(m) | InputError::Clipboard(m) => {
                write!(f, "{m}")
            }
        }
    }
}

impl std::error::Error for InputError {}

/// Read dump inputs according to CLI options.
///
/// Priority: `--clipboard` > positional files > stdin (when non-TTY or `-`).
pub fn read_inputs(opts: &CliOptions) -> Result<Vec<InputSource>, InputError> {
    if opts.clipboard {
        let text = read_clipboard()?;
        if text.trim().is_empty() {
            return Err(InputError::Clipboard("clipboard is empty".to_string()));
        }
        return Ok(vec![InputSource {
            label: "clipboard".to_string(),
            text,
        }]);
    }

    if !opts.files.is_empty() {
        let mut out = Vec::with_capacity(opts.files.len());
        for path in &opts.files {
            if path.as_os_str() == "-" {
                let text = read_stdin()?;
                out.push(InputSource {
                    label: "stdin".to_string(),
                    text,
                });
                continue;
            }
            out.push(read_file(path)?);
        }
        return Ok(out);
    }

    // No files: require piped stdin (non-TTY) so we do not hang on an empty terminal.
    if io::stdin().is_terminal() {
        return Err(InputError::Usage(
            "no input: pass a FILE, pipe a dump on stdin, or use --clipboard\n\
             try: jblock dump.txt | jstack <pid> | jblock | jblock -c"
                .to_string(),
        ));
    }

    let text = read_stdin()?;
    Ok(vec![InputSource {
        label: "stdin".to_string(),
        text,
    }])
}

fn read_file(path: &PathBuf) -> Result<InputSource, InputError> {
    let text = fs::read_to_string(path).map_err(|e| {
        InputError::Io(format!("failed to read {}: {e}", path.display()))
    })?;
    Ok(InputSource {
        label: path.display().to_string(),
        text,
    })
}

fn read_stdin() -> Result<String, InputError> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| InputError::Io(format!("failed to read stdin: {e}")))?;
    if buf.trim().is_empty() {
        return Err(InputError::Io("stdin is empty".to_string()));
    }
    Ok(buf)
}

/// Read UTF-8 text from the system clipboard via common OS tools.
///
/// Tries `pbpaste` (macOS), `wl-paste` (Wayland), `xclip` / `xsel` (X11),
/// then Windows PowerShell `Get-Clipboard -Raw`. Avoids heavy clipboard crates
/// so the host CLI builds on older rustc toolchains.
pub fn read_clipboard() -> Result<String, InputError> {
    read_clipboard_from_tools(CLIPBOARD_TOOLS)
}

/// Spawn each `(bin, args)` with [`Command`] until one exits 0 with UTF-8 stdout.
pub(crate) fn read_clipboard_from_tools(
    tools: &[(&str, &[&str])],
) -> Result<String, InputError> {
    let mut last_err = String::from("no clipboard tool found");
    for (bin, args) in tools {
        match Command::new(bin).args(*args).output() {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8(out.stdout).map_err(|e| {
                    InputError::Clipboard(format!("clipboard was not valid UTF-8: {e}"))
                })?;
                return Ok(strip_bom(text));
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                last_err = format!(
                    "{bin} exited {}: {}",
                    out.status.code().unwrap_or(-1),
                    stderr.trim()
                );
            }
            Err(e) => {
                last_err = format!("{bin}: {e}");
            }
        }
    }

    Err(InputError::Clipboard(format!(
        "clipboard unavailable ({last_err}); install wl-paste, xclip, xsel, pbpaste, or PowerShell"
    )))
}

fn strip_bom(text: String) -> String {
    text.trim_start_matches('\u{feff}').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static CLIPBOARD_PATH_LOCK: Mutex<()> = Mutex::new(());
    #[cfg(unix)]
    static SCRIPT_SEQ: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn read_file_loads_fixture() {
        let path = PathBuf::from("tests/fixtures/deadlock_real_jstack.txt");
        let src = read_file(&path).expect("fixture");
        assert!(src.text.contains("Full thread dump"));
        assert!(src.label.contains("deadlock_real_jstack"));
    }

    #[test]
    fn clipboard_tools_include_windows_powershell() {
        let has_ps = CLIPBOARD_TOOLS.iter().any(|(bin, args)| {
            (*bin == "powershell" || *bin == "powershell.exe")
                && args.iter().any(|a| a.contains("Get-Clipboard"))
        });
        assert!(
            has_ps,
            "CLIPBOARD_TOOLS must include Windows Get-Clipboard: {CLIPBOARD_TOOLS:?}"
        );
    }

    #[test]
    fn clipboard_missing_tool_is_error() {
        let err = read_clipboard_from_tools(&[("/nonexistent/jblock-pbpaste", &[])])
            .expect_err("missing binary");
        match err {
            InputError::Clipboard(msg) => {
                assert!(
                    msg.contains("clipboard unavailable"),
                    "{msg}"
                );
            }
            other => panic!("expected Clipboard error, got {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn clipboard_strips_utf8_bom() {
        let dir = unique_tmp_dir();
        let payload = dir.join("bom.txt");
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(b"hello dump\n");
        fs::write(&payload, bytes).expect("bom file");
        let script = write_unix_script(&format!("cat '{}'", payload.display()));
        let text = read_clipboard_from_tools(&[(script.to_str().unwrap(), &[])])
            .expect("script");
        assert_eq!(text, "hello dump\n");
    }

    #[cfg(unix)]
    #[test]
    fn clipboard_skips_failing_tool_then_reads_next() {
        let fail = write_unix_script("echo nope >&2; exit 2");
        let ok = write_unix_script("printf 'second tool dump\\n'");
        let text = read_clipboard_from_tools(&[
            (fail.to_str().unwrap(), &[]),
            (ok.to_str().unwrap(), &[]),
        ])
        .expect("fallback tool");
        assert_eq!(text, "second tool dump\n");
    }

    #[cfg(unix)]
    #[test]
    fn read_inputs_clipboard_loads_real_fixture_via_pbpaste() {
        let fixture = fs::read_to_string("tests/fixtures/deadlock_real_jstack.txt")
            .expect("fixture");
        let _guard = CLIPBOARD_PATH_LOCK.lock().expect("lock");
        let dir = with_path_pbpaste_script(&format!(
            "cat '{}'",
            PathBuf::from("tests/fixtures/deadlock_real_jstack.txt")
                .canonicalize()
                .expect("abs fixture")
                .display()
        ));
        let _restore = dir;
        let opts = CliOptions {
            clipboard: true,
            ..CliOptions::default()
        };
        let srcs = read_inputs(&opts).expect("clipboard input");
        assert_eq!(srcs.len(), 1);
        assert_eq!(srcs[0].label, "clipboard");
        assert_eq!(srcs[0].text, fixture);
    }

    #[cfg(unix)]
    #[test]
    fn read_inputs_empty_clipboard_is_error() {
        let _guard = CLIPBOARD_PATH_LOCK.lock().expect("lock");
        let _restore = with_path_pbpaste_script("printf ''");
        let opts = CliOptions {
            clipboard: true,
            ..CliOptions::default()
        };
        let err = read_inputs(&opts).expect_err("empty");
        match err {
            InputError::Clipboard(msg) => assert!(msg.contains("empty"), "{msg}"),
            other => panic!("expected empty clipboard, got {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn clipboard_flag_analyzes_deadlock_fixture() {
        let fixture_path = PathBuf::from("tests/fixtures/deadlock_real_jstack.txt")
            .canonicalize()
            .expect("abs fixture");
        let _guard = CLIPBOARD_PATH_LOCK.lock().expect("lock");
        let _restore = with_path_pbpaste_script(&format!("cat '{}'", fixture_path.display()));
        let opts = CliOptions {
            clipboard: true,
            ..CliOptions::default()
        };
        let mut out = std::io::Cursor::new(Vec::new());
        let mut err = std::io::Cursor::new(Vec::new());
        let code = crate::cli::run_with_writers(&opts, &mut out, &mut err);
        assert_eq!(code, crate::cli::CliExit::Problems);
        let s = String::from_utf8(out.into_inner()).unwrap();
        assert!(s.contains("FINDINGS"), "{s}");
        assert!(s.contains("DEADLOCK") || s.contains("deadlock"), "{s}");
        assert!(s.contains("clipboard"), "{s}");
    }

    #[cfg(unix)]
    struct PathRestore {
        old: String,
    }

    #[cfg(unix)]
    impl Drop for PathRestore {
        fn drop(&mut self) {
            std::env::set_var("PATH", &self.old);
        }
    }

    /// Prepend a temp dir containing an executable `pbpaste` to PATH.
    #[cfg(unix)]
    fn with_path_pbpaste_script(body: &str) -> PathRestore {
        let old = std::env::var("PATH").unwrap_or_default();
        let dir = unique_tmp_dir();
        write_unix_script_named(&dir, "pbpaste", body);
        std::env::set_var("PATH", format!("{}:{old}", dir.display()));
        PathRestore { old }
    }

    #[cfg(unix)]
    fn unique_tmp_dir() -> PathBuf {
        let seq = SCRIPT_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "jblock-clip-{}-{seq}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("tmpdir");
        dir
    }

    #[cfg(unix)]
    fn write_unix_script(body: &str) -> PathBuf {
        write_unix_script_named(&unique_tmp_dir(), "clip-tool", body)
    }

    #[cfg(unix)]
    fn write_unix_script_named(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }
}

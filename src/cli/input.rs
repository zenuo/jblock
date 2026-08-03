//! Resolve dump text from files, stdin, or clipboard (feat-056).

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command;

use super::CliOptions;

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
/// Tries `pbpaste` (macOS), `wl-paste` (Wayland), then `xclip` / `xsel` (X11).
/// Avoids heavy clipboard crates so the host CLI builds on older rustc toolchains.
pub fn read_clipboard() -> Result<String, InputError> {
    let attempts: &[(&str, &[&str])] = &[
        ("pbpaste", &[]),
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-selection", "clipboard", "-o"]),
        ("xsel", &["--clipboard", "--output"]),
    ];

    let mut last_err = String::from("no clipboard tool found");
    for (bin, args) in attempts {
        match Command::new(bin).args(*args).output() {
            Ok(out) if out.status.success() => {
                return String::from_utf8(out.stdout).map_err(|e| {
                    InputError::Clipboard(format!("clipboard was not valid UTF-8: {e}"))
                });
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
        "clipboard unavailable ({last_err}); install wl-paste, xclip, xsel, or pbpaste"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_loads_fixture() {
        let path = PathBuf::from("tests/fixtures/deadlock_real_jstack.txt");
        let src = read_file(&path).expect("fixture");
        assert!(src.text.contains("Full thread dump"));
        assert!(src.label.contains("deadlock_real_jstack"));
    }
}

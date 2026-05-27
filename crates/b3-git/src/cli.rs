use b3_core::{GitReaderConfig, GitStatusError, GitStatusErrorKind};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(crate) fn run_git(
    project_root: &Path,
    args: &[&str],
    config: GitReaderConfig,
) -> Result<String, GitStatusError> {
    if !config.allow_git_cli {
        return Err(GitStatusError::new(
            GitStatusErrorKind::GitNotFound,
            "Git CLI is disabled by configuration",
        ));
    }

    let mut child = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            GitStatusError::new(
                GitStatusErrorKind::GitNotFound,
                format!("failed to start git: {err}"),
            )
        })?;

    let mut stdout = child.stdout.take().ok_or_else(|| {
        GitStatusError::new(GitStatusErrorKind::Unknown, "failed to capture git stdout")
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        GitStatusError::new(GitStatusErrorKind::Unknown, "failed to capture git stderr")
    })?;

    let max_stdout_bytes = config.max_stdout_bytes;
    let (stdout_tx, stdout_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        let mut too_large = false;
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if output.len().saturating_add(n) > max_stdout_bytes {
                        too_large = true;
                        break;
                    }
                    output.extend_from_slice(&buffer[..n]);
                }
                Err(err) => {
                    let _ = stdout_tx.send(Err(GitStatusError::new(
                        GitStatusErrorKind::Unknown,
                        format!("failed to read git stdout: {err}"),
                    )));
                    return;
                }
            }
        }

        if too_large {
            let _ = stdout_tx.send(Err(GitStatusError::new(
                GitStatusErrorKind::OutputTooLarge,
                "git stdout exceeded configured limit",
            )));
        } else {
            let _ = stdout_tx.send(Ok(output));
        }
    });

    let (stderr_tx, stderr_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let _ = stderr.read_to_end(&mut output);
        let _ = stderr_tx.send(output);
    });

    let timeout = Duration::from_millis(config.command_timeout_ms);
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|err| {
            GitStatusError::new(
                GitStatusErrorKind::Unknown,
                format!("failed to wait for git: {err}"),
            )
        })? {
            break status;
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitStatusError::new(
                GitStatusErrorKind::TimedOut,
                "git command timed out",
            ));
        }

        thread::sleep(Duration::from_millis(10));
    };

    let stdout = stdout_rx.recv().map_err(|err| {
        GitStatusError::new(
            GitStatusErrorKind::Unknown,
            format!("failed to receive git stdout: {err}"),
        )
    })??;

    let stderr = stderr_rx.recv().unwrap_or_default();

    if !status.success() {
        let message = String::from_utf8_lossy(&stderr).trim().to_string();
        return Err(GitStatusError::new(
            GitStatusErrorKind::CommandFailed,
            if message.is_empty() {
                format!("git command failed with status {status}")
            } else {
                message
            },
        ));
    }

    String::from_utf8(stdout).map_err(|err| {
        GitStatusError::new(
            GitStatusErrorKind::InvalidUtf8,
            format!("git output was not valid UTF-8: {err}"),
        )
    })
}

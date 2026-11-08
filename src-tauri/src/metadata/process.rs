use super::Failure;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

// Shared across all Toolchain instances. Future playback must use this same gate.
static PROCESS_GATE: Mutex<()> = Mutex::new(());
pub(crate) const MAX_OUTPUT: usize = 1_048_576;

pub(super) fn run(
    mut command: Command,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, Failure> {
    let _permit = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Failure::Cancelled);
        }
        match PROCESS_GATE.try_lock() {
            Ok(permit) => break permit,
            Err(std::sync::TryLockError::Poisoned(_)) => return Err(Failure::ProcessFailed),
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Failure::ToolMissing
        } else {
            Failure::ProcessFailed
        }
    })?;
    // Readers retain at most the cap and continue draining; no pipe deadlock or unbounded stderr.
    let output = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();
    let overflow = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let read = |mut input: Box<dyn Read + Send>, limit| {
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                match input.read(&mut buffer) {
                    Ok(0) => return Ok(bytes),
                    Ok(n) => {
                        let keep = n.min(limit - bytes.len());
                        bytes.extend_from_slice(&buffer[..keep]);
                        if n > keep {
                            overflow.store(true, Ordering::Relaxed);
                        }
                    }
                    Err(_) => return Err(Failure::ProcessFailed),
                }
            }
        };
        let out = scope.spawn(move || read(Box::new(output), MAX_OUTPUT));
        let err = scope.spawn(move || read(Box::new(errors), 65536));
        let start = Instant::now();
        let result = loop {
            let failure = if cancel.load(Ordering::Relaxed) {
                Some(Failure::Cancelled)
            } else if start.elapsed() >= timeout {
                Some(Failure::Timeout)
            } else if overflow.load(Ordering::Relaxed) {
                Some(Failure::OutputInvalid)
            } else {
                None
            };
            if let Some(failure) = failure {
                let _ = child.kill();
                let _ = child.wait();
                break Err(failure);
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err(Failure::ProcessFailed)
                    }
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(Failure::ProcessFailed);
                }
            }
        };
        let bytes = out.join().map_err(|_| Failure::ProcessFailed)??;
        let _stderr = err.join().map_err(|_| Failure::ProcessFailed)??;
        result?;
        if overflow.load(Ordering::Relaxed) {
            return Err(Failure::OutputInvalid);
        }
        Ok(bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child() {
        match std::env::var("MEDIA_MANAGER_PROCESS_TEST").as_deref() {
            Ok("sleep") => std::thread::sleep(Duration::from_secs(30)),
            Ok("overflow") => {
                use std::io::Write;
                let _ = std::io::stdout().write_all(&vec![b'x'; MAX_OUTPUT * 2]);
            }
            Ok("fail") => std::process::exit(7),
            _ => (),
        }
    }
    fn child_command(case: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", "metadata::process::tests::child", "--nocapture"])
            .env("MEDIA_MANAGER_PROCESS_TEST", case);
        c
    }
    #[test]
    fn process_failure_timeout_cancel_and_output_limits_are_reaped() {
        let cancel = AtomicBool::new(false);
        assert_eq!(
            run(child_command("fail"), Duration::from_secs(5), &cancel).unwrap_err(),
            Failure::ProcessFailed
        );
        assert_eq!(
            run(child_command("sleep"), Duration::from_millis(100), &cancel).unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(
            run(child_command("overflow"), Duration::from_secs(5), &cancel).unwrap_err(),
            Failure::OutputInvalid
        );
        std::thread::scope(|scope| {
            let cancel_ref = &cancel;
            scope.spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                cancel_ref.store(true, Ordering::Relaxed);
            });
            assert_eq!(
                run(child_command("sleep"), Duration::from_secs(5), &cancel).unwrap_err(),
                Failure::Cancelled
            );
        });
        // A following child succeeding proves the shared process permit was released on every failure.
        assert!(run(
            child_command("success"),
            Duration::from_secs(5),
            &AtomicBool::new(false)
        )
        .is_ok());
    }
}

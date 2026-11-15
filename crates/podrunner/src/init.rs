//! `serve` as PID 1 of a container (T-180): orphans of the units come to
//! PID 1, and a process that reaps none of them fills the process table
//! with zombies. As PID 1, podrunner forks: the parent reaps each child
//! and passes each signal on, as `tini` does, and the child runs `serve`.
//! A reap of each child in `serve` itself would take the exit status of a
//! process that a driver waits for.

use std::sync::atomic::{AtomicI32, Ordering};

static CHILD: AtomicI32 = AtomicI32::new(0);

extern "C" fn forward(sig: libc::c_int) {
    let child = CHILD.load(Ordering::SeqCst);
    if child > 0 {
        // SAFETY: kill(2) is async-signal-safe.
        unsafe { libc::kill(child, sig) };
    }
}

/// As PID 1, fork: the parent reaps until the child ends and exits with its
/// status; the child returns. Another process returns at once. Call it
/// before any thread starts.
pub fn reap_as_pid1() {
    // SAFETY: getpid(2) has no precondition.
    if unsafe { libc::getpid() } != 1 {
        return;
    }
    // SAFETY: no thread runs yet; the child goes on with this one thread.
    let child = unsafe { libc::fork() };
    match child {
        -1 => eprintln!("podrunner: PID 1: no fork ({}): orphans are not reaped", std::io::Error::last_os_error()),
        0 => {}
        child => std::process::exit(reap(child)),
    }
}

/// Reap each child until `child` ends; its exit code (128 and the signal
/// for a signal).
fn reap(child: libc::pid_t) -> i32 {
    CHILD.store(child, Ordering::SeqCst);
    for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGQUIT, libc::SIGUSR1, libc::SIGUSR2] {
        // SAFETY: the handler calls only kill(2).
        unsafe { libc::signal(sig, forward as *const () as libc::sighandler_t) };
    }
    loop {
        let mut status = 0;
        // SAFETY: waitpid(2) on a local status word.
        let pid = unsafe { libc::waitpid(-1, &mut status, 0) };
        if pid == child {
            return if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else if libc::WIFSIGNALED(status) {
                128 + libc::WTERMSIG(status)
            } else {
                1
            };
        }
        if pid < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            return 1;
        }
    }
}

//! The errors of the start of a job, with the fix that podrunner knows,
//! and the socket file of the broker of a job.

use std::path::PathBuf;

/// A unit that did not start. podvm's advice to set `PODVM_STORE` does not
/// hold here: podrunner sets it for each unit, under `TMPDIR` (T-225).
pub(super) fn start_error(id: &podrunner_schema::UnitId, e: &str) -> String {
    match e.contains("PODVM_STORE") {
        true => format!(
            "start {id}: {e}; podrunner builds each unit under TMPDIR ({}): fix: set TMPDIR to a directory with room",
            std::env::temp_dir().display()
        ),
        false => format!("start {id}: {e}"),
    }
}

/// A file that goes when the job ends: the unix socket of its broker.
pub(super) struct Gone(pub(super) Option<PathBuf>);

impl Drop for Gone {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// A fake broker that did not start: the address and the fix (T-213).
pub fn broker_error(e: std::io::Error) -> String {
    format!("the fake broker: bind 127.0.0.1:0: {e}; fix: allow a bind of the loopback on this host")
}

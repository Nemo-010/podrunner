//! The errors of the start of a job, with the fix that podrunner knows,
//! and the socket file of the broker of a job.

use std::path::PathBuf;

/// A unit that did not start. podvm's advice to set `PODVM_STORE` does not
/// hold here: podrunner sets it for each unit, under `TMPDIR` (T-225).
pub(super) fn start_error(id: &podrunner_schema::UnitId, e: &str) -> String {
    // A reference into the record of podvm means nothing to a user (T-325).
    let e = &match (e.find(" (TODO/"), e.rfind(')')) {
        (Some(i), Some(j)) if j > i => format!("{}{}", &e[..i], &e[j + 1..]),
        _ => e.to_string(),
    };
    if e.contains("No space left on device") || e.contains("ENOSPC") {
        return format!(
            "start {id}: {e}; a podvm unit unpacks its image under TMPDIR ({}), about 2.5 GiB for the base image and 4 GiB for the larger one, for each job at a time: fix: free the disk, set TMPDIR, or give a lower --jobs (`podrunner doctor --run` gives the need)",
            std::env::temp_dir().display()
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// T-325: an ENOSPC names the disk of a unit and its fix; a reference
    /// into the record of podvm goes.
    #[test]
    fn a_full_disk_names_tmpdir_and_jobs() {
        let id = podrunner_schema::UnitId::parse("u-1").unwrap();
        let e = start_error(&id, "podvm run: exit 125: No space left on device (os error 28) (TODO/image.md T-1709)");
        assert!(e.contains("--jobs") && e.contains("TMPDIR") && !e.contains("TODO/"), "{e}");
    }
}

//! The artifact directory of a run (T-119, T-135, T-334, T-335). A new run
//! starts with no artifact, as on GitHub: the artifacts that an earlier run
//! left in `--artifacts DIR` or in `--state DIR` go first, by name. A rerun
//! keeps them and replaces what it uploads again. `--state` with
//! `--artifacts` notes the artifact directory in the state, so that a
//! rerun of the state reads it.

use std::path::{Path, PathBuf};

use super::Args;

/// The file of the state that names the artifact directory of its run.
const POINTER: &str = "artifacts-dir";

/// The artifact directory of the run. `state` is the directory of
/// `--state` or `--rerun`; `scratch` the scratch directory of the run.
pub(super) fn dir(a: &Args, state: Option<&Path>, scratch: &Path) -> PathBuf {
    let noted = || state.and_then(|d| std::fs::read_to_string(d.join(POINTER)).ok()).map(|p| PathBuf::from(p.trim()));
    let dir = match (&a.artifacts, state) {
        (Some(d), _) => d.clone(),
        (None, Some(s)) if a.rerun.is_some() => noted().unwrap_or_else(|| s.join("artifacts")),
        (None, Some(s)) => s.join("artifacts"),
        (None, None) => scratch.join("artifacts"),
    };
    if a.dry_run {
        return dir;
    }
    if let (Some(s), Some(d)) = (state, &a.artifacts) {
        let d = std::fs::canonicalize(d).unwrap_or_else(|_| d.clone());
        let _ = std::fs::create_dir_all(s).and_then(|()| std::fs::write(s.join(POINTER), d.display().to_string()));
    }
    if a.rerun.is_none() {
        let gone = clear(&dir);
        if !gone.is_empty() {
            eprintln!("podrunner: {}: the artifacts of an earlier run go: {}", dir.display(), gone.join(", "));
        }
    }
    dir
}

/// Remove each artifact of `dir` (a directory with `artifact.json`, or an
/// upload that did not end); give their names. Other files stay.
fn clear(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut gone: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            let p = e.path();
            p.join("artifact.json").is_file()
                || std::fs::read_dir(&p).is_ok_and(|mut r| {
                    r.any(|f| f.is_ok_and(|f| f.file_name().to_string_lossy().starts_with(".upload-")))
                })
        })
        .filter_map(|e| std::fs::remove_dir_all(e.path()).ok().map(|()| e.file_name().to_string_lossy().into_owned()))
        .collect();
    gone.sort();
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-334, T-335: a new run clears the artifacts of an earlier run; the
    /// state names the directory of `--artifacts`, and a rerun reads it.
    #[test]
    fn a_new_run_starts_with_no_artifact_and_a_rerun_finds_them() {
        let t = std::env::temp_dir().join(format!("podrunner-artifact-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let (arts, st) = (t.join("arts"), t.join("st"));
        std::fs::create_dir_all(arts.join("dup")).unwrap();
        std::fs::write(arts.join("dup/artifact.json"), "{}").unwrap();
        std::fs::write(arts.join("notes.txt"), "mine").unwrap();
        let run = Args { artifacts: Some(arts.clone()), state: Some(st.clone()), ..Args::default() };
        assert_eq!(dir(&run, Some(&st), &t), arts);
        assert!(!arts.join("dup").exists() && arts.join("notes.txt").exists(), "an artifact goes, a file stays");
        std::fs::create_dir_all(arts.join("art1")).unwrap();
        std::fs::write(arts.join("art1/artifact.json"), "{}").unwrap();
        let rerun = Args { rerun: Some(st.clone()), ..Args::default() };
        assert_eq!(dir(&rerun, Some(&st), &t), std::fs::canonicalize(&arts).unwrap());
        assert!(arts.join("art1").exists(), "a rerun keeps them");
        let _ = std::fs::remove_dir_all(&t);
    }
}

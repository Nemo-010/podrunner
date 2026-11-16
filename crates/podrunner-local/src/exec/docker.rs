//! The Docker actions of a job (T-270): an action of a repository whose
//! `action.yml` has `runs.using: docker`, also in a composite action. The
//! runner builds or pulls its image with the docker of the host, so on a
//! runner directory such a job must fail before it starts.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use pod_actions_workflow::yaml::{Kind, Limits, load};

/// The text of `action.yml` (or `action.yaml`) at `path` of the tarball of
/// `repo` at `commit` in the store `dir`; the tar has one top directory.
fn action_text(dir: &Path, repo: &str, commit: &str, path: &str) -> Option<String> {
    let file = std::fs::File::open(dir.join(pod_actions_store::archive_name(repo, commit))).ok()?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let want: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
    for entry in tar.entries().ok()? {
        let mut entry = entry.ok()?;
        let name = entry.path().ok()?.to_string_lossy().into_owned();
        let parts: Vec<&str> = name.split('/').filter(|c| !c.is_empty()).collect();
        let Some((file, dirs)) = parts.split_last() else { continue };
        if dirs.len() == want.len() + 1 && dirs[1..] == want[..] && (*file == "action.yml" || *file == "action.yaml") {
            let mut text = String::new();
            entry.read_to_string(&mut text).ok()?;
            return Some(text);
        }
    }
    None
}

/// Each Docker action that `uses` (`OWNER/REPO[/PATH]@REF`) is or holds,
/// to the depth of composite actions that GitHub allows (10).
fn docker_of(uses: &str, dir: &Path, pins: &BTreeMap<String, String>, depth: u8, out: &mut Vec<String>) {
    let Some((left, r)) = uses.rsplit_once('@') else { return };
    let mut parts = left.splitn(3, '/');
    let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else { return };
    let repo = format!("{owner}/{repo}");
    let Some(commit) = pins.get(&format!("{repo}@{r}")) else { return };
    let Some(text) = action_text(dir, &repo, commit, parts.next().unwrap_or("")) else { return };
    let Ok(root) = load(&text, Limits::default()) else { return };
    let Some(runs) = root.get("runs") else { return };
    match runs.get("using").map(|n| &n.kind) {
        Some(Kind::String(u)) if u.eq_ignore_ascii_case("docker") => out.push(uses.to_string()),
        Some(Kind::String(u)) if u == "composite" && depth < 10 => {
            let Some(Kind::Seq(steps)) = runs.get("steps").map(|n| &n.kind) else { return };
            for s in steps {
                if let Some(Kind::String(u)) = s.get("uses").map(|n| &n.kind) {
                    match u.strip_prefix("docker://") {
                        Some(_) => out.push(u.clone()),
                        None if !u.starts_with("./") => docker_of(u, dir, pins, depth + 1, out),
                        None => {}
                    }
                }
            }
        }
        _ => {}
    }
}

/// Each Docker action of the steps of the job `message`, with the store of
/// the actions `dir` and the commit of each `OWNER/REPO@REF` (`pins`).
pub fn docker_actions(message: &crate::Job, dir: &Path, pins: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    let steps = message.overrides.get("steps").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    for s in &steps {
        let r = &s["reference"];
        if r["type"] != "repository" || r["repositoryType"] != "GitHub" {
            continue;
        }
        let (Some(name), Some(git_ref)) = (r["name"].as_str(), r["ref"].as_str()) else { continue };
        let uses = match r["path"].as_str().filter(|p| !p.is_empty()) {
            Some(p) => format!("{name}/{p}@{git_ref}"),
            None => format!("{name}@{git_ref}"),
        };
        docker_of(&uses, dir, pins, 0, &mut out);
    }
    out.dedup();
    out
}

/// The refusal of a job with Docker actions on a runner directory, if any.
pub fn refusal(message: &crate::Job, dir: Option<&Path>, pins: &BTreeMap<String, String>) -> Option<String> {
    let found = docker_actions(message, dir?, pins);
    (!found.is_empty()).then(|| {
        format!(
            "the job uses the Docker action {}: run it in a unit (--podvm, or --microvm with --engine), not on a runner directory",
            found.join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tarball `OWNER_REPO/COMMIT.tar.gz` with the files `files` under one
    /// top directory.
    fn put(dir: &Path, repo: &str, commit: &str, files: &[(&str, &str)]) {
        let path = dir.join(pod_actions_store::archive_name(repo, commit));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let gz = flate2::write::GzEncoder::new(std::fs::File::create(&path).unwrap(), flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        for (name, text) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(text.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, format!("top/{name}"), text.as_bytes()).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn a_docker_action_and_one_in_a_composite_are_found() {
        let dir = std::env::temp_dir().join(format!("podrunner-docker-actions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        put(&dir, "o/d", "c1", &[("action.yml", "runs:\n  using: docker\n  image: Dockerfile\n")]);
        put(&dir, "o/n", "c2", &[("action.yml", "runs:\n  using: node20\n  main: x.js\n")]);
        put(&dir, "o/c", "c3", &[("sub/action.yaml", "runs:\n  using: composite\n  steps:\n    - uses: o/d@v1\n")]);
        let pins: BTreeMap<String, String> =
            [("o/d@v1", "c1"), ("o/n@v2", "c2"), ("o/c@v3", "c3")].map(|(k, v)| (k.into(), v.into())).into();
        let step = |name: &str, r: &str, path: Option<&str>| {
            let mut v = serde_json::json!({ "reference": { "type": "repository", "repositoryType": "GitHub", "name": name, "ref": r } });
            if let Some(p) = path {
                v["reference"]["path"] = serde_json::json!(p);
            }
            v
        };
        let mut job = crate::Job::scripts("a", &["true"]);
        job.overrides.insert("steps".into(), serde_json::json!([step("o/n", "v2", None)]));
        assert!(refusal(&job, Some(&dir), &pins).is_none(), "a node action passes");
        job.overrides.insert("steps".into(), serde_json::json!([step("o/c", "v3", Some("sub"))]));
        let e = refusal(&job, Some(&dir), &pins).expect("a composite with a Docker action");
        assert!(e.contains("o/d@v1") && e.contains("--podvm"), "{e}");
        job.overrides.insert("steps".into(), serde_json::json!([step("o/d", "v1", None)]));
        assert!(refusal(&job, Some(&dir), &pins).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

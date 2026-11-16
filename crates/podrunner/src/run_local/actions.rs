//! The actions and the images of `podrunner run` by tag (T-118, T-130): each
//! remote `uses:` of the workflow, and of each composite action that it
//! names, and each image that a tag names (for a unit kind), is pinned by
//! the lock file beside the workflow before the first job. Each tarball is
//! kept in the store (`--actions DIR`, by default the cache directory of the
//! user); each image is pulled into the store of the units.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use podrunner_local::images::{ImageSource, workflow_images};
use podrunner_local::localrepo::Local;
use podrunner_local::lock::{LOCK_FILE, Lock};
use podrunner_local::store::{Git, Mode, Store, add_trees, resolve, workflow_trees, workflow_uses};

use super::Args;

/// The forge of the actions, unless `--action-server URL` (T-139).
const FORGE: &str = "https://github.com/";

/// A directory of the cache of the user: `$XDG_CACHE_HOME/podrunner/NAME`,
/// or `$HOME/.cache/podrunner/NAME`.
pub(super) fn user_cache(name: &str) -> Result<PathBuf, String> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".cache")))
        .ok_or_else(|| format!("no XDG_CACHE_HOME and no HOME: give the directory with --{name} DIR"))?;
    Ok(cache.join("podrunner").join(name))
}

/// The actions and the images of a run.
pub(super) struct Pinned {
    /// The store of the tarballs, when the run has an action or a tree.
    pub dir: Option<PathBuf>,
    /// The commit of each `OWNER/REPO@REF`, and `NAME@sha256:HEX` of each
    /// `docker://NAME:TAG`.
    pub commits: BTreeMap<String, String>,
    /// A line for stderr when the lock changed.
    pub note: String,
}

/// The actions of the run, or `None` when the workflow names no action of a
/// repository.
pub(super) fn pin(
    a: &Args,
    text: &str,
    mode: Mode,
    self_repo: &str,
    images: Option<&dyn ImageSource>,
) -> Result<Option<Pinned>, String> {
    let (file, store) = (a.file.as_path(), a.actions.as_deref());
    // The errors of `pod-actions-store` as the text of a run.
    let s = |e: podrunner_local::store::Error| e.to_string();
    let workflow = pod_actions_workflow::parse(text).map_err(|e| e.to_string())?.workflow;
    let uses = workflow_uses(&workflow);
    let mut trees = workflow_trees(&workflow, self_repo).map_err(s)?;
    // The repository of each remote called workflow (T-131): its tree goes
    // into the store at the commit of the lock, as an action.
    for job in &workflow.jobs {
        if let Some(u) = job.uses.as_deref().filter(|u| !u.starts_with("./"))
            && let Some((left, r)) = u.rsplit_once('@')
        {
            let repo: Vec<&str> = left.splitn(3, '/').take(2).collect();
            let key = format!("{}@{r}", repo.join("/"));
            if !trees.contains(&key) {
                trees.push(key);
            }
        }
    }
    let tags = match images {
        Some(_) => workflow_images(&run_jobs(&workflow, &a.jobs)).map_err(s)?,
        None => Vec::new(),
    };
    if uses.is_empty() && trees.is_empty() && tags.is_empty() {
        return Ok(None);
    }
    let lock_path = file.parent().unwrap_or(Path::new(".")).join(LOCK_FILE);
    let lock = Lock::read(&lock_path).map_err(s)?;
    let name = lock_path.display().to_string();
    let (mut r, dir) = if uses.is_empty() && trees.is_empty() {
        (podrunner_local::store::Resolved { lock, ..Default::default() }, None)
    } else {
        let dir = match store {
            Some(d) => d.to_path_buf(),
            None => user_cache("actions")?,
        };
        // Any git server serves `git ls-remote` and a fetch: GitHub
        // Enterprise Server, Gitea, Forgejo (T-139).
        let base = a.action_server.as_deref().map_or(FORGE.to_string(), |s| format!("{}/", s.trim_end_matches('/')));
        // When `--local-repository` serves each reference of the file, no
        // forge is asked: the run opens no relay, and a reference that a
        // local action names comes from the lock and the store (T-211).
        let local = |u: &String| {
            u.rsplit_once('@').is_some_and(|(left, r)| {
                let repo: Vec<&str> = left.splitn(3, '/').take(2).collect();
                a.local_repos.iter().any(|(k, _)| *k == format!("{}@{r}", repo.join("/")))
            })
        };
        let mode = match uses.iter().chain(&trees).all(local) {
            true => Mode::Offline,
            false => mode,
        };
        // The fetch goes through one egress grant of the forge host only
        // (T-154); `--offline` fetches nothing and opens none.
        let fetch = match mode {
            Mode::Offline => None,
            _ => {
                let host = podrunner_local::fetch::host_of(&base).ok_or_else(|| format!("{base}: no host"))?;
                Some(podrunner_local::fetch::Fetch::open(&host)?)
            }
        };
        let mut git = Git::new(base, std::env::temp_dir());
        git.proxy = fetch.as_ref().map(|f| f.proxy.clone());
        let (store, forge) = (Store { dir: dir.clone() }, Local::new(a.local_repos.iter().cloned().collect(), &git));
        let r = resolve(&uses, &lock, &store, &forge, mode, &name).map_err(s)?;
        (add_trees(&trees, r, &store, &forge, mode, &name).map_err(s)?, Some(dir))
    };
    if let (Some(source), false) = (images, tags.is_empty()) {
        let (pins, changed) = podrunner_local::images::resolve(&tags, &mut r.lock, source, mode, &name).map_err(s)?;
        r.commits.extend(pins);
        r.changed |= changed;
    }
    let mut note = String::new();
    if r.changed {
        r.lock.write(&lock_path).map_err(s)?;
        note = format!("podrunner: {name} pins {} references\n", r.lock.pins.len() + r.lock.images.len());
    }
    Ok(Some(Pinned { dir, commits: r.commits, note }))
}

/// The workflow with the jobs whose images the run pulls (T-260): the jobs
/// of `--job` and each job that they need (each job with no `--job`), less
/// each job whose `container:` or service image is an expression, which is
/// named here and fails at its start, as its image is in no store.
fn run_jobs(w: &pod_actions_workflow::Workflow, selected: &[String]) -> pod_actions_workflow::Workflow {
    let mut keep: std::collections::BTreeSet<String> = match selected.is_empty() {
        true => w.jobs.iter().map(|j| j.id.clone()).collect(),
        false => selected.iter().cloned().collect(),
    };
    loop {
        let more: Vec<String> = w
            .jobs
            .iter()
            .filter(|j| keep.contains(&j.id))
            .flat_map(|j| j.needs.clone())
            .filter(|n| !keep.contains(n))
            .collect();
        if more.is_empty() {
            break;
        }
        keep.extend(more);
    }
    use pod_actions_workflow::yaml::{Kind, Node};
    // The image of a `container:` or of a service: the string, or `image:`.
    let image = |n: &Node| match &n.kind {
        Kind::String(s) => Some(s.clone()),
        Kind::Map(_) => n.get("image").and_then(|i| match &i.kind {
            Kind::String(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    };
    let mut out = w.clone();
    out.jobs.retain(|j| {
        let services = match j.services.as_ref().map(|s| &s.kind) {
            Some(Kind::Map(m)) => m.iter().filter_map(|(_, s)| image(s)).collect(),
            _ => Vec::new(),
        };
        let expr = j.container.as_ref().and_then(image).into_iter().chain(services).any(|i| i.contains("${{"));
        if keep.contains(&j.id) && expr {
            eprintln!("podrunner: job {}: an image of it is an expression, which no store holds: the job fails at its start (T-260)", j.id);
        }
        keep.contains(&j.id) && !expr
    });
    out
}

#[cfg(test)]
mod run_jobs_tests {
    /// T-260: a job with an image of an expression is left out of the pull,
    /// the others stay; an expression in `env:` of a service is no image.
    #[test]
    fn a_job_with_an_image_of_an_expression_is_left_out() {
        let text = "on: push\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps: [{run: 'true'}]\n  \
            db:\n    runs-on: ubuntu-latest\n    needs: lint\n    strategy: {matrix: {v: [1, 2]}}\n    \
            services:\n      pg: {image: 'postgres:${{ matrix.v }}'}\n    steps: [{run: 'true'}]\n  \
            other:\n    runs-on: ubuntu-latest\n    services:\n      pg:\n        image: postgres:16\n        \
            env: {P: '${{ secrets.P }}'}\n    steps: [{run: 'true'}]\n";
        let w = pod_actions_workflow::parse(text).unwrap().workflow;
        let ids = |w: &pod_actions_workflow::Workflow| w.jobs.iter().map(|j| j.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&super::run_jobs(&w, &[])), ["lint", "other"]);
        assert_eq!(ids(&super::run_jobs(&w, &["lint".to_string()])), ["lint"]);
        assert_eq!(ids(&super::run_jobs(&w, &["db".to_string()])), ["lint"], "db needs lint");
    }
}

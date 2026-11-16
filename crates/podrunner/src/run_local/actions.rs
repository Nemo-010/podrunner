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
        Some(_) => {
            let (w, more) = run_jobs(&workflow, &a.jobs);
            let mut t = workflow_images(&w).map_err(s)?;
            t.extend(more);
            t.sort();
            t.dedup();
            t
        }
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

/// The static values of each key of the matrix of a job: its vectors and
/// the values of `include`.
fn matrix_values(job: &pod_actions_workflow::Job) -> std::collections::BTreeMap<String, Vec<String>> {
    use pod_actions_workflow::yaml::Kind;
    let text = |n: &pod_actions_workflow::yaml::Node| match &n.kind {
        Kind::String(s) if !s.contains("${{") => Some(s.clone()),
        Kind::Number(x) => Some(x.to_string()),
        Kind::Bool(b) => Some(b.to_string()),
        _ => None,
    };
    let mut out: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let Some(Kind::Map(m)) = job.strategy.as_ref().and_then(|s| s.matrix.as_ref()).map(|m| &m.kind) else { return out };
    for (k, v) in m {
        match (&*k.name, &v.kind) {
            ("include", Kind::Seq(items)) => {
                for item in items {
                    if let Kind::Map(e) = &item.kind {
                        for (k, v) in e {
                            out.entry(k.name.clone()).or_default().extend(text(v));
                        }
                    }
                }
            }
            ("exclude", _) => {}
            (_, Kind::Seq(items)) => out.entry(k.name.clone()).or_default().extend(items.iter().filter_map(text)),
            _ => {}
        }
    }
    out
}

/// The workflow with the jobs whose images the run pulls (T-260): the jobs
/// of `--job` and each job that they need (each job with no `--job`). An
/// image that takes values of the matrix of its job gives one image for
/// each combination of those values (the second part), and leaves the
/// workflow; an image with another expression leaves it too, is named
/// here, and its job fails at its start, as no store holds it.
fn run_jobs(w: &pod_actions_workflow::Workflow, selected: &[String]) -> (pod_actions_workflow::Workflow, Vec<String>) {
    use pod_actions_workflow::yaml::{Kind, Node};
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
    let mut out = w.clone();
    out.jobs.retain(|j| keep.contains(&j.id));
    let mut more = Vec::new();
    for j in &mut out.jobs {
        let values = matrix_values(j);
        // The images of an image with values of the matrix, else `None`.
        let expand = |image: &str| -> Option<Vec<String>> {
            let keys: Vec<&String> = values.keys().filter(|k| image.contains(&format!("matrix.{k}"))).collect();
            let mut combos: Vec<std::collections::BTreeMap<&str, &str>> = vec![Default::default()];
            for k in keys {
                combos = combos
                    .iter()
                    .flat_map(|c| {
                        values[k].iter().map(move |v| {
                            let mut c = c.clone();
                            c.insert(k.as_str(), v.as_str());
                            c
                        })
                    })
                    .collect();
            }
            combos
                .iter()
                .map(|c| {
                    podrunner_local::message::image_with(image, |key| {
                        c.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.to_string())
                    })
                })
                .collect()
        };
        // Each image of a node: the node stays when it has no expression.
        let mut take = |n: &Node| -> bool {
            let image = match &n.kind {
                Kind::String(s) => Some(s.clone()),
                Kind::Map(_) => n.get("image").and_then(|i| match &i.kind {
                    Kind::String(s) => Some(s.clone()),
                    _ => None,
                }),
                _ => None,
            };
            let Some(image) = image.filter(|i| i.contains("${{")) else { return true };
            match expand(&image) {
                Some(list) => more.extend(list.iter().filter_map(|i| podrunner_local::images::by_tag(i))),
                None => eprintln!(
                    "podrunner: job {}: the image {image:?} is an expression that is not a value of its matrix, which no store holds: the job fails at its start (T-260)",
                    j.id
                ),
            }
            false
        };
        if j.container.as_ref().is_some_and(|c| !take(c)) {
            j.container = None;
        }
        if let Some(Node { kind: Kind::Map(m), .. }) = &mut j.services {
            m.retain(|(_, s)| take(s));
        }
    }
    (out, more)
}

#[cfg(test)]
mod run_jobs_tests {
    /// T-260: a job with an image of an expression is left out of the pull,
    /// the others stay; an expression in `env:` of a service is no image.
    #[test]
    fn a_job_with_an_image_of_an_expression_is_left_out() {
        let text = "on: push\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps: [{run: 'true'}]\n  \
            db:\n    runs-on: ubuntu-latest\n    needs: lint\n    strategy: {matrix: {v: [15, '16']}}\n    \
            services:\n      pg: {image: 'postgres:${{ matrix.v }}'}\n    steps: [{run: 'true'}]\n  \
            other:\n    runs-on: ubuntu-latest\n    services:\n      pg:\n        image: postgres:17\n        \
            env: {P: '${{ secrets.P }}'}\n      x: {image: 'x:${{ github.sha }}'}\n    steps: [{run: 'true'}]\n";
        let w = pod_actions_workflow::parse(text).unwrap().workflow;
        let ids = |w: &pod_actions_workflow::Workflow| w.jobs.iter().map(|j| j.id.clone()).collect::<Vec<_>>();
        let (all, more) = super::run_jobs(&w, &[]);
        assert_eq!(ids(&all), ["lint", "db", "other"]);
        assert_eq!(more, ["postgres:15", "postgres:16"], "one image for each value of the matrix");
        let tags = podrunner_local::images::workflow_images(&all).unwrap();
        assert_eq!(tags, ["postgres:17"], "the image of another expression leaves; an env expression is no image");
        assert_eq!(ids(&super::run_jobs(&w, &["lint".to_string()]).0), ["lint"]);
        assert_eq!(ids(&super::run_jobs(&w, &["db".to_string()]).0), ["lint", "db"], "db needs lint");
    }
}

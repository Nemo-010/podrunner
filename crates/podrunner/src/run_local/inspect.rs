//! `podrunner run --list`, `--graph`, `--validate` and `--dry-run` (T-128):
//! what a run would do, with no unit. A dry run builds the job message of
//! each cell, so each refusal (an action or an image with no pin, a
//! reusable workflow) and each missing pin shows before a job starts.

use std::time::Duration;

use pod_actions_workflow::{Graph, JobResult, Kind, Node, Workflow};
use podrunner_local::Job;
use podrunner_local::drive::{End, Executor};
use podrunner_schema::JobId;

/// The labels of `runs-on`, as text.
fn runs_on(n: Option<&Node>) -> String {
    match n.map(|n| &n.kind) {
        Some(Kind::String(s)) => s.clone(),
        Some(Kind::Seq(items)) => items
            .iter()
            .filter_map(|i| match &i.kind {
                Kind::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", "),
        Some(Kind::Map(_)) => "(a group)".into(),
        _ => "-".into(),
    }
}

/// One line for each job: its id, its name, its `runs-on` and its needs.
pub(super) fn list(w: &Workflow) -> String {
    let mut out = String::new();
    for j in &w.jobs {
        let name = j.name.as_deref().unwrap_or(&j.id);
        let needs = if j.needs.is_empty() { "-".to_string() } else { j.needs.join(", ") };
        let matrix = if j.strategy.as_ref().and_then(|s| s.matrix.as_ref()).is_some() { " (a matrix)" } else { "" };
        out.push_str(&format!("{}\t{name}{matrix}\truns-on: {}\tneeds: {needs}\n", j.id, runs_on(j.runs_on.as_ref())));
    }
    out
}

/// The stages of `needs`: a job is one stage after its last need.
pub(super) fn graph(w: &Workflow) -> Result<String, String> {
    let g = Graph::new(w).map_err(|e| e.to_string())?;
    let mut stage: std::collections::BTreeMap<String, usize> = Default::default();
    let mut stages: Vec<Vec<String>> = Vec::new();
    for job in g.order() {
        let s = g.needs(job).iter().map(|n| stage[*n] + 1).max().unwrap_or(0);
        stage.insert(job.to_string(), s);
        if stages.len() <= s {
            stages.resize(s + 1, Vec::new());
        }
        stages[s].push(job.to_string());
    }
    let mut out = String::new();
    for (i, jobs) in stages.iter().enumerate() {
        out.push_str(&format!("stage {}: {}\n", i + 1, jobs.join(", ")));
    }
    for (need, job) in g.edges() {
        out.push_str(&format!("{need} -> {job}\n"));
    }
    Ok(out)
}

/// `--validate`, `--list` and `--graph` of one file. `--validate` checks the
/// needs as a plan reads them: a need that is not a job, or a cycle (T-209).
pub(super) fn answer(a: &super::args::Args, text: &str) -> super::Out {
    let fail = |e: String| super::fail(super::Class::Config, format!("{}: {e}", a.file.display()));
    let p = match pod_actions_workflow::parse(text) {
        Ok(p) => p,
        Err(e) => return fail(e.to_string()),
    };
    let mut stdout = String::new();
    if a.validate {
        if let Err(e) = Graph::new(&p.workflow) {
            return fail(e.to_string());
        }
        for w in &p.warnings {
            stdout.push_str(&format!("warning: {w}\n"));
        }
        stdout.push_str(&format!("valid: {}\n", a.file.display()));
    }
    if a.list {
        stdout.push_str(&list(&p.workflow));
    }
    if a.graph {
        match graph(&p.workflow) {
            Ok(g) => stdout.push_str(&g),
            Err(e) => return fail(e),
        }
    }
    super::Out { code: 0, stdout, stderr: String::new() }
}

/// The executor of a dry run: each job message is built, and no unit runs.
/// On a runner directory only, a job with containers fails as its run
/// would (T-218).
pub(super) struct DryRun {
    pub host: bool,
}

impl Executor for DryRun {
    fn run(&mut self, _job: &JobId, message: Job, _limit: Duration) -> Result<End, String> {
        if self.host && podrunner_local::exec::needs_containers(&message) {
            return Err("the job needs containers: run it in a unit (--podvm, or --microvm with --engine), not on a runner directory".into());
        }
        Ok(End { result: JobResult::Success, outputs: vec![] })
    }
}

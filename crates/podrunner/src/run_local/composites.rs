//! The `uses:` of each local composite action (`uses: ./PATH`) of a
//! workflow, and of each local action that one names (T-346): the lock pins
//! them before the first job, as it pins a direct `uses:`.

use std::collections::BTreeSet;
use std::path::Path;

use pod_actions_workflow::{Kind, Limits, Node};

/// The deepest chain of local actions that the run follows, as GitHub
/// follows composite actions to 10 levels.
const DEPTH: usize = 10;

/// Each remote `uses:` of the local actions of the steps of `w`, read from
/// the tree `base` (the checkout). An `action.yml` that is not there is
/// left out: its job fails at its start, as on GitHub.
pub(super) fn local_uses(w: &pod_actions_workflow::Workflow, base: &Path) -> Result<Vec<String>, String> {
    let mut queue: Vec<(String, usize)> = w
        .jobs
        .iter()
        .flat_map(|j| j.steps.iter().filter_map(|s| s.uses.clone()))
        .filter(|u| u.starts_with("./"))
        .map(|u| (u, 1))
        .collect();
    let (mut seen, mut out) = (BTreeSet::new(), Vec::new());
    while let Some((path, depth)) = queue.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        if depth > DEPTH {
            return Err(format!("`uses: {path}`: the local actions nest deeper than {DEPTH} levels"));
        }
        let Some(text) =
            ["action.yml", "action.yaml"].iter().find_map(|f| std::fs::read_to_string(base.join(&path).join(f)).ok())
        else {
            continue;
        };
        for u in steps_uses(&text).map_err(|e| format!("the action.yml of `uses: {path}`: {e}"))? {
            match u.starts_with("./") {
                true => queue.push((u, depth + 1)),
                // The local mode gives the checkout with its own stand-in.
                false if !u.starts_with("docker://") && !u.starts_with("actions/checkout@") && !out.contains(&u) => {
                    out.push(u)
                }
                false => {}
            }
        }
    }
    Ok(out)
}

/// Each `uses:` of the steps of a composite `action.yml`.
fn steps_uses(text: &str) -> Result<Vec<String>, String> {
    let root = pod_actions_workflow::yaml::load(text, Limits::default()).map_err(|e| e.to_string())?;
    let Some(runs) = root.get("runs") else { return Ok(Vec::new()) };
    if !matches!(runs.get("using").map(|n| &n.kind), Some(Kind::String(u)) if u == "composite") {
        return Ok(Vec::new());
    }
    let Some(Kind::Seq(steps)) = runs.get("steps").map(|n| &n.kind) else { return Ok(Vec::new()) };
    Ok(steps
        .iter()
        .filter_map(|s: &Node| match s.get("uses").map(|n| &n.kind) {
            Some(Kind::String(u)) => Some(u.clone()),
            _ => None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    /// T-346: a remote `uses:` of a local composite action, also of a
    /// nested local one, is pinned; a loop of local actions ends.
    #[test]
    fn the_uses_of_local_composite_actions() {
        let dir = std::env::temp_dir().join(format!("podrunner-composites-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let comp = |p: &str, steps: &str| {
            std::fs::create_dir_all(dir.join(p)).unwrap();
            std::fs::write(dir.join(p).join("action.yml"), format!("runs:\n  using: composite\n  steps:\n{steps}"))
                .unwrap();
        };
        comp(
            ".github/actions/a",
            "    - uses: actions/setup-node@v4\n    - uses: actions/checkout@v4\n    - uses: ./.github/actions/b\n",
        );
        comp(
            ".github/actions/b",
            "    - uses: owner/x/sub@v1\n    - uses: ./.github/actions/a\n    - uses: docker://alpine:3\n",
        );
        let w = pod_actions_workflow::parse(
            "on: push\njobs:\n  j:\n    runs-on: x\n    steps:\n      - uses: ./.github/actions/a\n      - uses: ./missing\n",
        )
        .unwrap()
        .workflow;
        let mut got = super::local_uses(&w, &dir).unwrap();
        got.sort();
        assert_eq!(got, ["actions/setup-node@v4", "owner/x/sub@v1"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

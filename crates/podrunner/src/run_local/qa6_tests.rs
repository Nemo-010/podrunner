//! The checks of a user QA of `v0.1.0-beta.7` (GitHub #203 to #212).

use std::path::PathBuf;

use super::*;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// A new directory for one test.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("podrunner-qa6-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const WORKFLOW: &str = "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps: [{run: a}]\n";

/// T-340: a workflow file whose link leads out of its project is refused,
/// in file mode and in directory mode; a link in the project is taken.
#[test]
fn a_workflow_link_out_of_the_project_is_refused() {
    let dir = scratch("link");
    let flows = dir.join("p/.github/workflows");
    std::fs::create_dir_all(&flows).unwrap();
    std::fs::create_dir_all(dir.join("other")).unwrap();
    std::fs::write(dir.join("other/w.yml"), WORKFLOW).unwrap();
    std::fs::write(flows.join("in.yml"), WORKFLOW).unwrap();
    std::os::unix::fs::symlink(flows.join("in.yml"), flows.join("ok.yml")).unwrap();
    let ok = flows.join("ok.yml").display().to_string();
    let out = run(&args(&[&ok, "--runner-dir", "/r", "--validate"]));
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    std::os::unix::fs::symlink(dir.join("other/w.yml"), flows.join("out.yml")).unwrap();
    let out_file = flows.join("out.yml").display().to_string();
    let flows_dir = flows.display().to_string();
    for a in [
        vec![out_file.as_str(), "--validate"],
        vec![out_file.as_str(), "--dry-run"],
        vec![flows_dir.as_str(), "--list"],
    ] {
        let mut a = a.clone();
        a.extend(["--runner-dir", "/r"]);
        let out = run(&args(&a));
        assert_ne!(out.code, 0, "{a:?}: {}", out.stdout);
        assert!(out.stderr.contains("the symbolic link leads out of the project"), "{a:?}: {}", out.stderr);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-341: `.secrets` of the tree gets each check of `--secret-file` in a
/// dry run: the mode, a bad line, a directory and a link out of the tree.
#[test]
fn the_secrets_of_the_tree_are_checked_in_a_dry_run() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("secrets");
    let tree = dir.join("tree");
    std::fs::create_dir_all(&tree).unwrap();
    std::fs::write(dir.join("w.yml"), WORKFLOW).unwrap();
    std::fs::write(dir.join("outside"), "A=1\n").unwrap();
    let w = dir.join("w.yml").display().to_string();
    let t = tree.display().to_string();
    let secrets = tree.join(".secrets");
    let dry = || run(&args(&[&w, "--runner-dir", "/r", "--dry-run", "--no-cache", "--checkout", &t]));
    let set = |text: &str, mode: u32| {
        std::fs::write(&secrets, text).unwrap();
        std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    set("A=1\n", 0o600);
    let out = dry();
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    for (why, make) in [
        ("needs mode 0600", Box::new(|| set("A=1\n", 0o644)) as Box<dyn Fn()>),
        ("not NAME=VALUE", Box::new(|| set("BADLINE\n", 0o600))),
        (
            "not a file",
            Box::new(|| {
                std::fs::remove_file(&secrets).unwrap();
                std::fs::create_dir(&secrets).unwrap()
            }),
        ),
        (
            "leads out of the tree",
            Box::new(|| {
                std::fs::remove_dir(&secrets).unwrap();
                std::os::unix::fs::symlink(dir.join("outside"), &secrets).unwrap()
            }),
        ),
    ] {
        make();
        let out = dry();
        assert_eq!(out.code, 78, "{why}: {}{}", out.stdout, out.stderr);
        assert!(out.stderr.contains(".secrets") && out.stderr.contains(why), "{why}: {}", out.stderr);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-346: with `--offline`, a dry run fails on a reference that a local
/// composite action names and the lock does not pin, and names the fix.
#[test]
fn an_offline_dry_run_pins_the_uses_of_a_local_composite() {
    let dir = scratch("composite");
    let tree = dir.join("tree");
    let action = tree.join(".github/actions/nested");
    std::fs::create_dir_all(&action).unwrap();
    std::fs::write(
        action.join("action.yml"),
        "runs:\n  using: composite\n  steps:\n    - uses: actions/setup-node@v4\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("w.yml"),
        "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps: [{uses: ./.github/actions/nested}]\n",
    )
    .unwrap();
    let (w, t, store) =
        (dir.join("w.yml").display().to_string(), tree.display().to_string(), dir.join("store").display().to_string());
    let out = run(&args(&[
        &w,
        "--runner-dir",
        "/r",
        "--dry-run",
        "--no-cache",
        "--offline",
        "--checkout",
        &t,
        "--actions",
        &store,
    ]));
    assert_eq!(out.code, 78, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stderr.contains("actions/setup-node@v4") && out.stderr.contains("run with no --offline"),
        "{}",
        out.stderr
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-348: `--validate --json` writes an object for a file that is not valid.
#[test]
fn validate_json_names_a_file_that_is_not_valid() {
    let dir = scratch("validate");
    std::fs::write(dir.join("bad.yml"), "on: push\njobs: [\n").unwrap();
    let w = dir.join("bad.yml").display().to_string();
    let out = run(&args(&[&w, "--validate", "--json"]));
    assert_eq!(out.code, 78, "{}", out.stderr);
    let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
    assert_eq!(v["valid"], false);
    assert_eq!(v["file"], w.as_str());
    assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()), "{v}");
    assert!(out.stderr.starts_with("podrunner: "), "{}", out.stderr);
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-347: a loop with a view that runs once is refused, and `--schedule` of
/// a workflow with no `on.schedule` says so.
#[test]
fn a_loop_refuses_a_view_and_a_workflow_with_no_schedule() {
    let dir = scratch("loops");
    std::fs::write(dir.join("w.yml"), WORKFLOW).unwrap();
    let w = dir.join("w.yml").display().to_string();
    for (lp, view) in
        [("--watch", "--list"), ("--schedule", "--dry-run"), ("--watch", "--validate"), ("--schedule", "--graph")]
    {
        let out = repeat(&args(&[&w, "--runner-dir", "/r", lp, view]));
        assert_eq!(out.code, 64, "{lp} {view}: {}", out.stderr);
        assert!(out.stderr.contains(&format!("{lp} and {view} exclude each other")), "{}", out.stderr);
    }
    let out = repeat(&args(&[&w, "--runner-dir", "/r", "--schedule"]));
    assert_eq!(out.code, 78, "{}", out.stderr);
    assert!(out.stderr.contains("the workflow has no on.schedule trigger"), "{}", out.stderr);
    let _ = std::fs::remove_dir_all(&dir);
}

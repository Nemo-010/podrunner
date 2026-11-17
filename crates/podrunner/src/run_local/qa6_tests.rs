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

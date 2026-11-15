//! The checks of the flags of `run` that a user agent found open
//! (GitHub #155 to #162).

use super::*;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// T-206: `--platform` does not turn off the other checks.
#[test]
fn a_platform_keeps_each_check_of_the_flags() {
    let host = ["--platform", "ubuntu-latest=host"];
    for (bad, why) in [
        (vec!["w.yml", "--jobs", "2", "--runner-dir", "/r"], "--jobs N needs a unit kind"),
        (vec!["w.yml", "--toolcache", "/t", "--runner-dir", "/r"], "--toolcache needs --podvm"),
        (vec!["w.yml", "--allow", "example.com", "--runner-dir", "/r"], "--allow needs --podnet"),
        (vec!["w.yml", "--environment-allow", "e=example.com", "--runner-dir", "/r"], "--environment-allow needs"),
        (vec!["w.yml", "--engine", "/e", "--runner-dir", "/r"], "--engine needs --microvm"),
    ] {
        let mut with = bad.clone();
        with.extend(host);
        for a in [bad, with] {
            let e = parse(&args(&a)).unwrap_err();
            assert!(e.contains(why), "{a:?}: {e}");
        }
    }
}

/// T-207: the refusal names the flag that clashes or is missing.
#[test]
fn the_refusal_of_an_executor_names_its_flag() {
    let pin = format!("r@sha256:{}", "a".repeat(64));
    for (bad, why) in [
        (vec!["w.yml", "--runner-dir", "/r", "--podvm", "/p", "--images", "/s", "--image", &pin], "two executors"),
        (vec!["w.yml", "--runner-dir", "/r", "--image", &pin], "--images and --image need a unit kind"),
        (vec!["w.yml", "--runner-dir", "/r", "--images", "/s"], "--images and --image need a unit kind"),
        (vec!["w.yml", "--incus", "/p", "--image", &pin], "--incus needs --images STORE"),
        (vec!["w.yml", "--podvm", "/p", "--images", "/s"], "--podvm needs --image"),
        (vec!["w.yml"], "is necessary"),
    ] {
        let e = parse(&args(&bad)).unwrap_err();
        assert!(e.contains(why), "{bad:?}: {e}");
    }
    // A short option that run does not have is not a file (T-208).
    assert!(parse(&args(&["w.yml", "-x", "--runner-dir", "/r"])).unwrap_err().contains("an unknown option \"-x\""));
}

fn workflow(tag: &str, text: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("podrunner-flags-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("w.yml");
    std::fs::write(&file, text).unwrap();
    let w = file.display().to_string();
    (dir, w)
}

/// T-209: `--validate` refuses the needs that a plan refuses.
#[test]
fn validate_refuses_a_dangling_need_and_a_cycle() {
    for (tag, needs, why) in [("dangling", "nope", "not a job"), ("cycle", "b", "cycle")] {
        let text = format!(
            "on: push\njobs:\n  a:\n    needs: {needs}\n    runs-on: x\n    steps: [{{run: a}}]\n  b:\n    needs: a\n    runs-on: x\n    steps: [{{run: b}}]\n"
        );
        let (dir, w) = workflow(tag, &text);
        let out = run(&args(&[&w, "--validate"]));
        assert_eq!(out.code, 78, "{tag}: {}{}", out.stdout, out.stderr);
        assert!(out.stderr.contains(why), "{tag}: {}", out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// T-210: `--break` names a job and a step of the workflow.
#[test]
fn a_break_names_a_job_and_a_step_of_the_file() {
    let (dir, w) =
        workflow("break", "on: push\njobs:\n  build:\n    runs-on: x\n    steps: [{run: a}, {id: two, run: b}]\n");
    for (b, ok) in [
        ("nosuchjob:1", false),
        ("build:99", false),
        ("build:nosuchstep", false),
        ("build:2", true),
        ("build:two", true),
    ] {
        let out = run(&args(&[&w, "--runner-dir", "/nonexistent", "--dry-run", "--no-cache", "--break", b]));
        assert_eq!(out.code == 64, !ok, "{b}: {}{}", out.stdout, out.stderr);
        if !ok {
            assert!(out.stderr.contains(&format!("--break {b}")), "{b}: {}", out.stderr);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-212: a missing podvm binary needs a path, not a version.
#[test]
fn doctor_names_a_missing_podvm_as_a_path() {
    let (dir, w) = workflow("doctor", "on: push\njobs:\n  a:\n    runs-on: x\n    steps: [{run: a}]\n");
    for flag in ["--podvm", "--incus"] {
        let out = doctor(&args(&[&w, flag, "/nonexistent/podvm", "--images", "/s", "--image", "r@sha256:00"]));
        let line = out.stdout.lines().find(|l| l.contains(" podvm: ")).unwrap_or_default().to_string();
        assert!(line.contains(&format!("{flag} /nonexistent/podvm: not a file")), "{flag}: {}", out.stdout);
        assert!(!line.contains("take podvm"), "{flag}: {line}");
    }
    // T-213: doctor tries the bind of the loopback.
    let out = doctor(&args(&[&w, "--runner-dir", "/nonexistent"]));
    assert!(out.stdout.contains(" loopback: "), "{}", out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-211: when `--local-repository` serves each reference, no forge is
/// asked and no relay opens, also with no `--offline`. A forge with no
/// host cannot open a relay: the run must not try.
#[test]
fn a_local_repository_opens_no_relay() {
    let (dir, w) = workflow("relay", "on: push\njobs:\n  a:\n    runs-on: x\n    steps: [{uses: me/tool@main}]\n");
    std::fs::create_dir_all(dir.join("tool")).unwrap();
    std::fs::write(dir.join("tool/action.yml"), "runs:\n  using: composite\n  steps: [{run: hi, shell: sh}]\n")
        .unwrap();
    let local = format!("me/tool@main={}", dir.join("tool").display());
    let store = dir.join("store").display().to_string();
    let base = [w.as_str(), "--dry-run", "--no-cache", "--actions", &store, "--action-server", "http://"];
    let out = run(&args(&[&base[..], &["--local-repository", &local]].concat()));
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("planned a\n"), "{}", out.stdout);
    // The control: with no local repository the forge is asked.
    assert_ne!(run(&args(&base)).code, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

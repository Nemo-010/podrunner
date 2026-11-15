//! T-180: `serve` as PID 1 of a PID namespace forks a reaper; the reaper
//! exits with the code of `serve`, and the text of `serve` is the same. A
//! host with no user namespace for the user skips the test.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_podrunner");

fn unshare() -> Command {
    let mut c = Command::new("unshare");
    c.args(["-Urpf", "--mount-proc"]);
    c
}

#[test]
fn serve_as_pid_1_exits_with_the_code_of_serve() {
    if !unshare().arg("true").status().is_ok_and(|s| s.success()) {
        eprintln!("skip: no user and PID namespace here");
        return;
    }
    let d = std::env::temp_dir().join(format!("podrunner-pid1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let args = |c: &mut Command| {
        c.args(["serve", "--config"]).arg(d.join("missing.toml")).arg("--state").arg(d.join("state"));
    };
    let mut plain = Command::new(BIN);
    args(&mut plain);
    let plain = plain.output().unwrap();
    let mut as_pid1 = unshare();
    as_pid1.arg(BIN);
    args(&mut as_pid1);
    let as_pid1 = as_pid1.output().unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(plain.status.code(), Some(0));
    assert_eq!(as_pid1.status.code(), plain.status.code(), "{}", String::from_utf8_lossy(&as_pid1.stderr));
    assert_eq!(as_pid1.stderr, plain.stderr);
}

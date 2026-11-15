//! The limits of an offer on a podvm unit (T-162, T-198): a payload that
//! allocates past `memory` and one that forks past `pids` get an error,
//! and the same payload with no limit passes (the control). Ignored by
//! default. Run:
//!
//!   PODRUNNER_PODVM=PATH PODRUNNER_PODVM_IMAGES=STORE \
//!   PODRUNNER_PODVM_IMAGE=NAME@sha256:HEX \
//!   cargo test -p podrunner-podvm --test limits -- --ignored --nocapture
//!
//! The image needs sh, dd and sleep.

use std::path::{Path, PathBuf};
use std::time::Duration;

use podrunner_podvm::{Podvm, PodvmUnit};
use podrunner_schema::UnitId;
use podrunner_unit::limits::Limits;
use podrunner_unit::{Exit, StartSpec, UnitDriver};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// A small buffer passes (exit 10 if not); a buffer of 1 GiB fails (exit
/// 11 if not); then exit 0.
const MEMORY: &str = "dd if=/dev/zero of=/dev/null bs=1M count=1 2>/dev/null || exit 10; \
    if dd if=/dev/zero of=/dev/null bs=1024M count=1 2>/dev/null; then exit 11; fi; exit 0";
/// 1000 processes at one time: exit 12 when each fork passed.
const FORKS: &str = "i=0; while [ $i -lt 1000 ]; do sleep 30 & i=$((i+1)); done; exit 12";

/// The exit of `script` in a fresh unit with `limits`.
fn run(n: u32, script: &str, limits: Limits) -> Option<Exit> {
    let state = std::env::temp_dir().join(format!("podrunner-limits-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&state);
    let unit = PodvmUnit::new(
        Podvm { binary: PathBuf::from(env("PODRUNNER_PODVM")) },
        &state,
        Path::new(&env("PODRUNNER_PODVM_IMAGES")),
    )
    .unwrap();
    let platform = if std::env::consts::ARCH == "aarch64" { "linux/arm64" } else { "linux/amd64" };
    let image = unit.prepare(&env("PODRUNNER_PODVM_IMAGE"), platform, false).unwrap();
    let id = UnitId::parse(&format!("u-{n}")).unwrap();
    let spec = StartSpec {
        unit: id.clone(),
        image,
        argv: ["sh", "-c", script].map(String::from).to_vec(),
        env: Default::default(),
        credential: Default::default(),
        sockets: None,
        mounts: Vec::new(),
        limits,
    };
    unit.start(&spec).unwrap();
    let exit = unit.wait(&id, Duration::from_secs(120)).unwrap();
    let log = String::from_utf8_lossy(&unit.unit_logs(&id).unwrap_or_default()).to_string();
    let residue = unit.destroy(&id).unwrap();
    let _ = std::fs::remove_dir_all(&state);
    println!("unit {n} {limits:?}: {exit:?}\n{log}");
    assert!(residue.is_clean(), "{residue:?}");
    exit
}

#[test]
#[ignore]
fn a_payload_past_the_memory_limit_gets_an_error() {
    assert_eq!(run(1, MEMORY, Limits::default()), Some(Exit::Code(11)), "the control: 1 GiB passes with no limit");
    let held = Limits { memory_bytes: Some(256 << 20), ..Limits::default() };
    assert_eq!(run(2, MEMORY, held), Some(Exit::Code(0)), "1 GiB must fail under 256 MiB");
}

#[test]
#[ignore]
fn a_payload_past_the_pids_limit_gets_an_error() {
    assert_eq!(run(3, FORKS, Limits::default()), Some(Exit::Code(12)), "the control: 1000 forks pass with no limit");
    let held = Limits { pids: Some(256), ..Limits::default() };
    let exit = run(4, FORKS, held);
    assert!(matches!(exit, Some(Exit::Code(c)) if c != 0 && c != 12), "1000 forks must fail under pids 256: {exit:?}");
}

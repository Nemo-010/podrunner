//! T-177: `prepare` of an image that a digest pins and that the store of
//! the images does not have: podvm pulls it once by its digest, then saves
//! it. A failed pull names both causes.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use podrunner_podvm::{Podvm, PodvmUnit};
use podrunner_unit::UnitDriver;

const IMAGE: &str = "busybox@sha256:a7dbc319b1438115a207e11930dc4bb3e643dcb3ec0bdd694bb36cd8fad289ec";

/// A fake podvm: `save` fails until a `pull` ran; with `pull_fails`, the
/// pull fails too. Each call goes to `calls`.
fn fake(tag: &str, pull_fails: bool) -> (Podvm, PathBuf) {
    let d = std::env::temp_dir().join(format!("podrunner-podvm-pull-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let pull = if pull_fails { "echo 'pull: denied' >&2; exit 1" } else { "touch \"$d/pulled\"" };
    let script = format!(
        "#!/bin/sh\nd='{}'\necho \"$1\" >> \"$d/calls\"\ncase \"$1\" in\n probe) echo '{{\"podvm\":\"0.2.0-beta.19\",\"rung\":\"userland\"}}' ;;\n save) [ -f \"$d/pulled\" ] || {{ echo 'save: no such image' >&2; exit 1; }}; echo tar > \"$3\" ;;\n pull) {pull} ;;\nesac\n",
        d.display()
    );
    let f = d.join("podvm");
    std::fs::write(&f, script).unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    (Podvm { binary: f }, d)
}

fn calls(d: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(d.join("calls")).unwrap_or_default().lines().map(String::from).collect()
}

fn platform() -> &'static str {
    if std::env::consts::ARCH == "aarch64" { "linux/arm64" } else { "linux/amd64" }
}

#[test]
fn a_missing_pinned_image_is_pulled_once_then_saved() {
    let (podvm, d) = fake("ok", false);
    let unit = PodvmUnit::new(podvm, &d.join("state"), &d.join("images")).unwrap();
    unit.prepare(IMAGE, platform(), false).unwrap();
    let c = calls(&d);
    assert_eq!(c.iter().filter(|x| *x == "pull").count(), 1, "{c:?}");
    assert_eq!(c.iter().filter(|x| *x == "save").count(), 2, "{c:?}");
    // The second prepare takes the saved tar: no call.
    let before = calls(&d).len();
    unit.prepare(IMAGE, platform(), false).unwrap();
    assert_eq!(calls(&d).len(), before);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_failed_pull_names_both_causes() {
    let (podvm, d) = fake("fails", true);
    let unit = PodvmUnit::new(podvm, &d.join("state"), &d.join("images")).unwrap();
    let e = unit.prepare(IMAGE, platform(), false).unwrap_err();
    assert!(e.detail.contains("no such image") && e.detail.contains("denied"), "{}", e.detail);
    let _ = std::fs::remove_dir_all(&d);
}

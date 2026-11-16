//! `podrunner doctor --run FILE [the options of run]` (T-153): the checks of
//! the inputs of a local run before it starts. Each check that fails names
//! its fix; nothing changes.

use std::path::Path;

use podrunner_podvm::Podvm;
use podrunner_podvm::probe::{LOWEST, LOWEST_ENGINE, LOWEST_GUEST, LOWEST_INCUS};

use super::Args;

/// One check: its name, whether it holds, and the detail or the fix.
pub(super) struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub text: String,
}

fn check(name: &'static str, r: Result<String, String>) -> Check {
    match r {
        Ok(text) => Check { name, ok: true, text },
        Err(text) => Check { name, ok: false, text },
    }
}

/// The limit of file size: .NET maps its code through a large memfd, so a
/// small limit ends the runner by SIGXFSZ (T-144).
fn file_size_limit() -> Result<String, String> {
    let mut r = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit writes into the struct that this function owns.
    if unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut r) } != 0 {
        return Err("getrlimit failed".into());
    }
    if r.rlim_cur == libc::RLIM_INFINITY || r.rlim_cur >= 4 << 30 {
        return Ok("no small limit".into());
    }
    Err(format!("{} bytes: the runner dies by SIGXFSZ; fix: ulimit -f unlimited", r.rlim_cur))
}

/// The free bytes of the file system of `dir`.
fn free_disk(dir: &Path) -> Result<String, String> {
    let c = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: statvfs writes into a zeroed struct that this function owns.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(format!("{}: statvfs failed", dir.display()));
    }
    let free = s.f_bavail as u64 * s.f_frsize as u64;
    // Below 1 GiB, in MiB: "0 GiB" reads as no space at all (T-218).
    let size = if free >> 30 == 0 { format!("{} MiB", free >> 20) } else { format!("{} GiB", free >> 30) };
    if free >> 30 < 2 {
        return Err(format!(
            "{}: {size} free; a run needs about 2 GiB; fix: free the disk or set TMPDIR",
            dir.display()
        ));
    }
    Ok(format!("{}: {size} free", dir.display()))
}

/// `podvm inspect IMAGE` in `store`: the image is there.
pub(super) fn image_in_store(podvm: &Path, store: &Path, image: &str) -> Result<String, String> {
    let out = std::process::Command::new(podvm)
        .args(["inspect", image])
        .env("PODVM_STORE", store)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", podvm.display()))?;
    match out.status.success() {
        true => Ok(format!("{image} is in {}", store.display())),
        false => Err(format!(
            "{} does not hold {image}; fix: load the runner image (PODVM_STORE={} podvm load -i runner-ARCH.tar) or name another --image",
            store.display(),
            store.display()
        )),
    }
}

/// The egress hold of the units of the run (T-226).
pub(super) fn hold(a: &Args) -> String {
    match (&a.podvm, &a.guest, &a.podnet, &a.runner_dir) {
        (Some(_), _, Some(_), _) => "podnet, the seccomp hold (advisory): only the hosts of --allow".into(),
        (Some(_), _, None, _) => "none: the unit has the network of the host (give --podnet for a hold)".into(),
        (None, Some(_), _, _) => "the guest has no NIC: only the broker and the hosts of --allow (enforced)".into(),
        _ => "none: the runner directory has the network of the host".into(),
    }
}

/// The runner reads each directory above its root, up to `/` (T-213).
fn parents_readable(dir: &Path) -> Result<String, String> {
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for p in dir.ancestors().skip(1) {
        std::fs::read_dir(p).map_err(|e| {
            format!(
                "{}: {e}; fix: the runner reads each directory above its root: give it a read of {}, or put the runner directory where each parent can be read",
                p.display(),
                p.display()
            )
        })?;
    }
    Ok(format!("each directory above {} can be read", dir.display()))
}

/// The checks of a run with the options `a`.
/// What a job on `--runner-dir` takes of the proxy and the CA of the user
/// (T-265).
pub(super) fn proxy_line(vars: impl Iterator<Item = (String, String)>) -> String {
    let names: Vec<String> = vars
        .filter(|(k, v)| !v.is_empty() && k != "PATH" && podrunner_local::exec::PASSED.contains(&k.as_str()))
        .map(|(k, _)| k)
        .collect();
    match names.is_empty() {
        true => "a job takes no proxy and no CA file of the environment (direct egress)".into(),
        false => format!("a job takes {} of the environment", names.join(", ")),
    }
}

pub(super) fn checks(a: &Args) -> Vec<Check> {
    let mut out =
        vec![check("file-size-limit", file_size_limit()), check("scratch-disk", free_disk(&std::env::temp_dir()))];
    out.push(check(
        "workflow",
        std::fs::read_to_string(&a.file)
            .map_err(|e| format!("{}: {e}", a.file.display()))
            .and_then(|t| pod_actions_workflow::parse(&t).map(|_| "it parses".to_string()).map_err(|e| e.to_string())),
    ));
    // Each job needs a fake broker on the loopback (T-213).
    out.push(check(
        "loopback",
        std::net::TcpListener::bind("127.0.0.1:0")
            .map(|_| "a bind of 127.0.0.1:0 holds".to_string())
            .map_err(|e| format!("bind 127.0.0.1:0: {e}; fix: allow a bind of the loopback on this host")),
    ));
    let lock = a.file.parent().unwrap_or(Path::new(".")).join(podrunner_local::lock::LOCK_FILE);
    out.push(check(
        "lock",
        if lock.exists() {
            podrunner_local::lock::Lock::read(&lock)
                .map(|l| format!("{}: {} references", lock.display(), l.pins.len() + l.images.len()))
                .map_err(|e| e.to_string())
        } else {
            Ok(format!("{}: none yet; a run with no --frozen writes it", lock.display()))
        },
    ));
    if let Some(dir) = &a.runner_dir {
        out.push(check("proxy", Ok(proxy_line(std::env::vars()))));
        out.push(check("runner-dir-parents", parents_readable(dir)));
        let ok = dir.join("run.sh").is_file() && dir.join("bin/Runner.Listener").is_file();
        out.push(check(
            "runner-dir",
            if let (true, Some(e)) = (ok, podrunner_local::exec::space_refusal(dir)) {
                Err(e)
            } else if ok {
                Ok(dir.display().to_string())
            } else {
                Err(format!("{}: no unpacked runner; fix: unpack a release of actions/runner there", dir.display()))
            },
        ));
    }
    let (binary, lowest, flag) = match (&a.podvm, &a.guest) {
        (Some(b), _) => (Some(b.clone()), LOWEST, "--podvm"),
        (None, Some((true, b))) => {
            (Some(b.clone()), if a.engine.is_some() { LOWEST_ENGINE } else { LOWEST_GUEST }, "--microvm")
        }
        (None, Some((false, b))) => (Some(b.clone()), LOWEST_INCUS, "--incus"),
        _ => (None, LOWEST, ""),
    };
    if let Some(b) = binary {
        let podvm = Podvm { binary: b.clone() };
        // Only a podvm that answers with a low version needs another
        // version; a wrong path needs another path (T-212).
        let fix = |e: podrunner_unit::UnitError| match (b.is_file(), e.code) {
            (false, _) => format!("{flag} {}: not a file; fix: give the path of a podvm binary", b.display()),
            (true, podrunner_unit::UnitCode::Unsupported) => format!("{e}; fix: take podvm {lowest} or later"),
            (true, _) => format!(
                "{flag} {}: {e}; fix: give the path of a podvm binary (`podvm probe --json` must answer)",
                b.display()
            ),
        };
        out.push(check(
            "podvm",
            podvm.require(lowest).map(|v| format!("{} is {v} ({lowest} or later)", b.display())).map_err(fix),
        ));
        if let Some(store) = &a.images {
            out.push(check(
                "store",
                if store.is_dir() {
                    Ok(store.display().to_string())
                } else {
                    Err(format!("{}: not a directory; fix: give the podvm store with --images", store.display()))
                },
            ));
            // The store holds the image of `--image` (T-223).
            if let (Some(image), true) = (&a.image, store.is_dir()) {
                out.push(check("image", image_in_store(&b, store, image)));
            }
        }
    }
    if matches!(a.guest, Some((true, _))) {
        out.push(check(
            "kvm",
            if Path::new("/dev/kvm").exists() {
                Ok("/dev/kvm".into())
            } else {
                Err("no /dev/kvm: the microvm kind needs KVM; fix: run on a host with KVM".into())
            },
        ));
        if let Some(e) = &a.engine {
            out.push(check(
                "engine",
                if e.join("dockerd").is_file() {
                    Ok(e.display().to_string())
                } else {
                    Err(format!("{}: no dockerd; fix: unpack the static Docker binaries there", e.display()))
                },
            ));
        }
    }
    if let Some(p) = &a.podnet {
        out.push(check(
            "podnet",
            if p.is_file() { Ok(p.display().to_string()) } else { Err(format!("{}: not a file", p.display())) },
        ));
    }
    out.push(check("hold", Ok(hold(a))));
    out
}

/// The text of the checks: one line for each.
pub(super) fn text(checks: &[Check]) -> String {
    checks.iter().map(|c| format!("{} {}: {}\n", if c.ok { "ok  " } else { "FAIL" }, c.name, c.text)).collect()
}

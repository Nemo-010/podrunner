//! The executor of `--runner-dir` (T-089): the runner of an unpacked
//! release on the host, one job at a time, against a fake broker.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pod_actions_workflow::JobResult;
use podrunner_broker_fake::artifacts::ArtifactStore;
use podrunner_broker_fake::cache::CacheStore;
use podrunner_broker_fake::runner::start_runner;
use podrunner_broker_fake::{Broker, Job};
use podrunner_schema::JobId;

use super::{Live, needs_containers, pump, record};
use crate::drive::{End, Executor, result_of};

/// An unpacked release of the official runner, in a directory that the run
/// may change.
pub struct RunnerDir {
    pub dir: PathBuf,
    /// The log of each job, for the caller.
    pub logs: Vec<(JobId, String)>,
    /// The archives of the actions (`OWNER_REPO/SHA.tar.gz`, T-091).
    pub actions: Option<PathBuf>,
    /// The commit of each `OWNER/REPO@REF` of the run (T-118).
    pub pins: BTreeMap<String, String>,
    /// The artifact store of the run (T-119).
    pub artifacts: Option<Arc<ArtifactStore>>,
    /// The cache store (T-120).
    pub cache: Option<Arc<CacheStore>>,
    /// The issuer of the id-tokens of the run (T-140).
    pub oidc: Option<Arc<podrunner_broker_fake::oidc::Issuer>>,
    /// The live sink and the blobs of each job (T-134).
    pub live: Option<Live>,
    pub blobs: Vec<(JobId, Vec<(String, String)>)>,
    /// The tarball of the local tree for `actions/checkout` (T-092).
    pub checkout: Option<PathBuf>,
    /// `--export DIR`: the workspaces of each job go to `DIR/JOB` (T-138).
    pub export: Option<PathBuf>,
    /// The stop flag of the next job (T-132).
    stop: Option<crate::cancel::Stop>,
    /// The lock of the runner directory for this run (T-274).
    _lock: std::fs::File,
}

/// The lock file of a run in a runner directory.
const LOCK: &str = ".podrunner-run.lock";

/// Take the runner directory `dir` for this run: a second run on it fails
/// at once, as two runners in one directory break each other (T-274).
fn lock(dir: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::fd::AsRawFd;
    let path = dir.join(LOCK);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // SAFETY: flock of a descriptor that `f` owns.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(format!(
            "{}: another run of podrunner uses this runner directory; wait for it to end, or give each run its own --runner-dir",
            dir.display()
        ));
    }
    Ok(f)
}

impl RunnerDir {
    pub fn new(dir: PathBuf) -> Result<RunnerDir, String> {
        if !dir.join("run.sh").is_file() || !dir.join("bin/Runner.Listener").is_file() {
            return Err(format!("{}: no unpacked runner (run.sh, bin/Runner.Listener)", dir.display()));
        }
        let lock = lock(&dir)?;
        Ok(RunnerDir {
            _lock: lock,
            dir,
            logs: Vec::new(),
            actions: None,
            pins: BTreeMap::new(),
            artifacts: None,
            cache: None,
            oidc: None,
            live: None,
            blobs: Vec::new(),
            checkout: None,
            export: None,
            stop: None,
        })
    }
}

impl Executor for RunnerDir {
    fn take_logs(&mut self) -> Vec<(JobId, String)> {
        std::mem::take(&mut self.logs)
    }

    fn set_live(&mut self, live: Live) {
        self.live = Some(live);
    }

    fn set_stop(&mut self, stop: crate::cancel::Stop) {
        self.stop = Some(stop);
    }

    fn take_blobs(&mut self) -> Vec<(JobId, Vec<(String, String)>)> {
        std::mem::take(&mut self.blobs)
    }

    fn run(&mut self, job: &JobId, message: Job, limit: Duration) -> Result<End, String> {
        let end = self.job(job, message, limit);
        // The export of the workspaces, also of a failed job (T-138).
        if let Some(dir) = &self.export
            && let Err(e) = crate::work::export(&self.dir.join("_work"), &dir.join(job.as_str()))
        {
            match self.logs.iter_mut().rev().find(|(j, _)| j == job) {
                Some((_, log)) => log.push_str(&format!("\npodrunner: {e}")),
                None => self.logs.push((job.clone(), format!("podrunner: {e}"))),
            }
        }
        end
    }
}

impl RunnerDir {
    fn job(&mut self, job: &JobId, message: Job, limit: Duration) -> Result<End, String> {
        if needs_containers(&message) {
            return Err("the job needs containers: run it in a unit (--podvm, or --microvm with --engine), not on a runner directory".into());
        }
        if let Some(e) = super::docker::refusal(&message, self.actions.as_deref(), &self.pins) {
            return Err(e);
        }
        let broker = Broker::start().map_err(super::broker_error)?;
        if let Some(dir) = &self.actions {
            broker.serve_actions(dir, self.pins.clone());
        }
        if let Some(store) = &self.artifacts {
            broker.serve_artifacts(store.clone());
        }
        if let Some(store) = &self.cache {
            broker.serve_cache(store.clone());
        }
        if let Some(issuer) = &self.oidc {
            broker.serve_oidc(issuer.clone());
        }
        if let Some(tree) = &self.checkout {
            broker.serve_checkout(tree);
        }
        broker.queue(message);
        // Each job starts in a fresh work directory (T-146).
        crate::work::clear(&self.dir.join("_work"))?;
        // The pages of a runner that a signal ended make the next runner
        // fail ("The file ... already exists", T-273).
        crate::work::clear(&self.dir.join("_diag").join("pages"))?;
        // A scratch home for each job: a step must not change the home of
        // the user (T-266); the next job clears it with `_work`.
        let home = self.dir.join("_work").join("_home");
        std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
        let mut child = start_runner(&self.dir, &broker, &host_env(&home, std::env::vars()))?;
        // The limit of the job (`timeout-minutes`, T-115): at its end, the
        // whole process group of the runner ends, as the unit of `InUnit`.
        let end = Instant::now() + limit;
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| format!("the runner: {}", e.kind()))? {
                break Some(s);
            }
            // A signal ends the job as its limit does (T-141).
            if Instant::now() >= end || crate::cancel::stopped(&self.stop) {
                break None;
            }
            pump(&self.live, job, &broker);
            std::thread::sleep(Duration::from_millis(100));
        };
        let Some(status) = status else {
            end_group(child.id());
            let _ = child.wait();
            {
                let log = record(&self.live, &mut self.blobs, job, &broker);
                self.logs.push((job.clone(), log));
            }
            if crate::cancel::stopped(&self.stop) {
                return Ok(End { result: JobResult::Cancelled, outputs: vec![] });
            }
            return Err(format!("the job ran past its limit of {} s", limit.as_secs()));
        };
        if let Some(why) = signal_reason(&status) {
            {
                let log = record(&self.live, &mut self.blobs, job, &broker);
                self.logs.push((job.clone(), log));
            }
            return Err(why);
        }
        // The runner sends `completejob` before it ends; give the broker
        // time to record it.
        let t = Instant::now();
        while broker.completions().is_empty() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        {
            let log = record(&self.live, &mut self.blobs, job, &broker);
            self.logs.push((job.clone(), log));
        }
        let done = broker.completions();
        let Some(end) = done.first() else {
            return Err("the runner ended with no result".into());
        };
        Ok(End { result: result_of(&end.conclusion), outputs: end.outputs.clone() })
    }
}

/// The variables of the user that a job on a runner directory takes
/// (T-265): `PATH`, the CA files and the proxy. The broker is on the
/// loopback, so `NO_PROXY` always holds the loopback.
pub const PASSED: &[&str] = &[
    "PATH",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
];

/// The environment of the runner of one job: the variables of `PASSED`
/// from `vars`, `HOME` and `NO_PROXY`.
pub(crate) fn host_env(
    home: &std::path::Path,
    vars: impl Iterator<Item = (String, String)>,
) -> Vec<(&'static str, String)> {
    let vars: std::collections::BTreeMap<String, String> = vars.collect();
    let mut out: Vec<(&'static str, String)> =
        PASSED.iter().filter_map(|k| vars.get(*k).filter(|v| !v.is_empty()).map(|v| (*k, v.clone()))).collect();
    out.push(("HOME", home.display().to_string()));
    let mut no_proxy = vec!["127.0.0.1".to_string(), "localhost".to_string(), "::1".to_string()];
    for k in ["NO_PROXY", "no_proxy"] {
        no_proxy.extend(
            vars.get(k)
                .into_iter()
                .flat_map(|v| v.split(','))
                .map(str::trim)
                // The runner aborts at an entry `::` (a null host in its parser).
                .filter(|v| !v.is_empty() && !v.chars().all(|c| c == ':'))
                .map(String::from),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    no_proxy.retain(|v| seen.insert(v.clone()));
    let no_proxy = no_proxy.join(",");
    out.push(("NO_PROXY", no_proxy.clone()));
    out.push(("no_proxy", no_proxy));
    out
}

/// End the process group of `leader`: TERM, then KILL after 5 s.
fn end_group(leader: u32) {
    let Ok(pgid) = i32::try_from(leader) else { return };
    // SAFETY: kill of a process group that this process made.
    unsafe { libc::kill(-pgid, libc::SIGTERM) };
    let t = Instant::now();
    // SAFETY: a signal 0 checks that the group still has a process.
    while t.elapsed() < Duration::from_secs(5) && unsafe { libc::kill(-pgid, 0) } == 0 {
        std::thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: as above.
    unsafe { libc::kill(-pgid, libc::SIGKILL) };
}

/// The reason of a runner that a signal ended, with a word on SIGXFSZ (a
/// limit of file size, T-144).
fn signal_reason(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    match status.signal()? {
        libc::SIGXFSZ => Some("the runner ended by SIGXFSZ: a limit of file size (ulimit -f) is too small".into()),
        s => Some(format!("the runner ended by signal {s}")),
    }
}

#[cfg(test)]
mod env_tests {
    use super::host_env;

    /// The job takes `PATH`, the CA and the proxy of the user, a scratch
    /// home, and the loopback stays off the proxy (T-265, T-266).
    #[test]
    fn a_job_on_a_runner_directory_takes_the_path_the_ca_and_the_proxy() {
        let vars = [
            ("PATH", "/opt/x/bin:/usr/local/bin:/usr/bin:/bin"),
            ("HTTPS_PROXY", "http://proxy.example:3128"),
            ("SSL_CERT_FILE", "/etc/ca.pem"),
            ("NO_PROXY", "intra.example,::,10.0.0.0/8,localhost"),
            ("AWS_SECRET_ACCESS_KEY", "made-up"),
            ("HOME", "/home/user"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()));
        let env = host_env(std::path::Path::new("/r/_work/_home"), vars.into_iter());
        let get = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("PATH"), Some("/opt/x/bin:/usr/local/bin:/usr/bin:/bin"));
        assert_eq!(get("HTTPS_PROXY"), Some("http://proxy.example:3128"));
        assert_eq!(get("SSL_CERT_FILE"), Some("/etc/ca.pem"));
        assert_eq!(get("HOME"), Some("/r/_work/_home"));
        assert_eq!(get("NO_PROXY"), Some("127.0.0.1,localhost,::1,intra.example,10.0.0.0/8"));
        assert_eq!(get("AWS_SECRET_ACCESS_KEY"), None, "only the listed variables pass");
    }
}

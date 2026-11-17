//! The container hooks of a unit of the local mode (T-093).

use std::path::PathBuf;

use podrunner_broker_fake::Job;

use super::InUnit;
use super::images::registries;

/// The container hooks of one unit (T-093): the relay and the control
/// socket in its socket directory, served by a thread until [`Hooks::stop`].
pub(super) struct Hooks {
    dir: PathBuf,
    pub(super) env: std::collections::BTreeMap<String, String>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    unit: std::sync::Arc<std::sync::Mutex<podrunner_hook::unit::HookUnit>>,
    thread: std::thread::JoinHandle<()>,
}

impl Hooks {
    /// End the server; the refusals of the hook calls.
    pub(super) fn stop(self) -> Vec<String> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        // The server checks its flag at each connection: wake it.
        let _ = std::os::unix::net::UnixStream::connect(self.dir.join("ctl.sock"));
        let _ = self.thread.join();
        self.unit
            .lock()
            .map(|u| {
                let codes = u.refusals.iter().map(|r| format!("{r:?}"));
                // The cause of each refusal of the driver, for the log of the run.
                codes.chain(u.driver_errors.iter().map(|e| format!("the driver: {e}"))).collect()
            })
            .unwrap_or_default()
    }
}

impl InUnit {
    pub(super) fn hooks(
        &self,
        dir: &std::path::Path,
        message: &Job,
        id: &podrunner_schema::UnitId,
    ) -> Result<Hooks, String> {
        use podrunner_hook::policy::Policy;
        use podrunner_hook::server::{Unit, bind, serve_until};
        use std::os::unix::fs::PermissionsExt;
        let Some(guest) = self.driver.socket_mount().map(str::to_string) else {
            return Err("the job needs containers, and this unit kind has no container hooks here; run it with --microvm and --engine".into());
        };
        let dir = dir.to_path_buf();
        std::fs::create_dir_all(dir.join("logs")).map_err(|e| format!("{}: {e}", dir.display()))?;
        let relay = dir.join("podrunner-hook.sh");
        std::fs::write(&relay, podrunner_hook::RELAY).map_err(|e| format!("the relay: {e}"))?;
        std::fs::set_permissions(&relay, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("the relay: {e}"))?;
        let policy = Policy {
            container: podrunner_schema::ContainerRoute::Hooks,
            shared_host_net: true,
            registries: registries(message),
            work_root: "/home/runner".into(),
        };
        let unit = std::sync::Arc::new(std::sync::Mutex::new(podrunner_hook::unit::HookUnit::new(
            self.driver.clone(),
            id.clone(),
            policy,
            None,
            dir.join("logs"),
        )));
        let listener = bind(&dir.join("ctl.sock")).map_err(|e| format!("the control socket: {e}"))?;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (u, s): (std::sync::Arc<std::sync::Mutex<dyn Unit>>, _) = (unit.clone(), stop.clone());
        let thread = std::thread::spawn(move || serve_until(listener, u, s));
        let env = std::collections::BTreeMap::from([
            ("ACTIONS_RUNNER_CONTAINER_HOOKS".to_string(), format!("{guest}/podrunner-hook.sh")),
            ("PODRUNNER_CTL".to_string(), format!("{guest}/ctl.sock")),
        ]);
        Ok(Hooks { dir, env, stop, unit, thread })
    }
}

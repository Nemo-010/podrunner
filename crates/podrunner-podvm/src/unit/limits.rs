//! The limits of a podvm unit (G-12, T-162): rlimits of the payload, which
//! each child keeps. podvm holds no CPU limit with no cgroup.

use podrunner_unit::{UnitCode, UnitError};

use super::PodvmUnit;

impl PodvmUnit {
    /// The limits of memory and processes of each unit (`[unit.podvm] mem`,
    /// `pids`; T-162), as rlimits of the payload that each child keeps.
    /// podvm holds no CPU limit with no cgroup.
    pub fn with_limits(mut self, limits: podrunner_unit::limits::Limits) -> Result<PodvmUnit, UnitError> {
        if limits.cpus.is_some() {
            return Err(UnitError::new(UnitCode::UnitStartFailed, "podvm holds no CPU limit with no cgroup"));
        }
        limits.record(&mut self.capability);
        self.limits = limits;
        Ok(self)
    }
}

/// The rlimits of `limits` for `podvm run -d` (T-162). RLIMIT_AS bounds the
/// address space, and .NET reserves its heap at start: the GC gets a hard
/// limit of three quarters of it. RLIMIT_NPROC counts each process of the
/// user, also of another unit of the same user.
pub(super) fn ulimit_args(
    limits: &podrunner_unit::limits::Limits,
    env: &mut std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(m) = limits.memory_bytes {
        a.extend(["--ulimit".into(), format!("as={m}")]);
        // The GC of .NET reserves its regions (256 GiB by default) at start,
        // and fails under RLIMIT_AS (runner-image run 43690754546): its range
        // is the heap limit too.
        let heap = format!("{:x}", m / 4 * 3);
        env.insert("DOTNET_GCHeapHardLimit".into(), heap.clone());
        env.insert("DOTNET_GCRegionRange".into(), heap);
    }
    if let Some(n) = limits.pids {
        a.extend(["--ulimit".into(), format!("nproc={n}")]);
    }
    a
}

#[cfg(test)]
mod tests {
    use podrunner_unit::limits::Limits;

    #[test]
    fn the_limits_become_rlimits_of_the_payload() {
        let mut env = std::collections::BTreeMap::new();
        let l = Limits { memory_bytes: Some(8 << 30), cpus: None, pids: Some(512) };
        let a = super::ulimit_args(&l, &mut env);
        assert_eq!(a, ["--ulimit", "as=8589934592", "--ulimit", "nproc=512"]);
        assert_eq!(env["DOTNET_GCHeapHardLimit"], format!("{:x}", 6u64 << 30));
        assert_eq!(env["DOTNET_GCRegionRange"], format!("{:x}", 6u64 << 30));
        let mut none = std::collections::BTreeMap::new();
        assert!(super::ulimit_args(&Limits::default(), &mut none).is_empty() && none.is_empty());
    }
}

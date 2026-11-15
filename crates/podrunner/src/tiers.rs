//! The check of `check` and `serve` that a kind can serve a source (T-185).

use pod_core::failure::Class;
use podrunner_schema::Boundary;
use podrunner_slots::config::Config;

use crate::serve::{Failure, fail, fake_capability};

/// A file whose sources no kind can serve is refused, as `serve` says
/// `no-tier-served`. Each kind counts at the best boundary that it can
/// claim, so a self-test of `serve` can only lower it.
pub fn served(cfg: &Config) -> Result<(), Failure> {
    let best = |name: &str, u: &podrunner_slots::config::UnitConfig| match name {
        "fake" => fake_capability().boundary,
        "podvm" if u.confine => Boundary::Confined,
        "podvm" => Boundary::Redirect,
        "incus" => Boundary::Confined,
        _ => Boundary::Guest,
    };
    let arch = std::env::consts::ARCH;
    let served = cfg.source.iter().any(|s| {
        let floor = cfg.floor(s.tier);
        cfg.unit.iter().any(|(n, u)| {
            let kind_arch = if n == "fake" { fake_capability().arch } else { arch.to_string() };
            kind_arch == s.arch && best(n, u) >= floor
        })
    });
    if !cfg.source.is_empty() && !served {
        return Err(fail(Class::Config, "no-tier-served: no unit kind meets the floor of a source on this host"));
    }
    Ok(())
}

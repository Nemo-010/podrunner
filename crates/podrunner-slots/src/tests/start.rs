//! T-176: a unit that does not start after the mint: the source gets
//! `release` of the job (the sidecar removes the runner), and the audit
//! names the cause of the driver.

use podrunner_schema::{Boundary, EgressHold, Principal, RefusalCode};
use serde_json::json;

use super::*;

#[test]
fn a_failed_start_releases_the_job_and_names_the_cause() {
    let cap = capability(Boundary::Confined, Principal::Distinct, EgressHold::Advisory);
    let mut bed = Bed::new(
        config(""),
        vec![("a", "trusted", None, vec![offer(0, "j-1", json!({}))])],
        vec![KindSpec {
            name: "fake",
            slots: 1,
            capability: cap,
            command: vec!["/nonexistent/podrunner-payload".into()],
        }],
    );
    bed.until(50, |s| s.jobs().all(|j| j.state.is_final()) && s.jobs().count() == 1);
    let r = bed.slots.reports.iter().find(|r| r.job_id.as_str() == "j-1").expect("a report");
    assert_eq!(r.refusal_code, Some(RefusalCode::UnitStartFailed), "{r:?}");
    let audit = std::fs::read_to_string(bed.slots.state_dir.join("audit.jsonl")).unwrap();
    assert!(audit.contains("start: "), "the audit names no cause: {audit}");
}

//! T-175: the file of the placed job for the job-started hook of the
//! runner image.

use serde_json::{Value, json};

use crate::run::{JOB_FILE, job_file};

fn offer_of(with: Value) -> podrunner_schema::JobOffer {
    let step: Value = serde_json::from_str(&super::offer(0, "j-1", with)).unwrap();
    serde_json::from_value(step["offer"].clone()).unwrap()
}

#[test]
fn the_job_file_names_the_repository_and_the_run() {
    let f = job_file(&offer_of(json!({}))).expect("a job file");
    assert_eq!(f.name, JOB_FILE);
    assert!(f.name_is_plain());
    assert_eq!(f.content.expose(), "example-org/app 1\n");
}

#[test]
fn an_offer_with_no_run_gives_no_job_file() {
    assert!(job_file(&offer_of(json!({ "trust_inputs": { "run_id": 0 } }))).is_none());
    assert!(job_file(&offer_of(json!({ "trust_inputs": { "repo": "" } }))).is_none());
}

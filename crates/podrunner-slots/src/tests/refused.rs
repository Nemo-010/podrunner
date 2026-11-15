//! T-189: a source that offers a refused job again gets `withdraw` each
//! time, with the code of the refusal, so it can move on.

use std::sync::{Arc, Mutex};

use podrunner_schema::{Boundary, EgressHold, JobId, Principal};
use podrunner_source::{Event, Hello, JobSource, Offer, SourceError};
use serde_json::{Map, Value, json};

use super::{Bed, KindSpec, capability, config, offer, sh};

/// A source that gives its one offer on each `next`, and keeps each
/// `withdraw`.
struct Again {
    offer: Offer,
    withdrawn: Arc<Mutex<Vec<String>>>,
}

impl JobSource for Again {
    fn hello(&mut self, _: &str, _: &str) -> Result<Hello, SourceError> {
        Ok(Hello { source: "a".into(), operator_controlled: true })
    }
    fn next(&mut self, _: u32, _: std::time::Duration) -> Result<Vec<Event>, SourceError> {
        Ok(vec![Event::Offer { offer: self.offer.clone() }])
    }
    fn ack(&mut self, _: &JobId) -> Result<(), SourceError> {
        Ok(())
    }
    fn claim(&mut self, _: &JobId, _: &str) -> Result<(), SourceError> {
        Ok(())
    }
    fn mint(&mut self, j: &JobId, r: &str) -> Result<podrunner_schema::Credential, SourceError> {
        Ok(podrunner_fake::canned_credential(j, r))
    }
    fn report(&mut self, _: &JobId, _: &Map<String, Value>) -> Result<(), SourceError> {
        Ok(())
    }
    fn audit(&mut self, _: &Map<String, Value>) -> Result<(), SourceError> {
        Ok(())
    }
    fn withdraw(&mut self, j: &JobId, r: &str) -> Result<(), SourceError> {
        self.withdrawn.lock().unwrap().push(format!("{j} {r}"));
        Ok(())
    }
    fn release(&mut self, _: &JobId) -> Result<(), SourceError> {
        Ok(())
    }
}

#[test]
fn a_refused_job_offered_again_is_withdrawn_again() {
    let cap = capability(Boundary::Confined, Principal::Distinct, EgressHold::Advisory);
    let kind = KindSpec { name: "fake", slots: 1, capability: cap, command: sh("exit 0") };
    let mut bed = Bed::new(config(""), vec![("a", "trusted", None, vec![])], vec![kind]);
    // No kind holds a memory limit: `limit-not-enforceable`.
    let need = json!({ "limits": { "memory": 1 }, "limits_required": ["memory"] });
    let step: Value = serde_json::from_str(&offer(0, "j-1", need)).unwrap();
    let offer: Offer = serde_json::from_value(step["offer"].clone()).unwrap();
    let withdrawn: Arc<Mutex<Vec<String>>> = Arc::default();
    bed.slots.sources[0].source = Box::new(Again { offer, withdrawn: withdrawn.clone() });
    for _ in 0..4 {
        bed.tick();
    }
    let w = withdrawn.lock().unwrap().clone();
    assert!(w.len() >= 3, "{w:?}");
    assert!(w.iter().all(|x| x == "j-1 limit-not-enforceable"), "{w:?}");
}

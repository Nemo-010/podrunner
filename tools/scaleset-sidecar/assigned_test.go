package main

import (
	"testing"

	"github.com/actions/scaleset"
)

func assigned(jobID string) *scaleset.JobAssigned {
	return &scaleset.JobAssigned{JobMessageBase: scaleset.JobMessageBase{JobID: jobID, OwnerName: "o", RepositoryName: "r", WorkflowRunID: 5}}
}

func offers(rep map[string]any) []string {
	var ids []string
	for _, e := range rep["events"].([]any) {
		if m := e.(map[string]any); m["event"] == "offer" {
			ids = append(ids, m["offer"].(map[string]any)["job_id"].(string))
		}
	}
	return ids
}

// T-166: github.com assigns a job to the scale set with no JobAvailable
// and runnerRequestId 0. The sidecar offers it as a-JOBID, its claim
// needs no AcquireJobs, and its started and completed events carry the
// same id.
func TestAnAssignedJobIsOfferedAndClaimedWithNoAcquire(t *testing.T) {
	st := newStub()
	s := stubSidecar(t, st)
	waitPoll(t, st)
	st.put <- &scaleset.RunnerScaleSetMessage{MessageID: 1, JobAssignedMessages: []*scaleset.JobAssigned{assigned("6f1c2d3e-0000-4000-8000-000000000001")}}
	rep := s.handle([]byte(`{"schema":1,"op":"next","capacity":1,"wait_s":5}`))
	id := "a-6f1c2d3e-0000-4000-8000-000000000001"
	if got := offers(rep); len(got) != 1 || got[0] != id {
		t.Fatalf("%v", rep)
	}
	// s.session is nil: an AcquireJobs call would panic.
	if r := s.handle([]byte(`{"schema":1,"op":"claim","job_id":"` + id + `","runner_name":"r1"}`)); r["reply"] != "done" {
		t.Fatalf("claim: %v", r)
	}
	if s.jobs[id].state != claimed {
		t.Fatal("not claimed")
	}
	waitPoll(t, st)
	base := scaleset.JobMessageBase{JobID: "6f1c2d3e-0000-4000-8000-000000000001"}
	st.put <- &scaleset.RunnerScaleSetMessage{
		MessageID:            2,
		JobStartedMessages:   []*scaleset.JobStarted{{RunnerName: "r1", JobMessageBase: base}},
		JobCompletedMessages: []*scaleset.JobCompleted{{Result: "succeeded", JobMessageBase: base}},
	}
	rep = s.handle([]byte(`{"schema":1,"op":"next","capacity":0,"wait_s":5}`))
	ev := rep["events"].([]any)
	if len(ev) != 2 || ev[0].(map[string]any)["job_id"] != id || ev[1].(map[string]any)["job_id"] != id {
		t.Fatalf("%v", rep)
	}
	if len(s.byJobID) != 0 {
		t.Fatalf("a completed job is kept: %v", s.byJobID)
	}
}

// The JobAssigned of a job that came as JobAvailable (the classic flow)
// is not a second offer, and a later message of the job keeps its id.
func TestTheAssignedMessageOfAnAvailableJobIsNoSecondOffer(t *testing.T) {
	st := newStub()
	s := stubSidecar(t, st)
	waitPoll(t, st)
	base := scaleset.JobMessageBase{RunnerRequestID: 41, JobID: "g-41", OwnerName: "o", RepositoryName: "r"}
	st.put <- &scaleset.RunnerScaleSetMessage{MessageID: 1, JobAvailableMessages: []*scaleset.JobAvailable{{JobMessageBase: base}}}
	if got := offers(s.handle([]byte(`{"schema":1,"op":"next","capacity":1,"wait_s":5}`))); len(got) != 1 || got[0] != "41" {
		t.Fatalf("%v", got)
	}
	waitPoll(t, st)
	noRequest := scaleset.JobMessageBase{JobID: "g-41"}
	st.put <- &scaleset.RunnerScaleSetMessage{
		MessageID:           2,
		JobAssignedMessages: []*scaleset.JobAssigned{{JobMessageBase: noRequest}},
		JobStartedMessages:  []*scaleset.JobStarted{{RunnerName: "r", JobMessageBase: noRequest}},
	}
	rep := s.handle([]byte(`{"schema":1,"op":"next","capacity":1,"wait_s":5}`))
	ev := rep["events"].([]any)
	if len(ev) != 1 || ev[0].(map[string]any)["event"] != "started" || ev[0].(map[string]any)["job_id"] != "41" {
		t.Fatalf("%v", rep)
	}
}

// A withdrawn assigned job is forgotten with its jobId.
func TestAWithdrawnAssignedJobIsForgotten(t *testing.T) {
	st := newStub()
	s := stubSidecar(t, st)
	waitPoll(t, st)
	st.put <- &scaleset.RunnerScaleSetMessage{MessageID: 1, JobAssignedMessages: []*scaleset.JobAssigned{assigned("g-9")}}
	s.handle([]byte(`{"schema":1,"op":"next","capacity":1,"wait_s":5}`))
	if r := s.handle([]byte(`{"schema":1,"op":"withdraw","job_id":"a-g-9","reason":"tier-mismatch"}`)); r["reply"] != "done" {
		t.Fatalf("%v", r)
	}
	if len(s.jobs) != 0 || len(s.byJobID) != 0 {
		t.Fatalf("%v %v", s.jobs, s.byJobID)
	}
}

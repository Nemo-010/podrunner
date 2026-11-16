# Handoff

The state of podrunner for a session with no memory of the last one. Read
this page first, then `AGENTS.md`, `TODO/PROGRESS.md` (the state, the work
order, the questions), `docs/maintenance.md`, `docs/development.md` and
`docs/decisions.md`. Keep this page current: remove a line when its work
is done.

## Where podrunner stands

- Pins: podcrates `v0.52.0` (`Cargo.toml`); podvm `v0.2.0-beta.27`
  (`images/podvm.toml`; the podvm kind takes `probe::LOWEST` and later,
  the microvm kind `LOWEST_GUEST` (beta.10), the incus kind
  `LOWEST_INCUS` (beta.13), and an engine of the microvm kind needs
  beta.14, T-112); podnet `v0.4.0-beta.8` (`images/podnet.toml`); the
  official runner v2.338.2 (`images/runner/runner.toml`). Last beta of
  podrunner: `v0.1.0-beta.6` (release run 43708786078, `88cc4d7`); the
  next is `v0.1.0-beta.7` (below).
- The record: `cargo todo check` gives the counts. The last green
  `runner-image.yml` run is 43704601731 (`88cc4d7`, the tag
  `v0.1.0-beta.6`): the podvm suites,
  the held tests, the local suite on each kind, the parity suite in
  guests with an engine, the microvm kind on real guests, and the incus
  suite on real instances.
- M-A, M-B and L2 hold (T-039 is done). The work packages of M-C and M-D
  are done. The open gates are T-032, T-033 and T-040 (the live gates:
  GitHub #74 and #75).
- The podvm driver runs the official runner in a unit with no root: a
  canned job and a `container:` job through the hooks pass in
  `runner-image.yml` (T-026, T-035). The unit conformance suite passes on
  the fake unit, the podvm driver, the microvm kind and the incus kind.
- Each podvm unit and container carries the label `podrunner.unit=ID`,
  and `destroy` removes each one with it; the `run -d` of a unit writes
  podvm's report (`run_report`) (T-078).
- The egress hold: with `[unit.podvm] podnet`, a podvm unit runs under
  `podnet pod`: the netns hold (`enforced`) where user namespaces and
  `/dev/net/tun` exist, else the seccomp hold (`advisory`) (T-065, T-041).
  A held runner reaches an allowed host and not a denied one
  (`crates/podrunner-broker-fake/tests/held.rs`).
- The egress policy of a job: the policy of its offer narrowed by
  `[egress] default_policy` (T-107). slots gives each driver its socket
  directory.
- The `confined` boundary: the fake applies `pod-confine` through its
  shim; the podvm kind with `[unit.podvm] confine = true` runs each unit
  and container under `podvm --podvm-confine`, and says `confined` only
  after its self-test on the host (T-042, T-079). The layer denies the
  `/proc` paths of residual R2 and `env -i`, so tests 25 and 26 skip on a
  confined podvm unit (T-080).
- The `microvm` kind (`crates/podrunner-microvm`, `[unit.microvm]`): one
  guest of podvm's machine tier for each job, with no NIC and one channel
  to the egress socket of the unit (`PODRUNNER_CHANNEL_PORT`; the guest
  script sets the proxy variables), so its hold is `enforced`. Before each
  job the guest script checks its mounts (`MOUNT_CHECK`). The self-test,
  the unit tests and a canned job of the official runner pass on real
  guests (T-036, T-037). With `[unit.microvm] engine = DIR`, the guest runs
  `dockerd` (`--podvm-engine`) and the runner gets `DOCKER_HOST` of a
  socket in the guest (T-103, `docs/container-engine.md`); its capability
  says `container: engine` (podcrates `v0.42.0`), and placement sends a
  job with `container_engine` only to such a kind.
- The local mode (`podrunner run`, `docs/local-mode.md`): the executors
  `--runner-dir`, `--podvm`, `--microvm [--engine]` and `--incus` (T-123:
  a front on the egress socket of a guest gives the broker); actions by
  tag pinned by `podrunner-actions.lock` into a store (T-118); artifacts
  v4 and the cache v2 served by the fake broker (T-119, T-120); secrets
  and `github.token` (T-121, T-122); the `github` context of the git tree
  and the event payload (T-125); the working tree, `--with-git` and the
  checkout of another repository (T-133); the cancel at a signal (T-141);
  images by tag (T-130); the protection of an environment (T-136); the
  export of a workspace on `--runner-dir` (T-138); `--local-repository`
  and `--action-server` (T-139); OIDC id-tokens (T-140); `--watch` and
  `--schedule` (T-142); `run.toml` and `.actrc` (T-143); an `env:` of one
  expression (T-152); the fetch of the actions through an egress grant
  (T-154); `podrunner doctor --run` (T-153).
  The installer `scripts/install.sh` and the offline bundle
  (`docs/install.md`, T-202); a release attaches them with the runner
  image archives from the green `runner-image.yml` run of its commit
  (T-205, partial: dispatch `runner-image.yml` on the commit before a
  release; the notes of the release list the new files).
  The parity suite (`crates/podrunner/tests/parity.rs`, T-147) gives one
  verdict for each workflow: with `--runner-dir`, or in a unit kind
  through `PODRUNNER_PARITY_UNIT`.
- The `incus` kind (`crates/podrunner-incus`, `[unit.incus]`): one system
  container of podvm's incus tier for each job, the credential files on
  stdin. It claims `confined`. It has no NIC and one channel of podvm to
  the egress socket (the socket file at mode 0666 in the 0700 socket
  directory), so its hold is `enforced` and it serves `trusted` jobs
  (T-105, T-109).
- Each report carries `cpu_s` and `peak_rss_mib`
  (`crates/podrunner-slots/src/usage.rs`, T-108); the incus kind gives
  `unknown`.
- A musl build of the official runner (`scripts/runner-musl.sh`,
  `runner-musl.yml`) runs the canned jobs in Alpine on x86_64 and
  aarch64, and in a podvm unit on x86_64 (T-081).
- The run report names `claim_to_first_step_s`, `job_images` and the exit
  reason `limit` (T-075, T-074).

## The release of `v0.1.0-beta.7` (in flight)

The rules: `AGENTS.md`, section 12, and `docs/maintenance.md`, "A
release". A release needs a green gate, `runner-image.yml`, `level2.yml`
and user-path gate (`user-gate.yml`) on its commit, and an adversarial QA
with no P1 or P2 finding (`docs/qa-brief.md`, T-246).

- `main` has the fixes of two rounds of adversarial QA: round 1 (T-265 to
  T-286) and round 2 (T-287 to T-298). Each P1 and P2 of them is done;
  T-277, T-290 and T-298 hold the P3 that are open. The notes
  `docs/releases/v0.1.0-beta.7.md` list them, with the level of proof.
- Round 3 ran (2026-11-16) on the release files of `12a1300` (T-246):
  agent A gave T-300 to T-308, agent B T-309 to T-316; the lead session
  T-301 and T-317. Done since: T-301, T-302, T-309, T-317, T-303, T-305, T-306, T-307,
  T-310, T-311, T-312, T-313, T-314. Open P1 and P2 that block the
  release (T-309 is done with podvm beta.26): T-300 (P1,
  partial: the loader family is no boundary for a static binary; waits
  for podvm's binds of `run -d`, PodUtils/podvm#442), T-304 (not reproduced), T-315 and T-260 (partial).
  A round 4 runs on the files of the commit that holds them.
- The kit of the QA: `/srv/qa-kit` (the files of a user-gate run, podvm
  from its release, the documents, `env.sh` with the proxy and CA of the
  host); the users `qa1` and `qa2` with no root. The reports are in the
  entries (T-308 and T-316 hold the P3s).
- podvm `v0.2.0-beta.27` is pinned. On this host (user namespaces)
  `run -d` of a unit takes the namespace rung, where the job runs as
  `runner` with the host name `podrunner-u-N` (T-317); podrunner keeps
  `TAR_OPTIONS=--no-same-owner` (and two variables that do nothing for
  `runner`). In the CI lanes the units ran on the loader family (Go finds
  no GOROOT; uv and ruff get EACCES): the rows of uv and ruff on podvm are
  `xfail` (T-261), until podvm binds the volumes of `run -d`.
- The last runs: user-gate 43845950155 green on `0954146` (podvm
  beta.25, the images of runner-image 43839111223 of `7d62176`): each row
  passes or is a known xfail in the six lanes; `confine:podvm` passes in
  each. podvm beta.27 is pinned since: when the runner-image run of
  that commit is green, dispatch user-gate with `-f version=0.1.0-beta.7
  -f images_run=RUN_ID`. runner-image, level2
  and the gate of the release commit run again before a release.
- Each push that changes the crates starts `runner-image.yml` (about 2
  hours, no concurrency group): cancel the runs of older commits with
  `actions_run_trigger` (`cancel_workflow_run`) when a newer one covers
  them.
- Then T-264 (podcrates `v0.61.0`: the issue sync, pod-lab, the chroot
  row of T-283), after the release.
- The LIVE-2 report (GitHub #75, 2026-11-15) ran W1, W3 (20 of 20) and
  W4 on the podvm kind with a fixed sidecar, and W6 held. Its defects are
  T-176 to T-189; T-176 to T-179 are P1.
- The LIVE-1 report (GitHub #74) found T-164 to T-175; the fixes are in
  `v0.1.0-beta.6`, except T-169 (the cancel of a refused run), which is on
  `main` after the tag, with T-198 (the limits of an offer). The next beta
  carries both; then point #74, #75 and githome (#151) at it.
- The last green `runner-image.yml` is 43704601731 (`88cc4d7`, the tag
  `v0.1.0-beta.6`): the static image, the job-started hook of T-175, and
  the runner under `[unit.podvm] mem` (T-162).
- T-162 (partial, G-12): `[unit.podvm] mem` and `pids` are rlimits;
  `[unit.incus] mem`, `cpus` and `pids` are the keys of the instance
  (podvm `0.2.0-beta.20` or later, pinned `0.2.0-beta.27`). Next: a
  payload past `pids` and one past `mem` on the podvm and incus kinds.
  `cpus` on the podvm kind waits for the cgroup half
  (PodUtils/podvm@T-1747); COORD-050 (#110) stays open for it.
- T-161 (partial): `docs/alternate-payload.md` has the census of the
  calls of the runner and the call: measure an existing third-party
  runner against the fake broker first.
- T-159 (partial): the destroy of a podvm unit gave an unknown count of
  processes once; `destroy` now logs the cause (`podrunner: destroy UNIT:
  no count of processes: ...`). Conformance test 26 prints it on purpose.
  Read it at the next real failure.
- The live gates: GitHub #74 (LIVE-1, the remote mint and the App key)
  and #75 (LIVE-2, the unit host and the test jobs) name `v0.1.0-beta.6`
  and the runner-image run 43704601731 of its tag; the App needs
  "Actions: read and write" (Q7). Read their comments;
  fix each defect they find.
- A cloud session can dispatch a workflow with the MCP tool
  `actions_run_trigger` (gate 43814582586, level2 43814590789 and
  user-gate 43814594849 on `9a27d59`). When a dispatch is refused, ask
  the local agent with an issue `[gh action] requested`
  (`docs/maintenance.md`, "A run of a workflow").
- podvm answers COORD issues `(to podvm)` in its repository with
  `(to podrunner)`. podssh is on break.
- The x86_64 job of `runner-image.yml` takes up to 120 minutes; it has
  150 minutes.
- A push that changes only `TODO/` or `docs/` runs the gate only;
  `runner-image.yml` and `level2.yml` run on their paths. A newer push
  cancels a queued gate run. Run `cargo todo check` with no pipe before
  a commit: a pipe hides its exit code.
- The disk of a cloud session is small: run the checks with
  `CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
  CARGO_PROFILE_DEV_DEBUG=line-tables-only
  CARGO_PROFILE_TEST_DEBUG=line-tables-only`, and remove
  `target/x86_64-unknown-linux-musl/debug` and `target/debug` when a link
  fails with no space.

## What waits on whom

| Who | What | Entries |
| --- | --- | --- |
| podvm | `run -d --hostname` (#115, fixed on podvm `main`), the user of the image and binds of `run -d` (#196), the memo on descriptor 17 (#197), the machine tier of a user with no root (#198) | T-182, T-300, T-309, T-317, T-291 |
| LIVE-1 and LIVE-2 | the report of GitHub #75, and a new run on the next beta | T-032, T-033, T-040 |
| podcrates | `pod-unit`, `pod-release` (none planned for v1) | T-043 |
| actions/runner | a release with `Microsoft.AspNetCore.WebUtilities` 8.0.34 or later | T-082 |

## Open issues and asks

- GitHub #65, #66, #67: the trackers of the asks of cfb, githome and the
  CI plan that podrunner owns. Their bodies have the current state
  (2026-11-15); T-001 holds them.
- COORD issues open: #110 (COORD-050, to podvm); #111 (COORD-051, to
  podvm); #189 (COORD-066), #193 (COORD-068), #195 (COORD-069), #196 (COORD-070),
  #197 (COORD-071) and #198 (COORD-072) to podvm (T-261, T-262, T-291,
  T-309, T-317); #191 (COORD-067, to podcrates, T-283). No podcrates release notice is open; podrunner takes
  podcrates `v0.52.0`.
- GitHub #9, #32, #33, #40: wait for the live gates (#74, #75). GitHub
  #43 waits for podcrates.
- No question for the owner is open: Q7 (cancel the run of a refused job), the
  size of a job (apply the values of the offer), Q8 and `limits_required`
  (settled with githome on #151) on 2026-11-15.

## Talk between sessions

- podcrates: a COORD issue of this repository titled
  `COORD-NNN: subject (to podcrates)`; no direct message (the owner,
  2026-11-12, `docs/decisions.md`). Each ask for a crate, each take of a
  tag and each milestone report goes there.
- podnet: a COORD issue of this repository titled
  `COORD-NNN: subject (to podnet)`. podnet answers in its repository with
  `(to podrunner)`, and the watcher finds it.
- podvm: answers through COORD issues `(to podrunner)` in its repository
  and through the hand-offs of podcrates.
- The design copy is `docs/design/` (call C17: do not edit it).
  `docs/design-issues.md` maps each issue of the plan to its sections.

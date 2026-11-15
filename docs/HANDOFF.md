# Handoff

The state of podrunner for a session with no memory of the last one. Read
this page first, then `AGENTS.md`, `TODO/PROGRESS.md` (the state, the work
order, the questions), `docs/maintenance.md`, `docs/development.md` and
`docs/decisions.md`. Keep this page current: remove a line when its work
is done.

## Where podrunner stands

- Pins: podcrates `v0.51.0` (`Cargo.toml`); podvm `v0.2.0-beta.21`
  (`images/podvm.toml`; the podvm kind takes `probe::LOWEST` and later,
  the microvm kind `LOWEST_GUEST` (beta.10), the incus kind
  `LOWEST_INCUS` (beta.13), and an engine of the microvm kind needs
  beta.14, T-112); podnet `v0.4.0-beta.8` (`images/podnet.toml`); the
  official runner v2.338.2 (`images/runner/runner.toml`). Last beta of
  podrunner: `v0.1.0-beta.5` (release run 43656918895, `34d56b5`).
- The record: `cargo todo check` gives the counts. The last green
  `runner-image.yml` run is 43663327458 (`c695295`): the podvm suites,
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

## In flight

- The LIVE-2 report (GitHub #75, 2026-11-15) ran W1, W3 (20 of 20) and
  W4 on the podvm kind with a fixed sidecar, and W6 held. Its defects are
  T-176 to T-189; T-176 to T-179 are P1.
- **First:** the LIVE-1 report (GitHub #74, 2026-11-15): with the
  release binaries of `v0.1.0-beta.5`, the mint served no job. The
  defects are T-164 to T-174; T-164 to T-167 are done; T-169 and T-175 (a runner is not bound to its job) block the live
  gates. LIVE-1 ran the gates with a local build of the sidecar that fixed
  defects 1 to 4; LIVE-2 (#75) reports the gates next. Fix the P1 entries,
  then release a beta and point #74 and #75 at it.
- The last green `runner-image.yml` is 43686746902 (`9a7fb3c`, podvm
  `0.2.0-beta.21`): the static image (T-158), and the job-started hook
  of T-175 in a podvm unit.
- T-162 (open, G-12): podvm answered on PodUtils/podvm#382. `--ulimit
  nproc=N` and `--ulimit as=BYTES` hold on `run -d` now: add
  `[unit.podvm] mem` and `pids` with them, and the conformance tests of
  its Prove. The incus keys (`--memory`, `--cpus`, `--pids-limit`) come
  with the next podvm release (PodUtils/podvm@T-1897). COORD-050 (#110)
  stays open until then.
- T-161 (partial): `docs/alternate-payload.md` has the census of the
  calls of the runner and the call: measure an existing third-party
  runner against the fake broker first.
- T-159 (partial): the destroy of a podvm unit gave an unknown count of
  processes once; `destroy` now logs the cause (`podrunner: destroy UNIT:
  no count of processes: ...`). Conformance test 26 prints it on purpose.
  Read it at the next real failure.
- The live gates: GitHub #74 (LIVE-1, the remote mint and the App key)
  and #75 (LIVE-2, the unit host and the test jobs) name `v0.1.0-beta.5`
  and the runner-image run 43658370121 of its tag. Read their comments;
  fix each defect they find.
- The account of this session starts no GitHub Actions run: neither a
  push nor a dispatch. Ask the local agent with an issue `[gh action]
  requested` (`docs/maintenance.md`, "A run of a workflow"). #148 asks
  for the gate, secrets, `runner-image.yml` and `level2.yml` of `04ff34f`.
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
| podvm | `run -d --hostname` (#115) | T-182 |
| LIVE-1 and LIVE-2 | the report of GitHub #75, and a new run on the next beta | T-032, T-033, T-040 |
| podcrates | `pod-unit`, `pod-release` (none planned for v1) | T-043 |
| actions/runner | a release with `Microsoft.AspNetCore.WebUtilities` 8.0.34 or later | T-082 |

## Open issues and asks

- GitHub #65, #66, #67: the trackers of the asks of cfb, githome and the
  CI plan that podrunner owns. Their bodies have the current state
  (2026-11-15); T-001 holds them.
- COORD issues open: #110 (COORD-050, to
  podvm); #111 (COORD-051, to podvm). No podcrates release notice is open; podrunner takes
  podcrates `v0.51.0`.
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

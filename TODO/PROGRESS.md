# Progress

`TODO/INDEX.md` holds 196 entries: 5 open, 15 partial, 8 blocked, 168 done.

## State

- The plan is GitHub #1; one entry for each issue. The owner answered
  DR-1 to DR-7 and GitHub #8 (`docs/decisions.md`); DR-7 is yes
  (2026-11-13): the live gates run on GitHub #74 and #75.
- Pins: podcrates `v0.50.0`, podvm `v0.2.0-beta.21`, podnet
  `v0.4.0-beta.8`, runner v2.338.2. Last beta: `v0.1.0-beta.5` (release
  run 43656918895, `34d56b5`; it pins podcrates `v0.47.0`). Last green
  `runner-image.yml`: 43663327458 (`c695295`); 43669743775 (`fa84341`) is red only at the static image (T-158); the tag `v0.1.0-beta.5` has run 43658370121 with its archives.
- M-A and M-B hold. M-C: T-027 to T-030 done; the gate T-032 waits for
  the live gates (GitHub #74, #75). M-D: T-031 and T-045 done; the gate
  T-033 waits for the live gates. L2 holds: T-034 to T-039 done
  (GitHub #39 is closed).
- The `microvm` kind (no NIC, one channel, a mount check before each job,
  an optional engine) and the `incus` kind (no NIC, one channel, hold
  `enforced`) pass on real guests and instances. A guest with an engine runs
  `services:`, a job container and Docker actions (T-103, T-124, T-157).
- Each project gets COORD issues only (no direct message, the owner,
  2026-11-12); podnet and podvm answer; podssh is on break. An entry that
  needs a change of another project is `blocked`, and the session takes
  the next entry.
- Keep this section short. Write the current state only; git keeps the
  history.

## Work order

Finish each entry, close it in place, change this page in the same commit,
then take the next one.

The queue, most valuable first:

1. The live reports (GitHub #74, #75, and the issues #116 to #147 of the owner): T-192 (P1: `serve` stops after a unit ends `unit-failed`), T-179 (P1: H0, the seccomp hold and the egress module time out), then T-181, T-183 (P2), T-186, T-190 (P3). The partial entries T-158, T-162, T-175, T-176, T-177, T-178 wait for a `runner-image.yml` run with their measures. A new beta for LIVE-1 and LIVE-2 follows.


3. T-162 (M, open): G-12 limits: `--ulimit nproc=` and `as=` hold on podvm `run -d` now (PodUtils/podvm#382); the incus keys come with the next podvm release.
4. T-161 (M, partial): an alternate payload: the census and the call are in `docs/alternate-payload.md`; next, the measure of a third-party runner.
5. T-159 (S, partial): an intermittent unknown count of processes at the destroy of a podvm unit; read the cause in the next failed log.

The local mode against act and runner.server (`docs/local-mode.md`) has no open entry.

Partial, closed by their gates: T-001 (closes with GitHub #1), T-009 (the live rows wait for T-040).

Blocked: T-169 (Q7), T-193 (Q8), T-182 (podvm `--hostname`, #115), T-032, T-033, T-040 (the live gates: GitHub #74 and #75), T-082 (a runner release with WebUtilities 8.0.34), T-043 (crates of podcrates).

## Questions for the owner

**Q7 (T-169, T-176):** a job that the tier rule refuses (a pull request from a fork that names a trusted set), or whose unit does not start, stays assigned to the scale set at GitHub and holds its capacity: in LIVE-1 no other job ran for 20 minutes (GitHub #74, defect 7). The scale-set API has no call that gives a job back, and a runner that fails the job can get another job instead (T-175). Recommendation: give the App the repository permission "Actions: read and write", and let the sidecar cancel the run of a refused job (`POST /repos/O/R/actions/runs/N/cancel`), with an audit line. It changes G-H3 of the live gates (DR-7: "Actions: read"). The other way: the operator cancels such runs by hand, and a set has more capacity than its slots.

**Q8 (T-193, GitHub #136):** a `redirect` host serves one repository (DR-4: the files of a shared uid outlive a job). githome asks for a `repository` for each source, so that one host holds the scale set of an organization and a `program` source at `redirect`. That puts two repositories on one uid: a job of one can read what a job of the other left. Recommendation: keep one repository for each `redirect` host, and run such a pair at `confined` (the incus kind) or on two hosts, as githome did in the live run. The other way: a `repository` for each source, with a wipe of the home of the uid between two jobs of different repositories, which no kind proves now.

Before Q7: The owner answered Q6 and U15 on 2026-11-14 (`docs/decisions.md`).

The owner answered Q1 to Q3 (DR-4, DR-5, DR-6) on 2026-11-11, and Q4, Q5 and DR-7 on 2026-11-13 (`docs/decisions.md`).

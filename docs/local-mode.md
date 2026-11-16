# The local mode against act and runner.server

## Quick start

`docs/install.md` gives the installer: podrunner, podvm, podnet, the
official runner, the runner image in a podvm store, and the file of the
user `run.toml`, each checked against its pin. Take it from a release that
attaches `podrunner-install.sh`:

```sh
curl -fsSLO "https://github.com/PodUtils/podrunner/releases/download/vVERSION/podrunner-install.sh"
sh podrunner-install.sh
cd YOUR-PROJECT && podrunner run .github/workflows/ci.yml --checkout .
```

## A host with a proxy

A job on `--runner-dir` takes `PATH` and these variables of the
environment of podrunner: `HTTPS_PROXY`, `HTTP_PROXY` (and their lower
case forms), `SSL_CERT_FILE`, `SSL_CERT_DIR`, `NODE_EXTRA_CA_CERTS`,
`GIT_SSL_CAINFO`, `REQUESTS_CA_BUNDLE` and `CURL_CA_BUNDLE`. Its
`NO_PROXY` always holds the loopback, where the broker of the run is.
`podrunner doctor --run` names the variables that a job takes. Set each
one before the run; give another variable with `--env NAME=VALUE`.

A job in a podvm unit with no `--podnet` (the unit has the network of the
host) takes the same proxy variables; each CA file goes into the unit as a
file of the runner, and its variable names that file.

A job on `--runner-dir` gets a scratch `HOME` (`_work/_home` of the runner
directory), which the next job removes. It runs as the user of podrunner,
with the files and the network of that user: it is not a unit.

The work directory of the runner is `_work` of the runner directory. When
the path of the runner directory has a name that starts with `.` (the
installer puts it in `~/.local/share/podrunner`), the work directory is
`podrunner-work/NAME-HASH` in the directory before that name (for example
`~/podrunner-work/2.338.2-1a2b3c4d`). Some tools refuse a path with such a
name: the `send` module of Node gives 404 for it (T-255).

## The comparison

`podrunner run FILE` runs the jobs of one workflow file with the official
runner, against a fake broker on the loopback, one fresh unit for each job
(T-046, `docs/cli.md`). This note compares it with nektos/act and with
ChristopherHX/runner.server, as the owner's ghactions-local study shows
them (act 0.2.91, runner.server v3.14.0). Both are MIT. No code or text of
them is in this repository. An entry that plans to take third-party code
says so, and the take goes through `THIRD_PARTY.md`.

## How each one runs a job

| | act | runner.server | podrunner |
| --- | --- | --- | --- |
| Engine of the steps | its own Go engine | the official runner (a fork), with its own server | the official runner, unchanged, pinned |
| Isolation | a container of the Docker daemon of the host | a container of the host's Docker, or the host | a fresh unit for each job (podvm, microvm, incus), no root, no daemon; the isolation is that of the kind: a guest of `microvm` or an incus instance isolates, and the userland rung of podvm (a host with no user namespace) gives a fresh root but no isolation |
| Network of a job | the Docker network, open | open | the egress hold: only the hosts of `--allow` |
| Binary | static Go | .NET, not static | static musl |

## The features

The column "podrunner now" gives the state of `main`. The last column names
the entry that closes the gap, or the entry that did the work.

| Feature | act | runner.server | podrunner now | Entry |
| --- | --- | --- | --- | --- |
| `needs`, outputs, `if:`, matrix with `include` and `exclude` | yes | yes | yes | T-087, T-088 |
| `fail-fast`, job `continue-on-error` | yes | yes | yes | T-117 |
| The `github` context in the steps | yes | yes | yes, the context of the run over the canned one | T-114 |
| `run:` steps, `defaults.run`, `shell`, `working-directory` | yes | yes | yes | T-088 |
| Composite and JavaScript actions by commit | yes | yes | yes, from `--actions DIR` | T-091 |
| Actions by tag or branch, fetched | yes, git clone | yes, tarball of the forge | yes, pinned by a lock file with a SHA-256; a moved tag fails by name | T-118 |
| Offline mode, action cache | `--action-offline-mode`, `--action-cache-path` | no | `--offline`, `--frozen`, the store `--actions DIR` | T-118 |
| `--local-repository`, another forge | yes | yes | yes: a made-up commit of the directory, never in the lock; `--action-server URL` for any git server | T-139 |
| `actions/checkout` of the local tree | copy of the working tree | local checkout of the changes | the working tree with its changes, not the ignored files; `--checkout-head`; a `.git` in a git tree (`--no-git` leaves it out) | T-092, T-133, T-267 |
| Checkout of another repository | real action | `--remote-checkout` | from the store at the commit of the lock | T-133 |
| `./` local actions | yes | yes | yes | T-092 |
| Docker actions by image | yes | yes | yes, pinned by digest, through the hooks | T-093, T-098 |
| Dockerfile actions (`docker build`) | yes | yes | yes, in a `microvm` guest with an engine (`--engine DIR`); refused on the other unit kinds; a Dockerfile action on `--podvm` fails at its step today, not before the job (T-237) | T-124 |
| `container:` | yes | yes | yes, pinned by digest | T-093 |
| `services:` with health checks | yes | yes | yes, in a `microvm` guest with an engine: a service answers by `localhost` and by its name; refused on the other executors | T-123, T-124 |
| Images by tag, pull | yes, `--pull` | yes | yes: `podvm pull` into the store, pinned by the lock; the units never pull | T-130 |
| Images by `runs-on` label | `-P LABEL=IMAGE` | `-P LABEL=IMAGE` or the host | `--platform LABEL=IMAGE` (pinned) or `host`; no fallback to an image the user did not name. The larger image (`images/runner/full.txt`: C and C++, CMake, Python, Node.js) is the artifact `runner-full-x86_64` of `runner-image.yml`: `podvm load -i` it, then `--platform ubuntu-latest=runner@DIGEST` | T-129 |
| Another architecture | `--container-architecture` | yes | host only | T-151 |
| Reusable workflows | local and remote | yes, `secrets: inherit` | yes: local and remote (pinned by the lock), inputs, passed or inherited secrets, outputs, matrix and nested calls | T-131 |
| Event name | argument, `--detect-event` | `--event` | `--event` | T-089 |
| Event payload | `-e FILE` | `--payload` | `--event-path FILE`, or a minimal payload of the event | T-125 |
| `github` facts from the git tree | yes | yes | yes, and flags over each fact | T-125 |
| Filters of `on:`, a directory of workflows | event names, `-W DIR` | yes | yes: the filters of `on:` (`pod-actions-workflow`), `--changed-since`, a directory of workflows | T-125 |
| Secrets | `-s`, `.secrets`, mask | `--secret`, `--secret-file`, mask | `--secret NAME` from the environment, `--secret-file` at mode 0600, `.secrets` of the checkout (never copied into the workspace), mask; no value in argv | T-121, T-276 |
| `GITHUB_TOKEN` | the token of the GitHub CLI of the user | configured | a made-up token; an action that gives it to a real API fails with "Bad credentials", and the run names `--github-token-env` (T-254); a real one only by `--github-token-env NAME`, never to `permissions: {}` | T-122 |
| Vars | `--var`, `.vars` | `--var`, `--var-file` | `--var`, `--var-file` | T-089, T-126 |
| Inputs, defaults of `workflow_dispatch` | `--input`, `.input` | `--input`, `--input-file` | `--input`, `--input-file`; the defaults, types, `required` and `choice` of GitHub | T-126 |
| Extra environment | `--env`, `.env` | `--env`, `--env-file` | `--env`, `--env-file` | T-126 |
| Environments, their secrets and vars | name only | yes | secrets, vars and an egress policy for each environment; `--protected NAME` with `--approve NAME` or a prompt on the terminal | T-136 |
| Concurrency groups | no | yes | held across runs by a lock file; `cancel-in-progress` cancels the older run | T-137 |
| Artifacts v4 | yes | yes | yes, one store for each run, size and SHA-256 checked at finalize; v3 refused by name | T-119 |
| Cache of `actions/cache` | v1 server | v1 and v2 | v2, a store that stays across runs, Range downloads, a limit of 10 GB | T-120 |
| OIDC id-token | no | yes | yes: ES256 with one key for each run, in memory only; `--oidc-issuer`, `--oidc-keys` | T-140 |
| One job, a matrix filter | `-j`, `--matrix` | `--job`, `--matrix` | `--job` with its needs, `--matrix`; each other cell `skipped` with its reason | T-127 |
| List, graph, dry run, validate | yes | `--list` | `--list`, `--graph`, `--validate`; `--dry-run` builds each job message and checks the pins | T-128 |
| Parallel jobs | `--concurrent-jobs` | `--parallel N` | `--jobs N` with a unit kind, `max-parallel`, `fail-fast` stops the running cells | T-132 |
| Live logs, log files, JSON | yes | web console, `--log-output-dir` | live lines on stderr, `--json`, `--log-dir` with each step log and `summary.md`; secrets masked again | T-134 |
| Web view | no | yes | yes: `--web IP:PORT`, the loopback only, a random token in the path, the page in the binary | T-149 |
| Reruns | no | `rerun --failed`, keep artifacts | `--state DIR`, `--rerun DIR [--failed \| --job ID]`, kept results, outputs and artifacts; a changed workflow fails by name | T-135 |
| Watch, schedules | `--watch` | `--watch`, schedules | `--watch` (a change cancels the run in progress), `--schedule` (the crons of `on.schedule`, UTC) | T-142 |
| Configuration file | `.actrc`, trust of the project file | `.actrc` | `run.toml` of the user and of the project, with `trusted`; the flags of `.actrc` that podrunner knows | T-143 |
| Keep a container, bind the tree, tool cache | `--reuse`, `--bind` | `--keep-container`, shared tool cache | `--export DIR` on `--runner-dir` and `--podvm`; `--toolcache DIR` on `--podvm` (a tree of links into `_work/_tool`; the directory is mounted read-only where the rung of podvm has mounts); `--keep-failed` keeps a failed job at its failed step | T-138 |
| Pause before a step | no | no | `--break JOB:STEP`, `--break-on-failure`: the job waits for Enter; `s` opens a shell in a podvm unit or an incus instance | T-150 |
| Host check, bug report | `--bug-report` | no | `doctor --run FILE [the options of run]`: each input checked, with its fix | T-153 |
| Interrupt | cancels and removes | cancels | cancels: each unit is destroyed, the scratch directories go, exit 130 | T-141 |
| `env:` of one expression | not read | yes | yes: the workflow, the job and the step, as the schema of the runner allows | T-152 |
| Privileged containers, host binds, cap add | flags, with a policy | yes | refused by the hook policy; privileged work runs in the engine of the unit | T-124 |
| Azure Pipelines YAML | no | a subset | no: it needs the Azure Pipelines agent (`docs/azure-pipelines.md`, Q6) | T-148 |

## The faults that the study reproduced

| Fault | In podrunner | Entry |
| --- | --- | --- |
| .NET dies with SIGXFSZ under a small file size limit, with W^X on | fixed: each executor sets `DOTNET_EnableWriteXorExecute=0`; a runner that SIGXFSZ ends fails its job with the limit named | T-144 |
| The artifact step fails with ECONNRESET, then 403: no route of the results service | fixed: the ArtifactService is served (T-119); a route that the fake broker does not serve is named in the job log and on stderr | T-145 |
| A rerun fails with EACCES on files that a container wrote as another uid | fixed: `_work` gets its write bits back before the removal, and a failure fails the job with the path | T-146 |

The study's conformance suite has defects (T-046) and no licence. The
parity suite of T-147 (`crates/podrunner/tests/local/parity`) is its
replacement: each row passes in `microvm` guests with an engine and on a
runner directory in `runner-image.yml`.

## Gaps

- The egress hold of the local mode is the seccomp hold (`advisory`); the
  netns hold waits for a grant of the loopback port of the fake broker
  (T-187).
- `services:` and Dockerfile actions need the `microvm` kind with an
  engine (KVM and QEMU, or podvm's own QEMU).
- A job of another architecture fails by name (T-151).
- No user has run the local mode on the real workflows of other projects
  yet. GitHub #153 asks a user agent for that run (T-199).

## Where podrunner exceeds them

- **A fresh unit for each job.** act shares one Docker daemon and its
  containers; runner.server can run on the host. Each job of podrunner
  starts from a clean unit and ends with `destroy`; residue fails the job.
  A reuse never carries state from one job into the next (T-138).
- **No root and no daemon.** No Docker socket of the host reaches a job.
  `services:` and `docker build` run in an engine inside the guest of the
  job (T-124, `docs/container-engine.md`).
- **The egress hold.** A step reaches only the hosts of `--allow`, through
  the egress module. A secret or a real token cannot leave to another host
  (T-121, T-122). An environment can have its own egress policy (T-136).
- **Pins.** Actions are pinned by commit and image by digest, recorded in a
  lock file with a SHA-256 (T-118, T-130). A moved tag fails by name. The
  host fetches; a unit never fetches.
- **No listener on the network.** The broker, the artifact store and the
  cache listen on the loopback only. act binds its cache server to the
  outbound address of the host by default.
- **Secrets.** No secret in argv (act takes `-s NAME=VALUE`). A secret file
  needs mode 0600. No real token by default (act reads the token of the
  GitHub CLI of the user).
- **The official runner.** The steps run in the runner of GitHub, pinned
  by version and digest; act reimplements the engine.
- **Static binaries.** One static `podrunner`; runner.server needs .NET.
- **Guest kinds.** With `--microvm` a job runs in a guest with its own
  kernel and no NIC; with `--incus`, in a system container of incus.
- **Integrity.** Artifacts and cache entries carry their digest; the cache
  keeps the scope of a ref (T-119, T-120).

## Generic work for podcrates

Each ask goes to podcrates as a COORD issue when its entry starts:

- the resolve of an action reference and a store of tarballs by digest
  (T-118), for example `pod-actions-store`;
- the artifact and cache services of the results API (T-119, T-120), for
  example `pod-actions-results`;
- the filters of `on:` and the expansion of a called workflow, in
  `pod-actions-workflow` (T-125, T-131);
- the registry client `pod-oci` for images by tag (T-130).

# The labs of podrunner

Each script brings up one host class (a profile of COORD-061, GitHub
#180) on a disposable host, checks that the class holds, and runs a
command in it:

```sh
sh scripts/lab/PROFILE.sh -- COMMAND ARGS...   # inherited-fds.sh: bash
```

The scripts are the first draft of the profiles of `pod-lab` (podcrates)
and `podvm lab run` (podvm); podrunner drops each one for theirs when it
pins them (T-244). A script that needs root says so, and runs only on a
disposable host (a GitHub runner).

| Script | The class | Root |
| --- | --- | --- |
| `small-tmp.sh` | `TMPDIR` on a tmpfs of 1 GiB (`LAB_TMP_SIZE`) | yes (the mount) |
| `userns-off.sh` | no user namespace (`user.max_user_namespaces=0`) | yes (the sysctl) |
| `nonroot.sh` | a user with no root and no capability | yes (the user) |
| `inherited-fds.sh` | descriptors 3 to N open in the command (`LAB_FDS`, default 15); bash | no |

Each script checks its class first and fails with the part that does not
hold. `.github/workflows/user-gate.yml` (T-243) runs them as the
lanes of the user-path gate, one lane for each class. Each one is 0BSD, as the repository.

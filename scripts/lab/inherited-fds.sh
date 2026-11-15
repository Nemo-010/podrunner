#!/usr/bin/env bash
# The class inherited-fds (T-244, podrunner #176): the command starts with
# descriptors 3 to LAB_FDS (15) open, as under a supervisor. bash, not sh:
# the sh of POSIX takes a descriptor of one digit only.
set -eu
[ "${1:-}" = "--" ] && shift
n=${LAB_FDS:-15}
for ((i = 3; i <= n; i++)); do
    eval "exec $i</dev/null"
done
# The check: the last one is open.
[ -e "/proc/$$/fd/$n" ] || { echo "lab inherited-fds: descriptor $n is not open" >&2; exit 70; }
echo "lab inherited-fds: descriptors 3 to $n are open" >&2
exec "$@"

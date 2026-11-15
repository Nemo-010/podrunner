#!/bin/sh
# The class nonroot (T-244): the command runs as a user with no root and
# no capability. Needs root to make the user: a disposable host only.
set -eu
[ "${1:-}" = "--" ] && shift
user=${LAB_USER:-labuser}
id "$user" >/dev/null 2>&1 || sudo useradd -m -s /bin/sh "$user"
# The check: the user has no capability.
caps=$(sudo -u "$user" sh -c 'grep CapEff /proc/self/status' | awk '{print $2}')
[ "$caps" = "0000000000000000" ] || { echo "lab nonroot: $user has CapEff $caps" >&2; exit 70; }
echo "lab nonroot: as $user, CapEff 0" >&2
exec sudo -u "$user" --preserve-env=PATH,TMPDIR -- "$@"

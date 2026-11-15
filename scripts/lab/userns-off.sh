#!/bin/sh
# The class userns-off (T-244): no user namespace for the command.
# Needs root for the sysctl: a disposable host only.
set -eu
[ "${1:-}" = "--" ] && shift
sudo sysctl -q -w user.max_user_namespaces=0
# Ubuntu 24.04 also restricts them by AppArmor; either one is enough.
if unshare -Ur true 2>/dev/null; then
    echo "lab userns-off: unshare -Ur still works" >&2
    exit 70
fi
echo "lab userns-off: unshare -Ur fails" >&2
exec "$@"

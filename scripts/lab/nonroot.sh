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
# The proxy and the CA of proxy-only.sh stay.
keep=PATH,TMPDIR,PODRUNNER_GATE_TOKEN,HTTPS_PROXY,HTTP_PROXY,https_proxy,http_proxy,NO_PROXY,no_proxy
keep=$keep,SSL_CERT_FILE,NODE_EXTRA_CA_CERTS,GIT_SSL_CAINFO,REQUESTS_CA_BUNDLE,CURL_CA_BUNDLE
exec sudo -u "$user" --preserve-env="$keep" -- "$@"

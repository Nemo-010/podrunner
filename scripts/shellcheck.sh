#!/bin/sh
# SPDX-License-Identifier: 0BSD
# One pinned version of ShellCheck, the same in CI and on a host of an agent
# (T-251): the shellcheck of a runner image can be older and give other
# findings. The binary goes into .work/tools, checked by its SHA-256.
#
#   sh scripts/shellcheck.sh FILE...
set -eu
version=0.11.0
sum=8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198
root=$(cd "$(dirname "$0")/.." && pwd)
dir=$root/.work/tools/shellcheck-v$version
bin=$dir/shellcheck
if [ ! -x "$bin" ]; then
    [ "$(uname -m)" = x86_64 ] || { echo "shellcheck.sh: the pin is for x86_64; install shellcheck $version" >&2; exit 69; }
    mkdir -p "$dir"
    tmp=$dir/download.tar.xz
    curl -qfsSL -o "$tmp" "https://github.com/koalaman/shellcheck/releases/download/v$version/shellcheck-v$version.linux.x86_64.tar.xz"
    echo "$sum  $tmp" | sha256sum -c - > /dev/null || { echo "shellcheck.sh: the SHA-256 of the download is not the pin" >&2; rm -f "$tmp"; exit 76; }
    tar -xJf "$tmp" -C "$dir" --strip-components=1 "shellcheck-v$version/shellcheck"
    rm -f "$tmp"
fi
exec "$bin" "$@"

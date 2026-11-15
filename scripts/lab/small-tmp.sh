#!/bin/sh
# The class small-tmp (T-244): TMPDIR on a tmpfs of LAB_TMP_SIZE (1g).
# Needs root for the mount: a disposable host only.
set -eu
[ "${1:-}" = "--" ] && shift
size=${LAB_TMP_SIZE:-1g}
dir=${LAB_TMP_DIR:-/mnt/lab-small-tmp}
sudo mkdir -p "$dir"
mountpoint -q "$dir" || sudo mount -t tmpfs -o "size=$size,mode=1777" lab-small-tmp "$dir"
# The check: the free space is at most the size.
free_k=$(df -Pk "$dir" | awk 'NR==2 {print $4}')
max_k=$(numfmt --from=iec "$(echo "$size" | tr a-z A-Z)" | awk '{print int($1/1024)}')
[ "$free_k" -le "$max_k" ] || { echo "lab small-tmp: $dir has ${free_k} KiB free, more than $size" >&2; exit 70; }
echo "lab small-tmp: TMPDIR=$dir (${free_k} KiB free)" >&2
TMPDIR=$dir exec "$@"

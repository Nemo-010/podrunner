#!/bin/sh
# SPDX-License-Identifier: 0BSD
# The notes of a release name the level of the proof of each claim
# (AGENTS.md, section 12; T-247): the three sections below must be there,
# and "What is not proven" must not be empty. A reader must not take a
# claim of CI for a claim of a user host.
#
#   sh scripts/check-notes.sh NOTES
#   sh scripts/check-notes.sh --latest   the notes of the newest version
set -eu
die() {
    echo "check-notes: $*" >&2
    exit 1
}
[ $# -eq 1 ] || die "usage: sh scripts/check-notes.sh NOTES | --latest"
notes=$1
if [ "$notes" = --latest ]; then
    root=$(cd "$(dirname "$0")/.." && pwd)
    notes=$(printf '%s\n' "$root"/docs/releases/v*.md | sort -V | tail -n 1)
fi
[ -f "$notes" ] || die "$notes: no such file"
for h in "## What is proven in CI" "## What is proven on a constrained host as a user" "## What is not proven"; do
    grep -qx "$h" "$notes" || die "$notes: no section \"$h\": name the level of the proof of each claim"
done
# The lines of "What is not proven" up to the next section.
open=$(sed -n '/^## What is not proven$/,/^## /p' "$notes" | sed '1d;/^## /d' | grep -c '[^[:space:]]' || true)
[ "$open" -gt 0 ] || die "$notes: the section \"What is not proven\" is empty: write what no run proved"
echo "check-notes: $notes names the level of its proof"

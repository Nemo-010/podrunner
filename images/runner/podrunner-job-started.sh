#!/bin/sh
# The job-started hook of the runner image (T-175). GitHub binds no runner
# of a scale set to one job, so this runner can get another job than the
# one that podrunner placed in this unit, at the boundary of that job.
# podrunner writes the placed job, "OWNER/REPO RUN_ID", into the runner
# root (.podrunner-job). Another job ends here, before its first step. A
# job of the same run has the same trust inputs, so the run is enough.
# With no file (the local mode), there is nothing to compare.
want_file="$(dirname "$0")/../.podrunner-job"
[ -f "$want_file" ] || exit 0
want=$(tr 'A-Z' 'a-z' < "$want_file")
got=$(printf '%s %s\n' "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID" | tr 'A-Z' 'a-z')
[ "$got" = "$want" ] && exit 0
echo "podrunner: this runner was placed for another job: the job ends before its first step" >&2
exit 1

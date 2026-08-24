#!/usr/bin/env bash
# D-0010: no employer code, data, or confidential identifiers in this
# repository, ever. Run before publishing anything, and before any commit that
# touches docs/ or crates/.
#
# One nuance, learned by tripping it four times in a day: most of these names
# are distinctive enough to match case-insensitively, but one collides with an
# ordinary English word. Matching that one case-sensitively as a standalone
# token, plus the identifier forms it would really appear in, keeps the alarm
# meaningful — an alarm that cries wolf is one you stop reading, which is worse
# than no alarm at all.
set -u
targets=("$@")
[ ${#targets[@]} -eq 0 ] && targets=(docs crates)

status=0

# Distinctive names: any case, any position.
if grep -rniE '\b(employer|system-a|system-b|system-c|system-d)\b' "${targets[@]}"; then
  status=1
fi

# The English-word collision: the system name is capitalised, or appears as an
# identifier such as system-e-dev / system-e_kite.
if grep -rnE '\bSYSTEM-E\b|\bsystem-e[-_][a-z]+' "${targets[@]}"; then
  status=1
fi

if [ $status -eq 0 ]; then
  echo "boundary clean"
else
  echo "BOUNDARY VIOLATION — see matches above"
fi
exit $status

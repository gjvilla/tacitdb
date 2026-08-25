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

# Authorship is part of the boundary and was not being checked. A personal
# project every commit of which is authored and signed under the employer's
# domain is making a claim in its own record that its own decision record
# denies (D-0010, U-7).
#
# The split matters: what the *next* commit will say is fixable, so it fails.
# What history already says is reported and never failed on — history is not
# rewritten here (D-0019), and least of all the authorship record of a project
# whose ownership is the open question. Quietly restamping twenty commits with
# a different name would be tidying evidence.
if git rev-parse --git-dir >/dev/null 2>&1; then
  who="$(git config user.email 2>/dev/null || true)"
  if printf '%s' "$who" | grep -qiE '@employer\.'; then
    echo "BOUNDARY: the next commit here would be attributed to $who"
    echo "  This is U-7 and it is expected to stay red until U-7 resolves. It is not"
    echo "  a new problem each time you see it; it is the release gate, made"
    echo "  mechanical. U-7 already blocks publishing — this stops that depending on"
    echo "  somebody remembering. Do not silence it; resolve U-7."
    status=1
  fi
  past="$(git log --format='%ae%n%ce' 2>/dev/null | grep -ciE '@employer\.' || true)"
  if [ "${past:-0}" -gt 0 ]; then
    echo "note: $past commit identities already in history carry the employer's"
    echo "      domain. Left alone deliberately — see U-7 and D-0035."
  fi
fi

if [ $status -eq 0 ]; then
  echo "boundary clean"
else
  echo "BOUNDARY VIOLATION — see matches above"
fi
exit $status

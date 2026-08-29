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

# Authorship is part of the boundary (D-0035). U-7 resolved 2026-08-29
# (D-0038): no invention-assignment agreement exists, and history was
# restamped to the personal identity — with the pre-rewrite record preserved
# in a mirror clone — so both halves of this check should now be quiet. The
# check stays: the gate is mechanical, and it would fire again if the
# employer identity ever reappeared.
if git rev-parse --git-dir >/dev/null 2>&1; then
  who="$(git config user.email 2>/dev/null || true)"
  if printf '%s' "$who" | grep -qiE '@employer\.'; then
    echo "BOUNDARY: the next commit here would be attributed to $who"
    echo "  U-7 resolved (D-0038) and history was corrected; an employer identity"
    echo "  must not reappear here. Set git user.email to the personal address."
    status=1
  fi
  past="$(git log --format='%ae%n%ce' 2>/dev/null | grep -ciE '@employer\.' || true)"
  if [ "${past:-0}" -gt 0 ]; then
    echo "BOUNDARY: $past commit identities in history carry the employer's domain."
    echo "  History was restamped when U-7 resolved (D-0038); it should be clean."
    status=1
  fi
fi

if [ $status -eq 0 ]; then
  echo "boundary clean"
else
  echo "BOUNDARY VIOLATION — see matches above"
fi
exit $status

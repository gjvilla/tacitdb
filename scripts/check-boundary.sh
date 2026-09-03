#!/usr/bin/env bash
# D-0010: no employer code, data, or confidential identifiers in this
# repository, ever. Run before publishing anything, and before any commit that
# touches docs/ or crates/.
#
# The names this script looks for are not written in it (D-0054): a scrub that
# carries the identifiers it exists to keep out would publish them the day the
# repository does. They live in a file outside the tree, one extended regex per
# line, each prefixed by a class — `i` case-insensitive over file contents, `s`
# case-sensitive over file contents, `e` over commit identities — and the
# script refuses to report "clean" when that file is missing or empty, because
# a scrub with nothing to look for is not a scrub.
#
# One nuance, learned by tripping it four times in a day: most names are
# distinctive enough to match case-insensitively, but one collides with an
# ordinary English word. Matching that one case-sensitively as a standalone
# token, plus the identifier forms it would really appear in, keeps the alarm
# meaningful — an alarm that cries wolf is one you stop reading, which is worse
# than no alarm at all. That is what the `s` class is for.
set -u
terms="${TACIT_BOUNDARY_TERMS:-$HOME/.config/tacit/boundary-terms}"
targets=("$@")
[ ${#targets[@]} -eq 0 ] && targets=(docs crates scripts README.md Cargo.toml)

if [ ! -s "$terms" ]; then
  echo "BOUNDARY: no terms file at $terms"
  echo "  The scrub has nothing to look for and will not call that clean."
  echo "  Set TACIT_BOUNDARY_TERMS or create the file (see the header of this script)."
  exit 2
fi

status=0
have_terms=0

while IFS= read -r line; do
  case "$line" in ''|'#'*) continue ;; esac
  class="${line%% *}"
  pattern="${line#* }"
  have_terms=1
  case "$class" in
    i) grep -rniE "$pattern" "${targets[@]}" && status=1 ;;
    s) grep -rnE "$pattern" "${targets[@]}" && status=1 ;;
    e)
      # Authorship is part of the boundary (D-0035). U-7 resolved 2026-08-29
      # (D-0038): history was restamped to the personal identity, with the
      # pre-rewrite record preserved in a mirror clone, so both halves of this
      # check should be quiet. The check stays: the gate is mechanical, and it
      # would fire again if the employer identity ever reappeared.
      if git rev-parse --git-dir >/dev/null 2>&1; then
        who="$(git config user.email 2>/dev/null || true)"
        if printf '%s' "$who" | grep -qiE "$pattern"; then
          echo "BOUNDARY: the next commit here would be attributed to $who"
          echo "  U-7 resolved (D-0038) and history was corrected; an employer identity"
          echo "  must not reappear here. Set git user.email to the personal address."
          status=1
        fi
        past="$(git log --format='%ae%n%ce' 2>/dev/null | grep -ciE "$pattern" || true)"
        if [ "${past:-0}" -gt 0 ]; then
          echo "BOUNDARY: $past commit identities in history carry the employer's domain."
          echo "  History was restamped when U-7 resolved (D-0038); it should be clean."
          status=1
        fi
      fi
      ;;
    *)
      echo "BOUNDARY: unreadable line in $terms: $line"
      status=1
      ;;
  esac
done < "$terms"

if [ $have_terms -eq 0 ]; then
  echo "BOUNDARY: $terms holds no patterns; refusing to call that clean."
  exit 2
fi

if [ $status -eq 0 ]; then
  echo "boundary clean"
else
  echo "BOUNDARY VIOLATION — see matches above"
fi
exit $status

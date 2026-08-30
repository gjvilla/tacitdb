#!/usr/bin/env bash
# Fetch the pinned proposal slice the P-suite was agreed against.
#
# The documents are not vendored: U-11 (a designed removal that preserves
# chain integrity) is open, its trigger is any external or personal-data
# corpus, and the raw files carry authors' contact details. So the corpus
# lives outside the repository and this script is the pin — a fixed upstream
# commit and a fixed file list, because a question agreed against a corpus
# that moves stops measuring what it was agreed to measure. The upstream
# documents are dual public-domain and CC0-1.0; fetching them raises no
# licensing question.
#
# Usage: scripts/fetch-proposals.sh [dest-dir]
# Default destination is target/proposals, which git already ignores.
set -eu

COMMIT=fa5792c35c6a6e9eee738603a9068f29dc893858
BASE="https://raw.githubusercontent.com/python/peps/$COMMIT/peps"
DEST="${1:-target/proposals}"

# The slice: the packaging proposals, sixty documents. Chosen because they are
# one coherent domain (questions must discriminate among records that share a
# vocabulary), they carry five supersession chains and in-slice Requires
# links, and seven of the nine statuses appear. Nothing at this commit holds
# Provisional, and Active lives only in the meta-proposals — noted rather
# than patched around, because the suite grades retrieval, not the mapping.
NUMBERS="0241 0314 0345 0376 0386 0425 0426 0427 0438 0440 0453 0458 0470
0480 0491 0503 0508 0513 0517 0518 0527 0541 0566 0571 0582 0592 0599 0600
0610 0621 0627 0629 0631 0632 0639 0643 0650 0658 0660 0665 0668 0685 0691
0700 0708 0714 0715 0723 0725 0730 0735 0740 0751 0752 0753 0755 0763 0766
0771 0777"

mkdir -p "$DEST"
fetched=0
kept=0
for n in $NUMBERS; do
  f="$DEST/pep-$n.rst"
  if [ -s "$f" ]; then
    kept=$((kept + 1))
    continue
  fi
  curl -fsS "$BASE/pep-$n.rst" -o "$f"
  fetched=$((fetched + 1))
done

count=$(ls "$DEST"/pep-*.rst | wc -l | tr -d ' ')
echo "proposal slice at $DEST: $count documents ($fetched fetched, $kept already present)"
echo "pinned to python/peps @ $COMMIT"

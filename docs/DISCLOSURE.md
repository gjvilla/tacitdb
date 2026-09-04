# Tacit — what it is, for a conversation about ownership

This page exists to make one conversation easier to have. It sets out what this
project is, what it contains, what it deliberately does not contain, and how
that has been enforced — together with the facts that cut the other way, because
a summary that only helped its author would not be worth showing anyone.

**It states facts. It makes no legal claim and is not legal advice.** It was
written to ask one question — for a lawyer and for my employer — and that
question closed on 2026-08-29. The page now records how, under *How the question
closed* below. The register tracks it as U-7.

---

## What it is

An engine for organizational memory: a small database that stores assertions
with their provenance and refuses to let anything become *promoted* knowledge
without a human verdict. About fifteen thousand lines of Rust across four crates,
of which a large share is its own test suite — 208 tests — plus its own decision
records. There is no product, no revenue, no user, and no
deployment. Its only corpus is its own documentation, plus a generator that
makes a synthetic one.

## What it contains

Everything in this repository was written from first principles for it. It has
no dependency on any employer system, no extract of any employer database, no
copied source, and no configuration, credential or identifier belonging to
anyone but me. Its third-party dependencies are public crates.

## What it deliberately does not contain, and how that is enforced

The boundary was written down on the day the project started, before any code:

> **D-0010.** Tacit is a personal project built on personal time. Hard boundary:
> no employer code, data, or confidential identifiers flow into this repository
> — ever.

It is not only an intention. `scripts/check-boundary.sh` is that rule made
executable — it scans the documents and the source for employer names, system
names and identifiers, and it has been run at the end of every working session
since it was written. The names it scans for are kept in a file outside the
repository (D-0054); they were inside the script until 2026-09-03, which is the
third fact below. The design record states the same boundary in the places
where employer experience did inform the work: what informed it are *generic*
lessons about what a database ought to do, stated without reference to any
employer system, and the register records that distinction rather than assuming
it.

## When it was made

Twenty-four commits over four days: one on Sunday 2026-08-23, twenty on Monday
2026-08-24, two on Tuesday 2026-08-25, and one on Saturday 2026-08-29 — the
last being D-0038, which closed the question this page was written to ask.

## The facts that cut the other way

Three — the first two found by the project's own checks on 2026-08-24, the
third by the same check on 2026-09-03 — all recorded rather than tidied:

1. **Every commit was authored, committed and cryptographically signed under my
   employer email address.** The repository's own record therefore attributed all
   of this work to an employer identity, which is exactly what the decision
   record above denies. The boundary check had been reading the files and never
   the commits. It reads both now — red on the day this fact was recorded, green
   since the correction described below.

2. **Almost all of the commit timestamps fall on a weekday afternoon** — nineteen
   of the twenty commits that existed on 2026-08-24 between 13:00 and 19:00 local
   on a Monday, one on a Sunday afternoon. Of the twenty-four now in the record,
   twenty are that Monday afternoon; the four added since are two on a Tuesday
   morning and two on weekend afternoons. This fact still stands, because the
   timestamps were never altered. What it means depends on facts about my working
   arrangements that are not in this repository.

3. **The boundary script itself carried the employer's name, its mail domain
   and its system names** — as the patterns it greps for, tracked from the day
   it was written until 2026-09-03. It scanned the documents and the source and
   never itself, so the one file that named what must not be in the tree was
   the one file the scan skipped. Found the first time the scrub was run over
   every tracked file, ahead of making the repository public; nothing else was
   in the tree. The patterns moved to a file outside the repository and history
   was rewritten to replace them with placeholders, on D-0038's terms — the
   reason recorded first (D-0054), the pre-rewrite record preserved in a mirror
   clone. This is a fact about the gate, not about what it guards: no employer
   code, data or configuration was ever here, only the list of names.

While the question was open, neither of the first two was altered: rewriting the authorship
record of a project whose ownership is an open question would have been tidying
evidence, whatever else it would be (D-0035).

## How the question closed

**Resolved 2026-08-29 (D-0038): no employment agreement carrying an
invention-assignment clause exists at my employer — none was ever signed.** The
question this page was written to ask ("does the clause reach this project?")
has no clause to answer it.

With ownership no longer open, the authorship record was corrected: every
commit was restamped from the employer address to my personal identity via
`git filter-repo --mailmap`, with the complete pre-rewrite history preserved in
a mirror clone made first. Commit timestamps — including the weekday-afternoon
pattern in fact 2 above — were deliberately left untouched; they are facts about
when this was built and they stay in the record. This page stays in the tree as
the factual account it was.

What remains is narrower than U-7 was: a counsel review before any commercial
use (jurisdictional default doctrines were not examined by a lawyer), tracked
with U-6's counsel item.

---

*Recorded 2026-08-24; resolved 2026-08-29 (D-0038); amended 2026-09-03
(D-0054). The counts on this page — commits, lines, tests — are as of the
dates named beside them and are not updated: this is the record of a
closed question, and the repository is the current record of the work.
Tracked as U-7 in [REGISTER.md](REGISTER.md). The licence is settled
(D-0050); the source no longer waits on anything to be public, and publishing
the crates under the name still waits on U-6.*

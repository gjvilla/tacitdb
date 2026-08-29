# Tacit — what it is, for a conversation about ownership

This page exists to make one conversation easier to have. It sets out what this
project is, what it contains, what it deliberately does not contain, and how
that has been enforced — together with the facts that cut the other way, because
a summary that only helped its author would not be worth showing anyone.

**It states facts and asks a question. It makes no legal claim and is not legal
advice.** The question at the end is for a lawyer and for my employer, and the
register tracks it as U-7.

---

## What it is

An engine for organizational memory: a small database that stores assertions
with their provenance and refuses to let anything become *promoted* knowledge
without a human verdict. About fifteen thousand lines of Rust across four crates,
of which a large share is its own test suite — 194 tests — plus its own decision
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
since it was written. The design record states the same boundary in the places
where employer experience did inform the work: what informed it are *generic*
lessons about what a database ought to do, stated without reference to any
employer system, and the register records that distinction rather than assuming
it.

## When it was made

Twenty commits over two days: one on Sunday 2026-08-23 and nineteen on Monday
2026-08-24.

## The facts that cut the other way

Two, both found by the project's own checks on 2026-08-24 and recorded rather
than tidied:

1. **Every commit is authored, committed and cryptographically signed under my
   employer email address.** The repository's own record therefore attributes all
   of this work to an employer identity, which is exactly what the decision
   record above denies. The boundary check had been reading the files and never
   the commits. It reads both now, and it is currently red.

2. **Almost all of the commit timestamps fall on a weekday afternoon** — nineteen
   of twenty between 13:00 and 19:00 local on a Monday, one on a Sunday
   afternoon. What that means depends on facts about my working arrangements
   that are not in this repository.

While the question was open, neither was altered: rewriting the authorship
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

*Recorded 2026-08-24; resolved 2026-08-29 (D-0038). Tracked as U-7 in
[REGISTER.md](REGISTER.md). Public release now blocked only by U-6 (name) and
U-17 (licence).*

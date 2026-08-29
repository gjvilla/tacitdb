# Tacit — The Golden Suite

Representative questions with the answers a qualified person agrees are
correct, run against the engine on demand. This is the known-known room turned
into an instrument.

Two properties make it worth having.

**Abstention is a pass.** A question the record does not settle should come
back as an abstention, and where a registered unknown covers the territory it
should be cited. A system that answers everything confidently scores worse
here than one that knows its own boundary — which is the opposite of how
accuracy alone would score them.

**A single number is not the result.** Every failure is classified by which
room it came from, because "retrieval missed something the record holds" and
"retrieval answered something the record does not hold" have different owners
and different fixes.

Golden data decays like any other standard work. Each question below carries an
owner and a review trigger; the runner reports any that lack one, and any
marked as a known shortfall against a registered unknown.

## Expectations

| form | meaning |
|---|---|
| `answer D-0015` | the record settles this, and D-0015 is the record that does |
| `abstain U-5` | the record does not settle this, and U-5 is the registered open question covering it |
| `abstain` | the record does not settle this, and no registered question covers it either |
| `pending U-23` | appended to any expectation: known to fall short today, tracked against that unknown rather than counted as a pass or a build failure |

## Known shortfalls, and the rule for adding one

Four questions are marked `pending U-23` today. `pending` is not a way to make
the suite green: it requires a *registered* unknown that explains the failure,
the runner counts and prints the shortfalls separately from the passes, and it
announces any question that starts passing so the register entry can be
reconsidered. A shortfall with no registered cause is a regression.

Three remain, all the same cause — a ranker that matches words rather than
meaning:

- **G-08** surfaces the correct record at rank one and then declines to call it
  a match. Recall is fine; confidence calibration is not.
- **G-09** answers confidently from records that merely use the word "storage",
  instead of abstaining to the open question that actually covers it.
- **G-10** asks about a "licence" where the register writes "license". Vector
  candidates now bridge that spelling well enough to *reach* the record, which
  is why this failure moved from bluffing to simply not citing the right open
  question — but not well enough to pick U-17 out. The question keeps the
  spelling a person might reasonably type; tuning it to match the corpus would
  be tuning the instrument to the result.

A methodological note, learned the hard way and tracked as U-27: this corpus
describes itself, so a register entry that quotes a golden question's exact
phrasing will rank for that question and displace the record it was asking
about. U-23 did precisely that until it was reworded. When adding a question,
check that no record was written *about* it.

## Questions

| id | Question | Expect | Owner | Review trigger |
|----|----------|--------|-------|----------------|
| G-01 | why is the runtime embedded rather than a server | answer D-0015 | Greg Villa | when the runtime shape is revisited |
| G-02 | what is the atomic unit of memory | answer D-0004 | Greg Villa | when the envelope schema changes |
| G-03 | where does the write-path ratchet live, the engine or the keeper | answer D-0012 | Greg Villa | when invariants 1-8 change |
| G-04 | what separates the governed ledger from the instrument panel | answer D-0013 | Greg Villa | when the two-ledger boundary is redrawn |
| G-05 | what workload is v1 designed against | answer D-0003 | Greg Villa | when the v1 workload changes |
| G-06 | is this a personal project or owned by an employer | answer D-0010 | Greg Villa | re-read 2026-08-29 on resolution (D-0038): the answer stands, now backed by fact; re-review if the employer introduces an IP agreement or asserts a claim |
| G-07 | why was the working name retired | answer D-0011 | Greg Villa | when U-6 resolves |
| G-08 | what did the prior art survey conclude about building versus adopting | answer D-0008 (pending U-23) | Greg Villa | when a surveyed engine ships the wedge |
| G-09 | which storage engine does the project use | answer D-0019 (pending U-23) | Greg Villa | when the store stops being an append-only log |
| G-10 | what licence will the engine ship under | abstain U-17 (pending U-23) | Greg Villa | when U-17 resolves |
| G-15 | what happens in the ledger when a decision record is edited | answer D-0021 | Greg Villa | when the ingest stops being a sync |
| G-16 | what does the engine do if the machine clock moves backwards | answer D-0022 | Greg Villa | when more than one process writes one log |
| G-17 | what is recorded when an open question is reworded rather than answered | answer D-0023 | Greg Villa | when a fifth withdrawal reason is wanted |
| G-18 | does the review inbox list every unreviewed claim | answer D-0024 | Greg Villa | when an author can retract a proposal outright |
| G-19 | what stops someone editing a file from promoting a claim | answer D-0025 (pending U-23) | Greg Villa | when a second channel can carry a verdict |
| G-20 | which signing keys does the project accept for verdicts | answer D-0026 (pending U-23) | Greg Villa | when trust is re-checked automatically |
| G-21 | what happens to an old promotion when a signing key is revoked | answer D-0027 | Greg Villa | when a weakening must reach someone away from a terminal |
| G-11 | how does sharding across geographic regions work | abstain | Greg Villa | never — this is outside the record by design |
| G-12 | what is the maximum supported cluster size | abstain | Greg Villa | never — this is outside the record by design |
| G-13 | how many concurrent writers does the store support | answer D-0015 (pending U-23) | Greg Villa | when more than one process may write one store |
| G-14 | what colour is the logo | abstain | Greg Villa | never — this is outside the record by design |

## Vocabulary baseline

Recorded when each question was agreed: the words of the question that the
corpus did not contain at all. Absence is the stable thing to record — document
frequency and reach both drift as the corpus grows, so a baseline of either
would cry wolf on every new record, while whether a word is present does not
move unless somebody writes it.

If one of these words later appears in the corpus, the question has stopped
measuring what it was agreed to measure, and the suite turns red so it gets
re-read rather than quietly re-scored. That has happened three times (U-27), and
once it made the score *better*, which is the direction nobody investigates.

Regenerate with `GOLDEN_BASELINE=1 cargo run -p tacit-keeper --example golden`.

| id | words the corpus did not contain |
|----|----------------------------------|
| G-01 | — |
| G-02 | — |
| G-03 | — |
| G-04 | — |
| G-05 | — |
| G-06 | — |
| G-07 | — |
| G-08 | adopting conclude versus |
| G-09 | — |
| G-10 | licence |
| G-15 | — |
| G-16 | — |
| G-17 | — |
| G-18 | — |
| G-19 | — |
| G-20 | — |
| G-21 | — |
| G-11 | geographic region sharding |
| G-12 | cluster supported |
| G-13 | concurrent support writer |
| G-14 | colour logo |

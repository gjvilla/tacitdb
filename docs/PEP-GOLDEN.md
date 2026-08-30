# Tacit — The Proposals Suite

The golden suite's counterpart over a corpus this project did not write: sixty
Python packaging proposals, real language with real paraphrase, jargon drift,
and records that compete for the same vocabulary. This is the measurement U-9
existed to make possible, and the one U-38 and U-39 each name as their
precondition.

Two structural differences from [GOLDEN.md](GOLDEN.md), both deliberate:

**The corpus lives outside the repository.** U-11 is open and the raw
documents carry authors' contact details, so nothing is vendored.
`scripts/fetch-proposals.sh` fetches the slice from a pinned upstream commit,
and the runner refuses to grade a directory that is not exactly the pinned
slice — a suite agreed against one corpus and run over another measures
nothing.

**The corpus cannot describe the suite.** The self-hosting corpus quotes its
own questions and acquires their vocabulary (U-27); these documents were
finished before this suite existed and are pinned, so the only way the corpus
moves is a deliberate repin, at which point every question is due a re-read.

Grading, expectations, and the vocabulary baseline work exactly as in
[GOLDEN.md](GOLDEN.md). Question ids carry `P-` so a report names its corpus.
There are no gap records in this ledger, so abstentions are expected bare —
`abstain`, never `abstain U-x`.

Run with `cargo run -p tacit-keeper --example pep_golden` after fetching the
slice.

## Pinned slice

Agreed against python/peps commit `fa5792c35c6a6e9eee738603a9068f29dc893858`
(2026-08-30), sixty packaging proposals:

PEP-0241 PEP-0314 PEP-0345 PEP-0376 PEP-0386 PEP-0425 PEP-0426 PEP-0427
PEP-0438 PEP-0440 PEP-0453 PEP-0458 PEP-0470 PEP-0480 PEP-0491 PEP-0503
PEP-0508 PEP-0513 PEP-0517 PEP-0518 PEP-0527 PEP-0541 PEP-0566 PEP-0571
PEP-0582 PEP-0592 PEP-0599 PEP-0600 PEP-0610 PEP-0621 PEP-0627 PEP-0629
PEP-0631 PEP-0632 PEP-0639 PEP-0643 PEP-0650 PEP-0658 PEP-0660 PEP-0665
PEP-0668 PEP-0685 PEP-0691 PEP-0700 PEP-0708 PEP-0714 PEP-0715 PEP-0723
PEP-0725 PEP-0730 PEP-0735 PEP-0740 PEP-0751 PEP-0752 PEP-0753 PEP-0755
PEP-0763 PEP-0766 PEP-0771 PEP-0777

Chosen as one coherent domain, so questions must discriminate among records
sharing a vocabulary — the thing the generated corpus structurally cannot
test. The slice carries five supersession chains (metadata 1.0→1.1→1.2→2.1,
versioning, PyPI hosting, manylinux, lock files), in-slice `Requires` links,
and seven of the nine statuses; nothing at the pinned commit holds
Provisional, and Active lives only in the meta-proposals outside it.

Several questions are deliberate lifecycle traps: P-12's phrasing sits closer
to the *rejected* lock-file proposal than to the final one that replaced it,
and P-13's answer has three retired predecessors that share its whole
vocabulary. A ranker that ignores what the verdicts said will answer those
questions from records that no longer govern.

P-13's trap earned its keep on the first run: it surfaced two predecessors the
ingest had left governing, because a promotion retires one record per verdict
and PEP-0600 replaces three, and because `Superseded-By` naming a present
successor was trusted to mean the successor would actually retire it
(PEP-0621 declares nothing about PEP-0631). Both fixed in the ingest; the
question passed on the re-run without being touched.

## Known shortfalls, and where each was filed

Five questions carry `pending` markers. The rule from day one: no marker
without an `explain --proposals` run — and this suite's first week showed why
that is necessary and not sufficient. Three questions once filed here as
meaning faults (P-02, P-03, P-17, on real evidence of tight ranking margins)
recovered the moment assembly widened from one document to k (D-0041): their
answers had been at fused ranks one and two all along, graded as never
surfaced because the instrument conflated fused order with the assembled
list. Measurement filed them; a better instrument re-filed them.

- **P-09, P-12, P-16** — the calibration family, and all that remains of
  U-23 here: the right record is surfaced (P-12 at rank two, after two
  earlier filings under U-41 and then U-43 each peeled a real layer) and the
  confidence rule declines it at coverage 0.31–0.48.
- **P-08** — the answering record is Rejected-state, and the default view
  admits promoted claims only: absent from both rankers' candidates by
  design. A fair question about a refusal cannot currently be answered at all
  (U-40).
- **P-22** — bluffs at coverage = reach = 1.00; the corpus speaks every word
  of the question and does not settle it. The lexical margin over the
  runner-up is 1.6%, the first outside-corpus evidence that the margin
  clause in U-38's proposed rule is load-bearing, not decorative (U-38).

## Questions

| id | Question | Expect | Owner | Review trigger |
|----|----------|--------|-------|----------------|
| P-01 | how does a fresh python installation come with pip already available | answer PEP-0453 | Greg Villa | if the slice repins |
| P-02 | what must a project declare before its build tool can run | answer PEP-0518 | Greg Villa | if the slice repins |
| P-03 | what is the binary package format installed without a build step | answer PEP-0427 | Greg Villa | if the slice repins |
| P-04 | how are two release versions compared and ordered | answer PEP-0440 | Greg Villa | if the slice repins |
| P-05 | what does yanking a file from the index mean | answer PEP-0592 | Greg Villa | if the slice repins |
| P-06 | how does an installer record the url a package was installed from | answer PEP-0610 | Greg Villa | if the slice repins |
| P-07 | which fields of pyproject.toml hold a project's metadata | answer PEP-0621 | Greg Villa | if the slice repins |
| P-08 | why was a node modules style local directory turned down | answer PEP-0582 (pending U-40) | Greg Villa | if the slice repins |
| P-09 | how can license terms be stated precisely in package metadata | answer PEP-0639 (pending U-23) | Greg Villa | if the slice repins |
| P-10 | what stops pip writing into an operating system owned environment | answer PEP-0668 | Greg Villa | if the slice repins |
| P-11 | how can an index expose a wheel's metadata without serving the whole archive | answer PEP-0658 | Greg Villa | if the slice repins |
| P-12 | what file format records pinned dependencies for reproducible installs | answer PEP-0751 (pending U-23) | Greg Villa | if the slice repins; the rejected twin is the trap |
| P-13 | which platform tag covers portable linux binaries | answer PEP-0600 | Greg Villa | if the slice repins; three retired predecessors are the trap |
| P-14 | who can take over an abandoned project name on the index | answer PEP-0541 | Greg Villa | if the slice repins |
| P-15 | how are the names of optional extras normalised | answer PEP-0685 | Greg Villa | if the slice repins |
| P-16 | how does a standalone script declare what it needs to run | answer PEP-0723 (pending U-23) | Greg Villa | if the slice repins |
| P-17 | how did the simple repository api gain a json form | answer PEP-0691 | Greg Villa | if the slice repins |
| P-18 | why were egg uploads turned off | answer PEP-0715 | Greg Villa | if the slice repins |
| P-19 | which proposal added the walrus operator | abstain | Greg Villa | if the slice repins to include it |
| P-20 | who sits on the steering council | abstain | Greg Villa | never — governance is outside this slice by design |
| P-21 | what colour is the package index logo | abstain | Greg Villa | never — outside the record by design |
| P-22 | how often are new python versions released | abstain (pending U-38) | Greg Villa | if the slice repins to include the release schedule |
| P-23 | what is the maximum size of an uploaded distribution | abstain | Greg Villa | if an upload-limit proposal enters the slice |
| P-24 | where do installers cache downloaded wheels on disk | abstain | Greg Villa | never — installer internals are outside the record by design |

P-19 to P-21 abstain on vocabulary the corpus lacks; P-22 to P-24 are the
harder case — every word of them appears somewhere in sixty proposals about
uploads, releases, and wheels, and the record still does not settle them.

## Vocabulary baseline

Recorded when the questions were agreed, exactly as in GOLDEN.md: the words of
each question the corpus did not contain. The corpus is pinned, so a drift
alarm here means the slice was repinned — re-read the questions, then
re-record.

Regenerate with `PEP_GOLDEN_BASELINE=1 cargo run -p tacit-keeper --example pep_golden`.

| id | words the corpus did not contain |
|----|----------------------------------|
| P-01 | — |
| P-02 | — |
| P-03 | — |
| P-04 | — |
| P-05 | — |
| P-06 | — |
| P-07 | — |
| P-08 | — |
| P-09 | — |
| P-10 | — |
| P-11 | — |
| P-12 | — |
| P-13 | — |
| P-14 | — |
| P-15 | — |
| P-16 | — |
| P-17 | — |
| P-18 | — |
| P-19 | walrus |
| P-20 | council steering |
| P-21 | logo |
| P-22 | — |
| P-23 | — |
| P-24 | — |

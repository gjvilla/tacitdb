# Tacit — The Four-Rooms Register

The project's map of its own knowledge, kept in the discipline it exists to serve:
known knowns, known unknowns, unknown knowns, unknown unknowns. The register is a
working document with a cadence, not a filing cabinet — it leaks by default and is
re-read on a schedule (see the practices section).

---

## Room 1 · Known knowns

What the project has decided and recorded, with owners and review triggers:

- **[DECISIONS.md](DECISIONS.md)** — eleven founding decisions plus the dated
  success hypothesis H-0001. Each carries its own review trigger.
- **[REQUIREMENTS.md](REQUIREMENTS.md)** — eleven requirements from production scar
  tissue, each with an acceptance criterion, plus the shed list.
- **[priors/](priors/)** — the prior-art survey (one page per engine), filled by
  research as of 2026-08-23. Verdict in [priors/SUMMARY.md](priors/SUMMARY.md).
- **[design/001-data-model.md](design/001-data-model.md)** — the logical data
  model: two ledgers, envelope spec, lifecycle invariants (resolves U-1),
  bitemporality, projected graph, retrieval semantics. Promoted 2026-08-23
  (D-0012, D-0013, D-0014).
- **`crates/`** — the workspace (2026-08-23, per D-0015 and D-0017):
  `tacit-core` (the data model as types — sealed records, derived state,
  verdict grammar, instrument panel, and the projected graph — with the
  invariant suite and U-10 property tests), `tacit-keeper` (corpus parser and
  the `docs/DECISIONS.md` ingest, plus the `dogfood` example),
  `tacit-mcp` (the MCP host: ten typed, audited tools over stdio, per D-0018),
  `tacit-python` (binding shell, no PyO3 yet).
- **The corpus ingests itself.** `cargo run -p tacit-keeper --example dogfood`
  loads this project's own decision records *and this register* into the engine
  and interrogates them: provenance with evidence chains, record-time travel,
  the projected graph, and weighted paths over the instrument panel. It prints
  its own honest position against H-0001(a).
- **[GOLDEN.md](GOLDEN.md)** — the golden suite: representative questions with
  agreed answers, graded by `cargo run -p tacit-keeper --example golden`. It
  scores abstention as a pass and classifies every failure by which room it
  came from, so "retrieval missed what the record holds" and "retrieval
  answered what it does not" never collapse into one accuracy number. Known
  shortfalls must name a registered unknown; a failure with no registered
  cause turns the suite red and fails the build.
- **Trust is asked twice** (2026-08-24, D-0027). What the verdict recorded stays
  as it was; `review_trust` re-asks the repository on demand, and the host raises
  the alarm on the way up. A weakening changes nothing in the record — retiring
  what a revoked key signed is a person's verdict to declare.
- **Whose signature counts** (2026-08-24, D-0026). Only a key this machine's git
  trusts, and only an identity the signature binds — named with `--signed-by`
  from outside the repository, because a list of who may promote cannot live in
  the file it protects.
- **A transcribed verdict says who wrote the prose** (2026-08-24, D-0025).
  `cargo run -p tacit-mcp -- --require-signature .` declines to promote on words
  no signed commit carries; the default records what git can establish and says
  so in the verdict itself, so "which promotions rest on nothing" stays
  answerable long after the ingest that made them.
- **The inbox shows one wording per record** (2026-08-24, D-0024). A draft its
  author already replaced keeps its state and loses its separate place in the
  queue; the count of what was folded comes back beside the list, because a
  queue that quietly drops records is how a reviewer comes to believe they have
  seen everything.
- **A withdrawn question says which kind of withdrawal it was** (2026-08-24,
  D-0023). Reworded questions supersede; the four reasons keep "we asked it
  better" apart from "we stopped asking", and keep "the answer is somewhere
  else" from disappearing into either.
- **The documents stay upstream** (2026-08-24, D-0021). Re-reading them is a
  sync: unchanged records write nothing, edited ones supersede what they
  replace and retire it in the same verdict, and the three things an ingest may
  not decide on its own — a deleted record, a reworded question, a claim a
  person already retired — are reported rather than performed.
- **The record survives the process** (2026-08-23, D-0019). `cargo run -p
  tacit-mcp -- --store <path> .` keeps the ledger on disk, so what an agent
  proposes is still waiting for a person on the next run. Loading replays the
  log through the grammar rather than deserializing it, so a hand-edited store
  cannot smuggle in a promotion.
- **Agents can reach the record** (2026-08-23). `cargo run -p tacit-mcp -- .`
  serves this repository's corpus over MCP. The ratchet is visible in the tool
  surface as an absence: there is no promote tool, so no sequence of calls an
  agent can make moves a claim to promoted. An integration test asserts that.
- **Room 2 is in the ledger, not only in this file** (2026-08-23). Every row
  below is ingested as a gap record carrying its trigger, so the engine can
  answer "that is a registered open question" instead of "nothing found" —
  the raw material for honest abstention. Resolved rows are transcribed as
  `Answer` verdicts naming the promoted claim that settled them, which the
  engine refuses unless that claim really is promoted. Consequence worth
  knowing: this document is now load-bearing for the test suite. A malformed
  row is a hard error, by design.

A known known is only as good as its review trigger — see Room 4 for the backward
door.

## Room 2 · Known unknowns

Named questions without agreed answers. Each has a trigger — the event that forces
the decision — because an unregistered gap is how systems bluff.

| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-1 | ~~Write-path placement~~ **Resolved 2026-08-23** → D-0012: grammar in the engine (invariants in design/001 §3.2), identity/auth in the keeper | — | Storage code unblocked. Kept for the record; history is never rewritten. |
| U-2 | ~~Runtime shape~~ **Resolved 2026-08-23** → D-0015: embedded-first Rust library + Python bindings; an MCP host binary is the only served surface in v1 | — | Multi-app serving reopens later as a keeper-layer wrapper around the same core (see D-0015's trigger). Kept for the record. |
| U-3 | Query language: whether, when, and what shape | Observed real agent usage of the v1 MCP toolset | Deferred, not rejected (D-0007). |
| U-4 | Which graph algorithms live in-engine | Data-model doc + first retrieval implementation | Weighted Dijkstra/Yen's almost certainly (R-5); community detection / centrality unclear. |
| U-5 | ~~Storage layer~~ **Resolved 2026-08-23** → D-0019: an append-only JSON-lines event log, fsynced before the in-memory commit, replayed through the same validation an append runs. No external storage dependency. | — | The event-log candidate won. The load path was the real question, not the format: a store that is re-validated rather than trusted is what keeps the invariants true of records that came off disk. Residual costs are U-24 and U-25. |
| U-6 | Name: trademark counsel review of the live TACIT Class-9 mark before commercial use | Before any commercial use | Narrowed 2026-08-23: registries verified — bare `tacit` taken everywhere, **`tacitdb` clean** (crates.io/GitHub/PyPI; domains likely unregistered, confirm at registrar). Registrable identity `tacitdb`, product name Tacit (D-0011). Counsel item remains. |
| U-7 | IP clarity: employment-agreement invention-assignment reach over a domain-adjacent personal project | Before any public release; ideally sooner | Personal-time intent is not legal protection. Get clarity, ideally written. Flagged, not legal advice. |
| U-8 | Which layer has product pull: engine or keeper | First external adoption/interest signal | The two-layer bet (D-0002) defers this deliberately. |
| U-9 | Seed corpus beyond self-hosting: what public/synthetic corpus exercises the envelope model at realistic scale | Before the golden suite (H-0001c) | Must respect the D-0010 boundary — nothing proprietary. |
| U-10 | ~~Incremental projection maintenance~~ **Resolved 2026-08-23** → D-0016: the index carries no view parameters and nothing is removed from it, so `rebuild` *is* `empty().advance()` — equivalence is definitional. Backed by proptest properties (incremental == rebuild, advance idempotent, index state == ledger state, views never mutate). | — | The residual question is performance, not correctness: buckets accumulate dead slots. Tracked as U-18. |
| U-11 | Redaction vs append-only: a designed legal "remove this" that preserves chain integrity | Before any external or personal-data corpus | Redaction records / crypto-shredding candidates. |
| U-12 | ID & dedup: ULID vs content-addressing; exact-dup grammar vs semantic-dup keeper policy | Data-model implementation | Agents will re-propose duplicates. |
| U-13 | Envelope schema evolution: versioning + migration policy | First needed envelope change | Records carry `envelope_version` from day one. First real precedent arrived 2026-08-24 and was *content*, not envelope: D-0023 added a field to a verdict action, and the migration answer was a read-only value (`unstated`) that names the absence rather than a default that invents a meaning. Whether `envelope_version` should also gate content shape is the open half. |
| U-14 | Bitemporal edge cases: corrections-of-corrections, overlap, precision | Temporal implementation | Property-based tests against a reference semantics required. |
| U-15 | Contradiction detection scope: exact-scope in engine (invariant 7), semantic in keeper | Retrieval implementation | Where grammar ends and meaning begins. v1 core implements exact scope over *attributes* only — relation-predicate cardinality ("can `reports_to` hold twice?") is semantics the engine cannot know, so it stays keeper-side. |
| U-16 | Set verdicts: bulk mechanical ingest (e.g. a system catalog sync of 10^3–10^4 records) cannot take per-record human verdicts — design a verdict that targets a record set / ingestion run and promotes transitively, without weakening invariant 5 | Before the first bulk-ingest corpus; likely a design/001 amendment | Found by mapping a real production workload onto the model, 2026-08-23. Related: staged evaluation ("shadow-running" a proposed record with instruments attached, promotion evidence = its measurements) looks like a keeper pattern the model already supports — confirm when the keeper layer is designed. |
| U-17 | Engine license: Apache-2.0 vs MIT vs dual | Before the repo goes public; interacts with U-6/U-7 | Surfaced while scaffolding (all crates `publish = false` until settled). The priors argue for permissive (fork-safety as the answer to single-vendor fragility). |
| U-18 | Candidate-index compaction: dead slots accumulate because nothing is ever removed from the fold | When a corpus makes the scan cost measurable | The accepted cost of U-10's monotonicity. Fix has a known shape (periodic rebuild, or a compaction pass that drops slots no view can admit) — do not pre-optimize. |
| U-19 | ~~Ingest idempotency~~ **Resolved 2026-08-24** → D-0021: the documents are upstream and a re-ingest is a sync — each source record fingerprinted into its own provenance, so unchanged records write nothing and edited ones supersede what they replace | — | The trigger fired the day durable storage shipped and was not noticed for a session. The interim answer ("ingest into a fresh ledger") had quietly become the opposite failure: the host refused to re-read, so the upstream copy was upstream in name only. Content-addressing per U-12 turned out to be needed at the *document record* level, not the ledger record level. |
| U-20 | Set verdicts vs the transcription cost: the corpus needed two verdicts per record for what a person performed as one editorial act | With U-16 | Concrete evidence for U-16 produced by the first real ingest, not speculation. |
| U-21 | Relation-scope contradictions never surface (invariant 7 covers attributes only) | With U-15 | Raised by adversarial review 2026-08-23 and confirmed as real-but-deliberate: the engine cannot know a predicate's cardinality. Kept visible so it is a decision, not an oversight. |
| U-23 | Retrieval quality: the lexical ranker separates covered from uncovered reliably, but matches words rather than meaning, so two phrasings of one question do not meet. The query-side stopword list is an English crutch that document frequency cannot replace on a small technical corpus — there, function words are *rare*, so IDF rewards them | Before any claim that retrieval is good | **Narrowed 2026-08-23 (D-0020):** the plumbing is done — `Embedder` trait, `VectorIndex`, two rankers through the fusion stage. What remains is a model. The built-in hashing embedder buys robustness to spelling and morphology and one golden question, and cannot buy meaning. **Measured:** its top-hit similarity spans 0.49–0.66 on answerable questions and 0.47–0.60 on unanswerable ones — overlapping, so vector similarity cannot confer confidence here, only raise an offer. Note also that this row is itself part of the corpus: an entry that quotes the phrasing of a test question will rank for it, which is why this one no longer does. **Measured again 2026-08-24, and the failure mode has moved.** It was "the question and the record share no words". It is now also "several records share the same words": G-20 asks which signing keys count and ranks D-0025 above D-0026, which is the record D-0025 was amended to defer to. Adjacent records compete, so lexical ranking degrades faster than the corpus grows — and the wrong answer it reaches for is the superseded one, which is the worst neighbour to pick. |
| U-24 | Snapshots and compaction: replay is O(log) on every open, and the log only grows | When open time or log size becomes uncomfortable | The accepted cost of D-0019. A snapshot must itself be replay-validated or it reintroduces the bypass it exists to avoid — that is the design constraint, not the file format. |
| U-25 | `sync_data` per append is correct but slow for bulk ingest | When a bulk corpus makes ingest time uncomfortable | Batching needs a durability story for the batch boundary: what a caller is promised when a batch is half-written. Interacts with U-16's set verdicts. |
| U-26 | Approximate nearest neighbours: vector search is an exact scan over admitted records | When the corpus makes the scan cost measurable | Exact search is correct and fast at this scale, and it pre-filters rather than post-filters, so it satisfies R-1's semantics. The performance half awaits an index — HNSW with predicate-aware traversal is the shape the priors point at (NaviX, MIT). |
| U-27 | Corpus self-reference distorts its own measurement: a record describing a retrieval failure matches the queries that fail | Whenever the golden suite or the register is edited | Found the hard way — U-23 quoted a golden question's exact phrasing and then outranked the record that question was about. Not fixable in the engine; it is a curation discipline. Worth a note in GOLDEN.md and a check when adding questions. |
| U-28 | ~~The verdict grammar has no supersession path for a gap or a hypothesis~~ **Resolved 2026-08-24** → D-0023: withdrawal carries a reason (superseded, answered elsewhere, no longer relevant, registered in error), a hypothesis gains `abandoned`, and `supersedes` is enforced same-kind | — | The registered shape of the fix was the right one and was followed. Two things it turned up that were not foreseen: the corpus had *already* been recording "resolved, and this ledger does not hold the answer" as plain withdrawal, which is a provenance alarm that had no way to sound; and the append-path-versus-grammar distinction decided where a check belongs for the second time in two days (D-0022 was the first), which makes it a rule rather than a coincidence. |
| U-31 | ~~Whose signature counts~~ **Resolved 2026-08-24** → D-0026: only a key git says this machine trusts (`%G?` = `G`, not `U`), and when the caller names signers, only an identity the signature itself binds — named from outside the repository, because a list of who may promote cannot live in the file it protects | — | Half of it turned out to be already answered and thrown away: D-0025 had collapsed `G` and `U`, which accepts a key an agent minted a second earlier. The endpoint limit the entry predicted stands exactly as predicted, and is stated in D-0026 rather than papered over. What the work added that the entry did not foresee is U-32. |
| U-32 | ~~Trust is a history, not a current view~~ **Resolved 2026-08-24** → D-0027: the recorded attestation stays immutable and `review_trust` re-asks the repository on demand, sorting every promotion into verifying-as-it-did, weakened, strengthened, unverifiable, or naming no commit | — | Both readings now exist and neither replaces the other. Two things the entry did not foresee: *strengthened* is as real as weakened and needed its own column, or the review would read as an error report rather than a measurement; and the review had to be a read and never a write, because a revoked key is not a verdict and no person declared one. The predicted cost holds exactly — an unreachable commit is `unverifiable`, which is a third answer and not a failure. |
| U-30 | ~~A superseded claim that was never promoted is never closed~~ **Resolved 2026-08-24** → D-0024: it stays proposed, correctly — nobody ruled on it — and stops being a *separate* entry in the inbox. `pending_proposals` returns the head of each supersession chain plus the drafts folded behind it, both lists present | — | The fault was in the queue, not the state. Worth recording what was nearly done instead: transcribing a rejection for the replaced draft. It would have looked unremarkable beside the ingest's other transcribed verdicts, and it would have been a human verdict for a record no human read. |
| U-29 | ~~The upstream document is an unaudited promotion channel~~ **Narrowed 2026-08-24** → D-0025: every transcribed verdict now carries what git can establish about who wrote the words asserting it, and `--require-signature` declines to transcribe one that no signed commit carries | Before an agent is given write access to the corpus | Narrowed, not closed, and the residue is named as U-31. The channel is no longer unaudited: a promotion backed by nothing says so in its own author detail, and `unattested_promotions` will list every one of them at any later time. What is still true is that git attests the typist and not the intent, and that this keeper takes git's verdict on a signature without knowing which keys the project trusts. |
| U-22 | ~~A backwards system-clock step makes every `append` fail with no recovery path~~ **Resolved 2026-08-24** → D-0022: small steps hold record-time at the last entry, large ones refuse and name the moment appends resume, and the future-time check moves to the append path so replay stops applying it | — | Understated as registered. Once the log outlived the process, a clock set backwards did not block the next few writes — it made the store refuse to open at all, every record in it now reading as future-dated. Found by a test written for the smaller problem. |

## Room 3 · Unknown knowns

Knowledge in the builder's head but not yet in the record. The founding interview
extracted two — proof the practice works:

- The write-path omission (→ D-0006, U-1): a load-bearing architectural question
  that had never been consciously decided.
- The scar-tissue requirements (→ REQUIREMENTS.md): production lessons that existed
  only as workarounds in application code until asked for.
- The two-ledger tension (→ D-0013, data-model round 2026-08-23): R-5's
  cheaply-mutable weights and D-0004's append-only assertions contradicted each
  other in the founding records themselves — unnoticed until the phase interview
  forced the question. The register audits its own known knowns.

**Standing practice:** an interview-before-phase. Before each new phase (data model,
storage, retrieval, serving), run the veteran question against the builder: *"what
are you already sure of here that is written nowhere?"* Capture answers as decision
records with envelopes.

## Room 4 · Unknown unknowns

Nothing can be listed here — that is the point. What exists instead are practices
that convert surprises into registered entries while they are cheap:

- **Blind-spot pass before each phase.** State experience level honestly, then ask:
  what would someone who has built storage engines / query planners / vector indexes
  inspect, challenge, or measure here? File the answers as U-entries.
- **The priors survey** ([priors/](priors/)) — walking through other builders'
  known-known rooms before trusting this project's map. Reference-class thinking:
  what killed or stalled the engines that came before.
- **Quarterly register re-read.** The backward door is real: promoted decisions go
  stale while the record keeps speaking in the present tense. Every decision's
  review trigger gets checked; anything overtaken by events is retired, not edited
  in place.
- **Golden suite that rewards abstention** (H-0001c). A fluent wrong answer from the
  system is a drift alarm, not just a bug.
- **Pre-publication scrub pass.** Before the repo goes public: run
  `scripts/check-boundary.sh` — the D-0010 boundary as an executable rule
  rather than a remembered one — then check for private-path references that
  leak context, and confirm U-6 and U-7 are resolved. The script matches one
  name case-sensitively because it collides with an ordinary English word.
  That nuance exists because the naive rule cried wolf four times in a single
  day, and an alarm nobody reads is worse than no alarm at all.

---

*Recorded 2026-08-23, amended 2026-08-24. First scheduled re-read: 2026-11-23.
Owner: Greg Villa.*

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
| U-5 | Storage layer: build vs embed (redb / RocksDB / sled / custom) | Implementation phase, after U-1 and U-2 | An event-log + projection design (rejected as the *conceptual* unit in D-0004) remains a live *implementation* candidate. |
| U-6 | Name: trademark counsel review of the live TACIT Class-9 mark before commercial use | Before any commercial use | Narrowed 2026-08-23: registries verified — bare `tacit` taken everywhere, **`tacitdb` clean** (crates.io/GitHub/PyPI; domains likely unregistered, confirm at registrar). Registrable identity `tacitdb`, product name Tacit (D-0011). Counsel item remains. |
| U-7 | IP clarity: employment-agreement invention-assignment reach over a domain-adjacent personal project | Before any public release; ideally sooner | Personal-time intent is not legal protection. Get clarity, ideally written. Flagged, not legal advice. |
| U-8 | Which layer has product pull: engine or keeper | First external adoption/interest signal | The two-layer bet (D-0002) defers this deliberately. |
| U-9 | Seed corpus beyond self-hosting: what public/synthetic corpus exercises the envelope model at realistic scale | Before the golden suite (H-0001c) | Must respect the D-0010 boundary — nothing proprietary. |
| U-10 | ~~Incremental projection maintenance~~ **Resolved 2026-08-23** → D-0016: the index carries no view parameters and nothing is removed from it, so `rebuild` *is* `empty().advance()` — equivalence is definitional. Backed by proptest properties (incremental == rebuild, advance idempotent, index state == ledger state, views never mutate). | — | The residual question is performance, not correctness: buckets accumulate dead slots. Tracked as U-18. |
| U-11 | Redaction vs append-only: a designed legal "remove this" that preserves chain integrity | Before any external or personal-data corpus | Redaction records / crypto-shredding candidates. |
| U-12 | ID & dedup: ULID vs content-addressing; exact-dup grammar vs semantic-dup keeper policy | Data-model implementation | Agents will re-propose duplicates. |
| U-13 | Envelope schema evolution: versioning + migration policy | First needed envelope change | Records carry `envelope_version` from day one. |
| U-14 | Bitemporal edge cases: corrections-of-corrections, overlap, precision | Temporal implementation | Property-based tests against a reference semantics required. |
| U-15 | Contradiction detection scope: exact-scope in engine (invariant 7), semantic in keeper | Retrieval implementation | Where grammar ends and meaning begins. v1 core implements exact scope over *attributes* only — relation-predicate cardinality ("can `reports_to` hold twice?") is semantics the engine cannot know, so it stays keeper-side. |
| U-16 | Set verdicts: bulk mechanical ingest (e.g. a system catalog sync of 10^3–10^4 records) cannot take per-record human verdicts — design a verdict that targets a record set / ingestion run and promotes transitively, without weakening invariant 5 | Before the first bulk-ingest corpus; likely a design/001 amendment | Found by mapping a real production workload onto the model, 2026-08-23. Related: staged evaluation ("shadow-running" a proposed record with instruments attached, promotion evidence = its measurements) looks like a keeper pattern the model already supports — confirm when the keeper layer is designed. |
| U-17 | Engine license: Apache-2.0 vs MIT vs dual | Before the repo goes public; interacts with U-6/U-7 | Surfaced while scaffolding (all crates `publish = false` until settled). The priors argue for permissive (fork-safety as the answer to single-vendor fragility). |
| U-18 | Candidate-index compaction: dead slots accumulate because nothing is ever removed from the fold | When a corpus makes the scan cost measurable | The accepted cost of U-10's monotonicity. Fix has a known shape (periodic rebuild, or a compaction pass that drops slots no view can admit) — do not pre-optimize. |
| U-19 | Ingest idempotency: re-running into a non-fresh ledger duplicates the corpus | Before any durable store exists (interacts with U-5, U-12) | Today's honest answer is "ingest into a fresh ledger", printed loudly rather than silently handled. Content-addressing (U-12) is the likely real answer. |
| U-20 | Set verdicts vs the transcription cost: the corpus needed two verdicts per record for what a person performed as one editorial act | With U-16 | Concrete evidence for U-16 produced by the first real ingest, not speculation. |
| U-21 | Relation-scope contradictions never surface (invariant 7 covers attributes only) | With U-15 | Raised by adversarial review 2026-08-23 and confirmed as real-but-deliberate: the engine cannot know a predicate's cardinality. Kept visible so it is a decision, not an oversight. |
| U-23 | Retrieval quality is lexical only: BM25 separates covered from uncovered but cannot see that "storage engine" and "storage layer" are one question. The query-side stopword list is an English crutch that document frequency cannot replace on a small technical corpus — there, function words are *rare*, so IDF rewards them | Before the golden suite (H-0001c), and before any claim that retrieval is good | The fusion stage exists so vector candidates join the same plan (R-2). Shape, filters, outcome tags and abstention are settled and tested; only the candidate source is thin. **Measured 2026-08-23:** the golden suite scores 10/14 with 4 shortfalls, all this cause — two under-confident (right record at rank 1, declined to call it a match), one bluff, one spelling variance ("licence" vs "license"). That last is close to the shortest possible demonstration of the limit. |
| U-22 | A backwards system-clock step makes every `append` fail with no recovery path | Before durable storage (U-5) | The monotonicity guard that makes `state_of_at` sound has no escape hatch. Confirmed by review; deferred because the alternative (accepting backwards time) breaks bitemporal reads, and the real fix belongs with the storage layer's clock story. |

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
- **Pre-publication scrub pass.** Before the repo goes public: verify the D-0010
  boundary holds everywhere (no employer identifiers, no private-path references
  that leak context), and U-6/U-7 are resolved.

---

*Recorded 2026-08-23. First scheduled re-read: 2026-11-23. Owner: Greg Villa.*

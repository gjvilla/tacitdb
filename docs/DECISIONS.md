# Tacit — Decision Records

Founding decisions from the knowns-and-unknowns exercise of 2026-08-23 (a three-round
interview run with the four-rooms discipline: known knowns, known unknowns, unknown
knowns, unknown unknowns).

Every record below is authored in the **assertion + envelope** shape that Tacit itself
will store — this file is the engine's first corpus, written before the engine exists.
Envelope fields: `id`, `state` (proposed → promoted → retired), `author`, `recorded`
(record-time), `valid_from` (valid-time), `source`, `evidence`, `review_trigger`.
A record is *promoted* only by the owner's explicit verdict; the machine proposes, it
never decides.

---

## D-0001 · The forces driving the build

```yaml
id: D-0001
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 1
evidence: [REQUIREMENTS.md, REGISTER.md]
review_trigger: any force resolved externally — e.g. incumbent licensing clarified,
  or an incumbent ships engine-level provenance/temporal
```

**Assertion.** Four forces jointly motivate replacing an incumbent property-graph
database with a purpose-built engine: (1) licensing/cost friction, (2) operational
weight (server babysitting, plugin deploys, retry choreography), (3) AI-native
mismatch (provenance, temporal state, and hybrid retrieval must be hand-rolled in
application code), (4) ownership/mastery — the knowledge corpus is a strategic asset
whose backbone should be deeply understood, and building it is itself the mastery.

**Forces.** All four were selected; none alone would justify the build. (2) and (3)
are corroborated by production scar tissue (see REQUIREMENTS.md); (4) is the one that
survives even if the others are externally resolved.

---

## D-0002 · Identity: product seed, two-layer bet

```yaml
id: D-0002
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / rounds 1–2
review_trigger: U-8 in REGISTER.md — first external signal of which layer has pull
```

**Assertion.** Tacit is a product seed, not internal tooling or a pure learning
exercise. The bet is two-layered: an open-source engine as the foundation and
credibility play, with a "keeper" product (organizational memory as software —
curation, verdicts, drift detection) as the eventual commercial layer on top.

**Alternatives rejected.** Engine-only infrastructure product (trust moat too slow
alone); keeper-only with an adopted engine (surrenders force 4); undecided (would
have deferred rigor decisions that need making now).

---

## D-0003 · V1 workload: the keeper corpus

```yaml
id: D-0003
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 2
review_trigger: if the keeper corpus cannot produce a measurable v1 milestone
  (see H-0001), revisit against the measurable-incumbent-workload alternative
```

**Assertion.** Tacit v1 is designed against a new, thesis-native workload — a corpus
of assertions with provenance, verdicts, temporal state, and drift triggers — rather
than replicating any existing document-RAG or schema-graph workload. Consequence:
the design burden is "get the knowledge model right," not "beat the incumbent at X."

**Forces.** Product seed (D-0002) favors the differentiated workload; the absence of
an incumbent to benchmark against is the accepted cost, mitigated by H-0001.

---

## D-0004 · Unit of memory: assertion + envelope

```yaml
id: D-0004
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 3
evidence: [this file — every record here is one]
review_trigger: data-model design doc (next session) must confirm the envelope
  fields are sufficient and minimal
```

**Assertion.** Tacit's atomic record is an **assertion** (a claim) wrapped in a
**required envelope**: source, author, valid-time, record-time, evidence links,
review trigger, lifecycle state. Content stays flexible; the envelope is
non-negotiable schema. A bare fact with no envelope is not storable.

**Alternatives rejected.** Plain property graph (maximum generality, but then
"AI-native" reduces to indexes and the engine carries none of the thesis);
pattern-only unit (solution + forces — too specialized for the engine layer; it
becomes a *content type* of assertion instead); event-log-with-projection (strong
on audit, deferred as an implementation candidate rather than the conceptual unit).

---

## D-0005 · AI-native means in-engine

```yaml
id: D-0005
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 1
review_trigger: H-0001 falsifier — if these still need app-layer workarounds at
  score time, this decision was wrong
```

**Assertion.** Three capability families live in the engine, not the application
layer: (1) provenance & verdicts — queryable, schema-enforced; (2) hybrid retrieval
as one primitive — vector + lexical + traversal + rank fusion in a single plan;
(3) temporal/drift state — valid-time vs record-time, as-of queries, staleness and
review triggers as first-class, queryable state.

**Forces.** Every one of these is being hand-rolled today in application code around
an incumbent engine — the strongest evidence they belong below the app layer.

---

## D-0006 · Write-path placement must be designed before storage code

```yaml
id: D-0006
state: promoted        # the *decision to decide* is promoted; the placement itself is open (U-1)
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 3 — surfaced as an honest oversight
review_trigger: resolved by the data-model design doc (next session); no storage
  code before it
```

**Assertion.** The agent write-path — proposed → promoted → retired lifecycle, with
verdict provenance required for transitions — was initially left out of the engine's
scope by oversight, not by decision. Whether the ratchet is engine schema, neutral
engine primitives, or application convention is now registered unknown **U-1**, and
it blocks storage-engine code. The founding principle it must honor: *commitments
erode unless wired into structure; the machine proposes, it never decides.*

**Resolution (2026-08-23).** U-1 settled by D-0012 and
[design/001-data-model.md](design/001-data-model.md). Storage code is unblocked.

---

## D-0007 · Interface: typed API + MCP tools; no query language in v1

```yaml
id: D-0007
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 3
review_trigger: U-3 — revisit after observing real agent usage of the MCP toolset
```

**Assertion.** Tacit v1 exposes a typed Rust/Python API plus an MCP toolset as its
only interfaces. Agents get constrained, auditable operations; humans get whatever
the keeper layer builds. Query-language design is deferred until real agent usage
exists to inform it.

**Alternatives rejected.** Cypher-compatible subset (inherits the semantics being
escaped, plus parity expectations that take years); new agent-first language in v1
(a language is a second product with its own adoption problem).

---

## D-0008 · Prior art: build anyway, survey in parallel

```yaml
id: D-0008
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 2
evidence: [docs/priors/]
review_trigger: if the survey finds an engine that satisfies REQUIREMENTS.md
  outright, the build-vs-adopt question reopens honestly
```

**Assertion.** The prior-art field (HelixDB, Kuzu, CozoDB, SurrealDB, FalkorDB,
Memgraph, LanceDB) was an unknown unknown at project start — now converted to a
registered survey running in parallel with design work. Building proceeds regardless
(force 4), but the survey must produce the honest "why not X" answer a product needs,
and design ideas worth stealing.

**Survey verdict (2026-08-23).** The trigger fired and the decision holds: no
surveyed engine satisfies REQUIREMENTS.md outright — none combines bitemporal +
native provenance + weighted traversal + fused hybrid retrieval
([priors/SUMMARY.md](priors/SUMMARY.md)). Notable: Kuzu died upstream via Apple
acqui-hire (2025-10) — a reference-class warning that single-vendor engines are
fragile on both sides of the build/adopt line; HelixDB is the closest living
competitor (could serve if the provenance + bitemporal wedge were dropped — it is
not); CozoDB (valid-time), NaviX/LadybugDB (pre-filtered HNSW, MIT), LanceDB
(embedded MVCC shape), and Memgraph (planner-level weighted paths) supply the
steal list.

---

## D-0009 · Runtime shape: deferred with a trigger

```yaml
id: D-0009
state: promoted        # the deferral is decided; the shape itself is open (U-2)
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 2
review_trigger: U-2 — decide after the keeper data-model draft and the prior-art
  survey are both in hand
```

**Assertion.** Embedded-library vs server (vs embedded-core-plus-thin-server) is
deliberately undecided. Noted for the record: several of the worst operational scars
with the incumbent (retry storms, connection pools, plugin deploys) are artifacts of
the client-server model itself, which biases — but does not decide — toward an
embedded core.

**Resolution (2026-08-23).** U-2 settled by D-0015: embedded-first with an MCP host
binary as the only served surface in v1.

---

## D-0010 · Ownership: personal project, own time, hard boundary

```yaml
id: D-0010
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: founding-interview / round 3
review_trigger: U-7 — employment-agreement invention-assignment clarity; revisit
  this record if that review changes anything
```

**Assertion.** Tacit is a personal project built on personal time. Hard boundary:
no employer code, data, or confidential identifiers flow into this repository — ever.
Professional experience informs *generic* requirements (REQUIREMENTS.md states them
without attribution); seed corpora are Tacit's own artifacts or public content.

**Forces.** Clean separation is what makes the two-layer product bet (D-0002)
possible at all; it also constrains v1 to self-hosted and synthetic corpora, which
D-0003 already implies.

---

## D-0011 · Name: Tacit

```yaml
id: D-0011
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: naming round, post-plan
review_trigger: trademark counsel review before any commercial use (registry
  verification completed 2026-08-23 — see below); a counsel blocker reopens this
  record
```

**Assertion.** The project is named **Tacit** — Polanyi's term for the knowledge the
system exists to capture: the articulable-but-never-asked fraction of what
practitioners know. The working name `gdb` is retired (collision with the GNU
debugger). Registrable identity: **`tacitdb`**; "Tacit" remains the spoken/product
name.

**Verification (2026-08-23).** Bare `tacit` is taken on every registry: crates.io
(newtype-macro crate, 2024), GitHub user, PyPI (active pipeline library, v0.3.0
2026-04), npm (point-free JS library). `tacitdb` is clean on crates.io, GitHub, and
PyPI; tacitdb.com / tacitdb.dev do not resolve (likely unregistered — confirm at a
registrar); tacit.dev is parked for sale. Trademark: one **live** TACIT registration
in Class 9 (TacitWear Inc., reg. 6053870, 2020 — industrial AR/AI software) plus two
pending 2025 applications; the old Tacit Software (Oracle, 2008) marks appear dead.
The fallback clause fired: `tacitdb` is the registrable identity, and counsel review
of the live Class-9 mark is required before commercial use (registered as the
residual of U-6).

**Alternatives rejected.** Jidoka (deepest concept fit — automation that stops and
calls the human — but requires a told story every time); Lore (warmest, but a crowded
namespace); Engram (best unit-of-memory metaphor, unwanted baggage); bench: Yokoten,
Andon, Cairn, Trellis, Quipu, Mneme, Scholia.

---

## D-0012 · Write-path: grammar in the engine, truth in the keeper

```yaml
id: D-0012
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: phase interview / data-model round
evidence: [design/001-data-model.md §3]
review_trigger: any storage implementation that cannot honor invariants 1–8 of
  design/001 §3.2 reopens this record
```

**Assertion.** The propose → promote → retire ratchet lives in the engine as
*structural grammar*: no envelope no write; append-only; state changes only by
verdict; promotion/retirement verdicts must declare human authorship; agents can
propose but never promote; contradictions surface rather than resolve silently.
Authentication and authorization of *who* counts as the human (identity, roles,
huddles) belong to the keeper layer. The engine cannot verify truth, but it makes
the record structurally unable to lie about its shape — this resolves U-1.

**Alternatives rejected.** Full engine enforcement (drags org identity/auth into a
v1 storage engine); neutral primitives (the ratchet becomes a convention again —
exactly what claim 6 predicts will erode); app convention only (contradicts the
founding principle outright).

---

## D-0013 · Two ledgers: knowledge and instruments

```yaml
id: D-0013
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: phase interview / data-model round — resolved a latent conflict between
  R-5 (cheaply mutable weights) and D-0004 (append-only assertions)
evidence: [design/001-data-model.md §1.3]
review_trigger: if the instrument panel starts accumulating anything a human would
  need to adjudicate, the boundary is drawn wrong — reopen
```

**Assertion.** Tacit separates the **governed ledger** (claims, gaps, hypotheses,
verdicts — append-only, envelope-required, verdict-gated) from the **instrument
panel** (measurements — machine-owned, mutable in place, no ceremony: edge weights,
success rates, usage counts). Embeddings belong to neither: they are derived index
artifacts, rebuildable, never authoritative. Agents update instruments freely and
can only *propose* knowledge — the graph learns nightly without convening a huddle
over a decimal.

**Alternatives rejected.** Everything-is-an-assertion (verdict semantics for
numbers no human will adjudicate, plus append volume); weights-on-projection-only
(an ungoverned shadow store the record cannot explain — how drift hides).

---

## D-0014 · Graph shape: entities + assertions, projected graph

```yaml
id: D-0014
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: phase interview / data-model round
evidence: [design/001-data-model.md §1.1, §5]
review_trigger: U-10 — if incremental projection maintenance cannot be proven
  equivalent to deterministic rebuild, revisit the projection design
```

**Assertion.** Two layers: **entities** are stable identity anchors; **assertions**
are enveloped claims about entities and their relations. The traversable "current
graph" is a **projection** of promoted, currently-valid claims — entities as nodes,
relation-claims as edges, attribute-claims as properties, measurements overlaid —
so conflicting claims, corrections, and as-of views are natural, and traversal
stays fast on the materialized view.

**Alternatives rejected.** Property-graph-with-envelopes (conflicting claims about
one edge force duplicate edges; bitemporal corrections get awkward); pure assertion
graph (entity resolution loses its anchor; every query pays reification overhead).

---

## D-0015 · Runtime shape: embedded-first, MCP host as the only served surface

```yaml
id: D-0015
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: phase interview / U-2 verdict round (trigger fired: data model + priors
  survey both in hand)
evidence: [design/001-data-model.md, priors/SUMMARY.md, REQUIREMENTS.md R-9]
review_trigger: the first genuine multi-app sharing need, or the keeper product's
  serving-layer design — whichever comes first reopens the serving question as a
  wrapper around the same core, never as a second engine
```

**Assertion.** The Tacit engine is an in-process Rust library with Python bindings.
One small binary embeds the library to speak MCP (stdio/HTTP) — the only served
surface in v1, hosting the typed, audited toolset (D-0007/R-11). One process owns a
store at a time (file-lock semantics); intra-process concurrency control belongs to
U-5. There is no wire protocol, no driver, no connection pool — the R-9 failure
class (retry choreography, pool tuning, plugin deploys) becomes structurally
impossible rather than carefully mitigated.

**Forces.** The scar tissue is unanimous that client-server failure modes dominated
operations; the surveyed Apache-2.0 survivors are embedded-first while the
server-shaped priors carry the ops weight; the v1 keeper corpus is single-tenant;
the AI-stack adoption pattern the two-layer bet needs is "pip install, no ops."
Agent access still requires a running process — the MCP host is that process, and
it is a host, not a database server.

**Alternatives rejected.** Full thin server in v1 (auth + wire-protocol + deploy
surface before any workload demands it); server-first (rebuilds the escaped
operational surface); further deferral (the trigger had fired and both inputs were
in hand).

---

## D-0016 · The projection is a caller-held view; time is a read parameter

```yaml
id: D-0016
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: projection design round — amends design/001 §5 before implementation
evidence: [design/001-data-model.md §5]
review_trigger: a workload that genuinely needs a shared, engine-owned
  materialized graph reopens this — as a keeper-layer cache over the same
  index, never as engine state
```

**Assertion.** Three amendments to design/001 §5, made before the code was
written rather than discovered after: the projection is a value the caller
holds, not engine state; the write path holds no reference to any projection,
by construction; and valid-time is a read parameter that never enters the
maintained index. The maintained structure is a *candidate index* — a monotone
fold over the log from which nothing is ever removed — so `rebuild` is defined
as `empty().advance()` and U-10's equivalence is definitional rather than
hoped for.

**Forces.** A ledger-owned projection hands U-5's storage layer a
cache-coherence problem it has not agreed to solve; a write path that can see a
view can validate a verdict against a stale one, which in an append-only log is
permanent corruption rather than a recoverable cache bug; and materializing
valid-time would turn a pure fold into a cache with a TTL, because expiry
happens with no append to trigger maintenance.

**Alternatives rejected.** The original §5 text ("the engine maintains the
default projection incrementally", with non-default views computed on demand)
— superseded because it left valid-time inside the maintained state, which is
exactly what makes incremental maintenance unprovable. Amending the document
rather than quietly outgrowing it is the point: shipping code that contradicts
a promoted design record is the erosion mode D-0006 names.

---

## D-0017 · Crate topology: a fourth crate for the keeper layer

```yaml
id: D-0017
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: ingest implementation round — extends the topology named in D-0015
review_trigger: any crate that would need to depend on tacit-keeper from below
  means the boundary is drawn wrong — reopen
```

**Assertion.** The workspace has four crates, not the three D-0015 named:
`tacit-core` (grammar: the ledger, the invariants, the projection — no file
I/O, no markdown, no Python), `tacit-keeper` (content: corpus parsing,
ingestion, and editorial judgment), `tacit-mcp` (the host binary), and
`tacit-python` (the PyO3 shell). The engine/keeper split of D-0002 becomes a
compile-time boundary rather than a code-review convention.

**Forces.** Markdown parsing, filesystem access, and any opinion about what a
decision record *means* are keeper concerns; letting them into the engine would
make "AI-native" a matter of what the engine happens to know about one
document format. The precedent already existed in `tacit-python`, which was
split out for the same reason before it had a line of code in it.

---

## D-0018 · The MCP host takes the official SDK

```yaml
id: D-0018
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: mcp host implementation round
evidence: [REQUIREMENTS.md R-4]
review_trigger: if the SDK stalls upstream, or its transitive weight reaches
  tacit-core, reopen — a hand-rolled JSON-RPC transport stays a viable fallback
  because the tool surface is already independent of it
```

**Assertion.** `tacit-mcp` depends on `rmcp`, the official Rust MCP SDK
(Apache-2.0), and through it on tokio. The engine does not: `tacit-core` keeps
its four small dependencies, and the protocol lives entirely in the host crate.

**Forces.** R-4's "no plugin landmines" is about *runtime*-loaded extensions
that can be absent in production, not about compile-time dependencies; a
statically linked crate cannot fail to be installed on the target. Against
that, hand-rolling MCP means owning handshake, capability negotiation,
protocol-version selection and error codes — protocol compliance is exactly
the kind of work a reference implementation should do. D-0017's crate boundary
is what makes the trade safe: the dependency is quarantined one crate away
from the grammar.

**Alternatives rejected.** Hand-rolled JSON-RPC over stdio (compliance risk for
no benefit the project can bank); an HTTP server (D-0015 already settled that the
host is a host, not a database server).

---

## D-0019 · Durability: an append-only log, replayed through the grammar

```yaml
id: D-0019
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: storage round — resolves U-5
evidence: [design/001-data-model.md §3, REQUIREMENTS.md R-3]
review_trigger: when replay time or log size makes snapshots necessary
  (U-24), or when a workload needs random access the log cannot serve
```

**Assertion.** The store is an append-only log of *events* — an entity, a
record append, or a measurement — one JSON object per line, fsynced before the
in-memory commit. Opening a ledger replays the log through **the same
validation an append runs**: evidence must still resolve to a source, entity
refs must still exist, and a verdict must still be legal against the state the
earlier events built. Nothing is deserialized into a sealed type. There is no
external storage dependency.

**Forces.** The load path was the whole problem, and the veteran review named
it before any code existed: `Record` and `Envelope` deliberately have no
`Deserialize`, so the obvious move — read records off disk — would have made
every invariant true only of records that arrived through `append`. Since a
durable store is where records spend most of their life, the ratchet would
have ended up guarding an empty doorway. Validated replay closes that: **the
store is not trusted, it is re-validated.** A hand-edited log cannot promote
anything, because promotion is not a field anyone can write — it is a fold
over verdicts, and a forged verdict has to survive the same grammar a live one
does. Two tests hold that property down.

The format follows from the model rather than being chosen against it: the
governed ledger is already an append-only log, and derived indexes are already
rebuildable (§6), so there is nothing to persist but the events.

**Alternatives rejected.** An embedded key-value store (redb, RocksDB, sled)
— it would store records, which reintroduces the bypass unless replay
validates anyway, and adds a dependency plus a format the project does not
control; a custom binary format (no advantage over JSON lines at this scale,
and the log stops being readable with `tail`); trusting the store and
validating only on write (the failure this decision exists to prevent).

**Accepted costs, both registered.** Replay is O(log) on every open, so a large
corpus will eventually want snapshots (U-24). And `sync_data` per append is
correct but slow in bulk (U-25).

---

## D-0020 · Vector candidates: the engine owns the index, never the model

```yaml
id: D-0020
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: retrieval round — narrows U-23
evidence: [design/001-data-model.md, GOLDEN.md]
review_trigger: when a model whose similarity separates answerable from
  unanswerable questions is plugged in, revisit whether similarity may confer
  confidence
```

**Assertion.** The engine defines an `Embedder` trait and owns a `VectorIndex`
keyed by model id; it never owns a model. Vector candidates join lexical ones
through the fusion stage that was built for them. A built-in hashing embedder
over character n-grams ships as the default — deterministic, dependency-free,
no network — and is documented for what it is: robustness to spelling and
morphology, not meaning.

**Forces.** A vendor model inside the engine would make the corpus hostage to
it, which contradicts the reason the record is kept separately from the voice
that reads it. Embeddings were already specified as derived artifacts keyed by
model id, so swapping models rebuilds an index and moves not one governed
record. And a network dependency would have made the tests and the demo
unrunnable without a key, against R-9.

**Similarity may raise a question; it may not assert an answer.** This was
measured, not assumed. Across the golden questions the hashing embedder's
top-hit similarity spans 0.49–0.66 for answerable questions and 0.47–0.60 for
unanswerable ones — overlapping ranges, so no threshold separates them, and
any vector-derived confidence would be fitted noise. The first attempt did let
similarity confer confidence, and the suite caught it immediately: two
calibration failures fixed, three abstentions destroyed, net worse. Confidence
therefore stays on the lexical coverage signal, which does discriminate, while
similarity is allowed the weaker act of surfacing an open question the reader
can dismiss.

**What it bought, honestly.** One golden question, by improving the ranking
rather than the confidence rule: 10/14 to 11/14, with abstentions intact and
no regressions. It did not close U-23. A model that can see meaning remains
the gap, and the trait is how it arrives.

**Alternatives rejected.** An embedding API (network, key, and a vendor in the
engine); a local neural model (a heavy dependency and model weights in a
repository that has neither); shipping no vectors until a real model exists
(the plumbing is the part worth having in place, and building it revealed the
confidence finding above, which is worth more than the ranking improvement).

---

## D-0021 · Re-ingest is a sync: the documents stay upstream

```yaml
id: D-0021
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: storage round — resolves U-19
evidence: [docs/DECISIONS.md — this file is the upstream copy, design/001-data-model.md §3.1]
review_trigger: when a corpus arrives that has no stable per-record identity in
  its source document, or when bulk ingest (U-16) needs set verdicts
```

**Assertion.** `docs/DECISIONS.md` and `docs/REGISTER.md` are the copies a
person edits, and the ledger is downstream of them. Loading them a second time
is therefore a sync, not a load. Each source record is fingerprinted into its
own provenance, so a later run can tell three cases apart: a record the ledger
has never seen is appended; a record it holds word for word writes nothing at
all; and an edited record is appended superseding its predecessor, with the
document's `state:` promoting the new wording and retiring the old in one
verdict.

**Forces.** Once the ledger survives the process (D-0019), a re-run is the
normal case rather than an exotic one, and the two obvious behaviours are both
wrong: appending everything again duplicates the corpus, and refusing to
re-read freezes the ledger at whatever the documents said the first time. The
second is what the code did, which meant the upstream copy was upstream in
name only.

**Identity lives in the envelope, not in a side table.** The stable name and
the fingerprint go into `SourceRef.reference`, which already exists to say
where a record came from. No new engine concept, no state carried between
runs, and the provenance a reader sees is the same string the sync parses. A
reference this keeper did not write parses to nothing and is left alone — the
sync only claims what it can prove it wrote.

**One editorial act, one verdict.** An edited record uses `Promote { retiring }`
exactly as design/001 §3.1 intended: promoting the new wording *is* retiring
the old, not two decisions that happen to agree.

**Three things it reports and will not do.** A record that has vanished from
the document stays as it is — deleting a paragraph is not a person retiring a
decision, and the document no longer contains the words that would say so. A
reworded *question* — a register row still open, or a hypothesis not yet scored
— keeps the wording the ledger has, because the grammar can supersede a claim
and has nothing to say about either of those (U-28); appending anyway would
leave two live registered questions where the document asks one. And a claim a
person has retired is not resurrected by the
document still reading `promoted`; the disagreement is announced. Each of the
three is a verdict, and verdicts are human acts.

**The fingerprint is a change detector, not a tamper seal.** It is a
non-cryptographic hash, deliberately: anyone who can craft a colliding edit to
the document can also simply write what they like in it. Which is its own
finding — see U-29.

---

## D-0022 · A clock that holds the line, and a log that outlives it

```yaml
id: D-0022
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: storage round — resolves U-22
evidence: [design/001-data-model.md §3.2, docs/REGISTER.md]
review_trigger: when more than one process writes one log, or when record-time
  must be comparable across machines
```

**Assertion.** What `state_of_at` needs is that record-time never moves
backwards within the log — not that it agrees with the wall clock. So a
backwards clock step small enough to be clock discipline holds record-time at
the last entry and counts the hold; a step large enough to mean the clock is
simply wrong refuses, and the refusal names the moment appends resume. The
check that a record may not claim a time later than now is an append-path
guard and not a rule of the grammar, so replay does not apply it.

**Forces.** The monotonicity guard was sound and had no way out: once the
machine's clock stepped back, every append failed until the clock caught up,
with nothing in the error to say so. Absorbing every backwards step would have
been worse — stamping records from a clock known to be wrong buys convenience
with provenance.

**The larger half was invisible until durability existed.** A log whose entries
were written before the clock stepped back reads, afterwards, as a log full of
future record-times. Under the old rule replay refused every one of them, so a
wrong clock did not cost the next few writes — it cost the whole store, which
would not open at all. That is the actual repair here. Nothing is given up:
replay still enforces monotonicity, which is the property the temporal reads
rest on.

**Surfaced, not corrected.** Opening a log that leads the clock reports how far
by, because it is the one fact that explains the next refused append.

---

## D-0023 · A withdrawn question says which kind of withdrawal it was

```yaml
id: D-0023
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: grammar round — resolves U-28
evidence: [design/001-data-model.md §3.1, docs/REGISTER.md]
review_trigger: when a fifth reason is wanted, or when set verdicts (U-16) need
  to close many questions at once
```

**Assertion.** A question that leaves the register unresolved carries a reason,
the way a retired claim always has: superseded, answered elsewhere, no longer
relevant, or registered in error. A hypothesis gets the same door under the name
`abandoned`. A reworded question is therefore expressible: the new wording is
registered carrying a `supersedes` link, and the predecessor is closed as
superseded.

**Forces.** Without the reason, "we asked it better" and "we stopped asking" are
the same recorded event, and a reader looking for drift cannot tell a rewording
from a retreat. Three of the four reasons carry a distinct meaning to that
reader; the fourth carries an alarm. *Answered elsewhere* says the answer exists
and this ledger does not hold it, which is precisely the knowledge a keeper is
built to capture — the corpus was already recording it, indistinguishably, as
plain withdrawal.

**Two records, not one, and the asymmetry is structural.** A claim's replacement
is promoted and its predecessor retired in a single verdict, because promotion
*is* a verdict and the two transitions fold together. A gap is registered on
append; there is no verdict to fold into. So its supersession is the successor
carrying the link plus one verdict closing the predecessor. Reading that
asymmetry as a defect would mean inventing a promotion step for questions, which
would say that a registered question needs ratifying — and it does not.

**Abandoned is not falsified.** Stopping a prediction and finding it false are
different findings, and a count of falsified hypotheses that quietly includes the
abandoned ones is a wrong count. Separate state, separate verdict.

**Supersession is same-kind, and enforced.** A claim does not replace a question
and a question does not replace a claim. The link is where supersession lives —
the reason on the verdict names the *shape* of the change, and the successor's
own envelope names the record — so it has to be worth trusting.

**One reason may be read and never written.** Logs written before this decision
have no reason at all, and they load as `unstated` rather than being assigned a
meaning after the fact. The engine refuses to write it, which is what keeps it
readable as "this predates reasons" instead of becoming a way to decline to give
one. That check sits on the append path and not in the grammar, for the same
reason D-0022 moved the future-time check there: it is illegal to *claim*, not
illegal to *hold*. Twice now that distinction has decided where a check belongs,
which is worth remembering as a rule rather than rediscovering a third time.

---

## D-0024 · A draft its author replaced is not a second thing to review

```yaml
id: D-0024
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: grammar round — resolves U-30
evidence: [design/001-data-model.md §3.2, docs/REGISTER.md]
review_trigger: when an author wants to retract a proposal outright rather than
  replace it, or when set verdicts (U-16) need to clear many drafts at once
```

**Assertion.** A proposed claim that a later record supersedes keeps its state —
it is still proposed, because nobody ruled on it — and stops being a separate
entry in the keeper's inbox. `pending_proposals` returns the head of each
supersession chain alongside the drafts folded behind it, both lists present, so
nothing is filtered away silently.

**Forces.** The complaint in U-30 was that a reviewer seeing two wordings of one
record cannot tell which the document currently says. That is a fault in the
queue, not in the state. The state is honest: an author editing their own
unreviewed draft is not a verdict, and invariant 4 is right to refuse to let it
act like one.

**The tempting wrong fix was a transcribed rejection.** Rejecting means a person
looked at something and said no. Nobody looked. Manufacturing a human verdict
for a record no human judged is precisely the forgery the ratchet exists to
prevent — and it would have been easy, because the ingest already transcribes
verdicts and one more would not have looked out of place.

**Nothing is filtered silently.** Both lists come back, and the MCP tool reports
the folded count beside the listed one. A queue that quietly drops records is
how a reviewer comes to believe they have seen everything.

**A rejected replacement does not revive what it replaced.** The reviewer said no
to the wording the author stands behind; that is not a yes to the one they
abandoned. Only the author can put it back, by proposing it again.

**A fork is surfaced, not prevented.** Two records may declare they replaced the
same one. `replaced_by` returns both and the queue lists both — the same posture
invariant 7 takes toward contradictions, for the same reason: the engine does not
get to pick.

**Only claims needed this.** A superseded gap is withdrawn and a superseded
hypothesis abandoned (D-0023); both leave the live set by a verdict of their own.
A claim cannot, because the verdict that retires a predecessor is
`Promote { retiring }` and it can only retire one that reached promoted. A draft
replaced before anyone read it never did.

---

## D-0025 · A verdict transcribed from prose says who wrote the prose

```yaml
id: D-0025
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: keeper round — narrows U-29
evidence: [design/001-data-model.md, docs/REGISTER.md]
review_trigger: when a second person edits the corpus, when an agent is given
  write access to it, or when the keeper needs a real identity registry
```

**Assertion.** Every verdict the ingest transcribes from a document carries what
git can establish about who put those words there, written into the verdict's
own `Author.detail` and therefore into the permanent record. A policy chooses
what to do with that: `Observe` records it and carries on; `RequireSignature`
declines to transcribe a verdict whose text no signed commit carries, leaving
the claim proposed.

**Forces.** Invariant 5 says the engine enforces the declaration and the keeper
authenticates it (D-0012). The keeper did not authenticate anything. It read
`state: promoted` out of prose and asserted that the named person had declared
a promotion — so write access to `docs/DECISIONS.md` was promotion authority,
in a system whose MCP surface deliberately has no promote tool at all. D-0021
made that live by making re-ingest routine.

**What git can attest is the editor, not the decider.** A person may record a
decision someone else made, so the document's `author:` is whoever decided and
the commit's author is whoever typed. Requiring them to match would break the
ordinary case. The typist is the one worth attesting anyway, because the typist
is who the threat is about.

**The weakest line governs the record.** A record's current text may come from
several commits, and the question is whether anything untrusted could have been
slipped into it — so one unsigned line makes the whole record unsigned, however
well attested the lines around it. Those words did get there unsigned.

**Written into the record, not merely checked at the door.** A check that gates
and leaves no trace cannot answer the question afterwards. Because the
attestation is envelope data it replays through the grammar with everything
else, and `unattested_promotions` can ask, at any later time, which promoted
claims rest on nothing — including ones transcribed before any policy existed,
and ones that will stay unattested forever because they were made while the
document was still a draft. **The policy governs new transcriptions, never old
ones**, which is why the read matters more than the gate.

**The default observes rather than obstructs.** A working tree mid-edit is the
normal state of a document its author is writing, and refusing to ingest over
uncommitted work would make the strict setting the only usable one and then get
switched off. `RequireSignature` is for the case the register named: a corpus
something other than a person can write to.

**What this does not do.** It does not establish that the signer intended the
promotion — an agent running on an unlocked machine can sign as its owner, and
no signing scheme survives a compromised endpoint.

*Amended 2026-08-24 (D-0026).* As first written this record went on to say that
it did not know which keys the project trusts, and took git's verdict as given.
That was true of the code and untrue of git: `%G?` had always separated a key
this machine's keyring vouches for from one that merely arrived with the commit,
and this record's implementation collapsed the two. D-0026 stops collapsing them
and adds the second half — which identities may carry verdicts, named from
outside the repository. The endpoint limit in the paragraph above still stands
and always will.

---

## D-0026 · Whose signature counts, and where that answer is allowed to live

```yaml
id: D-0026
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: keeper round — resolves U-31
evidence: [docs/REGISTER.md, docs/DECISIONS.md — this file is the corpus in question]
review_trigger: when a signature must be weighed again after the key that made
  it is revoked, or when signers need roles rather than a flat list
```

**Assertion.** A signature counts when git says the key is one this machine
trusts — not merely one it can verify — and, when the caller names signers, when
the identity the signature binds is one of them. The names come from the caller
and never from the repository.

**A trust root inside the thing it protects is not a trust root.** The obvious
design was a `SIGNERS` file in the corpus. It is also the wrong one: an agent
that can edit `docs/DECISIONS.md` to promote a claim can edit `docs/SIGNERS.md`
to authorise itself first. Whoever runs the ingest supplies the list —
`--signed-by "Name"` — because that is the one place the corpus cannot reach.

**Git had already answered half of it and D-0025 was discarding the answer.**
`%G?` distinguishes `G`, a good signature from a key the machine's keyring
vouches for, from `U`, a good signature from a key that arrived with the commit.
D-0025 treated both as signed, which accepts a key an agent minted a second
earlier — most of what U-31 was actually asking. They are now separate rungs,
and only `G` carries a verdict. The keyring is git's own, held outside the
repository, so the trust root was already in the right place and merely unread.

**Matched against the signature, not against the commit.** An author field is
free text; the identity a signature binds is not. So `--signed-by` is matched
against `%GS`, and the record keeps the signer's name and the key alongside the
commit. The match is exact — a loose one would admit every Gregory who ever
signed anything — and a refusal names the identity it saw, so a mistyped name
says what to type.

**What it still cannot do, stated plainly.** It cannot establish that the signer
*intended* the promotion. An agent on an unlocked machine can sign as its owner,
and no signing scheme survives a compromised endpoint. The goal here was never
proof: it is attribution a reader can weigh, and a record that says which rung
each promotion stands on rather than implying they all stand on the same one.

**And a limit worth naming: the attestation is evaluated once.** It is written
into the verdict and the verdict is immutable, so a key revoked tomorrow does not
demote what it signed today. That is correct for a record of what was known at
the time, and it means the ledger's account of trust is a history rather than a
current view (U-32).

---

## D-0027 · Trust is asked twice: once when the verdict is made, and again on request

```yaml
id: D-0027
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: keeper round — resolves U-32
evidence: [docs/REGISTER.md, docs/DECISIONS.md — this file is the corpus in question]
review_trigger: when a snapshot or a rewrite makes commits routinely unreachable,
  or when a weakening should reach someone who is not watching a terminal
```

**Assertion.** The attestation written into a verdict stays exactly as it was —
it is the record of what was known when the promotion was made, and nothing may
edit it. Alongside it, `review_trust` re-asks the repository what those same
commits verify as today, and sorts every promotion into five: verifying as it
did, weakened, strengthened, unverifiable, or naming no commit to re-ask about.

**Two readings, both wanted, and only one existed.** The stored attestation
cannot answer "would we accept this now", and a live check cannot answer "what
did we know then" — a repository that has been rewritten no longer holds the
evidence. Keeping the first immutable and computing the second on demand is the
only arrangement where both questions have an answer.

**A review is a read, and never a write.** A revoked key does not demote a claim.
Nothing happened in the record; something happened in the world. Writing "this is
no longer trusted" into the ledger would be a verdict, and no person declared it
— so what this produces is an alarm, and what a person does about it, retiring
the claim or replacing it, stays theirs to declare. The engine could not have
been talked into it either: a verdict needs a human author, and there is no
human here.

**Strengthening is real and is not an alarm.** A signature from a key the keyring
had no opinion about becomes a signature from a trusted one the day that key is
imported. The same machinery reports it, in its own column, because a review
that only ever brought bad news would be read as an error report rather than as
a measurement.

**Unverifiable is a third answer, not a weakening.** A commit this repository no
longer holds — rewritten, collected, or simply absent — cannot be re-asked about,
and saying so is different from saying the signature failed. This is the cost the
register predicted, and it is precisely why the recorded reading stays in the
verdict: it is the one nobody can take away.

**Where it fires.** The host runs the review on the way up, because an alarm
nobody is standing in front of is not an alarm; it prints the first few and says
how many more, because a truncated list that does not admit it was truncated
reads as a complete one. Demonstrated by pointing the same binary at the same
store with an empty keyring: 52 promotions weakened, both readings shown, and not
one record altered.

---

## H-0001 · Success hypothesis (dated, falsifiable)

```yaml
id: H-0001
state: registered      # a hypothesis is never promoted — it is scored
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
score_by: 2027-02-23   # six months
source: founding-interview, applying the ROI-as-dated-hypothesis discipline
review_trigger: score on the date whether met or not; the original prediction
  stays on file beside whatever replaces it
```

**Hypothesis.** Within six months: (a) Tacit self-hosts its own decision corpus —
this file and its successors — with envelopes and lifecycle enforced by the engine;
(b) its MCP tools let an agent answer "why did Tacit choose X?" with provenance, and
honestly abstain on registered unknowns; (c) a small golden suite grades it, and the
suite rewards abstention at the record's boundary.

**Falsifier.** If by score time provenance or temporal queries still require
app-layer workarounds, the "engine-level AI-native" claim (D-0005) is wrong — reopen
the adopt-first posture rejected in D-0008.

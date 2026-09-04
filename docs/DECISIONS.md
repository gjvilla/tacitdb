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
review_trigger: re-read 2026-08-31 (D-0052) — the usage this deferral waited
  for was observed and D-0049 decided from it; kept as the record of declining
  to decide early
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
review_trigger: re-read 2026-08-31 (D-0052) — both arrived and D-0015 decided
  the runtime shape; kept as the reasoning that set that decision up
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
review_trigger: re-read 2026-08-31 (D-0052) — the clarity arrived as D-0038:
  no assignment clause exists, and the boundary this record draws stands, now
  backed by fact; revisit only if an employer claim ever surfaces
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
review_trigger: re-read 2026-08-31 (D-0052) — D-0016 proved incremental equals
  rebuild and property tests hold it, so the revisit this trigger reserved is
  spent; re-open only if a view parameter ever enters the index
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
  its source document; the set-verdict half was re-read 2026-08-31 (D-0052) —
  D-0034 built them and a generated sync now takes one verdict, as hoped
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
review_trigger: when a fifth reason is wanted; the set-verdict half was
  re-read 2026-08-31 (D-0052) — D-0034 settled how one act closes many
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
  replace it; the set-verdict half was re-read 2026-08-31 (D-0052) — D-0034
  settled it, and D-0051's duplicate fold publishes the pairings it needs
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

## D-0028 · Measure the ranker before believing what is wrong with it

```yaml
id: D-0028
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: retrieval round — narrows U-23
evidence: [docs/GOLDEN.md, docs/REGISTER.md]
review_trigger: when a model that sees meaning is plugged in, or when the
  corpus outgrows a single-machine index
```

**Assertion.** U-23 said what remained was a model. Measuring it found four
faults ahead of that, three of them fixable without one: gaps were occupying
answer slots, a word could not reach its own plural, and one number was
answering two questions. A diagnostic (`--example explain`) now exists for the
step the golden suite does not grade, and the suite checks its own review
triggers.

**Guessing was worse than measuring, repeatedly.** The fusion constant looked
wrong — RRF's `k = 60` comes from runs over thousands of documents and this
corpus has fifty. Sweeping it across six values changed the score by nothing at
all. The actual fault was that a question about signing *keys* could not reach a
record saying *key* seven times, and no amount of re-weighting two rankings
fixes a term that matches nothing.

**A registered gap is not an answer, and stopped being ranked as one.** Gaps have
their own channel and were also being returned among the answers, where they
took a slot and, worse, set the confidence. One test had been asserting a
confident match whose confidence came entirely from a *question* about the
subject. It now reads weak, correctly.

**Plurals fold, and this is an English crutch on purpose.** `key` and `keys` land
in the same bucket, `class` and `status` are left alone. Only plurals: `-ing` and
`-ed` cannot be stripped consistently without restoring the elided `e`, which is
a whole stemmer and a larger commitment to English than this earns. It is the
same bet as the stopword list beside it, and it is written down rather than
hidden.

**One number was doing two jobs, and separating them is a second condition and
never a relaxation.** Coverage asks how much of the question a record covered;
reach asks how much of it anything here could. Both are published on the result,
because a reader told "weak" and not why cannot tell a shallow answer from an
unanswerable question. Reach separates the suite cleanly — every unanswerable
question sits at or below 0.32 and every answerable one at or above 0.52.

**And the relaxation that came with it was measured and refused.** Dropping
unmatchable terms from the coverage denominator recovers one underconfident
answer and turns G-10 into a confident wrong one. The two sit at coverage 0.77
and 0.60 with reach 0.63 and 0.64: no threshold separates them. A bluff is the worse failure, so the denominator keeps
its missing terms and reach is only ever an extra reason to decline.

**The suite had gone stale in exactly the way it warns about.** Four questions
carried review triggers naming registered unknowns that had since been resolved,
and nothing was checking them. One expected an abstention citing U-5 — a gap the
engine could no longer cite, because it had been answered — and it *passed*, on a
system that failed to answer a question it had since learned the answer to. Two
failures cancelling is the worst way for a test to be green. The runner now
fails the build when a question rests on a trigger that has fired.

**What the numbers did, honestly.** Sixteen of twenty-one before, sixteen of
twenty-one after. One question genuinely recovered, and two that had been passing
against expectations the record outgrew now fail against correct ones. The score
did not move because the system got better and the test got harder at the same
time, and saying "no change" would describe neither.

**What is left needs meaning, and now only that.** Three shortfalls remain —
G-08, G-09, G-13 — where the record answers in words the asking does not use. No
lexical repair reaches those. The `Embedder` trait is still how a model arrives,
and the ground under it is now measured rather than assumed.

*Amended 2026-08-24 (D-0029).* This record originally illustrated those three by
restating what they ask. Doing so put their rarest words into the corpus, which
moved the very measurements above: one question's reach went from 0.52 to 1.00
because this paragraph existed. Golden questions are named by id here now, and
the suite checks for the rest.

---

## D-0029 · One letter inside a word is a spelling; one at the end is not

```yaml
id: D-0029
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: retrieval round — resolves U-33
evidence: [docs/GOLDEN.md, docs/REGISTER.md]
review_trigger: when a second dialect or a second language writes into the
  corpus, or when the term scan becomes measurable
```

**Assertion.** A query term the index holds no posting for may be read as an
index term one edit away, when the edit falls inside the word and both words are
at least five letters. The substitution is published on the result, never
silent. Two candidates mean the index does not know which was meant, so it
answers neither.

**Forces.** A corpus written in one dialect and questioned in another loses its
most discriminating word to a single letter, and the word it loses is the one
the whole question turns on. Folding plurals (D-0028) does not reach it:
`organise` and `organize` are the same word and differ in the middle.

**The edit must be inside the word, and that rule was measured.** An edit at the
end is a suffix, and a suffix is morphology — the words are related and are not
the same word. Without the rule the suite produced exactly one false neighbour,
reading a question about people who do a thing as a question about the thing,
and it cost that question the answer it had already found. With it, one
substitution occurs across twenty-one questions and it is the right one.

**Only for a term that reaches nothing.** A word the corpus really has is never
overridden. That guard is what keeps this from being a guess: it can only ever
turn zero into something, never turn one answer into another.

**It took half of what the vector ranker was for.** D-0020 justified vectors
partly as the thing that bridges a spelling, and the test demonstrating it used
exactly this case. A lexical bridge is strictly better there, because it counts
toward coverage while a close vector is only ever an offer. What is left to the
vector ranker is what this rule refuses — suffixes and wider differences — and
on this corpus it currently earns nothing the suite can measure. Worth saying
plainly rather than leaving the seat looking occupied.

**Golden questions are named by id in this corpus, never by their wording.** A
record that repeats a question ranks for it, so the record explaining why a
question fails outranks the record that would answer it. This is not a
hypothetical: two such quotes went into the corpus in a single commit, and one
of them moved a question's reach from 0.52 to 1.00 — the corpus answering a
question with the note about why it could not. Illustrations here use words the
suite does not, for the same reason.

**And the suite now audits itself twice over.** It fails the build when a
question quotes back, and when a question rests on a review trigger that has
already fired. Both were meant to be manual checks and both were demonstrably
not happening — four fired triggers and two quotes accumulated without anyone
noticing. A discipline nobody checks is a wish.

---

## D-0030 · A second corpus, generated, in words the first one cannot contain

```yaml
id: D-0030
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: scale round — narrows U-9
evidence: [docs/REQUIREMENTS.md, docs/REGISTER.md]
review_trigger: when retrieval quality must be graded on real language, or when
  the target scale moves past a single machine
```

**Assertion.** The keeper can generate a deterministic corpus of arbitrary size —
subjects, claims, human verdicts, gaps closed every way the grammar allows,
supersessions, planted contradictions, dated predictions — together with the
ground truth of what it built. `--example scale` runs the engine over it and
reports costs. Every topic's vocabulary is pseudo-words built from the seed.

**Two things the self-hosting corpus structurally cannot do.** It cannot say
anything about scale: R-7 names 10^5–10^7 nodes and fifty-four records is four
orders short, so every registered cost had been reasoned about from the shape of
the code and never observed. And it cannot grade retrieval honestly, because it
describes its own grading — a record explaining why a question fails contains
that question's rarest words and then ranks for it (U-27).

**The separation is structural, not a discipline.** Nobody can accidentally write
`tamiro` into a decision record, and no question about `tamiro` can be answered
by a record about retrieval. That is a stronger guarantee than the rule against
quoting questions, which catches phrases and cannot catch vocabulary.

**It found a bug the small corpus could never have surfaced.** Ranking sorted
candidates with a tie-break that located a record by scanning the whole log — an
O(n) lookup inside a comparator, called twice per comparison. Invisible at
fifty-four records and quadratic thereafter. Indexing the log position cut a
hybrid query at 68,000 records from 158ms to 39ms. Nothing about the code had
changed; only the size of what it was pointed at.

**And it turned two registered costs from adjectives into numbers.** U-25 says an
fsync per append is "correct but slow for bulk ingest": it is 2µs per record in
memory and **4,095µs durable**, a factor of two thousand, so 4,365 records take
eighteen seconds and a hundred thousand would take most of an hour. That is not
slow, it is a different regime, and the register understated it. U-26's exact
vector scan costs 6× the lexical half at a thousand records, 73× at seventeen
thousand and 135× at sixty-eight thousand — and the scan is not the whole of it,
because every candidate also pays a state fold to be admitted.

**What it deliberately is not.** Synthetic prose has no paraphrase, no dialect
and no jargon drift. This corpus measures ranking, filtering and cost, and it
cannot measure the thing U-23 is actually about. A public corpus of real
decisions — a body of published proposals with authors, dates, statuses and
supersessions — is the other half of U-9 and stays open, along with the licensing
and repository-weight questions that vendoring one raises.

**The engine, meanwhile, is fine at this size.** Appends, index builds,
projections, contradiction detection and the lexical ranker are all linear and
all comfortable at 68,000 records on one machine. Every ground-truth query
returns its own topic's record first, at both sizes and with either plan.

---

## D-0031 · The cost was not where the register said it was

```yaml
id: D-0031
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: scale round — resolves U-35
evidence: [docs/REGISTER.md, docs/DECISIONS.md]
review_trigger: when an approximate index lands (U-26), or when a query plan
  gains a third ranker
```

**Assertion.** U-35 said retrieval's cost was the per-candidate admission fold.
Timing it says otherwise: admission is 5.2ms and the similarity arithmetic 4.9ms
of what was a 42ms query — a quarter between them. The cost was that the gap
channel recomputed both candidate lists from scratch, so every query ran the
vector scan twice, and offering a handful of open questions cost as much as
answering the question did.

**And the duplicate was unconditional.** The gap path cleared the query's entity
scope before recomputing — and the branch that recomputed is only reached when
there is no entity scope, so the second pass was byte-for-byte the first, every
time, for as long as the code has existed. Sharing what the answer path already
computed took a query at 68,000 records from 42ms to 29ms with no change in what
it returns.

**U-35 was written by reading the code, and reading was wrong.** That is the
third guess this week that measurement refuted — after the fusion constant that
changed nothing and the model that turned out to be three-quarters lexical. The
pattern is consistent enough to state as a rule: a performance claim in this
register is a hypothesis until something times it, and it should be worded like
one.

**A second fault fixed for correctness rather than speed.** Fusion looked its
tie-break up from a map inside the sort comparator — the same shape as the log
scan D-0030 found one layer down. It measures as no change, because exact ties
between floating-point fusion scores are rare, and it is still a comparator
walking a map `n log n` times. Fixed because it is wrong, not because it was
slow.

**What remains, and whose job it is.** The pipeline still materialises 25,762
candidates to return ten. Capping the fusion depth would cut that further and
would be an approximation — a candidate outside the top of both rankings can no
longer win. An approximate index returns the top directly and subsumes the cap
entirely, which is U-26 and already registered. Doing the cap here would
pre-empt a registered design decision with a quiet approximation, so it is not
done here.

**Where a hybrid query stands.** 1.2ms at a thousand claims, 6.6ms at eight
thousand, 29ms at sixty-eight thousand records — still 125 times the lexical
half, and the vector scan is still the thing that does not scale.

---

## D-0032 · An approximate index, built and measured and left switched off

```yaml
id: D-0032
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: scale round — narrows U-26
evidence: [docs/REGISTER.md, docs/REQUIREMENTS.md]
review_trigger: when a model with a wider similarity range is plugged in, when
  the corpus passes a million vectors, or when a better index beats the numbers
  below
```

**Assertion.** The vector index now carries neighbourhoods — eight independent
divisions of the space by random hyperplanes, twelve bits each — and a query can
walk the ring around its own signature instead of reading everything. Exact
scanning remains the default. What was read is published on every result.

**The choice of method was made by an invariant, not by a benchmark.** A
signature depends on its own vector and nothing else, so folding a record in
later lands it in exactly the bucket a rebuild would, and `rebuild ==
empty().advance()` stays definitional (D-0016). A navigable graph whose edges
depend on insertion order, or cells whose centroids move as data arrives, would
both have cost that — and the property test that holds it down now covers the
vector index too.

**It is predicate-aware, which was the requirement and not a detail.** The index
yields candidates and judges nothing; the caller holds the view and stops when it
has enough records that view *admits*. So a filtered search narrows the
traversal rather than discarding its results afterwards, which is R-1 and the
production gap that started this project.

**And it does not earn the default. Measured, on 35,480 vectors:** at its best
setting it reads 17% of the index and returns 65% of the exact top ten, agreeing
on the single best match sixteen times in twenty. That is roughly two and a half
times faster for a third of the recall. Widening the probe walks up a curve that
flattens: eight tables and twenty-four measure the same, and more scanning buys
proportionally less each time.

**The fault is the method, not the data, and that was worth establishing.** The
space is genuinely approximable — the best match sits 115% above the median
similarity, so there are real neighbourhoods to find. Sign-random projection
simply needs a much larger corpus before its asymptotics beat a linear scan by
enough to pay for what it drops. At thirty-five thousand vectors the constants
win.

**A measurement that was wrong before it was right.** Recall first appeared to
*fall* as the probe widened, which is impossible for the approximation and was
true of what was being measured: end-to-end agreement of the fused result, which
mixes in the lexical ranker. Measured on the vector ranking alone it is monotone.
Two rules met here — measure the thing you are changing, and a number that moves
the wrong way is a fact about the instrument until proven otherwise.

**So it ships off, and honestly.** `Probe::Exact` by default, `Probe::Neighbourhoods`
for a caller who wants it, and `Retrieved::scanned` on every result so an
approximation is something anyone can see the size of rather than infer. The
numbers above are now the baseline a better index has to beat, which is more than
U-26 had before: it had a shape to build and no bar to clear.

---

## D-0033 · An option costs nothing until someone takes it

```yaml
id: D-0033
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: scale round — resolves U-36
evidence: [docs/REGISTER.md, docs/DECISIONS.md]
review_trigger: when probing becomes the default plan, or when the neighbourhood
  index is removed
```

**Assertion.** Neighbourhoods are kept only if a caller asks for them.
`VectorIndex::rebuild` keeps none; `rebuild_searchable` keeps them, and
`with_neighbourhoods` fills them in on an index that already holds vectors. A
probe asked of an index that cannot be probed falls back to scanning.

**Measured in one run rather than remembered from two.** Building the same
35,480 vectors takes 2.02s without neighbourhoods and 2.50s with — a quarter
again — and the buckets hold 283,840 record ids, about 4.5 MB, roughly 128 bytes
of index for every vector. U-36 estimated 2.0 to 2.4 seconds and was close, which
is worth recording precisely because this week's estimates have mostly not been.

**Every caller that exists takes the cheap one.** The host, the dogfood, the
golden suite and the retrieval diagnostic all build a plain index and pay none of
the above. Only the scale measurement builds the other, because it is the only
thing that probes.

**Falling back to scanning is the safe direction.** An unprobeable index returns
no neighbourhoods, so a probe against one would come back empty and look exactly
like an empty corpus — a wrong answer that reads as an honest one, which is the
failure this project cares most about. Scanning is slower and right, and
`Retrieved::scanned` says which happened. What it does not do is *warn*: a caller
who asks to probe and quietly gets an exact scan has a configuration mistake
visible only to someone reading the number.

**And the same edit nearly removed the test that guards it.** Making
neighbourhoods opt-in meant the property test asserting `incremental ==
rebuild` — added one record ago specifically to cover the buckets — silently
went back to testing an index that no longer had any. Changing a default is
enough to hollow out a test without touching it, which is worth remembering the
next time a default moves.

---

## D-0034 · One verdict over many records, saying what it is worth

```yaml
id: D-0034
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: grammar round — resolves U-16 and U-20
evidence: [design/001-data-model.md §3.1, docs/REGISTER.md]
review_trigger: when a set needs to be named by a run rather than enumerated, or
  when a fourth footing is wanted
```

**Assertion.** A verdict may name a set of claims, and must say on what footing
one person speaks for all of them: an *ingestion run* they ratify without reading
row by row, *one editorial act* the keeper split across several records, or a set
*reviewed in full*. It promotes every target and may retire what those targets
replace, in one declaration.

**Invariant 5 is untouched, and that is the point.** A human still declares it
and an agent still cannot — there is a test. Bulk was never the reason agents may
not promote; the reason is that promotion is a person's act, and doing it to ten
thousand records at once does not make it someone else's. What bulk changes is
what the declaration *means*, so the meaning is written down instead of being
left to whoever remembers the afternoon.

**The footing is the whole design.** "I ratify this run and its source" and "I
read each of these" are both honest and they are not the same, and a corpus that
cannot tell you the mix cannot tell you what it is worth. `Ledger::ratification`
reports it: how many claims were promoted one at a time, and how many in sets on
each footing.

**Enumerated, not named by a run.** A verdict that identified its set by an
ingestion id would be smaller and would need the ledger to say what it touched —
and the state fold is a pure function of the action precisely so that it cannot
be argued with. The same constraint decided the shape of D-0023, which is twice
now that this property has chosen an interface. The record is larger and says
exactly what it did.

**All or nothing.** One illegal target refuses the whole verdict. A
partly-applied set verdict would be a record of something nobody declared.

**U-20 was the small version of the same thing.** The ingest needed two verdicts
per record — a claim and its title — for what an author performed once by writing
`state: promoted`. The keeper split the record because the model wanted the parts
apart, and then charged the split back to the author as a second declaration. It
is one verdict now, on the footing of one act, and the corpus holds thirty-three
verdicts where it held sixty-six.

**Measured at the size it was registered for.** A generated catalogue sync of two
thousand rows is ratified by one verdict rather than two thousand, and the tally
reports two thousand claims promoted on the footing of an ingestion run beside
the fifteen hundred read one at a time.

**And it says something uncomfortable about this corpus straight away.** Every
promoted claim here now reports the footing *one editorial act* and none reports
having been read one at a time — because every one of them was transcribed from a
document, and none has ever been ratified inside the ledger by a person looking
at that record. That was equally true yesterday and the ledger could not say it:
sixty-six verdicts looked like sixty-six declarations and were thirty-three acts.
The number did not get worse, the record got honest, which is the entire reason
the footing exists.

**And adding it made two audits go quietly blind.** `unattested_promotions` and
the trust review both asked "is this a `Promote` naming my claim?", which a set
verdict is not — so every bulk-ratified claim would have reported as having
nothing wrong with it. Both now ask the action what it *did*, through a
`promotes` derived from `effects`, so a verdict added later is covered the day it
is added. An audit that reports nothing wrong because it stopped looking is the
worst failure this record has a word for, and the compiler cannot catch it: the
matches were exhaustive over the enum and wrong about the question.

---

## D-0035 · Who the commits say made this is part of the boundary

```yaml
id: D-0035
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: boundary round — sharpens U-7
evidence: [docs/DISCLOSURE.md, docs/REGISTER.md]
review_trigger: re-read 2026-08-31 (D-0052) — the resolution came as D-0038,
  which performed exactly the correction this record deferred while the
  question was open; still live: when a second person commits to this
  repository
```

**Overtaken in part by D-0038, 2026-08-29.** The review trigger above fired: U-7
resolved on a fact, and there is no invention-assignment clause to reach this
project. What that lifted is this record's refusal to rewrite history — a refusal
conditional on ownership being the open question, not an absolute one — and the
authorship record has since been restamped, with the pre-rewrite history
preserved in a mirror clone. The rest stands as written: both facts below, the
reason the check reads commit identity at all, and the split between failing on
what is changeable and reporting what is not. This record is not edited to match;
D-0038 says what changed and why.

**Assertion.** D-0010's boundary covers the record of who made this work, not
only the text of it. `scripts/check-boundary.sh` now reads the commit identity
as well as the files: it fails on the identity the *next* commit would carry, and
reports what history already carries without failing on it.

**It had been watching the wrong thing, and the finding is the point.** Every one
of the first twenty commits is authored, committed and cryptographically signed
under an employer email address. The boundary script passed on all of them,
because it had only ever been asked about file contents. A project whose founding
record says *personal project, personal time* has been asserting the opposite in
its own metadata since the first commit, and the check written to catch exactly
this class of thing did not look there.

**History is not rewritten, and that is a decision rather than an oversight.**
Restamping twenty commits with a different name would be tidying evidence in a
matter where ownership is the open question — whatever else it would be. It would
also contradict the append-only ethic the engine is built on, which would be a
poor advertisement for it. The identities stand; what changes is what the next
commit says.

**So the check fails on what is still changeable and reports what is not.** That
split is the whole design: an alarm on the fixable thing, a note on the fact. The
repository is red until an identity is set for it, and being red is correct — the
boundary is genuinely violated and has been all along.

**The signing key carries it too, and is being left alone.** The GPG key these
commits are signed with has a UID bearing the same address, so every attestation
D-0026 records names that identity as the signer. Noted rather than changed:
generating or switching keys, or stopping signing, would each alter the record in
a way worth thinking about before doing, and none of it is the engine's business.

**A red check is the gate, not a nuisance.** This one is expected to stay red
until U-7 resolves, which risks becoming the alarm nobody reads that this very
script was written to avoid. The framing is what saves it: U-7 already blocks
publishing, so a check that fails on U-7 is the release gate made mechanical
rather than remembered — the same move that stopped the golden suite's review
triggers rotting unnoticed. It says so when it fires, and says not to silence it.

**A second fact, recorded without a conclusion.** Nineteen of twenty commits fall
between 13:00 and 19:00 on a Monday, one on a Sunday afternoon. What that means
depends on working arrangements that are not in this repository and are not the
engine's to judge. It is written down because a record that only kept the
convenient facts would not be worth keeping.

**And a page to hand someone.** [DISCLOSURE.md](DISCLOSURE.md) states what this
project is, what it contains, what it does not, how that is enforced, and both
of the facts above — for the conversation U-7 has been asking for since the
founding interview. It makes no legal claim, because none of this is mine to
make.

---

## D-0036 · An index may only answer for a present it has seen

```yaml
id: D-0036
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: temporal round — resolves U-14
evidence: [design/001-data-model.md §3.1, docs/REGISTER.md]
review_trigger: when a view is held across a process boundary, or when the
  reference semantics and the implementation are ever allowed to diverge
```

**Assertion.** The temporal reads now have a reference semantics — the state
machine of §3.1 written out separately, short enough to be obviously right — and
eight properties hold the implementation against it over generated ledgers. A
projection that has not folded the whole log no longer answers for the present,
and `contradictions_at` gives overlap the past tense every other read already
had.

**The defect the properties were written to find, and did.** A view over a stale
projection reported a retired claim as promoted, when asked about *now*, with the
current ledger passed to the very same call. The index has a fast path for
"record-time at or after my frontier — use my own fold", which is sound only if
the fold is current, and nothing checked that it was. The keeper layer knew to
advance after every write and the engine let anyone who forgot get a confident
wrong answer. It falls back to folding the ledger now: slower and right, the same
direction to fail as D-0033.

**A reference is only worth having if it is derived independently.** This one is
transcribed from the design document's transition table, not from the code that
answers the question — a reference copied off the implementation agrees with it
by construction and proves nothing. It is checked at every record-time in the
ledger and the instants either side of each, because that is where a boundary is
off by one.

**Six properties passed on the first run, and taking that as reassurance was the mistake.**
They tied both temporal axes to one instant, which exercises the cross-product
only by accident, and they used an index advanced after every operation. Pulling
the axes apart found nothing. Holding an index while the world moved on found the
defect immediately. The lesson is not "write properties" but "write the property
that could fail" — a suite that only tests the arrangement the code was written
for is a suite that agrees with it.

**And writing this record moved the score, which is the finding inside the
finding.** Two ordinary words of prose in the paragraph above were rare terms of
a golden question, and their arrival lifted that question from failing to
passing — an apparent gain of one, caused entirely by describing the work. It was
caught by reading the diagnostic rather than banking the number, and the words
were changed. Three times in one day now, and this was the first where the
contamination flattered the result: a score that moves the wrong way gets
investigated, and one that moves the right way does not. The phrase check added
for U-27 cannot see this — it catches quotations, and these were not quotations.
The structural answer already exists and is not being used: a corpus the record
does not describe was built for U-9 and the suite still grades on this one.

**Overlap has a past tense now.** `contradictions` was the one read with no
temporal twin, so "what did we hold to be contradictory last Tuesday" had no
answer in an engine whose entire claim is that you can ask what the record said
at a time. A contradiction resolved today was still a contradiction then, and
saying so is not a defect in the resolution.

**What the corrections-of-corrections case actually does, written out.** A claim
replaced, its replacement replaced again, and each record's state read back at
every moment: the past does not move as the present does. It was already correct.
It is now written down as a test rather than as a belief.

---

## D-0037 · A question is agreed against a corpus, and the corpus moves

```yaml
id: D-0037
state: promoted
author: Greg Villa
recorded: 2026-08-24
valid_from: 2026-08-24
source: measurement round — resolves U-27
evidence: [docs/GOLDEN.md, docs/REGISTER.md]
review_trigger: when the suite grades on a corpus the record does not describe,
  or when a baseline needs re-recording for a reason other than drift
```

**Assertion.** Every golden question records the words of it that the corpus did
not contain when the question was agreed. If one of those words later appears in
the corpus, the suite turns red. Together with the phrase check from D-0029 —
which catches a record quoting a question — that covers both ways a
self-describing corpus moves its own measurement.

**Absence is the stable thing to record.** Document frequency drifts with corpus
size, and so does reach, so a baseline of either would cry wolf on every new
record. Whether a word is in the corpus at all does not move unless somebody
writes it. That makes the alarm quiet when nothing happened and loud when
something did, which is the only arrangement anyone keeps reading.

**D-0029 said this could not be mechanised, and that was half right.** Checking
the *state* of the corpus is hopeless: a rule flagging rare question words found
in other records fires thirteen times here, and most are a corpus legitimately
holding the topic it is about. Checking the *change* is precise, because a word
arriving is an event with a date and a cause. The two are not the same problem
and only one of them was intractable.

**It fires on innocent records too, and that is correct.** A word can enter the
corpus because someone wrote carelessly about a failing question, or because the
project genuinely decided something about that subject. Both mean the same thing
for the suite: a question chosen because the record was silent on a topic is not
the same question once the record speaks. Re-read it, then re-record the
baseline.

**The baseline doubles as an explanation.** Reading it says why each abstention
question abstains — the corpus has no words for it — which was previously
something you could only discover by running a diagnostic.

**And the suite document now has two tables of the same shape.** A malformed
question row is a hard error by design, so a second table of `| G-` rows would
have broken the suite it exists to protect. Both parsers are section-aware now;
there is a test that neither reads the other's rows.

**What this does not do.** It does not stop the corpus contaminating itself, and
nothing in the engine can — the structural answer is a corpus the record does not
describe, which was built for U-9 and which the suite still does not use, because
synthetic prose cannot grade real language. That trade stands where it was. What
changes is that the contamination is now caught the same day rather than three
times in one.

---

## D-0038 · No assignment clause exists, and the authorship record is corrected

```yaml
id: D-0038
state: promoted
author: Greg Villa
recorded: 2026-08-29
valid_from: 2026-08-29
source: resolution of U-7 — employer confirmed to have no IP/invention-assignment
  agreement in force
evidence: [docs/DISCLOSURE.md, docs/REGISTER.md]
review_trigger: if the employer later introduces an IP agreement, or if any
  claim on this work is asserted; and before any public release, confirm the
  factual basis below is still true
```

**Assertion.** U-7 resolves on a fact: there is no employment agreement carrying
an invention-assignment clause at the author's employer — no such agreement was
ever signed. The question D-0035 kept open ("does the clause reach this
project?") has no clause to answer it. With ownership no longer an open
question, the authorship record is corrected: every commit is restamped from the
employer address to the author's personal identity
(`Greg Villa <gjvilla121@gmail.com>`), via `git filter-repo --mailmap`.

**Why rewriting is now permissible when D-0035 forbade it.** D-0035's refusal
was conditional, not absolute: restamping authorship *while ownership was the
open question* would have been tidying evidence. The condition has lifted — the
resolution is recorded here, in a commit made before the rewrite, and the
pre-rewrite history is preserved in full in a mirror clone
(`../tacit-backup.git`, head `29d1fa15f2e173d0c04aaf509083def8460c42ab`). The
evidence is kept, not tidied; what changes is the working record going forward.

**What the rewrite changes and what it deliberately does not.** Author and
committer identities change; every commit hash changes with them; the old GPG
signatures — made under a key whose UID carries the employer address, and
invalidated by any rewrite regardless — are dropped. Commit *timestamps* are
untouched: the weekday-afternoon pattern D-0035 recorded is a fact about when
this was built, and it stays in the record. DISCLOSURE.md stays in the tree as
the factual page it was, updated to say how the question closed.

**What this does not resolve.** The absence of a signed agreement is the
strongest fact available, but it is a layperson's reading of it; jurisdictional
default doctrines (shop rights, scope-of-employment work) were not examined by
counsel. U-7 is resolved on the recorded facts; the residual counsel item joins
U-6's, to be closed before commercial use. The boundary script keeps reading
commit identity — the gate stays mechanical; only the violation is gone.

---

## D-0039 · Retrieval is measured on words nobody here wrote

```yaml
id: D-0039
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: U-9's real-language half — the reader shipped 2026-08-29; this is the
  suite and the first measurement, resolving U-9
evidence: [docs/PEP-GOLDEN.md, scripts/fetch-proposals.sh,
  crates/tacit-keeper/examples/pep_golden.rs, docs/REGISTER.md]
review_trigger: if the slice repins; before any claim that retrieval is good
  (the bar U-23 set now has an outside number). Re-read 2026-08-31 (D-0052) —
  both unknowns this suite registered were settled by D-0040 and D-0048, and
  the suite's own recoveries recorded each
```

**Assertion.** Retrieval quality is now graded on a corpus this project did not
write: sixty packaging proposals pinned to one upstream commit, fetched by
script and never vendored (U-11 stands; the raw documents carry contact
details), with twenty-four questions agreed against them in
[PEP-GOLDEN.md](PEP-GOLDEN.md). The runner refuses to grade a directory that is
not exactly the pinned slice, because a suite agreed against one corpus and run
over another measures nothing. First grading: **16/24, five of them earned by
declining to answer** — the number H-0001(c) wanted and the self-corpus could
not honestly produce.

**What the questions caught before scoring anything.** P-13 was written as a
trap — its answer has three retired predecessors sharing its whole vocabulary —
and it fired on the ingest, not the ranker: a promotion retires one record per
verdict, so PEP-0600's three replacements left two predecessors governing, and
a `Superseded-By` header naming a present successor was trusted to mean the
successor would retire it, which PEP-0621 never does for PEP-0631. Both fixed:
extra replacements retire in their own verdicts, and a superseded proposal
still governing after every successor has spoken retires itself — the status is
the last witness, read after the loop because it cannot be known inside it.
Third time the state fold has chosen an interface, and the second modelling
error this corpus has caught in two days.

**Every shortfall is filed by measurement.** The G-suite's scar — three of four
failures once filed under a cause that does not explain them — is the rule
here from day one: `explain --proposals` ran before any `pending` marker was
written. Five failures are U-23's (paraphrase and same-vocabulary neighbours,
margins of 0.02 and 4%; calibration declining the right answer at coverage
0.31). Three are not, and two of those are new registrations: **U-40**, a fair
question about a refusal cannot be answered because refused records are
invisible to the governed view — correct per design/001 §7 and still the wrong
outcome, with the open half being who decides a question is *about* a record
rather than answered *by* it; and **U-41**, fusion losing a record both rankers
held in their top three, which retires the "the fusion constant does not
matter" reading D-0028's sweep suggested — that result was about the corpus,
not the constant. The eighth, P-22, bluffs at coverage = reach = 1.00 with a
1.6% margin, handing U-38 exactly the outside measurement its trigger demanded:
the ratio clause of its proposed rule separates nothing on real language, and
the margin clause is the whole question now.

**What the vector ranker turns out to be worth.** Four questions pass with
vector candidates that fail without them (12/24 → 16/24). U-33 recorded that
the second ranker earned nothing the suite could measure; that was true of a
54-record corpus that questions its own vocabulary, and false in general —
the ranker was never useless, only unmeasurable at that scale. First
register-recorded conclusion the outside corpus has reversed.

**What this deliberately does not do.** The suite does not gate the build the
way the G-suite does — the corpus is not in the repository, so CI cannot
assume it; the runner is red on regressions when it runs, and the doc-only
invariants (governance, pending markers naming registered unknowns) are
tested without the corpus. Dates still parse to strings and do not fill
valid_from, so record-time travel over this ledger is not yet meaningful. And
the questions were agreed by the same hand that chose the slice — an outside
corpus, not yet an outside examiner.

---

## D-0040 · First place is evidence: the fusion default, and two corrections

```yaml
id: D-0040
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: the U-41 repair, run as the register asked — swept over both suites
  before being believed
evidence: [docs/REGISTER.md, docs/GOLDEN.md, docs/PEP-GOLDEN.md,
  crates/tacit-keeper/examples/fusion_sweep.rs]
review_trigger: when a third ranker joins the plan, or when the embedder stops
  being a hashing one — both change what a first place is worth. Re-read
  2026-08-31 (D-0052) — the budget question moved as D-0041 and the k=0
  default stood still through it and through D-0044's sweeps
```

**Assertion.** The default fusion is reciprocal rank with `k = 0`, and the
zero is a statement rather than a tuning: with two rankers, first place in
either list is evidence and depth is not. The literature's k=60 blunts every
list's top across a large ensemble; with exactly two rankers it inverts the
evidence — a record held at rank 0 by a score margin rank fusion never sees
loses to a record at ranks 1 and 2, because 1/61 < 1/62 + 1/63. Zero is the
only value in the family where a first place beats a middling pair, and k=1
already re-inverts that case (it passed the sweep on a lucky vector rank).
Swept over both suites: k ≤ 10 recovers G-07 and moves nothing else anywhere;
score-normalized fusion was measured and refused — it costs every question
the vector ranker earns on the proposals corpus. The served plan no longer
loses to lexical-only on the self-corpus (17/21 both) and keeps the vector
ranker's four rescues on the proposals corpus (16/24 against 12/24).

**The register was wrong twice, and the instrument said so.** Yesterday's
U-41 entry filed G-10 as fusion's casualty via "the gap channel reads the
same fused candidates." It does not — the gap channel has its own ranking,
and it ranked by `coverage.max(closeness)`, which let three gaps sharing no
words with the question outrank the one gap covering it, on the strength of
a similarity this engine elsewhere refuses to let confer confidence. That is
U-42, registered and resolved today: coverage ranks, closeness only opens
the door and breaks ties — the same asymmetry the answer path already gives
similarity. G-10 recovered. And P-12, U-41's founding evidence, was not
fusion's casualty either: under the new default its answer sits at fused
rank 2, inside the grading window, and the assembled result still holds one
record — the 4,000-token budget assembles a single 3,700-token document, so
rank information below first place exists in the plan and cannot reach a
consumer. That is U-43, open: on a long-document corpus the token budget,
not the ranker, decides how many answers exist, and the suite's rank-3
window grades a door the budget has already closed. P-12 is re-filed there.
`explain` printed the truncated assembly as "fused" until this — it now
shows fused order and assembly separately, because an instrument that
conflates them is how both misfilings survived a day.

**What this deliberately does not do.** It does not touch the confidence
rule (U-38 stands, margin clause and all), does not resolve the two-champion
case (a wrong record leading each list is a ranking fault, not a fusion
one), and does not raise the token budget — U-43 interacts with U-39's "a
repair worth the name would change how a record is indexed", and widening
the window before deciding how long documents should be indexed would be
tuning the symptom.

---

## D-0041 · The budget assembles k answers, not one document

```yaml
id: D-0041
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: the U-43 repair — assembly-time excerpting, the budget's own
  arithmetic as the window
evidence: [docs/REGISTER.md, docs/PEP-GOLDEN.md,
  crates/tacit-core/src/retrieval.rs]
review_trigger: if excerpt quality is ever graded and found wanting; when a
  consumer needs a window the equal share cannot hold. Re-read 2026-08-31
  (D-0052) — the indexing question resolved as D-0044 keeping whole records,
  so nothing subsumes this and excerpting carries the long-document load
```

**Assertion.** A record that would not fit its share of the assembly budget is
excerpted to the window of it that covers the most of the question — ranked by
distinct query terms, then occurrences, then earliest, following any spelling
the index read a term as. The share is `max_tokens / k`, deliberately not a
new constant: a budget that promises k answers within an allowance has
already said how much any one answer may take. The MCP host serves the
window, marked `excerpted`, with the full record one `tacit_get_record` away
— assembly, not loss, because the record itself is untouched and the
provenance chain still reaches it. Ranking is also untouched: records are
scored whole, and whether they should be *indexed* in pieces stays U-39's
question.

**What it measured to.** The proposals suite moves 16/24 to 19/24 with zero
regressions and the self-corpus stands still at 17/21 — and the three
recoveries are a third correction of cause in two days. P-02, P-03 and P-17
were filed under U-23 as meaning faults on explain evidence of tight margins;
the margins were real, but their answers sat at fused ranks one and two the
whole time, and a one-item assembly graded everything below first place as
never surfaced. The explain instrument had conflated fused order with
assembly until D-0040 split them, so even measurement-based filing inherited
the conflation. What U-23 keeps is what survives the widened window: the
calibration family — P-09, P-12 (re-filed here from U-43, its third and
narrowest filing: found at rank two, declined at coverage 0.48), P-16 — where
the right record is surfaced and the confidence rule declines it.

**What this deliberately does not do.** The window selection is graded only
by the suites — nobody has judged excerpt *readability*, and the review
trigger says so. `ANSWER_RANK_LIMIT` stays at three, now meaning what it
says. And the interaction with U-39 is one-directional by design: this
change makes long documents *deliverable*; it does nothing about the length
discount that decides how they *rank*.

---

## D-0042 · The confidence rule stands, and the relaxation is refused with numbers

```yaml
id: D-0042
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: resolution of U-38 — the outside measurement its trigger demanded,
  read off the calibration instrument over both suites
evidence: [docs/REGISTER.md, crates/tacit-keeper/examples/calibration.rs]
review_trigger: when the embedder stops being a hashing one, or a semantic
  ranker joins the plan — either changes what the columns can hold; and any
  future move on the confidence rule starts by rerunning the calibration
  table over both corpora
```

**Assertion.** The confidence rule stays as D-0020 and D-0028 left it —
coverage, score, and reach against their thresholds, nothing else — and
U-38's proposed clause ("covered everything answerable, by a decisive
margin") is refused. Not for lack of appeal but because the measurement its
own row demanded came back conclusive against it, three ways, on
forty-five questions across two corpora.

**The exhibits.** First: no margin threshold can exist. P-22 bluffs at
ratio 1.00 with a lexical margin of 1.02 over its runner-up; G-01 answers
correctly at ratio 1.00 with a margin of 1.01. A clause that must thread a
one-percent gap between a bluff and an honest answer is a lookup table of
the suite, not a rule. Second: the motivating case no longer satisfies the
rule proposed for it. G-08's coverage equalled its reach exactly when U-38
was raised; today the ratio reads 0.78 and its reach has drifted below the
mostly-unknown gate as the corpus grew — the anchor drift U-37 describes,
acting on the very question that proposed the rule, which is what a rule
fitted to one row does. Third: the columns do not contain the answer. P-12
must be answered and P-24 must be abstained on, and they read identically —
coverage 0.48, reach 1.00, margins 1.06 and 1.11. Separating them requires
knowing what the words mean, which is U-23's model half and no threshold's.

**What survives of U-38.** Its second fault was never about the clause and
stays open as U-44: the outcome is judged from the first assembled item's
coverage, and fused order chooses that item — so the ranking, not the
calibration, decides whose confidence counts. P-12 is now live evidence
(the wrong record's 0.48 is the number the outcome read), but whether the
expected record's coverage differs is unmeasurable until per-item coverage
is visible to an instrument, and shipping a fix unmeasured is the thing
this register keeps declining to do.

**Consequence for the suite.** P-22 refiles from U-38 to U-23: it bluffs
because every one of its words is spoken by a corpus that does not settle
it, and no lexical quantity distinguishes that from an answer — the
words-not-meaning fault in its purest form. The calibration table joins
explain and fusion_sweep as a permanent instrument, because this refusal is
corpus-relative and any future proposal starts by reading it again.

---

## D-0043 · Confidence is published per item and judged from the first

```yaml
id: D-0043
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: resolution of U-44 — per-item coverage made visible, then the
  known-shape fix measured against it and half-refused
evidence: [docs/REGISTER.md, crates/tacit-core/src/retrieval.rs,
  crates/tacit-keeper/examples/calibration.rs]
review_trigger: when a ranker that understands meaning joins the plan — it
  could weigh a later item's coverage where this engine declines to; and any
  future move on the outcome rule starts from the calibration table
```

**Assertion.** Every assembled item now carries its own coverage — through
the engine and out the MCP tool — and the outcome is still judged from the
first item's, now as a recorded decision rather than an accident of
wording. U-44's known-shape fix had two halves. *Publish confidence per
item* is adopted: it costs nothing, and a consumer who knows what the words
mean can see when the second item covers more of the question than the
first. *Judge the best coverage among assembled items* is refused, on the
table it was proposed to be measured against.

**Why "best" is refused.** On the self-corpus it changes nothing at all —
no question's best-of-three crosses a bar its first item misses. On the
proposals corpus it would flip three shortfalls to passes and one honest
abstention to the suite's costliest failure: the record that covers 1.00 of
"what is the maximum size of an uploaded distribution" is the corpus's
longest document, which answers questions about signing and not about
sizes. Coverage asks whether a record holds the question's words, and the
longest document holds the most words: judging the best coverage among
items manufactures confidence out of document length. Worse, two of the
three flips it buys are coincidence passes — the coverage crossing the bar
belongs to a *wrong* record while the right one sits adjacent — and
nothing mechanical separates the one legitimate case (P-12, whose own
record covers 0.71 at rank three) from the bluff (a wrong record covering
1.00 at rank two). The bluff covers more. Third refusal in this lineage
(D-0028, D-0042), each on a better instrument.

**What was corrected on the way.** `Retrieved::coverage` had documented
itself as "the best item" while the code read the first — the same
contract-against-code gap the gap channel had (U-42), found the same way.
The doc now states the measured rule and the reason; a test pins the
inversion case, built so the first item ranks on term density while the
second covers more.

**What this deliberately does not do.** It does not weigh per-item coverage
into ranking or outcome — that weighing takes meaning, which is U-23's
model half and the review trigger above. And the length bias it exposed in
coverage itself is recorded with U-39, where the other length effects live.

---

## D-0044 · The length constant stopped mattering, and the predicted repair is built, measured, and off

```yaml
id: D-0044
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: resolution of U-39 — the sweep its trigger demanded, rerun on both
  corpora, and the repair its row predicted, built and swept against them
evidence: [docs/REGISTER.md, crates/tacit-core/src/retrieval.rs,
  crates/tacit-keeper/examples/indexing_sweep.rs]
review_trigger: if either suite's sweep stops being flat in BM25_B, or a
  corpus arrives whose documents dwarf even the proposals'; any move on
  passage size starts from the indexing_sweep table
```

**Assertion.** BM25_B stays at 0.75 and a record is indexed whole, and both
are now measured positions rather than defaults. The sweep U-39's trigger
demanded came back flat: on the self-corpus every value of B from 0.00 to
1.00 scores 17/21, and on the proposals corpus every value from 0.25 up
scores 19/24. The one-for-one trade recorded on 2026-08-29 — G-09 bought at
G-07's expense — is gone, and nothing about B changed. What changed was
fusion (D-0040) and assembly (D-0041): the constant was never the fault, it
was the visible dial on a fault that lived two stages away. U-39's central
sentence — the constant selects which kind of question can be answered — was
true of the engine that existed when it was written and is not true of the
engine that exists now.

**The predicted repair, built and refused.** The row said a repair worth the
name would change how a record is indexed, so it was changed: passage
indexing, each record scored as its best window, title and body competing at
comparable lengths. Swept at six sizes over both corpora, it loses or ties
everywhere — 15 to 17 against 17 on the self-corpus, 13 to 18 against 19 on
the proposals. The mechanism of the loss is the mirror of D-0043's: a
window's coverage understates every record whose answer is spread across its
document, so four questions whose answers legitimately cover the question at
document scale fell underconfident. Coverage's two length failure modes are
now both measured and in tension — the whole document overstates the longest
record, the window understates the spread answer — and every rule proposed
over either has been refused, which is D-0043's publish-and-do-not-decide
holding from the other side.

**Kept, switched off.** The passage machinery stays in the index behind
`with_passage_tokens`, exactly as the approximate vector index stayed behind
its own door (U-26, D-0032): the refusal is corpus-relative, the sweep is a
permanent instrument, and a corpus of book-length documents can reopen the
question by running it. A test pins the default as whole-record and the door
as open.

**What this leaves standing.** G-09 still loses to two title claims — a real
loss, now a question-level ranking fault that no length constant reaches,
filed where it always belonged: with meaning (U-23). And the skew numbers
stay true (titles of 7 tokens against bodies of 3,683); what fell was only
the claim that a constant, or a re-slicing, was the fix.

---

## D-0045 · A real model is plugged in, and the suites cannot tell

```yaml
id: D-0045
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: U-23's model half, finally measured — a real embedding model behind
  the trait D-0020 built for one, graded on both suites against the stand-in
evidence: [docs/REGISTER.md, crates/tacit-keeper/src/embed.rs,
  crates/tacit-keeper/examples/meaning.rs]
review_trigger: if the meaning instrument's separation line ever reads
  SEPARATED, the confidence rule's model half reopens (D-0020 said so).
  Re-read 2026-08-31 (D-0052) — the caps were lifted the next day (D-0046)
  and the ceiling did not move, so a second attempt now means a materially
  larger model, not more plumbing
```

**Assertion.** The keeper gains a real embedding model — `BAAI/bge-small-en-v1.5`
over ONNX behind the `real-embedder` cargo feature, off by default so the
default build stays dependency-free (R-4) and fetches nothing. It implements
the same trait the hashing stand-in does, and the `meaning` instrument runs
both suites under both embedders with everything else held still. The
measured result: the real model moves the proposals suite by net zero (one
recovery, one loss — and the loss is a question the stand-in's character
n-grams were rescuing), costs one question on the self-corpus, and its
similarity distributions for answerable against unanswerable questions
overlap on both corpora, exactly as the stand-in's did when D-0020 refused
to let similarity confer confidence. Nothing ships differently. The feature
is an instrument until a measurement says otherwise.

**What the measurement actually found.** The register's standing sentence —
what remains needs meaning — was half wrong, and the half matters. Five of
the eight remaining shortfalls across both suites are confidence-shaped:
the right record surfaced and declined, or every word present and none of
it an answer. Confidence is lexical by design, a decision D-0020 made on
measured overlap and this measurement re-confirms, so those five were never
reachable from the vector channel by any model. The three that were
reachable trade one-for-one at this model's size and plumbing. Meaning in
the ranker was not the missing piece; meaning in the confidence rule would
be, and the distributions still refuse to license it.

**The caps, named rather than hidden.** The model reads at most its context
window, so a 3,700-token body is embedded by its opening; the trait embeds
queries and documents through one method, so the asymmetric prefix this
model family prefers is not applied; and the model is the small end of its
family. Any of the three could be why the separation line still reads
overlapping — U-45 registers them, because a second attempt that does not
lift them first would measure the same handicaps again and call it the
model's ceiling.

---

## D-0046 · The caps are lifted and the ceiling does not move

```yaml
id: D-0046
state: promoted
author: Greg Villa
recorded: 2026-08-30
valid_from: 2026-08-30
source: resolution of U-45 — both plumbing caps lifted as its trigger
  required, and the meaning instrument rerun with them in place
evidence: [docs/REGISTER.md, crates/tacit-core/src/embedding.rs,
  crates/tacit-keeper/examples/meaning.rs]
review_trigger: a materially larger model, or a corpus whose questions and
  documents genuinely share meaning the words hide — either reruns the
  meaning instrument before anything else is argued
```

**Assertion.** The two caps U-45 named are lifted and stay lifted. The
`Embedder` trait grows `embed_query`, defaulting through to `embed`, so an
asymmetric model states its purpose on the question and a symmetric one
never notices the seam exists — that half is simply correct plumbing and
costs nothing. And the vector index gains embedding windows behind
`with_embedding_windows`, the twin of the text index's passage door: a long
record embedded piece by piece answers as its best window, so text past a
model's context horizon has a voice. Both measured on both suites with the
real model; neither ships as a default.

**What the rerun showed.** The prefix changes nothing measurable. The
windows recover one self-corpus question and, on the proposals corpus,
manufacture the same bluff for the third time this week: the longest
document's fifteen windows are fifteen chances at a high similarity, its
best window climbs the fused order, and the abstention P-23 owed becomes a
confident wrong answer. This is the law D-0043 found in coverage and
D-0044 found in the lexical sweep, now in vector space — any maximum over
per-piece scores hands the longest record the most lottery tickets,
honest and adversarial questions alike. The windowed unanswerable
similarity range *rises*. And the line the lifts existed to test still
reads the same: answerable against unanswerable similarity overlaps in
every configuration, on both corpora.

**What this settles.** The overlap is not the plumbing's. With the prefix
applied, the whole document voiced, and the record answering as its best
window, this model still cannot tell a question the corpus settles from
one it does not — so the refusal to let similarity confer confidence
(D-0020, re-confirmed in D-0045) now rests on a properly-plumbed
measurement, and the register stops predicting that the vector channel
will ever fund confidence at this model scale. What would reopen it is in
the review trigger, and it is not more plumbing.

---

## D-0047 · Removal is declared in the ledger and performed by a proven rewrite

```yaml
id: D-0047
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-11, whose trigger — any external or personal-data
  corpus — fired the day the proposals reader shipped and real authors'
  names began entering ledgers at runtime
evidence: [crates/tacit-core/src/redact.rs, crates/tacit-core/src/content.rs,
  docs/REGISTER.md]
review_trigger: before the repo goes public (U-6 remains); when the first
  real erasure request arrives, re-read this against what the request actually
  asks; if a court or regulator requires provable rather than matchable
  removal, the fingerprint upgrade and crypto-shredding reopen. Re-read
  2026-08-31 (D-0052) — the license half settled as D-0050
```

**Assertion.** Append-only and erasure meet in two halves, in D-0038's
shape: record first, rewrite second, witness kept. The *declaration* is a
new record kind — human-only like a verdict and for the same reason (no
sequence of agent calls may order data destroyed), refused without an
existing target and a stated ground, and as permanent as anything in the
log: the fact of a removal, who ordered it, and why, cannot themselves be
removed, only their wording withheld. The *removal* is `redact_store`: a
rewrite that replaces the withheld fields of the target's event with a
marker, stamps the husk with a receipt naming the declaration and a
fingerprint of what stood there, proves the rewritten log replays through
the full grammar, and only then lets it take the old log's place.

**Why this is not a tampering door.** The load path is the only source of
husks — a live append cannot mint the receipt — and a store refuses to open
when any husk's receipt fails to name a redaction record targeting that
very husk. "Redacted" is not a word anyone may write over anything; the
receipt is the difference between lawful removal and forgery, and the test
that forges one watches the store refuse it. What survives every scope is
what replay stands on: entity references, verdict actions, timestamps, and
the author's *kind* — so a promoted claim stays promoted after its author's
name is withheld, and a verdict still counts as humanly declared long after
the ledger stops knowing which human.

**What is deliberately not promised.** The rewrite renames a file; it does
not scrub disk sectors, backups, or upstream copies — destroying those is
the operator's legal duty, and crypto-shredding is the registered mechanical
shape if it is ever wanted. The fingerprint is a 64-bit hash: enough to
match a retained original against a husk, stated plainly as not
cryptographic proof. Entity labels and source references sit outside the
scope and are registered as U-46 rather than half-covered. And the MCP host
gains only visibility (`redacted_by` on every served record), not a tool:
declaring a redaction is a keeper-side human act, and applying one is an
operator running the rewrite against a store nothing else holds open.

---

## D-0048 · The view is a parameter, and its refusals are disclosed

```yaml
id: D-0048
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-40 — the view question P-08 raised, resolved without
  the heuristic the row warned against
evidence: [crates/tacit-core/src/retrieval.rs, crates/tacit-mcp/src/server.rs,
  docs/REGISTER.md, docs/PEP-GOLDEN.md]
review_trigger: if the disclosure is ever observed teaching agents to
  routinely re-ask with full history, the default view question reopens.
  Re-read 2026-08-31 (D-0052) — the language question closed as D-0049
  deciding no, so the view stays a parameter and nothing here waits
```

**Assertion.** A fair question about a rejected record was unanswerable from
the governed view, and the row that recorded this warned that routing such
questions is semantics no heuristic should guess. The resolution guesses
nothing, twice over. First, the view becomes a parameter the asker actually
holds: `tacit_search` takes `full_history`, the forensic `StateFilter::All`
that always existed, with every record labeled by its state. Second, the
engine discloses what the view refused: the lexical scan already reads the
postings of refused records and dropped them without a trace, so the same
pass now keeps them, and when the governed outcome is less than confident
the result names the strongest view-refused record — at the same coverage,
score, and reach bars a confident match must clear, so no new threshold
exists to tune. A caller can now tell "the corpus has nothing" from "your
view withholds what it has", which was the whole of the fault.

**The ratchet holds.** The disclosure is never an answer: it enters no
items, confers no confidence, and appears only beside weakness — a
confident answer is not second-guessed by its superseded predecessors,
which is the door P-13's trap closed and this deliberately does not reopen.
Acting on a disclosure means re-asking with the wider view, a choice left
to whoever knows what the question means — the same division of labor
D-0043 and D-0045 settled: the engine publishes, the meaning-bearing
consumer decides. The suite grades this honestly with its own verdict
class, `pointed+beyond`: a pass earned by pointing at exactly the agreed
record, distinct from answering and counted as neither abstention nor
assertion. P-08 recovers on it; the proposals suite stands at 20/24 and the
self-corpus did not move, because nothing about the governed plan changed.

**What this deliberately does not do.** It does not decide which questions
are *about* records rather than answered by them — that stays with the
asker, and with U-3 if a query language ever wants to express it. And the
disclosure is lexical only: a vector-reached refusal is not disclosed,
consistent with similarity never conferring anything (D-0020, D-0046).

---

## D-0049 · No query language: the first observed agent session asked for parameters

```yaml
id: D-0049
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-3 on its own trigger — observed real agent usage of
  the v1 MCP toolset, made observable first and then observed
evidence: [crates/tacit-mcp/src/store.rs, crates/tacit-mcp/src/server.rs,
  docs/REGISTER.md]
review_trigger: when a second, differently-shaped agent workload leaves an
  audit this reading does not cover; any move toward a query language starts
  by reading the accumulated audits, which now exist to be read
```

**Assertion.** The v1 toolset gains no query language, and for the first
time that is a decision made on evidence rather than deferral. U-3's
trigger read "observed real agent usage" — and usage was unobservable as
built, because the audit died with the host process. So the audit now
persists beside a durable store, plain lines appended and read back at
open, telemetry rather than record: losing a tail line in a crash loses a
data point, never knowledge. Then the usage happened: a real agent session
drove all ten tools against this repository's corpus over stdio, with every
call recorded — why-questions answered with provenance envelopes in one
call each, the search-to-history chain walked, both time axes probed, the
inbox and the contradictions read, and two records written back: a
proposed claim and a registered question, both stating faults the session
itself found, both now waiting for a person, which is the write path doing
exactly what D-0012 designed.

**What the observation showed.** Every friction found decomposes into a
typed parameter or a bound on an existing tool; nothing observed wanted
composition, expressions, or a grammar. The open-questions listing could
not be narrowed, so a question about one topic bought all eighteen gaps —
it now takes a query, ranked by the same rule the search's gap offers use,
because two rankings for one intent would drift apart. The pending inbox
returned one hundred forty-three full records for being asked — both
listings now take a limit and still report the true total, so a bound is
never mistaken for the whole. And record-time travel on a store synced
this morning honestly answers "not in the record" about last week — the
as-of output now publishes the valid-time answer beside it, read from the
envelope the ingest already fills, so the axis the question usually means
is visible even where the store's own memory is young. The multi-call
chains, by contrast, earned no composition: each step is a choice point
where the agent reads before deciding, and a language that batched them
would save nothing observed.

**What is deliberately not built.** An entity-centric read ("everything
about X") was imagined during the session and never actually reached for —
it stays unbuilt for exactly the reason this record exists: U-3 waited
eight days for evidence rather than taste, and the next tool should too.
The host also accepts unknown arguments silently, noted and left, pending
a session it actually misleads. The audit files now accumulate; whoever
next re-reads this decision starts by reading them.

---

## D-0050 · The engine ships under MIT OR Apache-2.0, at the user's option

```yaml
id: D-0050
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-17, declared by the owner when asked directly — the
  agent prepared the choice, the person made it, which is the write path's
  own rule applied to the project's paperwork
evidence: [LICENSE-MIT, LICENSE-APACHE, docs/priors/SUMMARY.md,
  docs/REGISTER.md]
review_trigger: relicensing is a one-way door and is not planned; re-read
  only if a patent claim arrives, a contributor agreement becomes necessary,
  or counsel's U-6 review surfaces a conflict
```

**Assertion.** Every crate in this workspace is dual-licensed MIT OR
Apache-2.0, the downstream user choosing which. The reasoning was on the
register for eight days before the choice: the priors argue permissive,
because fork-safety is the answer to single-vendor fragility — the surveyed
engines that thrived are permissive and the source-available ones carry
adoption friction the two-layer bet cannot afford. Within permissive, dual
is the Rust ecosystem's own convention, and it buys both halves at once:
the MIT text for integrators who want nothing but simplicity, and the
Apache text for the express patent grant an engine should offer — whose
trademark exclusion also quietly protects the registrable name, which is
the U-6 interaction this row predicted when it was written.

**What the license does not decide.** The crates stay `publish = false`:
whether the source is free and whether crates are pushed to a registry
under a name counsel has not reviewed are different questions, and the
second waits on U-6. Contributions are a future question — a single-author
repository needs no inbound terms today, and inventing them ahead of the
first contributor would be the register's least favorite kind of work.

**Consequence for the suite, taken in the same breath.** G-10 spent its
life as the canonical honest abstention — the license question, expected
to abstain citing U-17 — and this record makes that expectation
unsatisfiable, which is exactly the two-failures-cancelling shape the
stale-trigger check was built for after U-5. The question is re-agreed as
answerable by this record, and the abstain-with-citation path the suite
must keep exercising moves to a new question citing U-46, which is open
and expects to stay so until a person is modeled as an entity.

---

## D-0051 · A duplicate is a witness: disclosed at the door, folded in the inbox, never refused

```yaml
id: D-0051
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-12, whose trigger — data-model implementation —
  fired on 2026-08-23 and sat fired for eight days, the second row this week
  to teach that lesson
evidence: [crates/tacit-core/src/ledger.rs, crates/tacit-mcp/src/server.rs,
  docs/REGISTER.md]
review_trigger: when a reviewer first rules on a folded set, re-read whether
  the pairing served the set verdict it was built for; the meaning half
  reopens when U-23 does
```

**Assertion.** U-12 bundled two questions and practice had already answered
the first: records carry engine-minted ULIDs (D-0019 replays them; identity
is the engine's to assign, invariant 3), and content-addressing found its
real home at the document level, where the sync fingerprints source records
(D-0021 said so at the time — this record just stops the row pretending
otherwise). The second question — agents re-proposing duplicates — is
answered in three moves that are all precedent. The append stays legal:
byte-identical content from a second author is a second witness to one
claim, and refusing it would destroy an envelope the record is entitled to.
The tool discloses: a proposal or a registered question now comes back
naming the earliest identical record and its state, so an agent knows
whether it just witnessed, retried, or re-proposed the settled — published,
not decided, the D-0048 shape. And the inbox folds: a pending proposal
identical to an earlier pending one keeps its record and loses its separate
claim on a reviewer's attention, exactly as D-0024 folds superseded drafts,
with the pairing published so one set verdict (D-0034) can rule on all
copies as the single editorial act they are. Promote the head and the twin
re-emerges as its own queue entry, because the question it then poses — do
we need this twice? — is no longer "which copy".

**The boundary, drawn where the row drew it.** Identity here is
byte-identity: the fingerprint narrows, an equality check decides, and a
hash collision costs a comparison rather than a false duplicate. Two
paraphrases of one claim are invisible to all of this, deliberately — that
is the meaning half, it belongs to the keeper, and D-0045 measured that
meaning is not available at this model scale. It waits where U-23 waits,
and pretending a fingerprint can see it would be the bluff this project
grades hardest.

---

## D-0052 · A trigger is checked by the build, and acknowledged by a re-read

```yaml
id: D-0052
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: two rows in one week (U-11, U-12) sat with fired triggers for days —
  one of them for eight — while the golden questions' triggers could not
  have, because theirs had a mechanical check and the register's did not
evidence: [crates/tacit-keeper/src/register.rs, docs/REGISTER.md]
review_trigger: if the alarm is ever found firing on rows nobody intends to
  tend, it has become the cried-wolf failure this register names — narrow it
  or kill it, never mute it
```

**Assertion.** The stale-trigger check that has guarded the golden questions
since D-0028 now guards the register's own rows and the decisions' review
triggers. An open row, or a promoted decision, whose trigger names a
question that has since been resolved turns the build red — in the golden
gate and in a test — until someone re-reads it. The acknowledgment *is* the
re-read: reword the trigger to say what was found, naming the decision that
resolved things and never the resolved question, which is the one-line
convention that makes the alarm self-clearing exactly when the work
happened and not before. For a decision record, that rewording is an edit,
and the sync supersedes the record with provenance like any other edit —
re-reading is itself on the record.

**The first run, dealt with in the same change.** One row and fourteen
decisions were in arrears — including four from this very week, filed while
this week's own resolutions were being celebrated. Every one was re-read
and acknowledged before the check was allowed to gate, which is the only
honest way to give an alarm its first day (D-0028 did the same for the
question checks, and the register has said twice that an alarm nobody
reads is worse than none).

**Mechanical honesty about scope.** A trigger written as pure prose is
invisible to this check, and stays the quarterly re-read's to catch — this
owns only what a build can own: the subset that names ids, plus everything
the convention will cause future triggers to name. The quarterly re-read
(next: 2026-11-23) now starts from a smaller pile.

---

## D-0053 · Redaction reaches the entity's label and the source's reference

```yaml
id: D-0053
state: promoted
author: Greg Villa
recorded: 2026-08-31
valid_from: 2026-08-31
source: resolution of U-46, built ahead of its trigger on the owner's
  direction — removal capability is held before the demand, which is the
  same preparedness U-11 was resolved on
review_trigger: when a person is actually modeled as an entity in a synced
  corpus, re-read this against the real shape of that corpus; and with
  D-0047's own triggers, which govern the mechanism this extends
```

**Assertion.** The two gaps D-0047 stated plainly are closed with the same
two-halves mechanism it built. A redaction may now target an entity: the
declaration carries a tagged target (record or entity — tagged on the wire
because both ids serialize as bare ulids a reader cannot tell apart, and
widened the day after the mechanism shipped, while no durable store yet
held a declaration to migrate). The rewrite husks the entity's label,
stamps the receipt on the entity event, and the store refuses to open when
an entity's mark names no declaration of it — the same forgery rule record
husks answer to. Kind survives, because kind is structure; every record
about the entity still resolves by id; and a husked entity is no longer
findable by its label, which means a re-sync of the same upstream would
mint a fresh entity rather than reunite with the husk — correct, and worth
knowing: re-importing removed personal data is an operator's act, not a
collision. The second gap closes as a new scope: `Source` withholds the
envelope's source reference — a URL or citation carries a person as surely
as a body does — while the channel stays, naming a kind of provenance
rather than anyone in particular. `Record` scope now means all three.

**What a label redaction does not scrub, said where it can be read.**
Records that mention the person's name in prose keep it — each is its own
Content-scope declaration, and the entity's own listing enumerates exactly
the records a full erasure must visit. The primitive stays per-target
because every removal is a declared act; the walk across them is the
keeper's workflow, not the engine's guess.

**Ahead of the trigger, on the record.** No corpus yet models a person as
an entity, and U-46's trigger had not fired. It was built anyway, on the
owner's direction and on the argument that resolved U-11: a legal removal
capability is worth holding before the law knocks, and "we will build it
now" is the one answer an erasure request must never get. What D-0049
refused for a product feature — building ahead of evidence — is refused
here too for anything speculative about *shape*: the shape was already
agreed in the row, and only the build was waiting.

---

## D-0054 · The scrub carried the names it scans for, and the tree is corrected

```yaml
id: D-0054
state: promoted
author: Greg Villa
recorded: 2026-09-03
valid_from: 2026-09-03
source: the pre-publication scrub, run over every tracked file for the first
  time on the day the repository was to go public
evidence: [scripts/check-boundary.sh, docs/DISCLOSURE.md]
review_trigger: before any further rewrite of history, re-read the argument
  that made this one permissible; and when anyone other than the owner first
  runs the scrub, because the terms file is per-machine and that is the limit
  named below
```

**Assertion.** `scripts/check-boundary.sh` hard-coded the employer's name, its
mail domain and the names of its internal systems as the patterns it greps
for — the identifiers D-0010 forbids, written into the one file whose job was
to keep them out, and tracked since 2026-08-24. The scrub never saw it because
it scanned `docs/` and `crates/` and not itself. Found on 2026-09-03 by running
it over every tracked file before making the repository public; every other
file was clean. The patterns now live outside the tree
(`~/.config/tacit/boundary-terms`, or wherever `TACIT_BOUNDARY_TERMS` points),
one extended regex per line with a class prefix, and the script refuses to
report "clean" when that file is missing or holds nothing — a scrub with
nothing to look for is not a scrub. Its default targets now include
`scripts/`, the README and the manifest, so the gate reads itself.

**Why the rewrite is permissible, and what it changes.** D-0038 set the terms:
history is rewritten only when the reason is recorded first, in a commit made
before the rewrite, and the pre-rewrite record is preserved in full. Both hold.
This record is committed and signed ahead of the rewrite; a mirror clone is
taken after that commit (`../tacit-backup-20260903.git` — its head is named in
the register row this record amends, written after the mirror existed); and
the rewrite replaces the identifiers in every historical blob with neutral
placeholders and nothing else. Commit hashes change; the signatures on every
commit are dropped by the rewrite, as D-0038's were; timestamps and messages
are untouched — the messages were checked and carry none of the names. The
older mirror from D-0038 stays where it is.

**What this does not fix, said plainly.** The terms file is per-machine. A
second contributor cannot run the scrub until they have one, and the file
cannot be shipped without shipping the names — so the gate is the owner's,
not the project's, until someone needs it to be otherwise. And the finding is
a fact about the gate, not the boundary: nothing employer-owned was ever in
the tree except the list of things that must not be. DISCLOSURE.md records it
beside the two facts that already cut the other way, for the same reason they
are there.

---

## D-0055 · The person's half of the ratchet gets a command, and the store gets a lock

```yaml
id: D-0055
state: promoted
author: Greg Villa
recorded: 2026-09-04
valid_from: 2026-09-04
source: the first cold read of the public repository, which found an agent's
  proposal with nowhere to go
evidence: [REQUIREMENTS.md R-8, REQUIREMENTS.md R-11]
review_trigger: when a keyboard verdict needs to carry a signature rather than
  an asserted name; or when a second machine, or a second user on one machine,
  needs to hold a store
```

**Assertion.** `tacit-keeper` is a binary in the keeper crate with four
commands over a `--store`: `pending` lists the inbox; `promote`, `reject` and
`retire` each append one verdict under a typed name and a required rationale.
The actions are the ledger's own — no new grammar, no bypass — and an illegal
one is refused by the same check that refuses a transcribed verdict, with the
ledger's reason printed. The tool surface is unchanged: an agent still cannot
promote, and the integration test that says so still passes. What the command
records about identity is the honest minimum. The name is asserted at the
keyboard and the verdict's author detail says exactly that, in the
attestation vocabulary D-0025 established, so `review_trust` files a keyboard
promotion under nothing-to-recheck rather than losing it, and "which
promotions rest on a name someone typed" is a question the record answers.

**The lock.** D-0015 stated that one process owns a store at a time and
called it file-lock semantics; nothing implemented it, because nothing needed
to — only the host ever opened a store. A second process that appends to the
same log is the event D-0022 named as its review trigger, and the re-read
concluded what the trigger implied: the host holds the ledger in memory and
appends at the file's end, so a verdict appended underneath it would carry a
later record-time than the host's next append, and the log would stop
replaying. The answer is not to make record-time comparable across processes
but to keep two processes out of one log. A sidecar file beside the store,
created exclusively, holding the holder's pid and name, removed on drop; both
the host and the command take it before they open, and the refusal names who
has it. A dead holder's file is believed only while its pid answers, then
taken over and said so. The measurement examples that accept `--store` do not
take it; they are run against stores nobody serves, and that is stated rather
than enforced.

**Why the ingest path was not enough.** Transcription is right for decisions:
the document is upstream, the words have a git history, and the verdict
carries what git can establish. An agent's proposal has no document. Writing
a decision *about* it creates a new promoted claim and leaves the proposal
pending forever — the inbox filled and nothing could empty it, which is R-8's
lifecycle with one transition missing. The first stranger to read the
repository found this in an afternoon, and it was the difference between a
project that demonstrates the ratchet and one a team could run.

**Alternatives rejected.** A promote tool behind a flag on the MCP host (the
absence of one is the surface's most legible property, and a flag is an
agent's `--yes`); requiring a signed commit for a keyboard verdict (there is
no commit — the verdict is the act, and an ad hoc signing scheme would be a
second attestation vocabulary; the review trigger names the day this is
wanted); a set verdict from the command line (enumerating ids by hand is how
the wrong one gets in; D-0034's editorial act stays with the ingest); an
advisory lock through a crate (a dependency in the engine's neighbour for a
sidecar file the standard library writes).

**Stated limits.** Liveness is asked with `kill -0`, which also fails for a
live process owned by another user, so on a shared machine a holder running
as someone else reads as gone; single-tenant is the v1 regime. The name on a
keyboard verdict is whatever was typed. Both are in the record beside the
mechanism, and the review trigger names the two events that would change
either.

---

## D-0056 · An abstention travels with its reasons

```yaml
id: D-0056
state: promoted
author: Greg Villa
recorded: 2026-09-04
valid_from: 2026-09-04
source: the first cold read of the public repository, which met a weak match
  with the right record in first place and could not tell why
evidence: [REQUIREMENTS.md R-10, docs/GOLDEN.md]
review_trigger: when a fourth condition joins coverage, known and score in the
  confidence rule, or when a client is found reading the summary sentence
  instead of the numbers it is made from
```

**Assertion.** `tacit_search` returns a `why` beside its outcome: the first
item's coverage and the bar it was read against, how much of the question the
record can speak to at all and that bar, which of the rule's conditions fell
short by name, every term the index read as a neighbour, every discriminating
term the record has never written, and the same in one sentence. The numbers
are the engine's own — the two that `Retrieved` has carried since D-0043 with
a comment saying they were published beside the outcome, which was true of
the library and not of the wire. One addition to the engine: the words behind
`known` are now named, not only weighed. Nothing is judged twice, and no new
quantity confers confidence; D-0042's refusal stands.

**Forces.** R-10 makes abstention an answer, and an answer that cannot say
why it declined is a shrug with a tag on it. The stranger's case was exact: a
three-record store, the right record in first place, and the reply "weak
matches". Read from outside, that is the engine finding the answer and
refusing to say so. Read with the numbers — which this record's own field
produced, and which corrected the account its author was about to write —
the record could speak to only half the question, because one word of it,
"keep", had never been written there and carried the other half of the
weight; the first item covered everything that could be covered. The
reader's next move is to rephrase around that word, not to distrust the
record, and nothing in the reply said which word. The instrument that says
this existed (`explain`), as an example
binary over this repository's own suites, which is the one place a user's
question never is. What the client needs is the smaller half of what the
instrument prints, and it needs it in the reply.

**Alternatives rejected.** Printing the summary sentence alone (a sentence is
for a person; a client that wants to act needs the fields, and a sentence
without them invites paraphrase — the trigger names the day that is seen);
lowering the bar so the stranger's case passes (measured and refused twice,
D-0042 and D-0043, and a bar moved for one corpus is fitted to it); exposing
`explain` as a tool (its other half — fused against lexical against vector,
with the expected record's rank in each — is about a suite's question, and a
client has no expected record).

---

## D-0057 · A durable store is rehearsed before it is written

```yaml
id: D-0057
state: promoted
author: Greg Villa
recorded: 2026-09-04
valid_from: 2026-09-04
source: the first cold read of the public repository, whose first refused
  ingest left fifteen events in the store it had refused
evidence: [REQUIREMENTS.md R-9, docs/REGISTER.md]
review_trigger: when a fault survives the rehearsal and fails the real pass
  — that is a sync bug to file, and if it recurs the rehearsal needs the
  store's history; or when a corpus is large enough that ingesting it twice
  in memory is a cost anyone can measure
```

**Assertion.** When the ledger is durable, `ingest_text_with` first ingests
the same documents, with the same attestation, into a scratch ledger in
memory, and begins the real pass only if every record passes. A document
fault — an unaccepted state, a bad date, evidence that resolves to no file, a
hypothesis whose sections say claim, a register without an owner — is now
found before the first byte reaches the log. In-memory ledgers are not
rehearsed: a failed pass there leaves nothing anyone opens again.

**Forces.** The parsers ran first and always had, which is why this looked
solved from inside. But a record's state, its dates and its evidence are
judged inside the append phases, record by record, and the phases append as
they go — so a fault in the third record landed after the first two, their
titles, their mention edges and the register's gaps were on disk. The store
then held a corpus its author had just been told was refused. Reruns were
idempotent, so nothing duplicated, and the README said so — a caveat where a
guarantee belonged. R-9 asks for restarts that are boring, and a refusal that
leaves state behind is the opposite: the next run's "replayed fifteen
events" is a question with no good answer.

**Why a rehearsal and not a transaction.** The engine has no transaction and
this record does not give it one. An append is fsynced before the in-memory
commit (D-0019) and that discipline is worth more than atomicity across a
batch; staging the batch in the engine would mean either a second commit
path or records that exist in memory and not on disk, each of which is the
bypass D-0019 exists to prevent. The keeper already owns judgment about
documents; judging the whole document before writing any of it is the same
job done in the right order. The cost is one extra in-memory ingest, which
is microseconds per record (U-25's measurement), against a durable pass that
is milliseconds per record.

**Stated limit.** The rehearsal sees the document and not the store's
history. A disposition that exists only against prior records — an edited
record superseding its predecessor, a resolution the store has since
retired — runs paths the rehearsal never took; those paths report rather
than fail by design (U-19, D-0021), so a failure that survives the rehearsal
is a bug in the sync and the review trigger says what to do about it.

---

## D-0058 · Two kinds of noise leave the list, measured first

```yaml
id: D-0058
state: promoted
author: Greg Villa
recorded: 2026-09-04
valid_from: 2026-09-04
source: the first cold read of the public repository, which called two things
  in every search result noise, and the sweep that agreed
evidence: [REQUIREMENTS.md R-10, docs/GOLDEN.md, docs/PEP-GOLDEN.md]
review_trigger: when `noise_sweep` shows either setting moving a question on
  either suite; or when a corpus arrives whose title attribute is not the
  record's own shorter half
```

**Assertion.** Two query settings, both defaulted on, both swept on both
suites before they were. `drop_uncovered` removes, from a list in which some
item covers part of the question, the items reached by similarity alone that
cover none of it — beside a covered item such an item cannot be evidence
under D-0020, and fusion can seat one first, so that the outcome is read from
a record sharing no word with the question. When nothing covers the question
they stay: they are the reach D-0020 bought, reported as weak, as two tests
older than this record insist. `titles: PreferBody` treats a record's title claim as the shorter half of the
same record: listed after its body it is dropped, listed before it it hands
its slot to the body, and with no body in the list it stays. Both are fields
on `Query`, so the old list is one setting away and the measurement can be
re-run. Nothing is re-scored; a body that takes a title's slot brings its
own coverage and relevance.

**The measurement.** `cargo run --release -p tacit-keeper --example
noise_sweep` grades six settings over both corpora and counts the noise each
removes from the lists it graded. On the self-hosting suite, twenty-four
questions assembled 240 items, of which 34 were similarity-only with zero
coverage and 19 were titles listed beside an item about the same record. On
the proposals suite, 214 items, 10 uncovered, no duplicate titles. Under
every setting, on both suites, the pass count stayed at 20 of 24 and no
question's verdict moved. The setting adopted here leaves 0 duplicate titles
on each and 0 uncovered items on the proposals suite; on the self-hosting
suite 10 uncovered items remain, all of them in lists where nothing covers
the question — the reach, kept on purpose. Folding titles only behind their
bodies left 6 and 8 duplicates, because a title that outranks its body
arrives first. The first draft of the rule dropped every uncovered item and
two tests older than it failed; the narrowing is theirs. And this record's
own first wording turned G-07 underconfident — the vector ranker seated
it first with a fifth of the coverage, on two incidental words — which is
U-37 happening to the record that was being written about drift; the two
words went, and the suite came back.

**Why this is not D-0043's refusal.** That record refused to read confidence
from the best-covering item because the best-covering item of an
unanswerable question is the longest one. Handing a title's slot to its body
is not a search for the best: the slot was earned by the title's rank, the
body is the same record's fuller text, and the swap was graded for exactly
the flip D-0043 feared and produced none across forty-eight questions. The
review trigger is that flip, and it reopens this.

**Alternatives rejected.** Merging title text into the body's indexed text
(changes every ranking on both corpora for a display problem, and the title
as its own document is what lets a query of the title alone find the
record); folding by shared subject rather than by title role (hides distinct
claims about one entity, which is the graph's whole use); dropping every
zero-coverage item regardless of how it was reached (expanded context is
zero-coverage by construction and is asked for explicitly); dropping
similarity-only items even when they are all the list has (that is the
spelling-and-suffix reach of D-0020, and the tests that hold it down are the
reason the rule reads as it does).

---

## D-0059 · The suite grades any record, not only this one

```yaml
id: D-0059
state: promoted
author: Greg Villa
recorded: 2026-09-04
valid_from: 2026-09-04
source: the first cold read of the public repository, which found the graded
  suite to be the project's most transferable idea and its least transferable
  code
evidence: [docs/GOLDEN.md, REQUIREMENTS.md R-10]
review_trigger: when a second corpus format exists and its golden file cannot
  be expressed in the five-column table; or when a user's suite needs an
  audit this repository's does not
```

**Assertion.** `golden` takes a corpus root as its argument and `explain`
takes `--corpus <root>`. Given one, each ingests that root's decision and
register documents and grades or explains it against the root's own
`docs/GOLDEN.md`, in the five-column format this repository uses, under the
same audits: fired triggers, quoted questions, acquired vocabulary, unowned
questions, and pending markers that must name a registered unknown. Without
an argument both behave exactly as before. Nothing in the runner was
specific to this repository except the path.

**Forces.** Abstention graded as a pass is the idea a stranger takes away
from this repository, and until now the only corpus it could be applied to
was this one — the runner joined a hard-coded root to three file names. A
team that has written its decisions in the document format and served them
over MCP had no way to ask the question the suite asks: does our record
answer what it should and decline what it should not, and does that stay
true as the record grows. The format needs no change; the Acme corpus from
the cold read carried a four-question suite on the first try, two answers
and two abstentions, one citing its register, and graded four of four with
the baseline printed for pasting — after the quote audit refused the first
draft, because one question repeated five words of a record's title. The
audits are for the first-time user before they are for anyone.

**Alternatives rejected.** A separate binary (the runner is the instrument,
and two of them would drift); a golden file path independent of the root
(a suite agreed against one corpus and run over another measures nothing —
the proposals suite's lesson — so the file lives with the record it grades);
relaxing the audits for a user corpus (they are the reason the suite stays
honest as a record grows, and a first-time user is exactly who they protect).

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

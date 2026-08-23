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

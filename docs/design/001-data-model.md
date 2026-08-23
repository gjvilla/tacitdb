# Design 001 — The Tacit Data Model

```yaml
id: DES-001
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: phase interview / data-model round (verdicts: D-0012, D-0013, D-0014)
evidence: [../DECISIONS.md, ../REQUIREMENTS.md, ../priors/SUMMARY.md]
review_trigger: first storage implementation that cannot honor an invariant below,
  or envelope schema evolution (U-13)
```

This document settles **U-1** (the write-path placement) and defines the logical
data model for Tacit v1 — the last document before Rust. It deliberately does not
choose storage structures (U-5), runtime shape (U-2), or the algorithm set (U-4);
it defines what those choices must preserve.

The one-paragraph model: **Tacit stores two ledgers. The governed ledger holds
append-only *records* — claims, gaps, hypotheses, and verdicts — each wrapped in a
required envelope, moving through a lifecycle whose grammar the engine enforces.
The instrument panel holds *measurements* — machine-owned, cheaply mutable signals.
*Entities* anchor identity. The traversable knowledge graph is a *projection* of
promoted, currently-valid claims over those entities, overlaid with measurements,
with vectors and fulltext as derived indexes. Retrieval is one hybrid plan that can
return honest abstention, including "this is a registered open question."**

---

## 1. Concepts

### 1.1 Entity — the identity layer

An entity is a stable anchor for something claims can be about: a person, machine,
process, standard, document, team, term.

```
Entity { id, kind, label }
```

`kind` is an open vocabulary (no fixed ontology in the engine). `label` is a
display convenience; truth about an entity — including its names and aliases —
lives in claims. Entities carry no envelope of their own; the record that
introduced an entity is ordinary provenance (the creating claim references it).
Entities are never deleted; an entity with no promoted claims is simply dark.

### 1.2 Records — the governed ledger

Four record kinds share one envelope (§2) and one append-only store:

- **claim** — an assertion about the world. Content shapes (v1):
  - `attribute` — (subject entity, name, typed value)
  - `relation` — (subject entity, predicate, object entity, properties)
  - `pattern` — (context, forces, solution) — the pattern-language unit as a
    first-class shape, honoring "the answer together with the conditions that
    make it true"
  - `text` — free-form content for what fits no shape yet
- **gap** — a registered known-unknown: a named question without an agreed answer,
  with the territory it covers (text + optional entity refs). Gaps are what make
  honest abstention possible (R-10).
- **hypothesis** — a dated, falsifiable prediction with a `score_by` date and,
  eventually, a scored outcome (the H-0001 shape).
- **verdict** — a decision about another record's lifecycle: promote, retire,
  reject, answer (for gaps), score (for hypotheses). Verdicts are the only
  mechanism of state change and are themselves immutable once written.

### 1.3 Measurements — the instrument panel

Machine-owned signals attached to an entity or a projected edge:

```
Measurement { target_ref, name, value, updated_at, updated_by }
```

Edge success rates, traversal costs, usage counts, staleness scores. Mutable in
place, cheap to update, no envelope, no verdicts; optional decimated history.
Measurements are **non-authoritative**: they inform ranking, pathfinding (R-5),
and drift detection, but they are never an answer to "what does the organization
know," and retrieval excludes them from knowledge results unless instrumentation
is explicitly requested. Agents update measurements freely — this is the ledger
where the graph learns nightly without convening a huddle over a decimal.

**Embeddings are neither ledger.** They are derived index artifacts keyed by
(record id, content hash, model id) — rebuildable at any time, never a source of
truth (see §6).

### 1.4 Sources and evidence

A source is an entity of kind `source` (a document, an interview, a huddle, a
system). Evidence links in an envelope reference a source, optionally with a span:

```
Evidence { source_ref, span? }
```

An answer must be reconstructable down to its sources (R-6): claim → evidence →
source, every hop queryable.

---

## 2. The envelope

Required on every record. The envelope is what makes a bare fact unstorable.

| Field | Type | Rules |
|---|---|---|
| `id` | ULID | engine-assigned (see U-12 for content-addressing question) |
| `kind` | enum | claim / gap / hypothesis / verdict |
| `author` | { name, kind: human\|agent, detail? } | required; `kind` is load-bearing (§3) |
| `source` | { channel, ref? } | where this entered the record: interview, huddle, ingest, migration, agent-pipeline |
| `recorded_at` | timestamp | engine-assigned record-time; immutable |
| `valid_from` | timestamp | when the content is/was true from; defaults to `recorded_at` |
| `valid_to` | timestamp? | open-ended if absent |
| `evidence` | Evidence[] | may be empty on proposal; promotion of evidence-less claims is a keeper-policy choice, not an engine rule |
| `review_trigger` | { due_at? , on_event? } | queryable; promoted claims without one are flagged (drift hygiene) |
| `supersedes` | record ref? | the record this corrects or replaces |
| `state` | derived | never written directly — computed from verdicts (§3) |

Envelope versioning: every record stores `envelope_version` (v1 = 1). The envelope
will be wrong somewhere; U-13 owns evolution policy.

---

## 3. Lifecycle and the ratchet (resolves U-1)

**Verdict of the phase interview (D-0012): the grammar lives in the engine; the
truth of identity lives in the keeper.** The engine cannot verify that a promotion
was really decided by the right humans — but it can make the record structurally
unable to lie about its shape. Authentication and authorization of *who* counts
(identity, roles, huddle membership) belong to the keeper layer above.

### 3.1 States

- claim: `proposed → promoted → retired`, with `proposed → rejected`
- gap: `registered → answered | withdrawn` (answered links the promoted claim
  that answers it)
- hypothesis: `registered → scored { outcome }`
- verdict: immutable; a later verdict supersedes an earlier one, nothing retracts

A single verdict may promote a superseding claim *and* retire the record it
supersedes — one decision, one record, both transitions.

### 3.2 The engine invariants

The ratchet as schema. Every invariant is testable and none can be skipped by an
application:

1. **No envelope, no write.** Every record carries a complete envelope.
2. **Append-only.** No in-place mutation, no deletion. Corrections supersede.
   (Legal redaction is a designed exception that preserves the chain — U-11.)
3. **Record-time is engine-assigned.** `recorded_at` is set by the engine;
   history is never rewritten.
4. **State changes only by verdict.** A record's state is derived exclusively
   from verdict records referencing it. *Corollary (added 2026-08-23, D-0016):
   the write path holds no reference to any projection or derived view. A
   verdict validated against a stale view would write permanent corruption
   into an append-only log, where a stale read is merely a stale read.*
5. **Promotion and retirement verdicts must declare `author.kind = human`.**
   The engine enforces the declaration; the keeper authenticates it.
6. **Agents propose; they never promote.** Agent authors may create proposed
   claims, registered gaps, registered hypotheses, and measurements — nothing
   agent-authored reaches `promoted` without a human verdict.
7. **Contradictions surface; they are not silently resolved.** Two promoted
   claims about the same subject and attribute/predicate with overlapping
   valid-time are legal but flagged and queryable — a drift alarm, not a
   constraint violation. Promotion never auto-wins over a standing claim.
8. **Measurements never masquerade as knowledge.** They are excluded from
   knowledge retrieval unless instrumentation is explicitly requested.

Invariants 4–6 are the write-path ratchet of thesis claim 6, made schema: the
machine proposes, the human verdict promotes, and erosion of that commitment now
requires changing the engine, not skipping a convention.

---

## 4. Bitemporality

Two independent axes on every record:

- **Record-time** (`recorded_at`, plus the verdict timestamps): what the record
  contained at any past moment. Answers *"what did we know on date D?"* —
  including which claims were promoted then.
- **Valid-time** (`valid_from` / `valid_to`): when the content is true in the
  world. Answers *"what was true at time T?"*

Corrections are supersessions: a new record with `supersedes` set, its own
valid-time, and its own verdict. The superseded record keeps its history — the
original prediction stays on file beside whatever replaces it. As-of queries
parameterize both axes plus a state filter; the default view is
(record-time = now, valid-time = now, state = promoted). Semantics follow the
valid-time design proven in prior art (per-record validity keys with as-of query
sugar) extended with record-time; edge cases are owned by U-14 with
property-based tests required before the temporal layer is called done.

---

## 5. The projected graph

*Amended 2026-08-23 by D-0016; the original text is preserved in that record's
alternatives. The amendment was written before the implementation, not after.*

The traversable knowledge graph is a **view**, not the store:

- **Nodes**: entities, including dark ones with no admitted claims — identity
  is not claim-derived, and hiding an entity would make the graph lie about
  what exists.
- **Edges**: `relation` claims admitted by the view.
- **Node properties**: `attribute` claims admitted by the view.
- **Overlay**: measurements, as mutable properties on nodes and edges.

Three amendments to the original design, each load-bearing:

**The projection is a caller-held value, not engine state.** The ledger does
not own one. A ledger-owned cache hands the storage layer (U-5) a
cache-coherence problem it has not agreed to solve, and it forecloses holding
several views at different frontiers at once.

**The write path holds no reference to any projection, by construction.** This
is the difference between a recoverable cache bug and permanent corruption of
an append-only log: no verdict may ever be validated against a stale view.
(See the note on invariant 4 in §3.2.)

**Valid-time is a read parameter and never engine state.** This is what makes
U-10 provable rather than a cache with a TTL. The maintained structure is a
*candidate index*: a pure fold over the log carrying no view parameters at all
— no valid-time, no state filter, no record-time. Nothing is ever removed from
it; retirement, rejection, and expiry are read-time predicates over candidate
slots. Because the fold is monotone in the log, incremental maintenance can
only append and flip, never retract, and `rebuild` is *defined* as
`empty().advance()` — one fold, one cursor, so equivalence is definitional
rather than hoped for. The cost is that buckets accumulate dead slots: a
bounded, measurable cost, not a correctness risk.

A view therefore costs nothing to vary. Changing valid-time, state filter, or
author filter constructs a different reader over the same index; only a
historical record-time takes a slower path, resolving state through the
ledger's own `state_of_at` rather than the fold. Every projected element
reports its own lifecycle state, so **a non-default view labels rather than
lies**: an include-proposed view shows proposed edges *marked proposed*, and
the ratchet stays visible in the graph, not only in the write path.

Conflicting promoted claims surface as a conflicted property with no accessor
that returns a single value — the caller must visibly decide what to do about
a conflict, because silently picking a winner is what invariant 7 exists to
prevent. Two `relation` claims over the same triple are two parallel edges,
not a conflict: relation cardinality is semantics the engine cannot know
(U-15), and edge identity is the claim record, which is what keys the
instrument panel.

Traversal and weighted shortest paths (Dijkstra over measurement-valued costs
— R-5) run against views. Because edge weights live in the instrument panel,
agents update them nightly with no projection rebuild, no claim, and no
verdict.

---

## 6. Derived indexes

Vector (ANN over embeddings of claim/gap content), fulltext (lexical over the
same), and any future index are **derived artifacts**: rebuildable from the
ledger, versioned by (content hash, model id), never authoritative. Index entries
carry the record's envelope discriminants (state, author-kind, validity, entity
refs) so that filtered search is native (R-1): a query scoped to
"promoted claims valid now, about entity E" is answered *inside* the index
traversal, never by post-filtering an oversized candidate set.

---

## 7. Retrieval semantics

One call, one plan (R-2):

```
retrieve {
  query: text | vector,
  lexical?: terms,
  filter: envelope predicates (state, author.kind, valid_at, source, entity scope),
  expand?: { hops, predicates, direction },
  fusion: rrf | weighted,
  budget: { k, max_tokens }
}
```

Returns ranked context items — each a claim with its envelope and evidence refs,
optionally with the traversal path that justified its inclusion — plus an
**outcome tag** that makes abstention first-class (R-10):

- `matches` — results above threshold
- `weak_matches` — best results below threshold, explicitly labeled, never
  silently blended
- `none` — the record has nothing
- `registered_gap` — the query's territory intersects one or more registered
  gaps; returned *with* the gap records, and it can co-occur with `matches`
  ("here is what is promoted, and here is the open question standing next to it")

Gaps are indexed like claims (vector + lexical), which is what makes
`registered_gap` detection a retrieval outcome instead of an application
heuristic. An honest "I don't know, and here is the registered question" is a
successful query.

---

## 8. API sketch (v1 surface)

Typed operations only (D-0007); MCP tools mirror them one-to-one with an audit
log (R-11). Names indicative, not final:

- **Write**: `add_entity`, `add_source`, `propose_claim`, `register_gap`,
  `register_hypothesis`, `render_verdict` (promote / retire / reject / answer /
  score), `record_measurement`
- **Read**: `get_record`, `record_history`, `retrieve` (§7), `paths` (weighted),
  `as_of` (view spec), `pending_proposals`, `due_for_review`, `contradictions`,
  `instrument_panel`

`pending_proposals`, `due_for_review`, and `contradictions` are the keeper's
work-queue primitives — the curation cadence as three queries.

---

## 9. Requirements mapping

| Req | Where the model answers it |
|---|---|
| R-1 filtered ANN | §6 — envelope discriminants inside index traversal |
| R-2 hybrid one-plan | §7 — single retrieve call with fusion and budget |
| R-3 bounded bulk mutation | storage-phase obligation; model helps — append-only supersession streams naturally |
| R-4 no plugins | unchanged; nothing here requires an extension tier |
| R-5 learning edges | §1.3, §5 — measurements overlay; weighted paths on projections |
| R-6 provenance native | §1.4, §2 — envelope + evidence chain; envelope-less writes rejected (inv. 1) |
| R-7 temporal native | §4 — bitemporal axes, as-of, supersession |
| R-8 lifecycle states | §3 — resolved from "pending U-1" to invariants 4–6 |
| R-9 boring ops | runtime-phase obligation (U-2) |
| R-10 abstention | §7 outcome tags; gaps as indexed records |
| R-11 constrained agent surface | §8 — typed ops + audited MCP mirror |

---

## 10. Blind-spot pass (what a storage veteran would challenge)

Run per the register's standing practice; each finding is now a registered
unknown:

- **U-10 — Projection maintenance correctness.** Incremental materialized-view
  maintenance under concurrent promotions/supersessions is the classic trap.
  Mitigation direction: the ledger is an event source — the projection must be
  deterministically rebuildable, and the incremental path must be proven
  equivalent to rebuild (property-based tests). *Trigger: before storage code
  relies on incremental updates.*
- **U-11 — Redaction vs append-only.** A keeper of organizational knowledge will
  eventually face a legally binding "remove this" (privacy, discovery). Design a
  redaction record that blanks content while preserving the record's existence,
  chain, and verdicts — possibly crypto-shredding. *Trigger: before any external
  or personal-data corpus.*
- **U-12 — ID and dedup strategy.** ULIDs vs content-addressing; agents will
  re-propose semantically duplicate claims. Exact-duplicate rejection is engine
  grammar; semantic dedup is likely keeper policy — where is the line? *Trigger:
  data-model implementation.*
- **U-13 — Envelope evolution.** Envelope v1 will be wrong somewhere; records
  carry `envelope_version`, and migration policy must be designed, not improvised.
  *Trigger: first needed envelope change.*
- **U-14 — Bitemporal edge cases.** Corrections-of-corrections, overlapping
  validity, timezone/precision discipline. Adopt property-based tests against a
  reference semantics before the temporal layer is called done. *Trigger:
  temporal implementation.*
- **U-15 — Contradiction detection scope.** Same subject + same attribute is
  cheap grammar; "same meaning, different phrasing" is semantics. Engine detects
  exact-scope contradictions (invariant 7); semantic contradiction is a keeper
  concern feeding on retrieval. *Trigger: retrieval implementation.*

---

## 11. Deliberately deferred

- Runtime shape (U-2) — this model is agnostic; the projection/serving split is
  compatible with embedded and served forms.
- Algorithm set beyond weighted paths (U-4).
- Storage structures, on-disk format, concurrency control (U-5) — with one new
  input: the governed ledger is naturally event-sourced, which makes
  log-plus-projections a strong storage candidate; decide there, not here.
- Identity/auth (keeper layer, per D-0012).
- Ontology/schema for entity kinds and predicates (keeper/corpus concern; the
  engine stays vocabulary-open).

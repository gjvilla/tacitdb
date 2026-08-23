# Tacit — Requirements from Scar Tissue

These requirements are derived from several years of operating property-graph
databases under production graph-RAG and agentic workloads. They are stated
generically and carry no third-party code, data, or identifiers — the experience is
the author's; the source systems are not. Each requirement names the scar that
taught it and an acceptance criterion Tacit must meet.

Scale context for all of them: the target is the **modest-scale regime** —
10^5–10^7 nodes, up to ~10^8 edges, 10^5–10^7 embedding vectors at 1024–1536
dimensions, single machine. Nothing here is a billion-node problem; the hard
requirements are feature depth and operational predictability, not distribution.

---

## R-1 · Filtered vector search is a first-class index operation

**Scar.** Incumbent vector indexes could not pre-filter on a property, so scoped
similarity queries in production abandoned the index and brute-forced cosine
similarity over a label scan.

**Acceptance.** A kNN query with a property/label/envelope predicate uses the index
(pre-filtered or filter-aware traversal, not post-filtering an oversized candidate
set), returns exact-enough results at the scale regime above, and never silently
degrades to a full scan.

## R-2 · Hybrid retrieval is one query plan

**Scar.** Vector search, lexical/fulltext search, graph expansion, and rank fusion
(RRF and weighted variants) each lived in separate systems, glued together with
application code, per-call network round-trips, and hand-tuned caches.

**Acceptance.** One engine call expresses: vector candidates + lexical candidates +
bounded graph expansion + fusion + a result budget, and returns assembled context.
The application never re-implements fusion.

## R-3 · Bounded-memory bulk mutation

**Scar.** A large detach-delete blew past transaction memory caps mid-transaction;
the driver retried the same OOM silently, forever. The fix required chunked
transactions, which changed commit semantics and leaked into every caller's API.

**Acceptance.** Bulk delete/load/update streams in bounded memory by design. There
is no transaction-size tuning knob whose wrong value is discovered in production.

## R-4 · Zero plugin landmines

**Scar.** A production deploy crashed on boot because a procedure library the code
assumed was present hadn't been baked into the new image. Extension jars were also
manually deleted to save memory.

**Acceptance.** Tacit ships as a single library/binary. Every advertised function
exists in the default build. There is no plugin tier.

## R-5 · The graph learns: mutable weights, native weighted paths

**Scar.** The most valuable production pattern was agents updating edge weights
nightly from observed outcomes (success rates as costs), consumed by weighted
shortest-path queries — and it required a licensed add-on library, warm-cached
in-memory projections with TTLs, and memory headroom tuning.

**Acceptance.** Edge weights are ordinary, cheaply-mutable properties. Weighted
shortest-path (Dijkstra; k-shortest/Yen's) runs natively on current weights with no
projection step, no add-on, no rebuild.

## R-6 · Provenance is native

**Scar.** Evidence chains (assertion → evidence → source span → source document)
and trust envelopes (source system, as-of stamps) were hand-rolled node conventions,
invisible to the engine, unenforceable, and re-invented per application.

**Acceptance.** The assertion + envelope unit (D-0004) is the storage schema.
Queries can require, filter on, and return provenance. An answer can always be
reconstructed down to its sources. Envelope-less writes are rejected.

## R-7 · Temporal is native

**Scar.** Staleness was handled by naming conventions filtered at query time,
decay was a batch job halving a score, and "what did the record say when?" was
unanswerable.

**Acceptance.** Valid-time and record-time on every assertion; as-of queries;
staleness/review-trigger state queryable ("what is due for review?", "what changed
since?"). Records retire; they are not silently overwritten.

## R-8 · Lifecycle states (pending U-1)

**Scar.** Agent write-back in production was fire-and-forget: proposals became
truth by insertion. Nothing distinguished machine-proposed knowledge from
human-promoted knowledge.

**Acceptance.** Proposed → promoted → retired is representable and queryable, with
verdict provenance on transitions. Whether the engine *enforces* transition rules or
provides primitives for the layer above is registered unknown U-1 (blocks storage
code; see REGISTER.md).

## R-9 · Boring operations

**Scar.** Production required a seven-day retry-backoff layer, a circuit breaker,
a two-tier cache, connection-pool liveness tuning, and log-noise suppression — all
compensating for the engine's client-server failure modes.

**Acceptance.** Crash-safe by default; restarts are boring; no client is ever
expected to implement retry choreography. If a serving layer exists, its failure
modes must not require any of the machinery listed above.

## R-10 · Honest retrieval: abstention is an answer

**Scar.** Retrieval pipelines returned their best match no matter how bad; "no
answer" and "low-confidence answer" were indistinguishable, and known gaps in the
record were invisible to the machine.

**Acceptance.** The engine distinguishes: match / matches-below-threshold / no
match / **registered gap** (a known-unknown record covering the query's territory).
Registered gaps are first-class, storable, and retrievable — so the system can say
"this is a known open question" with provenance.

## R-11 · Agent-facing surface is constrained and audited

**Scar.** Production agent access worked best through a small set of typed,
rate-limited, audited tools — not through raw query strings.

**Acceptance.** The MCP toolset (D-0007) exposes typed operations with an audit
log. Raw storage access exists only behind the typed API.

---

## Non-requirements (deliberately shed)

Inherited use cases Tacit does **not** carry, per the founding decision to shed
what does not serve AI-native knowledge work:

- Cypher (or any query-language) compatibility or parity — see D-0007
- Billion-node scale, sharding, distributed consensus
- General-purpose OLTP for arbitrary applications
- Multi-tenant server administration (revisit only after U-2)
- A plugin/extension ecosystem — see R-4
- Visualization tooling (keeper-layer concern, not engine)
- BI/analytics workloads beyond what retrieval and the golden suite need

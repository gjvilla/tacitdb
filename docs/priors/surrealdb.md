# SurrealDB — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
Multi-model database written in Rust: documents, graph edges, vectors, full-text, time-series, and key-value in one engine with one query language (SurrealQL) and ACID transactions. Since 3.0 (GA Feb 2026) it is explicitly positioned as "the future of AI agent memory" — agent memory and context graphs inside the database.

## Runtime shape & implementation
- Rust throughout. Ships as a single server binary *and* as an embeddable library (`surrealdb` crate) — same SurrealQL, same API in both modes.
- Pluggable KV storage: in-memory, RocksDB, SurrealKV (their own versioned LSM + Versioned Adaptive Radix Trie, MVCC), TiKV/FoundationDB for distributed; IndexedDB-backed WASM in the browser; embedded Node.js.
- SurrealKV is the piece that enables time-travel (`VERSION`) queries; a newer in-memory engine, SurrealMX, also advertises time travel (2026 blog).
- 3.0 added native WASM extensions, first-class file storage, an indexing overhaul, computed fields, client-side transactions, stable GraphQL.

## Data model & query interface
- Records in tables with record IDs; record links; graph edges via `RELATE` — edges are first-class records in their own tables, so edge properties (e.g. weights) are ordinary mutable fields.
- SurrealQL (SQL-like) is the primary surface; GraphQL stable in 3.0; SDKs for Rust, JS, Python, Java, Go (Java/Go production-ready with 3.0).
- Recursive graph traversal syntax `{min..max}` with built-in path algorithms since 2.2 (Feb 2025): `{..+path}` (all paths), `{..+collect}` (unique nodes), `{..+shortest=record:id}` (shortest path).
- Shortest path is hop-count based; no documented weighted (Dijkstra-style) shortest path as of Aug 2026.

## AI-native surface
- **Vector index — pre-filtered?** Yes, effectively. Indexes: brute force (exact), HNSW, DISKANN (3.1+). KNN operator `<|K,EF|>`; a `WHERE` condition combined with an indexed KNN search "is pushed into the index search and evaluated during the graph traversal, so non-matching candidates are rejected before they occupy one of the K slots" (docs, operators page) — filtered ANN, not post-filter.
- **Fulltext/hybrid:** BM25 full-text indexes with configurable analyzers; hybrid vector+BM25 fusion (reciprocal rank fusion) expressible in a single SurrealQL query — SurrealDB's own docs search runs HNSW+BM25 with RRF reranking.
- **Temporal/versioning:** `VERSION` clause on SELECT/CREATE gives time travel when running on SurrealKV. Transaction-time only (when the record was written), not valid-time; no bitemporal model. Limitation: `VERSION` accepts only literal datetimes, not expressions.
- **Provenance:** none native. Changefeeds and events can capture history, but source/author/evidence envelopes are DIY schema.
- **LLM/agent consumption:** SurrealMCP, the official MCP server (launched Aug 2025), gives agents permission-aware live query/memory access; 3.0 marketing centers agent memory; LangChain/LlamaIndex integrations exist.

## License & governance
- Core: Business Source License 1.1, converting to Apache 2.0 four years after each release (e.g. 2.0 → Apache on 2029-09-17). Free to use, embed, ship in products, and run internally at any scale; only offering SurrealDB itself as a DBaaS requires a commercial agreement.
- Single-vendor governance (SurrealDB Ltd); SDKs/libraries carry separate (more permissive) licenses per their license repo.

## Maturity & momentum (as of Aug 2026)
- ~32.9k GitHub stars, Rust, actively pushed (Aug 2026); 749 open issues.
- 3.0 GA Feb 2026 alongside a $23M raise (~$45M+ total, prior rounds unverified); SurrealDB Cloud; Surrealist 3.0 GUI; claimed deployments scaling to 700k users.
- Watch-item: storage-engine churn (RocksDB → SurrealKV → SurrealMX) and open reliability issues in exactly the new layers, e.g. HNSW index unrecoverable after SurrealKV file corruption (#6872) and HNSW search returning all records (#6949).

## What Tacit should steal
- **Filter pushdown into ANN traversal**: evaluating the predicate while descending HNSW so rejected candidates never consume K slots — this is the engine-level pre-filtered vector search Tacit specifies.
- **One-query hybrid retrieval**: BM25 + KNN + graph traversal + RRF composed in a single plan proves the "one engine, one plan" fusion goal is practical at this scale.
- **Versioned-KV substrate**: building record-time as-of on an MVCC/versioned key-value layer (SurrealKV's VART) rather than bolting history onto the data model.
- **Embedded/server duality behind a storage trait**: one codebase, compile-time choice — lets Tacit defer its embedded-vs-server decision without forking APIs.
- **Edges as first-class records**: mutable edge weights fall out for free.

## Why not just use it
- **No bitemporal**: `VERSION` is transaction-time only (and literal-datetime only). Tacit's keeper corpus needs valid-time *and* record-time as-of; that would have to be simulated in schema, losing engine support.
- **No provenance envelope**: required source/author/evidence/lifecycle (proposed → promoted → retired) and review triggers would be unenforced application convention, not engine invariants.
- **No weighted shortest path**: traversal algorithms are hop-based; Dijkstra over mutable weights would be application-side.
- **QL-first surface**: SurrealQL is the product; Tacit v1 wants a typed API + MCP tools with no QL, so most of the surface area (and its bug surface) is dead weight.
- **Breadth vs depth**: a very large multi-model codebase under single-vendor BSL, with reliability churn in the storage/vector layers a ≤10^7-node personal corpus depends on most.

## Sources
- https://surrealdb.com/blog/introducing-surrealdb-3-0--the-future-of-ai-agent-memory (accessed 2026-08-23)
- https://tech.eu/2026/02/17/surrealdb-secures-23m-and-launches-surrealdb-3-0-to-address-ai-agent-memory-challenges/ (accessed 2026-08-23)
- https://surrealdb.com/docs/surrealql/operators (KNN filter pushdown; accessed 2026-08-23)
- https://surrealdb.com/docs/learn/data-models/vector-search/vector-indexes (accessed 2026-08-23)
- https://surrealdb.com/blog/a-real-world-example-of-hybrid-fusion-search-using-the-surrealdb-docs-search (accessed 2026-08-23)
- https://surrealdb.com/blog/surrealdb-2-2-benchmarking-graph-path-algorithms-and-foreign-key-constraints (accessed 2026-08-23)
- https://surrealdb.com/docs/surrealkv and https://github.com/surrealdb/surrealkv (accessed 2026-08-23)
- https://surrealdb.com/docs/sdk/rust/embedding (accessed 2026-08-23)
- https://github.com/surrealdb/license and https://surrealdb.com/license (accessed 2026-08-23)
- https://surrealdb.com/blog/introducing-surrealmcp (accessed 2026-08-23)
- https://api.github.com/repos/surrealdb/surrealdb (stars/activity; accessed 2026-08-23)
- https://github.com/surrealdb/surrealdb/issues/6872, https://github.com/surrealdb/surrealdb/issues/6949 (accessed 2026-08-23)

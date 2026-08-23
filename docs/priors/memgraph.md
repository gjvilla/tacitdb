# Memgraph — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
High-performance in-memory property-graph database, Cypher-compatible and Neo4j Bolt wire-compatible, written in C++. Since 3.0 (Feb 2025) heavily repositioned around GraphRAG, "AI memory," and agentic AI, adding native vector search so one store serves both explicit relationships and embeddings.

## Runtime shape & implementation
- Native C++ (no JVM/GC pauses), single-process single binary exposing Bolt, WebSocket, and metrics servers. Server-only: no embeddable library mode.
- In-memory-first with durability via write-ahead log + periodic snapshots. Storage modes: `IN_MEMORY_TRANSACTIONAL` (default, full ACID), `IN_MEMORY_ANALYTICAL` (no WAL/snapshots, faster bulk analytics), `ON_DISK_TRANSACTIONAL` (for datasets exceeding RAM, with documented limitations; in-memory remains the primary mode).
- 3.8 (Feb 2026) added a parallel query runtime (plan fragments across worker threads) and concurrent edge writes on supernodes.
- High availability / automatic failover is Enterprise-only.

## Data model & query interface
- Property graph: labeled nodes and typed edges with arbitrary mutable properties. Strongly consistent ACID transactions (MVCC).
- Query interface: Cypher over Bolt; works with Neo4j drivers/tooling. Extensible via query modules (C++/Python/Rust) — the MAGE library ships ~60+ algorithms (PageRank, communities, dynamic/streaming variants, etc.) as Cypher procedures.
- Deep path traversals are built into the planner as relationship-expansion syntax: BFS, DFS, `*WSHORTEST` (Dijkstra weighted shortest path), and `*ALLSHORTEST`. Weight is an inline lambda over edge/node properties, e.g. `MATCH p=(a)-[*WSHORTEST (r, n | r.weight)]-(b)` — weights are ordinary mutable properties.

## AI-native surface
- **Vector index — pre-filtered?** No. Vector search (3.0+) is backed by USearch; node and edge vector indexes (`CREATE VECTOR INDEX` / `CREATE VECTOR EDGE INDEX`) are queried via procedures `vector_search.search()` / `search_edges()`. Docs state the query planner "currently does not utilize vector indices"; no metadata filtering during search — filtering is post-hoc Cypher on the returned top-k. 3.8's "single store vector index" cuts vector memory ~85% by storing embeddings only in the index.
- **Fulltext/hybrid:** text indexes powered by Tantivy (stored on disk, transaction-batched, replicated); stable (non-experimental) since 3.6. Tantivy query syntax (boolean, phrase, field-specific). Hybrid = compose text + vector + traversal procedures in one Cypher query ("Atomic GraphRAG," 3.8: one-query context generation); rank fusion is hand-rolled, not a built-in fused operator.
- **Temporal/versioning:** none. Temporal property types and TTL exist, but no time-travel, no as-of queries, no bitemporal model.
- **Provenance:** none native; triggers/streams (Kafka/Pulsar ingestion) can implement audit trails by convention.
- **LLM/agent consumption:** official Memgraph MCP server (modular architecture, vector-search tools); GraphRAG ecosystem push (JumpStart program, Nov 2025 toolkit for non-graph users); LangChain/LlamaIndex integrations.

## License & governance
- Community Edition: Business Source License 1.1 (source-available; production use allowed; converts to Apache 2.0 at each release's Change Date).
- Enterprise Edition: proprietary Memgraph Enterprise License (security/RBAC, multi-tenancy, HA/failover, support).
- MAGE algorithm library repo: Apache-2.0. Single-vendor governance (Memgraph Ltd).

## Maturity & momentum (as of Aug 2026)
- ~4.4k GitHub stars (memgraph/memgraph), C++, active (pushed Aug 2026); claimed 150k open-source community.
- Funding: $9.34M round (2021, led by M12); later rounds unverified.
- Named production users: NASA (people/skills knowledge graph for GraphRAG, ~27k nodes / 230k edges, targeting 500k+), Cedars-Sinai (Alzheimer's knowledge base), Capitec Bank, Precina Health.
- Fast release cadence in 2025–26 (3.0 Feb 2025 → 3.8 Feb 2026): vector search, edge vectors, text search stabilization, parallel runtime.

## What Tacit should steal
- **`*WSHORTEST` design**: weighted shortest path as a first-class planner operation with an inline weight expression over mutable properties — precisely Tacit's weighted-path requirement; copy the "weight = arbitrary property expression" contract rather than baking in a fixed weight field.
- **Traversals in the planner, not bolted-on procedures**: BFS/DFS/weighted variants composing with filters inside one query plan.
- **In-memory graph + WAL/snapshot durability**: at ≤10^7 nodes this topology is simple, fast, and crash-safe; the analytical-mode escape hatch (drop WAL during bulk load, snapshot after) is a clean pattern for bounded-memory bulk mutation.
- **Text index lifecycle**: Tantivy index kept transactional by batching updates to commit time and treating index ops as replication events.
- **Single-store vector index**: store the embedding once (index-resident) with references from records, avoiding double memory.

## Why not just use it
- **C++, server-only**: not Rust, not embeddable as a library; conflicts with single-binary/zero-plugin preference (MAGE algorithms ship as separate loadable modules, often via Docker images).
- **Vector search is a bolt-on**: procedure-based, invisible to the planner, no pre-filtering — the opposite of Tacit's engine-level pre-filtered vector requirement; hybrid rank fusion is manual.
- **No temporal story at all**: bitemporal as-of over assertions would be entirely schema convention.
- **No provenance envelope or lifecycle states**: would live in application code with no engine enforcement.
- **RAM-bound primary mode** plus Enterprise gating (HA, RBAC, multi-tenancy) under BSL/proprietary split; Cypher-first surface where Tacit v1 wants a typed API + MCP tools, no QL.

## Sources
- https://github.com/memgraph/memgraph (accessed 2026-08-23)
- https://memgraph.com/docs/fundamentals/storage-memory-usage and https://memgraph.com/blog/memgraph-storage-modes-explained (accessed 2026-08-23)
- https://memgraph.com/docs/advanced-algorithms/deep-path-traversal (WSHORTEST syntax; accessed 2026-08-23)
- https://memgraph.com/blog/how-to-find-all-weighted-shortest-paths-between-nodes-and-do-it-fast (accessed 2026-08-23)
- https://memgraph.com/docs/querying/vector-search (USearch backend, planner/filtering limitations; accessed 2026-08-23)
- https://memgraph.com/docs/querying/text-search and https://memgraph.com/blog/text-search-in-memgraph (accessed 2026-08-23)
- https://memgraph.com/blog/memgraph-3-graph-database-llm-context-problem (3.0; accessed 2026-08-23)
- https://memgraph.com/blog/memgraph-3-8-release-atomic-graphrag-vector-single-store-parallel-runtime (Feb 12, 2026; accessed 2026-08-23)
- https://github.com/memgraph/memgraph/blob/master/LICENSE and https://memgraph.com/legal (BSL/MEL; accessed 2026-08-23)
- https://api.github.com/repos/memgraph/mage (Apache-2.0; accessed 2026-08-23)
- https://memgraph.com/blog/nasa-memgraph-people-knowledge-graph (accessed 2026-08-23)
- https://venturebeat.com/business/graph-database-company-memgraph-raises-9-34m (accessed 2026-08-23)
- https://www.businesswire.com/news/home/20251111832729/en/ (GraphRAG toolkit; accessed 2026-08-23)

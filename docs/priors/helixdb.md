# HelixDB — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
An OLTP "graph-vector" database written in Rust, aimed squarely at AI agent memory, RAG, and knowledge graphs — graph, vector, full-text, and (claimed) temporal in one engine. Built by a 6-person, London-based startup (founders George Curtis, Xavier Cochran), Y Combinator Spring 2025 batch. Publicly launched on HN in May 2025; declared "generally available" in 2026 with a v3.x line.

## Runtime shape & implementation
- ~93% Rust (GitHub language stats). Server-shaped: the `helix` CLI compiles and deploys queries to a running instance; clients hit `POST /v2/query` over HTTP. Managed cloud offering with single-writer + auto-scaling readers.
- Storage has churned: at launch (May 2025) the engine was LMDB-backed, with founders stating a custom storage engine was on the roadmap (Show HN thread). As of Aug 2026 the repo describes itself as "built in Rust on Object Storage" (S3-compatible; local mode uses in-memory or bundled MinIO). No formal architecture write-up of the migration was found — details unverified.
- Not an embeddable in-process library in the SQLite sense; local dev still runs an instance.

## Data model & query interface
- Property graph (nodes, edges) plus first-class vectors; also markets KV/document/relational support.
- HelixQL (HQL): a typed, schema-checked DSL that is **compiled ahead of deployment** — queries become named, typed endpoints rather than ad-hoc strings. Type-safe SDKs for Rust, TypeScript, Python, Go call those endpoints.
- This "queries compile to a typed API" pattern means apps never ship a runtime query language — close in spirit to Tacit's no-QL-in-v1 stance, though HQL itself is still a language someone must write.

## AI-native surface
- **Vector**: HNSW index. At launch, filtering was post-filter (founder: you "retrieve surplus vectors and then perform the filter"). Pre-filtering support for `SearchV` landed around Jan 2026; HelixDB now claims combined graph-traversal + pre-filtered vector search (vendor claim; independent benchmarks not found — treat recall/latency numbers as unverified).
- **Lexical/hybrid**: BM25 full-text in the same engine; graph traversal, ANN, and BM25 composable in a single HQL query. Explicit rank-fusion primitives: not documented (unverified).
- **Temporal/versioning**: the marketing site lists "temporal" among features; no documented as-of/bitemporal query surface was found — thin/unverified.
- **Provenance**: nothing native; would be app-level schema.
- **Agent consumption**: official MCP support (Helix MCP plus a query-skills/docs MCP), typed SDKs, positioned as "the database for AI memory."

## License & governance
- AGPL-3.0 at launch (explicitly to block hyperscaler hosting), relicensed to **Apache-2.0** (LICENSE file verified 2026-08-23). Single-vendor project; no foundation, no external committer base of note. VC-backed (YC; NVIDIA affiliation reported via its ecosystem/backing — exact nature unverified).

## Maturity & momentum (as of Aug 2026)
- ~5.8k GitHub stars; very active: v3.1.1 released 2026-08-16, commits through 2026-08-22; 18 open issues; ~3k total commits.
- Young company (founded 2024/25, 6 people) with visible architecture pivots (LMDB → object storage; AGPL → Apache). Momentum is real but the foundation is still moving.

## What Tacit should steal
- **Compiled, typed queries as the only API**: schema-checked query definitions that become generated, typed endpoints in Rust/Python SDKs — exactly the shape Tacit wants without inventing a runtime QL.
- One engine, one transaction domain for graph + ANN + BM25, so hybrid retrieval is a single plan, not a fan-out.
- MCP as a first-class product surface, not an afterthought.
- Pre-filtered HNSW living inside the graph store (their ~2026 direction validates Tacit's requirement).

## Why not just use it
- Closest living competitor to Tacit's engine-level wishlist, honestly: if Tacit dropped the provenance envelope and bitemporal requirements, HelixDB could plausibly serve the workload today.
- Gaps against requirements: no provenance model (required envelope would be pure app-side convention, unenforced); no verified as-of/bitemporal queries; rank fusion unverified; weighted shortest-path with agent-mutable weights not a documented strength; bulk-mutation memory behavior undocumented.
- Shape risk: server + object-storage architecture is heavier than a single binary/embedded library, and the storage layer has already been swapped once — a 6-person single-vendor startup mid-pivot is a fragile foundation for a keeper corpus meant to outlive tools.
- Apache-2.0 does make it fork-safe and a legitimate source of design (and potentially code) to borrow.

## Sources
- https://github.com/HelixDB/helix-db (accessed 2026-08-23; license, stars, releases v3.1.x, repo description)
- https://news.ycombinator.com/item?id=43975423 — Show HN, May 2025 (accessed 2026-08-23; LMDB, HNSW post-filtering at launch, AGPL rationale)
- https://www.ycombinator.com/companies/helixdb (accessed 2026-08-23; batch, founders, team size)
- https://www.helix-db.com/ (accessed 2026-08-23; positioning, feature list incl. "temporal")
- https://docs.helix-db.com/ (accessed 2026-08-23; HQL, SDKs, MCP)
- https://www.codeline.co/thoughts/repo-review/2026/helixdb-graph-vector-database-rust (accessed 2026-08-23; 2026 third-party review: LMDB-era architecture, BM25, HQL compile step)

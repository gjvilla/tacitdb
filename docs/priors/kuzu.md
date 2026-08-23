# Kuzu (Kùzu) — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
An embedded, columnar property-graph database ("DuckDB for graphs") from University of Waterloo research, commercialized by Kùzu Inc. (Toronto, ~10 people). First released Nov 2022; became the default embedded graph engine for AI/GraphRAG tooling (GitLab's Knowledge Graph preview built on it). **Upstream is dead**: the repo was archived 2025-10-10, and EU Digital Markets Act filings revealed in Feb 2026 that Apple agreed on 2025-10-09 to acquire Kùzu Inc. The story now is Kuzu-the-lineage: MIT-licensed forks.

## Runtime shape & implementation
- C++ core, in-process embedded library (SQLite/DuckDB shape) with Python, Rust, Node, Java, etc. bindings; optional extensions loaded as shared libraries (final release bundled them to remove the dependency on Kuzu's now-dead download server).
- Columnar disk-based storage; vectorized execution, factorized joins, worst-case-optimal joins — research-grade query processing on a single machine.
- The embedded shape is *why* it won AI-tooling adoption: zero server, pip-install, data lives in a directory.

## Data model & query interface
- Structured property graph with strict schemas (node/rel tables), queried in Cypher; interop with Parquet/Arrow for bulk load.
- No typed-API-first surface — Cypher strings are the interface; bindings are thin.

## AI-native surface
- **Vector**: native **disk-based HNSW** index (v0.9+, design published as the NaviX paper, arXiv:2506.23397) supporting **pre-filtered / predicate-agnostic search**: Kuzu computed the filtered subset (via "projected graphs"), then ran a modified HNSW search over only that subset — precisely the pre-filter-ANN semantics Tacit requires, and disk-based so index size wasn't RAM-bound.
- **Lexical**: full-text search extension (BM25) on node string properties; vector + FTS + Cypher composable in one query.
- **Temporal/versioning**: none. **Provenance**: none.
- **Agents/LLM**: wide GraphRAG integration (LangChain, LlamaIndex), community MCP servers.

## License & governance
- MIT, single-vendor governed — which is exactly what made the shutdown survivable via forks and the acquisition non-poisonous. VC-funded startup, research-lab lineage (Semih Salihoğlu's group).

## Maturity & momentum (as of Aug 2026)
- **Upstream**: archived/read-only since 2025-10-10; final release v0.11.3 same day; ~4k stars, 329 issues frozen. kuzudb.com, docs.kuzudb.com, blog.kuzudb.com no longer resolve in DNS (checked 2026-08-23). No further fixes ever.
- **Apple acquisition**: agreement 2025-10-09, disclosed 2026-02-11 via EU DMA register; all shares + select employees; price undisclosed; no public Apple statement on the technology's future.
- **Forks** (per a Mar 2026 survey and direct repo checks 2026-08-23):
  - **LadybugDB** (`LadybugDB/ladybug`) — the credible successor: 1.6k stars, 132 forks, MIT, v0.19.1 (2026-08-04), active commits through 2026-08-22; claims 1000+ commits and 80+ contributors since Nov 2025; direction is "graph lakehouse" (DuckDB storage interop, Arrow/Parquet, object stores). Backed by "Ladybug Memory"; lead maintainer Arun Sharma; publicly endorsed by ex-Kuzu staff.
  - **Bighorn** (Kineviz) — 133 stars; near-dormant until Aug 2026, now some activity; tied to Kineviz's GraphXR product.
  - **Vela-Engineering/kuzu** — 41 stars; agent-memory focus, experimenting with concurrent multi-writer; tagged releases through Jun 2026.
  - **Ryu** (predictable-labs) — listed in the Mar 2026 fork survey; repo not found on GitHub 2026-08-23 (dead or renamed — unverified).
- Community caveat, quoted in press at archive time: only ~six people ever understood the codebase.

## What Tacit should steal
- **NaviX**: the published design for a disk-based HNSW with robust *pre-filtered* search inside a graph DBMS (arXiv:2506.23397) — the single best implementation reference for Tacit's filtered-ANN requirement.
- The embedded, zero-server, data-in-a-directory shape as the adoption driver for AI tooling.
- Columnar node/rel-table storage with strict schemas; Arrow/Parquet as the bulk in/out path.
- Governance lesson: permissive license = the work survives the company; also, a bus factor of ~6 on a research-grade C++ codebase is a real cost of "clever" engines.

## Why not just use it
- Upstream: unmaintainable by definition (archived, read-only, no security fixes).
- LadybugDB could serve the embedded graph+vector+FTS core competently, but adopting means betting a keeper corpus on a 9-month-old fork of a codebase few people understand, in C++ not Rust, moving toward lakehouse interop rather than Tacit's priorities.
- Regardless of fork: no bitemporal as-of, no provenance envelope, no lifecycle states — all would be app-level convention; weighted shortest-path with agent-mutable edge weights is not a first-class feature; bulk-delete memory behavior unproven for Tacit's workload.
- Verdict: mine it (especially NaviX + storage design), don't build on it.

## Sources
- https://www.theregister.com/2025/10/14/kuzudb_abandoned/ (accessed 2026-08-23; archive event, quotes, forks)
- https://9to5mac.com/2026/02/11/kuzu-database-company-joins-apples-list-of-recent-acquisitions/ (accessed 2026-08-23; Apple/DMA disclosure)
- https://betakit.com/apple-strikes-deal-to-acquire-canadian-database-software-startup-kuzu/ (accessed 2026-08-23; deal terms/date, company size)
- https://github.com/kuzudb/kuzu (accessed 2026-08-23; archived flag, v0.11.3, MIT)
- https://szarnyasg.org/posts/kuzu-forks/ (accessed 2026-08-23; Mar 2026 fork survey)
- https://github.com/LadybugDB/ladybug · https://blog.ladybugdb.com/post/ladybug-spreading-its-wings/ · https://thedataquarry.com/blog/from-kuzu-to-ladybug/ (accessed 2026-08-23; successor status, roadmap, maintainers)
- https://arxiv.org/abs/2506.23397 — NaviX: native vector index with predicate-agnostic search (accessed 2026-08-23)
- https://github.com/Kineviz/bighorn · https://github.com/Vela-Engineering/kuzu (accessed 2026-08-23; fork activity)

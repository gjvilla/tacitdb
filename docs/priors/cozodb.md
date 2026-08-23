# CozoDB — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
A transactional relational-graph-vector database written in Rust, queried in Datalog (CozoScript), self-described as "the hippocampus for AI." Essentially one prolific author's (Ziyang Hu, `zh217`) 2022–2023 design sprint that anticipated most of the 2025-era "AI memory database" feature set — vectors, FTS, graph algorithms, and time travel in one embeddable engine. **Effectively unmaintained since late 2024.**

## Runtime shape & implementation
- Rust core, **embedded-first** (in-process like SQLite) with the same codebase also shipping as a standalone server, WASM (browser), and mobile builds — one engine, many shapes; the exact optionality Tacit wants to keep open.
- Pluggable storage backends behind one trait: in-memory, SQLite, RocksDB (recommended for real workloads), sled, TiKV (distributed). Bindings: Python, NodeJS, Java, Go, C, Swift.

## Data model & query interface
- Relations (not a native property graph) + Datalog with recursion and aggregations; graphs are modeled as edge relations, with a library of built-in algorithms (BFS/DFS, PageRank, community detection, **Dijkstra/A\* shortest path with weights from relation data** — weights are just stored values, hence mutable by writers).
- Interface is CozoScript strings via the client APIs — no typed query API.

## AI-native surface
- **Vector**: HNSW indices (v0.6+) that are themselves queryable *inside Datalog* — vector search results join ad-hoc with any other rule, so filtering/hybrid composition is native query planning rather than a bolted-on post-filter. (Whether the HNSW walk itself prunes by predicate, Kuzu-NaviX-style, is not documented — the composition is at plan level; unverified below that.)
- **Lexical**: FTS indices (v0.7) plus MinHash-LSH near-duplicate search — an unusual and useful trio (ANN + FTS + LSH) in one engine. Rank fusion left to the query author.
- **Temporal**: per-relation opt-in **time travel**: a `Validity` key component `[timestamp, assert/retract flag]`; `@ <timestamp>` queries a consistent as-of snapshot; `ASSERT`/`RETRACT` write current-time facts. This is **valid-time only** (timestamp semantics are user-interpreted; no separate record/transaction-time axis), so bitemporal needs a second, app-managed dimension.
- **Provenance**: none native. **Agents/LLM**: predates MCP; no official server; consumed via embeddings-era Python tooling.

## License & governance
- MPL-2.0 (file-level copyleft, embedding-friendly). Personal project of a single author; a `cozo-community` org formed to keep it moving.

## Maturity & momentum (as of Aug 2026)
- **Dormant.** Last release v0.7.6 on 2023-12-11; last commit to `main` 2024-12-04 (a RocksDB regression fix merging community PRs); ~4.1k stars; 50 open issues; no maintainer statements since.
- The `cozo-community/cozo` fork also stalled: last push 2024-12-12. No active successor found (checked 2026-08-23).

## What Tacit should steal
- **The Validity design**: as-of queries implemented as a typed key suffix `(timestamp, is_assert)` with snapshot semantics and ASSERT/RETRACT — a compact, proven blueprint for Tacit's valid-time axis (Tacit adds the record-time axis Cozo skipped).
- **Indexes as relations**: HNSW/FTS results that participate in ordinary joins, giving hybrid retrieval one planner instead of a fusion layer bolted on top.
- One codebase → embedded/server/WASM shapes, and the storage-backend trait that makes the engine testable in-memory and durable on RocksDB.
- Honest per-relation opt-in for history (time travel costs write amplification; don't pay it everywhere).

## Why not just use it
- Maintenance is disqualifying for a keeper corpus: ~20 months without a commit, ~2.7 years without a release, bus factor of one, and the community fork is equally stalled. Adopting it means adopting a fork you maintain.
- Feature gaps even if revived: valid-time-only (not bitemporal); no provenance envelope or lifecycle states; Datalog-string interface conflicts with typed-API-first (though it embeds cleanly behind one); HNSW recall/perf at 10^6–10^7 × 1024–1536-dim vectors is unproven at this dormancy (its HNSW predates most 2024+ ANN hardening); no MCP story.
- Verdict: the strongest *conceptual* prior for Tacit — closest to the intended feature intersection — but as code, a cautionary tale about single-maintainer engines.

## Sources
- https://github.com/cozodb/cozo (accessed 2026-08-23; description, MPL-2.0, stars, features)
- https://github.com/cozodb/cozo/releases (accessed 2026-08-23; v0.7.6 = 2023-12-11 latest)
- GitHub API repo/commit metadata for `cozodb/cozo` and `cozo-community/cozo` (accessed 2026-08-23; last pushes 2024-12-04 / 2024-12-12)
- https://docs.cozodb.org/en/latest/timetravel.html (accessed 2026-08-23; Validity, `@` as-of, ASSERT/RETRACT semantics)
- https://docs.cozodb.org/ (accessed 2026-08-23; v0.7 features: HNSW-in-Datalog, FTS, MinHash-LSH, backends)
- https://dbdb.io/db/cozodb (accessed 2026-08-23; classification, storage backends)

# FalkorDB — prior art
*Researched: 2026-08-23 · Status: filled*

## What it is
A low-latency property-graph database that executes OpenCypher via GraphBLAS sparse-matrix linear algebra. Founded 2023 by RedisGraph's creators (Guy Korland, Roi Lipman, Avi Avni) as the continuation/fork of RedisGraph after Redis EOL'd it; the company's entire positioning since ~2024 is GraphRAG and agent memory ("the best Knowledge Graph for LLMs").

## Runtime shape & implementation
- Runs as a **Redis module** (`libfalkordb.so`) — a server inside a server: you deploy Redis (or their Docker image / FalkorDB Cloud), the module handles graph commands. Multi-tenant by design: thousands of lightweight graphs per instance keyed like Redis keys.
- Historically C (RedisGraph lineage) on the SuiteSparse GraphBLAS C library; heavy ongoing Rust migration — repo is now ~44% Rust vs ~10% C (GitHub language stats, 2026-08-23). Adjacency as sparse matrices; traversals as matrix ops.
- The module shape works commercially for them (Redis ops ecosystem for free, cloud upsell) but is the definition of a deployment dependency chain: Redis version × module ABI × GraphBLAS.

## Data model & query interface
- Schema-optional property graph; OpenCypher with proprietary extensions; index types: range, full-text, vector. Clients for Python, Rust, TS/JS, Java, Go, etc.; LangChain/LlamaIndex integrations.

## AI-native surface
- **Vector**: `vecf32` attribute type; HNSW-backed vector indexes (cosine/euclidean, 1–4096 dims, M etc. tunable) on node or relationship attributes. Queried via procedures `db.idx.vector.queryNodes/queryRelationships(label, attr, k, query) YIELD node, score` — a top-k over the whole index whose results you then constrain in Cypher, i.e. **post-filtering**; no pre-filtered ANN parameter is documented (checked docs 2026-08-23).
- **Lexical/hybrid**: RediSearch-derived full-text indexes; hybrid = compose FTS + vector procedures + Cypher traversal in one query; scoring fusion is hand-rolled in Cypher.
- **Temporal/versioning**: none shipped; "temporal graph capabilities" appear on the public roadmap for 2026-Q4 — vaporware until it lands.
- **Provenance**: none native; their GraphRAG pattern links answers to source chunks at the app layer.
- **Agents/LLM**: strongest packaging of the four: official **GraphRAG-SDK** (ontology-driven KG construction + retrieval) and an official, actively developed **FalkorDB-MCPServer** (pushed 2026-08-23).

## License & governance
- **SSPLv1** (not OSI-approved open source) — single-vendor, commercial cloud, CLA-style control. SSPL is fine for self-hosted internal use but is a hard stop for redistribution/embedding scenarios and makes long-term fork-as-escape-hatch unattractive.

## Maturity & momentum (as of Aug 2026)
- Very active: v4.20.4 released 2026-08-20, commits daily; ~5.6k stars; ~640 open issues+PRs. $3M seed (Jun 2024, Angular Ventures + angels); Israel-based; revenue via FalkorDB Cloud.
- Healthy short-term signals, but classic single-vendor risk profile — the founders have already lived one sponsor-kills-project event (RedisGraph), which is why FalkorDB exists.

## What Tacit should steal
- **MCP + SDK packaging**: ship the retrieval patterns (ontology-assisted graph construction, question→traversal templates) as a first-class SDK/MCP layer above the engine — Falkor proves agents adopt the tool surface, not the query language.
- Multi-tenancy as cheap named graphs — maps neatly onto per-corpus or per-agent workspaces.
- Procedure-style entry points (`queryNodes(..., k) YIELD node, score`) as a model for Tacit's typed retrieval functions.
- GraphBLAS itself: probably overkill at ≤10^7 nodes, but a reminder that bulk traversal can be batched algebra instead of pointer chasing.

## Why not just use it
- Shape is the antithesis of Tacit's constraint: a C/Rust Redis *module* plus Redis plus GraphBLAS — exactly the plugin/deployment landmine class Tacit exists to avoid; nothing embeddable, no single binary.
- Vector filtering is post-filter top-k; with Tacit's selective provenance/lifecycle predicates over 10^5–10^7 vectors, post-filtering is the known failure mode (recall collapse or k inflation).
- No bitemporal (roadmap-only), no provenance envelope, no lifecycle states; hybrid fusion is manual Cypher; SSPL constrains redistribution and forkability.
- Could it serve? For a plain GraphRAG service workload, yes, capably — but for Tacit's provenance-first embedded corpus it fails on shape, license, filtering semantics, and temporal model simultaneously.

## Sources
- https://github.com/FalkorDB/FalkorDB (accessed 2026-08-23; SSPLv1 LICENSE.txt, GraphBLAS design, v4.20.4, language stats)
- https://docs.falkordb.com/cypher/indexing/vector-index.html (accessed 2026-08-23; vecf32, HNSW options, queryNodes procedure semantics)
- https://www.falkordb.com/ (accessed 2026-08-23; positioning, multi-tenancy, 2026-Q4 temporal roadmap)
- https://www.openpr.com/news/3544960/falkordb-a-startup-by-redis-veterans-raises-3-million (accessed 2026-08-23; $3M seed, founders, RedisGraph lineage)
- https://github.com/FalkorDB/GraphRAG-SDK (accessed 2026-08-23)
- https://github.com/FalkorDB/FalkorDB-MCPServer (accessed 2026-08-23; official MCP server, active)

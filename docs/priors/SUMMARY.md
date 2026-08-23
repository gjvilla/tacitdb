# Prior-Art Survey — Cross-Engine Verdict

*Compiled: 2026-08-23 · Detail and sources in the per-engine files in this directory.*

## The wedge

**No surveyed engine combines Tacit's four core requirements: bitemporal as-of
queries + native provenance/lifecycle + weighted mutable-cost traversal + fused
hybrid retrieval in one plan.** The provenance-envelope + bitemporal pairing in
particular is unbuilt everywhere — it is the defensible reason Tacit exists. The
vector, hybrid, and embedded-runtime parts, by contrast, all have strong published
designs to borrow rather than invent.

## Status board

| Engine | Alive (Aug 2026)? | Shape / language | License | Filtered ANN | Hybrid one-plan | Temporal | Provenance | Weighted paths |
|---|---|---|---|---|---|---|---|---|
| [HelixDB](helixdb.md) | Active (v3.1.1) | Server + object storage / Rust | Apache-2.0 | Pre-filter ~2026, vendor claim | Graph+ANN+BM25; fusion unverified | Marketed, unverified | None | Not documented |
| [Kuzu](kuzu.md) | **Dead upstream** — Apple acquired 2025-10, repo archived; MIT fork **LadybugDB** active | Embedded / C++ | MIT | **Yes — NaviX pre-filtered HNSW** | Partial | No | None | Partial |
| [CozoDB](cozodb.md) | Dormant (last release 2023-12) | Embedded / Rust | MPL-2.0 | HNSW as relation | Yes, conceptually (joinable relations) | **Valid-time `Validity` + `@` as-of** | None | Some (Datalog algos) |
| [FalkorDB](falkordb.md) | Active | Redis module / C | SSPL | Limited | Partial | Roadmap 2026-Q4 | None | Some |
| [SurrealDB](surrealdb.md) | Active (3.0 GA, $23M) | Server or embedded / Rust | BSL 1.1 | **Yes — predicates pushed into HNSW/DiskANN traversal** | **Yes — one-query BM25+vector RRF** | Transaction-time `VERSION` only | None | Hop-based only |
| [Memgraph](memgraph.md) | Active | Server-only / C++ | BSL + ent. | No — procedure-only, no planner integration, no filter-during-search | No | No | None | **Best: `*WSHORTEST` inline weight lambda over mutable props** |
| [LanceDB](lancedb.md) | Active ($30M A) | **Embedded-first** / Rust | Apache-2.0 | **Yes — prefilter by default** | BM25 + pluggable rerankers (RRF default) | MVCC time travel (dataset-level, txn-time) | None | **No graph at all** |

## The steal list

Design ideas Tacit adopts rather than reinvents:

1. **NaviX** (Kuzu → LadybugDB, arXiv:2506.23397): disk-based HNSW with
   predicate-agnostic *pre-filtered* search inside a graph engine — the reference
   implementation for R-1, available under MIT.
2. **CozoDB**: `Validity` as part of the relation sort key + `@ timestamp` as-of
   queries + ASSERT/RETRACT semantics — a proven valid-time blueprint for R-7.
   Steal the design, not the dormant code.
3. **SurrealDB**: predicate pushdown into ANN traversal (reject candidates before
   they occupy K-slots), and single-query BM25+vector RRF — validates R-1/R-2 shape.
4. **Memgraph**: weighted shortest path as a *planner* feature with an inline weight
   expression over ordinary mutable properties — exactly R-5.
5. **LanceDB**: the embedded-first + Apache-2.0 posture and manifest-based MVCC
   versioning (time travel, tags, branches) — the shape/licensing north star, and a
   candidate storage substrate for the vector side (U-5).
6. **HelixDB**: compiled, schema-checked query definitions that become generated
   typed SDK endpoints (no runtime QL shipped to apps — D-0007's shape), and MCP as
   a first-class product surface.

## Build-vs-adopt verdict (feeds D-0008's review trigger)

No engine satisfies REQUIREMENTS.md outright; the trigger fired and the build
decision **holds**. Honest closest call: HelixDB could plausibly serve today *if*
Tacit dropped the provenance envelope and bitemporal requirements — but those are
the wedge, and HelixDB is a six-person single-vendor startup mid-architecture-pivot.
Cautionary reference class: Kuzu — a technically excellent embedded graph engine
with real adoption, dead within ~three years via acqui-hire; single-vendor
engine risk is real on both sides of the build/adopt line, which is an argument for
Tacit's permissive-license, boring-operations posture, not against building.

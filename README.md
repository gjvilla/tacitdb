# Tacit

A provenance-first knowledge engine for agentic workloads, built from
production scar tissue and run under its own discipline: every claim carries
an envelope (who said it, from what source, when it holds, what would trigger
its review), state is a fold over human verdicts rather than a field anyone
can write, and honest abstention — "the record does not settle this, and here
is the registered open question" — is a first-class retrieval outcome, graded
as a pass.

The project is its own first user. This repository's decision records and its
register of open questions are ingested into the engine on every start, the
retrieval quality claims are numbers from graded suites rather than
adjectives, and several of the engine's faults were found by questions
designed to catch them.

## The shape

Two layers, deliberately ([D-0002](docs/DECISIONS.md)):

- **`tacit-core`** — the engine: an embedded Rust library. Sealed records, an
  append-only replay-validated event log, bitemporal state, a projected
  graph with weighted paths, hybrid retrieval (BM25 + pluggable vector
  candidates + rank fusion) with calibrated abstention, native provenance
  and contradiction surfacing, and a designed legal redaction that preserves
  chain integrity.
- **`tacit-keeper`** — the discipline: corpus parsers, the register and
  golden-suite machinery, attestation (which promotions rest on signed
  commits), and the measurement instruments.
- **`tacit-mcp`** — the only served surface: a small stdio host exposing ten
  typed, audited tools. The ratchet is visible as an absence — there is no
  promote tool, so no sequence of agent calls turns a proposal into
  something the organization knows. That takes a person.

## Reading order

- [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) — eleven requirements, each
  naming the production scar that taught it.
- [docs/DECISIONS.md](docs/DECISIONS.md) — every decision as a dated record
  with provenance and a review trigger, D-0001 onward.
- [docs/REGISTER.md](docs/REGISTER.md) — the four-rooms register: what is
  known, what is openly unknown, and the practices that catch the rest. The
  project's actual state lives here.
- [docs/GOLDEN.md](docs/GOLDEN.md) and [docs/PEP-GOLDEN.md](docs/PEP-GOLDEN.md)
  — the graded suites: one over the self-hosting corpus, one over sixty
  pinned public documents this project did not write.

## Seeing it run

```bash
cargo run -p tacit-keeper --example dogfood     # the corpus interrogating itself
cargo run -p tacit-keeper --example golden      # the self-corpus suite, graded
scripts/fetch-proposals.sh                      # fetch the pinned outside corpus
cargo run -p tacit-keeper --example pep_golden  # the outside-corpus suite, graded
cargo run -p tacit-mcp -- .                     # serve this repo's corpus over MCP
```

The measurement instruments (`explain`, `calibration`, `fusion_sweep`,
`indexing_sweep`, `meaning`) exist because every retrieval claim here was
wrong at least once until an instrument said otherwise; the register records
the misfilings alongside the fixes.

## Status, stated plainly

Working v1, pre-release, single author. **Dual-licensed MIT OR Apache-2.0**
([D-0050](docs/DECISIONS.md)) — use it under either, at your option. The
crates stay `publish = false` until the name's counsel review completes
([U-6](docs/REGISTER.md)); the registrable name is settled
([D-0011](docs/DECISIONS.md)). If you arrived
here with a question the record should answer and it does not, that is
exactly the signal the register's U-8 is waiting to read — open an issue and
say which layer you came for.

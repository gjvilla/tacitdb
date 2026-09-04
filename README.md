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
  commits), the measurement instruments, and the one binary that renders a
  person's verdict on a store.
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
cargo run -p tacit-mcp -- --store target/tacit.log .   # serve this repo's corpus over MCP, durably
cargo run -p tacit-keeper -- pending --store target/tacit.log   # what agents proposed; promote|reject|retire rule on it
```

The measurement instruments (`explain`, `calibration`, `fusion_sweep`,
`indexing_sweep`, `noise_sweep`, `meaning`) exist because every retrieval claim here was
wrong at least once until an instrument said otherwise; the register records
the misfilings alongside the fixes.

## Your own corpus

The host reads two files beneath whatever directory you hand it:
`docs/DECISIONS.md` and `docs/REGISTER.md`. Nothing else is looked at, no
git repository is required, and the format is the one this repository's own
documents use — strict on purpose, because a corpus about honesty must not
silently drop what it does not understand. A malformed record is a hard
error that names the record and the problem.

The smallest corpus that loads:

````markdown
# Acme Platform — Decisions

## D-0001 · Postgres is the system of record

```yaml
id: D-0001
state: promoted
author: Jordan Lee
recorded: 2026-06-02
valid_from: 2026-06-02
source: platform guild meeting
evidence: []
review_trigger: sustained write load above 20k rows per second on any table
```

**Assertion.** All services persist durable state in the shared Postgres
cluster. No service runs its own database without a decision superseding
this one.

**Forces.** One backup story, one on-call runbook, one credential rotation.
````

```markdown
# Acme Platform — Register

## Room 2 · Known unknowns

| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-1 | Which region should the disaster-recovery replica live in | the 2027 compliance audit | Legal has not said whether EU data may leave eu-west-1. |

Owner: Jordan Lee.
```

The rules the parser enforces, so you meet them the first time:

- A record heading is `## D-nnnn · Title` — the `·` separator and four
  digits are load-bearing. Hypotheses use `H-nnnn`.
- The yaml block needs `id`, `state`, `author`, `source`, and `valid_from`;
  `recorded`, `evidence`, and `review_trigger` are optional. Any other key
  is an error. Every `evidence` path must exist, looked up under `docs/`
  first and then the repository root; an unresolvable one is an error.
- `state` is an instruction to the ingester, not a stored field, and only
  `promoted` (a decision) and `registered` (a hypothesis) are accepted. A
  document does not hold proposals; agents make those through the tool
  surface, and they wait there.
- Prose lives in bold-labelled sections — `**Assertion.**` is required, the
  rest are yours. Unlabelled prose is an error.
- The register needs an `Owner: Name` line somewhere in the file, because
  every open question is a record and a record has an author. Only rows
  beginning `| U-` are read; each is one line with four cells.

Then serve it, durably:

```bash
cargo run -p tacit-mcp -- --store acme.log /path/to/acme
```

Any MCP client that speaks stdio can attach to it. Build the binary once,
then register the command. With Claude Code:

```bash
cargo build --release -p tacit-mcp
claude mcp add tacit -- "$PWD/target/release/tacit-mcp" --store /path/to/acme.log /path/to/acme
```

With Claude Desktop, or any client that reads the standard JSON form:

```json
{
  "mcpServers": {
    "tacit": {
      "command": "/absolute/path/to/target/release/tacit-mcp",
      "args": ["--store", "/path/to/acme.log", "/path/to/acme"]
    }
  }
}
```

The client then gets `tacit_search` with citations and calibrated
abstention — and a `why` on every result: the numbers the outcome was read
from, the bars, which fell short, and the words the record has never used —
plus `tacit_open_questions`, `tacit_propose_claim`, the pending inbox, and
an audit log beside the store. Every start is a sync, so edit the
documents and restart: unchanged records write nothing, edited ones
supersede what they replace. `tacit-mcp --help` lists the options.

Three things worth knowing before you rely on it. This is the only ingest
format today; the proposals suite shows a second parser is a few hundred
lines against `tacit-core`, but you would be writing it. An ingest that the
documents cause to fail writes nothing: a durable store is rehearsed in
memory first, so fix the document and start again. And a person promotes an
agent's proposal by writing the decision into
`docs/DECISIONS.md`, or rules on it directly:

```bash
cargo run -p tacit-keeper -- pending --store /path/to/acme.log
cargo run -p tacit-keeper -- promote --store /path/to/acme.log \
  --as "Jordan Lee" --why "measured twice, budget was agreed in D-0001" rec_01M1P58RMFAD6DC86Q3D6JV6H2
```

`reject` and `retire --reason <superseded|no-longer-true|promoted-in-error>`
take the same shape. The name is recorded as asserted, not verified, and the
verdict says so. The store is locked while either the host or the command
holds it, so stop the host first; the verdict is in the log when it next
starts.

### Grade your own record

The graded suite is not only this repository's test. Write questions your
record should and should not answer into `docs/GOLDEN.md` beside the other
two files, and the same runner grades them, abstention counted as a pass:

```markdown
## Questions

| id | Question | Expect | Owner | Review trigger |
|----|----------|--------|-------|----------------|
| G-01 | where are runtime feature flags managed | answer D-0002 | Jordan Lee | when the contract is renewed |
| G-02 | which region hosts the disaster recovery replica | abstain U-1 | Jordan Lee | when U-1 resolves |
| G-03 | what framework does the mobile app use | abstain | Jordan Lee | never — nothing here is about mobile |
```

```bash
cargo run -p tacit-keeper --example golden -- /path/to/acme
cargo run -p tacit-keeper --example explain -- --corpus /path/to/acme G-02   # why a question graded as it did
```

`answer D-0002` means the record settles it and that decision is the one;
`abstain U-1` means it does not, and that open question covers the territory;
`abstain` alone means nobody has even registered the question. The runner
exits non-zero on a failure nothing predicted, and runs the same audits it
runs here: a question resting on a trigger that has fired, a record that
quotes a question back, a word the corpus has acquired since the question
was agreed. That last one needs a baseline; `GOLDEN_BASELINE=1` prints one
to paste into the file after the first run.


## Status, stated plainly

Working v1, pre-release, single author. Rust 1.88 or later. The `tacit-python`
crate is a placeholder — a re-export of `tacit-core` with no PyO3 and no
bindings yet — so "Python bindings" in the decision record is a plan, not a
thing you can install. **Dual-licensed MIT OR Apache-2.0**
([D-0050](docs/DECISIONS.md)) — use it under either, at your option. The
crates stay `publish = false` until the name's counsel review completes
([U-6](docs/REGISTER.md)); the registrable name is settled
([D-0011](docs/DECISIONS.md)). If you arrived
here with a question the record should answer and it does not, that is
exactly the signal the register's U-8 is waiting to read — open an issue and
say which layer you came for.

# Tacit

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue) ![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange)

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
cargo run -p tacit-keeper -- pending --store target/tacit.log   # what agents proposed; stop the host first — one process holds a store
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
| G-01 | where do services persist durable state | answer D-0001 | Jordan Lee | when a service is allowed its own database |
| G-02 | which region hosts the disaster recovery replica | abstain U-1 | Jordan Lee | when U-1 resolves |
| G-03 | what framework does the mobile app use | abstain | Jordan Lee | never — nothing here is about mobile |
```

```bash
cargo run -p tacit-keeper --example golden -- /path/to/acme
cargo run -p tacit-keeper --example explain -- --corpus /path/to/acme G-02   # why a question graded as it did
```

`answer D-0001` means the record settles it and that decision is the one;
`abstain U-1` means it does not, and that open question covers the territory;
`abstain` alone means nobody has even registered the question. The runner
exits non-zero on a failure nothing predicted, and runs the same audits it
runs here: a question resting on a trigger that has fired, a record that
quotes a question back, a word the corpus has acquired since the question
was agreed. That last one needs a baseline; `GOLDEN_BASELINE=1` prints one
to paste into the file after the first run.


## Running it at an organization

The sections above give the format and the commands. This is the loop they
add up to, as a team would actually run it, from an empty directory to a
record that agents consult and people rule on.

**1. Write down what is already decided, with its envelope.** Someone — call
them the keeper — creates a directory holding `docs/DECISIONS.md` and
`docs/REGISTER.md`, in the format shown under [Your own corpus](#your-own-corpus).
The founding pass is an interview, not a migration: ask the people who hold
the organization's knowledge in their heads *"what are you already sure of
here that is written nowhere?"*, and file each answer as a decision with an
author, a source, a validity date, and the event that would make it worth
re-reading. What nobody can settle goes into the register as an open
question with an owner and a trigger. An organization that admits five
things it does not know has a better record than one that claims to know
everything.

**2. Serve it, durably.** Build the host once and point it at the directory
with a store file beside it:

```bash
cargo build --release -p tacit-mcp
./target/release/tacit-mcp --store /path/to/acme.log /path/to/acme
```

Register that command with whichever MCP clients the team uses — Claude Code,
Claude Desktop, an internal agent runner — as shown above. Every start is a
sync, so the documents stay the source of truth: edit the markdown, restart
the host, and edited records supersede what they replace while unchanged
ones write nothing.

**3. Let agents and people ask.** Day to day, most traffic is `tacit_search`.
An answer arrives with citations; a miss arrives as an abstention with a
`why` — the numbers, the bars, which fell short, and the words the record has
never used — rather than a fluent guess. `tacit_open_questions` lets an agent
say "the record does not settle this, and here is U-1, which resolves at the
2027 audit" instead of inventing an answer. `tacit_as_of` and `tacit_history`
answer what the record said at a past instant and every verdict that moved a
record, which is what a post-mortem or an audit actually needs.
`tacit_contradictions` surfaces promoted claims that disagree about the same
subject, so the record's own inconsistencies are found by the engine rather
than by the next person to trip over them.

**4. Agents propose; nothing they do promotes.** When an agent learns
something the record should hold, it calls `tacit_propose_claim`. The claim
lands as `proposed` and waits. When it hits a question the record cannot
answer and nobody has registered, it calls `tacit_register_gap`, and the gap
becomes a citable open question with the agent named as the one who found
it, closed only when a person answers it with a promoted claim. There is no
promote tool. No sequence of agent calls, however well-intentioned,
turns a proposal into something the organization knows. That absence is the
design, not an omission.

**5. A person rules on the inbox.** On whatever cadence fits — daily for a
busy record, weekly for most — the keeper stops the host, which holds the
store's lock, and reads what agents proposed:

```bash
cargo run -p tacit-keeper -- pending --store /path/to/acme.log
```

Each item is promoted, rejected, or left to wait, with a name and a rationale
recorded as asserted. Or the keeper writes the decision straight into
`docs/DECISIONS.md` and restarts, which is the same verdict by a different
door. Decisions that events have overtaken are retired with a reason —
`superseded`, `no-longer-true`, or `promoted-in-error` — never edited in
place, so the history keeps saying what was believed and when.

**6. Grade the record against itself.** The team writes `docs/GOLDEN.md`
beside the other two files, as shown under [Grade your own record](#grade-your-own-record):
questions the record should answer and which decision answers them,
questions it should abstain on because a registered gap covers them, and
questions it should abstain on because nothing here is about that subject.
The runner grades all three, with abstention counted as a pass, and fails
the build on anything nobody predicted. Run it in CI. A record that starts
answering a question it used to abstain on has changed, and the suite is
how the team finds out before a user does.

**7. Keep the cadence.** The register's four rooms give the practice its
rhythm. Before each new phase of work, run the interview from step 1 again;
the knowledge that matters most is usually the kind nobody thought to write
down. Quarterly, re-read every review trigger in the decisions file and the
register, and retire whatever has been overtaken. Part of that re-read is
mechanical: a trigger that names a question the register has since resolved
fails the build until someone re-reads the record and rewords the trigger to
name the resolving decision. The rest is the keeper's own work, and the
suite in step 6 is what catches the drift the re-read misses.

Three limits to plan around, restated from [Status](#status-stated-plainly):
the two markdown files are the only ingest format today, so an organization
whose decisions live in a ticket tracker or a wiki would write a parser
against `tacit-core` — the proposals suite is the worked example, a few
hundred lines; one process holds a store at a time, so the host and the
keeper's verdicts take turns; and the name on a verdict is recorded as
asserted, not verified, so the trust in the record is the trust in whoever
has the keyboard.

## Using the engine directly

`tacit-core` is an ordinary Rust library with no I/O of its own; the host and
the keeper are callers. The crates are not on a registry yet
([U-6](docs/REGISTER.md)), so depend on the repository:

```toml
[dependencies]
tacit-core = { git = "https://github.com/gjvilla/tacitdb", package = "tacit-core" }
```

The whole loop is forty lines, and the same program runs as the crate's doc
test (`cargo test -p tacit-core --doc`):

```rust
use tacit_core::{
    Author, ClaimContent, Content, Draft, Ledger, Outcome, Projection, Query, SourceRef,
    TextIndex, VerdictAction, VerdictContent, ViewSpec,
};

fn main() -> Result<(), tacit_core::Error> {
    let mut ledger = Ledger::new();
    let billing = ledger.add_entity("service", "billing")?;

    // An agent proposes. It lands as `proposed` and stays there.
    let claim = ledger.append(Draft::new(
        Author::agent("cost-bot"),
        SourceRef::channel("nightly report"),
        Content::Claim(ClaimContent::Text {
            body: "billing runs on the shared Postgres cluster".into(),
            about: vec![billing],
        }),
    ))?;

    // A person rules. Only a human may author a verdict; the ledger refuses otherwise.
    ledger.append(Draft::new(
        Author::human("Jordan Lee"),
        SourceRef::channel("keyboard"),
        Content::Verdict(VerdictContent {
            action: VerdictAction::Promote { target: claim, retiring: None },
            rationale: Some("confirmed in the runbook".into()),
        }),
    ))?;

    // Ask. The default view is what the organization has actually agreed.
    let index = TextIndex::rebuild(&ledger);
    let projection = Projection::rebuild(&ledger);
    let retriever = index.retriever(&ledger, &projection, ViewSpec::now());

    let found = retriever.retrieve(&Query::text("where does billing run"));
    assert_eq!(found.outcome, Outcome::Matches);

    let miss = retriever.retrieve(&Query::text("who owns the mobile app"));
    assert!(miss.is_abstention());
    println!("never written here: {:?}", miss.unknown_terms);
    Ok(())
}
```

`Ledger::open(path)` instead of `Ledger::new()` makes it durable; add
`.with_vectors(&VectorIndex::rebuild(&ledger, &embedder), &embedder)` to the
retriever for the similarity channel. `cargo doc --no-deps -p tacit-core`
is the reference.

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
say which layer you came for. If you mean to change something,
[CONTRIBUTING.md](CONTRIBUTING.md) says what to run, why an edit to the
record arrives as a proposal, and which gate cannot run on your machine.

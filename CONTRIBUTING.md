# Contributing

Tacit is a single-author project that is its own first user: the decision
records and the register of open questions under `docs/` are ingested into the
engine on every start and graded by a suite. That shapes what a contribution
is, so the rules below are about the record before they are about the code.

## Where the record lives

`docs/DECISIONS.md` and `docs/REGISTER.md` are the corpus, and the only two
files the host reads. There is no wiki, deliberately: a page the engine cannot
ingest is a page the record does not know about, and a fact that lives beside
the record rather than in it is exactly the drift this project exists to
refuse. Write to the documents or to an issue, never to a page.

Both documents are the owner's verdicts. A decision is promoted by the person
who can attest it, and a row of the register is resolved by naming the decision
that settled it — the same rule the host applies to agents, who may propose and
cannot promote. So a pull request that edits a decision record or a register
row will be read as a proposal: open an issue first, say what the record gets
wrong, and let the verdict be written by the hand that signs it.

If you arrived with a question the record should have answered and did not,
that is the signal the register's U-8 is waiting to read. Open an issue and
say which layer you came for — the library, or the discipline.

## What to run

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p tacit-keeper --example golden
```

Clippy is expected clean. `cargo fmt` is not enforced. The golden suite is
the gate that matters: it grades this repository's own record against the
questions in `docs/GOLDEN.md`, and it goes red on a regression, on a question
resting on a trigger that has fired, on a question the corpus quotes back, or
on a question whose vocabulary the corpus has since acquired.

Those last two are the recursion to know about. Because the documents are the
corpus, every edit to `docs/` re-ranks the suite — prose can displace an
answer as surely as code can ([D-0058](docs/DECISIONS.md) records the day it
did). After any change under `docs/`, run golden and iterate until it holds;
never quote a golden question's wording into the corpus, name it by id; and
check the vocabulary table in `docs/GOLDEN.md` before writing a word the
corpus did not contain when a question was agreed.

## The boundary gate, and why you cannot run it

`scripts/check-boundary.sh` enforces [D-0010](docs/DECISIONS.md): no employer
code, data, or confidential identifier enters this repository. It runs before
any commit that touches `docs/` or `crates/`, and it runs against a terms file
that lives outside the tree, at `~/.config/tacit/boundary-terms`.

The terms are outside the tree by decision ([D-0054](docs/DECISIONS.md)),
because a scrub that carries the identifiers it exists to keep out would
publish them the day the repository did. Without that file the script does
not report clean; it exits 2 and says it has nothing to look for. That is the
correct result on your machine. Do not reconstruct the file, and do not
substitute one of your own: the scrub is the maintainer's, it is run on every
change before it is merged, and a contributor's tree is not what it guards.

Continuous integration runs the tests, clippy, and the golden suite. It does
not run the boundary gate, for the reason above.

## Commits

The repository's own commits are signed, and merges will be. Yours need not
be. A commit message here says what changed, why the previous state was
wrong, and what was deliberately not done; the counts at the end — tests,
clippy, golden — are how the next reader knows what the commit's author saw.

## Licence

Tacit is dual-licensed under MIT or Apache-2.0, at your option
([D-0050](docs/DECISIONS.md)). Unless you state otherwise, any contribution
you intentionally submit for inclusion is licensed the same way, without
additional terms or conditions.

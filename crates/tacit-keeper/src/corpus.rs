//! Ingest the decision-record corpus into a ledger.
//!
//! The dogfood: the file recording the project's decisions becomes the
//! engine's first corpus, and the engine's own grammar validates the file's
//! claims about itself. Nothing here bypasses the ratchet — the yaml `state:
//! promoted` is *transcribed* as a human-authored promote verdict, exactly as
//! if the author had rendered it at the keyboard, because that is what the
//! document records them doing.
//!
//! Bitemporal note: record-time is when this ledger learned the record (now,
//! engine-assigned), and the document's own date becomes valid-time. Nothing
//! is backdated and invariant 3 is untouched.
//!
//! Ingest is a **sync**, not a load (U-19). The documents are upstream and the
//! ledger is downstream, so running this twice must not duplicate the corpus,
//! and running it after an edit must carry the edit through. Each source record
//! is fingerprinted into its own provenance ([`crate::origin`]), which lets a
//! second run tell three cases apart:
//!
//! - **fresh** — the ledger has never seen it; append as usual.
//! - **unchanged** — append nothing at all, and reuse what is already there.
//! - **changed** — append a new claim superseding the old one, and let the
//!   document's `state:` promote the new while retiring the old in one verdict,
//!   which is exactly the transition design/001 §3.1 built `Promote { retiring }`
//!   for.
//!
//! Two things the sync deliberately does *not* do, because both are verdicts
//! and verdicts are human acts:
//!
//! - A record that has vanished from the document is **reported, not retired**.
//!   Deleting a paragraph is not the same act as retiring a decision, and the
//!   document no longer contains the words that would say so.
//! - A *question* reworded after it was settled — an answered register row, a
//!   scored hypothesis — is **reported as drift** and left exactly as it is.
//!   That is not a limitation: the register says history is never rewritten,
//!   and polishing the phrasing of a question the project already closed is
//!   editing history.
//!
//! A question reworded while it is *still open* does supersede, since D-0023:
//! the new wording is registered carrying a `supersedes` link, and the old is
//! withdrawn (a gap) or abandoned (a hypothesis) with the reason `Superseded`,
//! so "we asked it better" and "we stopped asking" are no longer the same
//! recorded event.
//!
//! One loose end the sync accepts rather than hides: when an edit removes a
//! cross-reference, the edge claiming it was there is not re-appended and so is
//! never superseded. It stays exactly what it always was — an unratified machine
//! proposal — and shows up where those belong, in the pending queue.

use crate::attest::{Attestation, Blame, blame};
use crate::origin::{Origin, digest};
use crate::parse::{ParseError, ParsedRecord, mentioned_ids, parse_corpus, split_evidence};
use crate::register::{ParsedUnknown, parse_register, register_owner};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tacit_core::{
    Author, AuthorKind, ClaimContent, ClaimState, Content, Draft, EntityId, Evidence, GapContent, GapState,
    HypothesisContent, HypothesisState, Ledger, RecordId, RecordState, ReviewTrigger, SourceRef,
    SetBasis, VerdictAction, VerdictContent, WithdrawReason,
};

/// The two documents this ingester reads, named once so provenance and lookup
/// cannot drift apart.
pub const DECISIONS_DOC: &str = "docs/DECISIONS.md";
pub const REGISTER_DOC: &str = "docs/REGISTER.md";

/// Entity kind for a corpus record's identity anchor.
pub const DECISION_KIND: &str = "decision";
/// Entity kind for a register unknown's identity anchor.
pub const UNKNOWN_KIND: &str = "unknown";
/// Predicate for "this record's text names that record". A bare textual
/// mention is all that was observed, so the predicate claims nothing more.
pub const MENTIONS: &str = "mentions";

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Ledger(#[from] tacit_core::Error),

    #[error("io reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("record {record}: evidence {entry:?} resolves to no file under the repo root")]
    UnresolvableEvidence { record: String, entry: String },

    #[error("record {record}: evidence {entry:?} resolves outside the repo root")]
    EvidenceEscapesRepo { record: String, entry: String },

    #[error("record {record}: bad date {value:?} for key {key}")]
    BadDate { record: String, key: String, value: String },

    #[error(
        "record {record}: unsupported corpus state {state:?} — a document holds only `promoted` (a \
         decision) or `registered` (a hypothesis); proposals are made through the tool surface and \
         wait there"
    )]
    UnsupportedState { record: String, state: String },

    #[error("record {record}: id says hypothesis but sections say claim (or vice versa)")]
    HypothesisSignalMismatch { record: String },

    #[error("record {record}: score_by is only meaningful on a hypothesis")]
    StrayScoreBy { record: String },

    #[error(
        "the register does not state an owner, so its gaps have no author — add a line `Owner: \
         Name` to docs/REGISTER.md"
    )]
    MissingRegisterOwner,
}

/// How much a transcribed verdict must be able to show for itself.
///
/// `Observe` records what git can establish about who put the words there and
/// carries on, which is right for a document its author is still editing.
/// `RequireSignature` declines a verdict whose text no commit signed by a
/// trusted key carries. `RequireSignatureFrom` adds the other half of U-31 —
/// *whose* signature — by naming the signers whose verdicts count.
///
/// The names come from the caller and never from the repository, which is the
/// whole point: a list of who may promote, kept in the file it protects, is not
/// a trust root. It is one more file an agent can edit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Attest {
    #[default]
    Observe,
    RequireSignature,
    /// Signed by a trusted key *and* by one of these signers, matched against
    /// the identity the signature is bound to rather than the commit's author
    /// field, which is free text. An empty set asks only for the signature.
    RequireSignatureFrom(BTreeSet<String>),
}

impl Attest {
    /// Whether a verdict resting on this attestation may be transcribed, and
    /// the reason when it may not — the reason is the part worth reporting.
    pub fn admits(&self, attestation: &Attestation) -> Result<(), String> {
        match self {
            Attest::Observe => Ok(()),
            Attest::RequireSignature | Attest::RequireSignatureFrom(_)
                if !attestation.is_signed() =>
            {
                Err(format!("no commit signed by a trusted key carries these words ({attestation})"))
            }
            Attest::RequireSignatureFrom(signers) if !signers.is_empty() => {
                let signer = attestation.signer().unwrap_or_default();
                // Exact, because a loose match is a sharp edge: "Greg" would
                // admit every Gregory who ever signed anything. The error names
                // the identity it actually saw, so a caller who mistypes is
                // told what to type.
                if signers.iter().any(|allowed| allowed.trim() == signer) {
                    Ok(())
                } else {
                    Err(format!(
                        "signed by {signer}, who is not named as a bearer of verdicts \
                         ({attestation})"
                    ))
                }
            }
            _ => Ok(()),
        }
    }
}

/// What can be established about who wrote each corpus document, and how much
/// this run insists on.
pub struct Attestations {
    decisions: Blame,
    register: Blame,
    policy: Attest,
}

impl Attestations {
    /// Ask git about both documents beneath a repository root.
    pub fn of_repo(repo_root: &Path, policy: Attest) -> Self {
        Self {
            decisions: blame(repo_root, DECISIONS_DOC),
            register: blame(repo_root, REGISTER_DOC),
            policy,
        }
    }

    /// For a corpus supplied as text: there is no file, so there is nobody to
    /// ask. Stated as a reason rather than left blank, because "we did not
    /// check" and "we checked and found nothing" read the same in a record and
    /// mean different things.
    pub fn none(policy: Attest) -> Self {
        let because = "the corpus was supplied as text, not as a file under version control";
        Self { decisions: Blame::unattested(because), register: Blame::unattested(because), policy }
    }

    fn decisions(&self, lines: (usize, usize)) -> Attestation {
        self.decisions.over(lines)
    }

    fn register(&self, line: usize) -> Attestation {
        self.register.over((line, line))
    }

    fn admits(&self, attestation: &Attestation) -> Result<(), String> {
        self.policy.admits(attestation)
    }
}

/// The author a transcribed verdict is attributed to: the person the document
/// names, plus what could be established about who put those words there.
///
/// Two different people, deliberately. The document's `author:` is whoever made
/// the decision; the attestation is whoever typed it. Requiring them to match
/// would break the ordinary case of one person recording another's decision —
/// and the typist is the one the threat is about anyway.
fn transcriber(name: &str, attestation: &Attestation) -> Author {
    Author {
        name: name.to_string(),
        kind: AuthorKind::Human,
        detail: Some(attestation.to_string()),
    }
}

/// Whether a source record was new to the ledger, unchanged since the last
/// ingest, or edited since. The three cases are what makes a re-ingest a sync
/// rather than a duplication (U-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Fresh,
    Unchanged,
    Changed,
}

/// What a previous ingest of the same documents left in the ledger.
///
/// Built by reading provenance back out of the records themselves, so the sync
/// carries no state between runs beyond the ledger. A record whose reference
/// this keeper did not write parses to nothing and is left alone — the sync
/// only claims what it can prove it wrote.
#[derive(Debug, Default)]
struct Prior {
    by_identity: BTreeMap<String, (RecordId, String)>,
}

impl Prior {
    fn scan(ledger: &Ledger) -> Self {
        let mut by_identity = BTreeMap::new();
        // Log order, last wins: a source record edited twice is represented by
        // its most recent ingest, which is the one a third edit supersedes.
        for id in ledger.log() {
            let Some(record) = ledger.record(*id) else { continue };
            let Some(reference) = record.envelope().source().reference.as_deref() else {
                continue;
            };
            let Some(origin) = Origin::parse(reference) else { continue };
            by_identity.insert(origin.identity(), (*id, origin.digest));
        }
        Self { by_identity }
    }

    fn record(&self, origin: &Origin) -> Option<RecordId> {
        self.by_identity.get(&origin.identity()).map(|(id, _)| *id)
    }

    fn disposition(&self, origin: &Origin) -> Disposition {
        match self.by_identity.get(&origin.identity()) {
            None => Disposition::Fresh,
            Some((_, digest)) if *digest == origin.digest => Disposition::Unchanged,
            Some(_) => Disposition::Changed,
        }
    }

    /// Source records the ledger holds that this run never saw — present in a
    /// past version of a document and gone from the current one.
    fn absent(&self, seen: &BTreeSet<String>) -> Vec<String> {
        self.by_identity
            .keys()
            .filter(|identity| !seen.contains(*identity))
            .filter_map(|identity| identity.split_once('#'))
            // Top-level source records only: a derived title or cross-reference
            // disappears with its parent and is not separately missing.
            .filter(|(_, key)| !key.contains('/'))
            .map(|(_, key)| key.to_string())
            .collect()
    }
}

/// What one ingest run put into the ledger.
#[derive(Debug, Default)]
pub struct IngestReport {
    pub sources: Vec<(String, EntityId)>,
    pub decisions: Vec<(String, EntityId)>,
    pub unknowns: Vec<(String, EntityId)>,
    pub gaps: Vec<(String, RecordId)>,
    /// Resolved unknowns, as (unknown id, the decision that settled it).
    pub answered: Vec<(String, String)>,
    pub content_claims: Vec<(String, RecordId)>,
    pub title_claims: Vec<(String, RecordId)>,
    pub mention_claims: Vec<(String, String, RecordId)>,
    pub verdicts: Vec<RecordId>,
    pub evidence_links: usize,
    /// Forces vectors the split heuristic produced, for the keeper to check.
    pub proposed_forces: Vec<(String, Vec<String>)>,
    /// One entry per source record considered, in document order.
    pub dispositions: Vec<(String, Disposition)>,
    /// Register rows reworded while their question is still open. The ledger
    /// keeps the old wording, because the grammar has no supersession path for
    /// a gap (U-28) and this ingester does not get to invent one.
    pub drifted: Vec<String>,
    /// Source records the ledger holds that the documents no longer contain.
    /// Reported, never retired: retirement is a verdict, and a deletion is not
    /// a person declaring one.
    pub absent: Vec<String>,
    /// Promotions the document asserts that the ledger declined, with the state
    /// that declined them — a retired claim does not quietly come back.
    pub refused: Vec<(String, String)>,
    /// The ledger already held records and not one of them carried provenance
    /// this sync could read. A store written before D-0021 is the likely
    /// cause, and ingesting into it duplicates the corpus instead of syncing
    /// it — so it is said out loud rather than discovered later. A store built
    /// only from agent proposals looks the same and is harmless, which is why
    /// this is a report and not an error.
    pub unreadable_provenance: bool,
    /// Verdicts the document asserts that this run declined to transcribe,
    /// with the reason: nothing signed carries the words asserting them, or
    /// the signature is not one this run accepts.
    pub withheld: Vec<(String, String)>,
    /// Verdicts transcribed with nothing established about who wrote them.
    /// Recorded in the verdict itself as well, so it stays answerable later.
    pub unattested: Vec<String>,
    /// Records actually written this run.
    written: usize,
}

impl IngestReport {
    /// Records this run actually wrote. Not derivable from the id lists: on a
    /// sync those name what the ledger *holds* for each source record, most of
    /// which some earlier run wrote.
    pub fn appended(&self) -> usize {
        self.written
    }

    pub fn count(&self, disposition: Disposition) -> usize {
        self.dispositions.iter().filter(|(_, d)| *d == disposition).count()
    }

    pub fn with_disposition(&self, disposition: Disposition) -> Vec<&str> {
        self.dispositions
            .iter()
            .filter(|(_, d)| *d == disposition)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Whether this run changed the ledger at all — what a caller needs to
    /// decide between "the corpus is current" and "something moved".
    pub fn quiet(&self) -> bool {
        self.written == 0
            && self.drifted.is_empty()
            && self.refused.is_empty()
            && self.withheld.is_empty()
            && !self.unreadable_provenance
    }

    pub fn decision(&self, id: &str) -> Option<EntityId> {
        self.decisions.iter().find(|(k, _)| k == id).map(|(_, e)| *e)
    }

    pub fn unknown(&self, id: &str) -> Option<EntityId> {
        self.unknowns.iter().find(|(k, _)| k == id).map(|(_, e)| *e)
    }

    /// An anchor of either kind — what a cross-reference resolves against.
    pub fn anchor(&self, id: &str) -> Option<EntityId> {
        self.decision(id).or_else(|| self.unknown(id))
    }

    pub fn gap(&self, id: &str) -> Option<RecordId> {
        self.gaps.iter().find(|(k, _)| k == id).map(|(_, r)| *r)
    }

    pub fn content_claim(&self, id: &str) -> Option<RecordId> {
        self.content_claims.iter().find(|(k, _)| k == id).map(|(_, r)| *r)
    }
}

/// Read and ingest `docs/DECISIONS.md` beneath `repo_root`.
pub fn ingest_decisions(
    ledger: &mut Ledger,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    let decisions = read_doc(repo_root, DECISIONS_DOC)?;
    let attest = Attestations::of_repo(repo_root, Attest::default());
    ingest_text_with(ledger, &decisions, None, repo_root, &attest)
}

/// Ingest both founding documents: the decision records and the register's
/// known unknowns. The register's open questions become gap records, which is
/// what lets the engine answer "that is a registered open question" rather
/// than "nothing found".
pub fn ingest_corpus(
    ledger: &mut Ledger,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    ingest_corpus_with(ledger, repo_root, Attest::default())
}

/// Ingest both documents, insisting on as much as `policy` asks for.
pub fn ingest_corpus_with(
    ledger: &mut Ledger,
    repo_root: &Path,
    policy: Attest,
) -> Result<IngestReport, IngestError> {
    let decisions = read_doc(repo_root, DECISIONS_DOC)?;
    let register = read_doc(repo_root, REGISTER_DOC)?;
    let attest = Attestations::of_repo(repo_root, policy);
    ingest_text_with(ledger, &decisions, Some(&register), repo_root, &attest)
}

fn read_doc(repo_root: &Path, relative: &str) -> Result<String, IngestError> {
    let path = repo_root.join(relative);
    std::fs::read_to_string(&path).map_err(|source| IngestError::Io { path, source })
}

pub fn ingest_text(
    ledger: &mut Ledger,
    text: &str,
    register_text: Option<&str>,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    ingest_text_with(ledger, text, register_text, repo_root, &Attestations::none(Attest::default()))
}

pub fn ingest_text_with(
    ledger: &mut Ledger,
    text: &str,
    register_text: Option<&str>,
    repo_root: &Path,
    attest: &Attestations,
) -> Result<IngestReport, IngestError> {
    // A durable ledger is rehearsed before it is written (D-0057). The
    // parsers run first, but a record's state, its dates, its evidence and
    // its hypothesis signals are judged inside the append phases, so a fault
    // in the fortieth record used to land after thirty-nine were on disk —
    // the store then held a corpus its author had been told was refused.
    // The rehearsal ingests the same texts into a scratch ledger with the
    // same attestation; only if every record passes does the real pass
    // begin. What the rehearsal cannot see is the store's own history — a
    // disposition that exists only against prior records — and those paths
    // report rather than fail by design (U-19), so a failure that survives
    // the rehearsal is a bug in the sync, not a fault in the document.
    // In-memory ledgers are not rehearsed: a failed pass leaves nothing
    // anyone will open again.
    if ledger.journal_path().is_some() {
        ingest_pass(&mut Ledger::new(), text, register_text, repo_root, attest)?;
    }
    ingest_pass(ledger, text, register_text, repo_root, attest)
}

fn ingest_pass(
    ledger: &mut Ledger,
    text: &str,
    register_text: Option<&str>,
    repo_root: &Path,
    attest: &Attestations,
) -> Result<IngestReport, IngestError> {
    let parsed = parse_corpus(text)?;
    let unknowns = match register_text {
        Some(register) => parse_register(register)?,
        None => Vec::new(),
    };
    let register_author = match register_text {
        Some(register) => Some(Author::human(
            register_owner(register).ok_or(IngestError::MissingRegisterOwner)?,
        )),
        None => None,
    };
    let mut report = IngestReport::default();
    // What an earlier run left behind, read back out of the records' own
    // provenance. Empty for a fresh ledger, which is the common case and the
    // one where every disposition below is `Fresh`.
    let prior = Prior::scan(ledger);
    report.unreadable_provenance = !ledger.log().is_empty() && prior.by_identity.is_empty();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    // New claim → the promoted claim it supersedes, for phase 3's single
    // promote-and-retire verdict.
    let mut retiring: BTreeMap<RecordId, RecordId> = BTreeMap::new();

    // Identity first, for both corpora, so a cross-reference resolves in
    // either direction: a decision naming U-1 and an unknown naming D-0012
    // both find an anchor. `upsert_entity` is already idempotent, so anchors
    // need no sync of their own.
    for record in &parsed {
        let entity = ledger.upsert_entity(DECISION_KIND, &record.id)?;
        report.decisions.push((record.id.clone(), entity));
    }
    for unknown in &unknowns {
        let entity = ledger.upsert_entity(UNKNOWN_KIND, &unknown.id)?;
        report.unknowns.push((unknown.id.clone(), entity));
    }

    // Phase 1 — every decision record lands proposed, unless the ledger
    // already holds it word for word.
    for record in &parsed {
        let origin = Origin::new(DECISIONS_DOC, &record.id, &digest(&record.raw))
            .noted(record.yaml.get("recorded").map(String::as_str));
        let disposition = prior.disposition(&origin);
        report.dispositions.push((record.id.clone(), disposition));
        seen.insert(origin.identity());

        // A reworded hypothesis supersedes its predecessor and abandons it —
        // but only while that predecessor is still an open prediction. One
        // already scored is history, and history is not rewritten.
        let abandoning = (disposition == Disposition::Changed && is_hypothesis(record))
            .then(|| prior.record(&origin))
            .flatten()
            .filter(|old| {
                ledger.state_of(*old) == Some(RecordState::Hypothesis(HypothesisState::Registered))
            });
        let settled_history = disposition == Disposition::Changed
            && is_hypothesis(record)
            && abandoning.is_none();
        if settled_history {
            report.drifted.push(record.id.clone());
        }
        if disposition == Disposition::Unchanged || settled_history {
            carry_forward(&prior, &origin, record, &mut seen, &mut report);
            continue;
        }
        ingest_one(ledger, record, &origin, &prior, repo_root, &mut seen, &mut retiring, &mut report)?;
        if let Some(old) = abandoning
            && let attestation = attest.decisions(record.lines)
            && attest.admits(&attestation).is_ok()
        {
            let verdict = ledger.append(Draft::new(
                transcriber(record.require("author")?, &attestation),
                SourceRef {
                    channel: "corpus-ingest".into(),
                    reference: Some(format!("{DECISIONS_DOC} {} reworded", record.id)),
                },
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Abandon {
                        hypothesis: old,
                        reason: WithdrawReason::Superseded,
                    },
                    rationale: Some(format!(
                        "transcribed from {DECISIONS_DOC}: {} is stated differently than \
                         the wording this record replaces",
                        record.id
                    )),
                }),
            ))?;
            report.verdicts.push(verdict);
            report.written += 1;
        }
    }

    // Phase 2 — every register row lands as a registered gap.
    if let Some(author) = &register_author {
        for unknown in &unknowns {
            let origin = Origin::new(REGISTER_DOC, &unknown.id, &digest(&unknown.raw));
            let disposition = prior.disposition(&origin);
            report.dispositions.push((unknown.id.clone(), disposition));
            seen.insert(origin.identity());

            let held = prior.record(&origin);
            let open =
                held.filter(|g| ledger.state_of(*g) == Some(RecordState::Gap(GapState::Registered)));

            match (disposition, open) {
                (Disposition::Fresh, _) => {
                    ingest_gap(ledger, unknown, &origin, None, author, &mut report)?;
                }
                // Reworded while still open: register the new wording carrying
                // the link back, and withdraw the old as superseded. Two
                // records because registration is not a verdict — there is no
                // promotion to fold the retirement into, the way a claim has.
                (Disposition::Changed, Some(old)) => {
                    ingest_gap(ledger, unknown, &origin, Some(old), author, &mut report)?;
                    let attestation = attest.register(unknown.line);
                    let verdict = ledger.append(Draft::new(
                        transcriber(&author.name, &attestation),
                        SourceRef {
                            channel: "register".into(),
                            reference: Some(format!("{REGISTER_DOC} {} reworded", unknown.id)),
                        },
                        Content::Verdict(VerdictContent {
                            action: VerdictAction::Withdraw {
                                gap: old,
                                reason: WithdrawReason::Superseded,
                            },
                            rationale: Some(format!(
                                "transcribed from {REGISTER_DOC}: {} is asked differently \
                                 than the wording this row replaces",
                                unknown.id
                            )),
                        }),
                    ))?;
                    report.verdicts.push(verdict);
                    report.written += 1;
                }
                // Reworded after it was settled. Left alone: the register says
                // history is never rewritten, and this is history.
                (Disposition::Changed, None) => {
                    report.drifted.push(unknown.id.clone());
                    report.gaps.push((unknown.id.clone(), held.expect("not fresh")));
                }
                (Disposition::Unchanged, _) => {
                    report.gaps.push((unknown.id.clone(), held.expect("not fresh")));
                }
            }
        }
    }

    // Phase 3 — transcribe the verdicts the decision document records.
    for record in &parsed {
        let state = record.require("state")?;
        // Who put these words in the document, as far as git can say. The
        // promotion about to be transcribed is for the record as it now reads,
        // so what is attested is the whole of the record's current text.
        let attestation = attest.decisions(record.lines);
        let author = transcriber(record.require("author")?, &attestation);
        match state {
            "promoted" => {
                let targets = [
                    report.content_claim(&record.id),
                    report
                        .title_claims
                        .iter()
                        .find(|(k, _)| *k == record.id)
                        .map(|(_, r)| *r),
                ];
                if let Err(why) = attest.admits(&attestation)
                    && targets.iter().flatten().any(|t| {
                        ledger.state_of(*t) == Some(RecordState::Claim(ClaimState::Proposed))
                    })
                {
                    // The document asserts a promotion and nothing this run
                    // will accept carries the words asserting it. Declining
                    // leaves the claim proposed, which is the safe direction to
                    // fail: the record still loads, and nobody was promoted by
                    // prose.
                    report.withheld.push((record.id.clone(), why));
                    continue;
                }
                if matches!(attestation, Attestation::None { .. }) {
                    report.unattested.push(record.id.clone());
                }
                // One editorial act, one verdict. A person wrote `state:
                // promoted` once; the keeper split the record into a claim and
                // a title because the model wanted them apart, and charging
                // that split back to the author as two declarations was the
                // transcription cost U-20 recorded (D-0034).
                let mut promoting: Vec<RecordId> = Vec::new();
                let mut retires: Vec<RecordId> = Vec::new();
                for target in targets.into_iter().flatten() {
                    // On a sync most targets are already promoted by an earlier
                    // run's transcription of the same line.
                    match ledger.state_of(target) {
                        Some(RecordState::Claim(ClaimState::Proposed)) => {}
                        Some(RecordState::Claim(ClaimState::Promoted)) => continue,
                        Some(state) => {
                            report.refused.push((record.id.clone(), state.to_string()));
                            continue;
                        }
                        None => continue,
                    }
                    if let Some(prior) = retiring.get(&target).copied().filter(|prior| {
                        ledger.state_of(*prior) == Some(RecordState::Claim(ClaimState::Promoted))
                    }) {
                        retires.push(prior);
                    }
                    promoting.push(target);
                }
                if !promoting.is_empty() {
                    let replaced = !retires.is_empty();
                    let verdict = ledger.append(Draft::new(
                        author.clone(),
                        SourceRef {
                            channel: "corpus-ingest".into(),
                            reference: Some(format!("{DECISIONS_DOC} {} state:", record.id)),
                        },
                        Content::Verdict(VerdictContent {
                            action: VerdictAction::PromoteSet {
                                targets: promoting,
                                retiring: retires,
                                basis: SetBasis::OneAct,
                            },
                            rationale: Some(if replaced {
                                format!(
                                    "transcribed from {DECISIONS_DOC}: {} was edited and \
                                     still carries `state: promoted`; the prior wording is \
                                     retired as superseded",
                                    record.id
                                )
                            } else {
                                format!(
                                    "transcribed from {DECISIONS_DOC}: {} carries \
                                     `state: promoted`",
                                    record.id
                                )
                            }),
                        }),
                    ))?;
                    report.verdicts.push(verdict);
                    report.written += 1;
                }
            }
            // A hypothesis is never promoted — it is scored, and this one is
            // not yet due.
            "registered" => {}
            other => {
                return Err(IngestError::UnsupportedState {
                    record: record.id.clone(),
                    state: other.to_string(),
                });
            }
        }
    }

    // Phase 4 — transcribe the register's resolutions. Last, because the
    // engine refuses to answer a gap with a claim that is not yet promoted:
    // the ordering is forced by the grammar, not chosen for convenience.
    if let Some(author) = &register_author {
        for unknown in &unknowns {
            let Some(resolution) = &unknown.resolved else { continue };
            let gap = report.gap(&unknown.id).expect("gap appended in phase 2");
            // Already settled by an earlier run's transcription of the same row.
            if ledger.state_of(gap) != Some(RecordState::Gap(tacit_core::GapState::Registered)) {
                continue;
            }
            // Marking a question resolved is a verdict too, and the register is
            // as editable as the decision document is.
            let attestation = attest.register(unknown.line);
            if let Err(why) = attest.admits(&attestation) {
                report.withheld.push((unknown.id.clone(), why));
                continue;
            }
            if matches!(attestation, Attestation::None { .. }) {
                report.unattested.push(unknown.id.clone());
            }
            let settled_by = resolution.by.as_ref().and_then(|d| report.content_claim(d));
            let (action, rationale) = match (settled_by, &resolution.by) {
                (Some(with_claim), Some(decision)) => (
                    VerdictAction::Answer { gap, with_claim },
                    format!(
                        "transcribed from docs/REGISTER.md: {} resolved {} by {decision}",
                        unknown.id, resolution.date
                    ),
                ),
                _ => (
                    // The row says it was resolved and names nothing that
                    // resolved it. That is not "we stopped asking" — it is an
                    // answer this ledger does not hold, which is exactly the
                    // kind of thing a keeper should be able to go looking for.
                    VerdictAction::Withdraw {
                        gap,
                        reason: WithdrawReason::AnsweredElsewhere,
                    },
                    format!(
                        "transcribed from docs/REGISTER.md: {} resolved {} with no \
                         settling record named",
                        unknown.id, resolution.date
                    ),
                ),
            };
            let verdict = ledger.append(Draft::new(
                transcriber(&author.name, &attestation),
                SourceRef {
                    channel: "corpus-ingest".into(),
                    reference: Some(format!("{REGISTER_DOC} {}", unknown.id)),
                },
                Content::Verdict(VerdictContent { action, rationale: Some(rationale) }),
            ))?;
            report.verdicts.push(verdict);
            report.written += 1;
            if let Some(decision) = &resolution.by {
                report.answered.push((unknown.id.clone(), decision.clone()));
            }
        }
    }

    report.absent = prior.absent(&seen);
    Ok(report)
}

/// An unchanged source record: nothing is written, and the report points at
/// what the ledger already holds so the later phases resolve exactly as they
/// would have on a fresh run.
fn carry_forward(
    prior: &Prior,
    origin: &Origin,
    record: &ParsedRecord,
    seen: &mut BTreeSet<String>,
    report: &mut IngestReport,
) {
    if let Some(id) = prior.record(origin) {
        report.content_claims.push((record.id.clone(), id));
    }
    let title = origin.role("title");
    seen.insert(title.identity());
    if let Some(id) = prior.record(&title) {
        report.title_claims.push((record.id.clone(), id));
    }
    for other in mentioned_ids(&record.raw, &record.id) {
        let edge = origin.role(&format!("mentions/{other}"));
        seen.insert(edge.identity());
        if let Some(id) = prior.record(&edge) {
            report.mention_claims.push((record.id.clone(), other, id));
        }
    }
}

/// One register row becomes one gap: the question, the territory it covers,
/// and its trigger as the review trigger. The Notes column is carried into the
/// question rather than dropped — a register that loses its own commentary on
/// ingest would be a poor advertisement for a corpus about honesty.
fn ingest_gap(
    ledger: &mut Ledger,
    unknown: &ParsedUnknown,
    origin: &Origin,
    supersedes: Option<RecordId>,
    author: &Author,
    report: &mut IngestReport,
) -> Result<(), IngestError> {
    let anchor = report.unknown(&unknown.id).expect("anchor minted above");
    let mut territory = vec![anchor];
    for mention in unknown.mentions() {
        if let Some(entity) = report.anchor(&mention) {
            territory.push(entity);
        }
    }

    let question = if unknown.notes.is_empty() {
        unknown.question.clone()
    } else {
        format!("{}\n\nNotes. {}", unknown.question, unknown.notes)
    };

    let trigger = unknown.trigger.trim();
    let review_trigger = (!trigger.is_empty() && trigger != "—").then(|| ReviewTrigger {
        due_at: None,
        on_event: Some(trigger.to_string()),
    });

    let mut draft = Draft::new(
        author.clone(),
        SourceRef { channel: "register".into(), reference: Some(origin.to_string()) },
        Content::Gap(GapContent { question, territory }),
    );
    draft.review_trigger = review_trigger;
    draft.supersedes = supersedes;
    let id = ledger.append(draft)?;
    report.gaps.push((unknown.id.clone(), id));
    report.written += 1;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ingest_one(
    ledger: &mut Ledger,
    record: &ParsedRecord,
    origin: &Origin,
    prior: &Prior,
    repo_root: &Path,
    seen: &mut BTreeSet<String>,
    retiring: &mut BTreeMap<RecordId, RecordId>,
    report: &mut IngestReport,
) -> Result<(), IngestError> {
    let subject = report.decision(&record.id).expect("anchor minted above");
    let author = Author::human(record.require("author")?);
    let channel = record.require("source")?.to_string();
    // The reference carries the document's own stated record-time verbatim
    // rather than backdating the ledger's, and the digest that lets the next
    // run recognise this record.
    let source = SourceRef { channel: channel.clone(), reference: Some(origin.to_string()) };
    let valid_from = parse_date(record, "valid_from")?;

    let review_trigger = record.yaml.get("review_trigger").map(|prose| ReviewTrigger {
        due_at: None,
        on_event: Some(prose.clone()),
    });

    let evidence = resolve_evidence(ledger, record, repo_root, report)?;
    report.evidence_links += evidence.len();

    let content = build_content(record, subject)?;
    if let Content::Claim(ClaimContent::Pattern { forces, .. }) = &content {
        report.proposed_forces.push((record.id.clone(), forces.clone()));
    }

    let mut draft = Draft::new(author.clone(), source.clone(), content);
    draft.valid_from = valid_from;
    draft.evidence = evidence;
    draft.review_trigger = review_trigger.clone();
    draft.supersedes = prior.record(origin);
    let content_claim = ledger.append(draft)?;
    report.content_claims.push((record.id.clone(), content_claim));
    report.written += 1;
    note_supersession(ledger, prior, origin, content_claim, retiring);

    // The heading title, transcribed verbatim as a node property.
    let title_origin = origin.role("title");
    seen.insert(title_origin.identity());
    let mut title = Draft::new(
        author.clone(),
        SourceRef { channel: channel.clone(), reference: Some(title_origin.to_string()) },
        Content::Claim(ClaimContent::Attribute {
            subject,
            name: "title".into(),
            value: tacit_core::Value::Text(record.title.clone()),
        }),
    );
    title.valid_from = valid_from;
    title.supersedes = prior.record(&title_origin);
    let title_claim = ledger.append(title)?;
    report.title_claims.push((record.id.clone(), title_claim));
    report.written += 1;
    note_supersession(ledger, prior, &title_origin, title_claim, retiring);

    // Cross-references, as observed: this record's text names that record.
    // These stay *proposed* — no human has ratified the machine's reading, so
    // the default graph shows no edges until someone does.
    for other in mentioned_ids(&record.raw, &record.id) {
        let edge_origin = origin.role(&format!("mentions/{other}"));
        seen.insert(edge_origin.identity());
        let Some(object) = report.anchor(&other) else { continue };
        let mut edge = Draft::new(
            Author::agent("corpus-ingest"),
            SourceRef { channel: channel.clone(), reference: Some(edge_origin.to_string()) },
            Content::Claim(ClaimContent::Relation {
                subject,
                predicate: MENTIONS.into(),
                object,
                properties: BTreeMap::new(),
            }),
        );
        edge.valid_from = valid_from;
        edge.supersedes = prior.record(&edge_origin);
        let id = ledger.append(edge)?;
        report.mention_claims.push((record.id.clone(), other, id));
        report.written += 1;
    }

    Ok(())
}

/// Note the promoted claim a new claim replaces, so phase 3 can promote the
/// new and retire the old in one verdict. Only a *promoted* predecessor is
/// recorded: retiring is a transition out of the promoted set, and a claim
/// that never entered it has nothing to leave.
/// The document's own signal, matched by `build_content`, which cross-checks
/// it against the record's sections and hard-errors when the two disagree.
fn is_hypothesis(record: &ParsedRecord) -> bool {
    record.id.starts_with('H')
}

fn note_supersession(
    ledger: &Ledger,
    prior: &Prior,
    at: &Origin,
    new: RecordId,
    retiring: &mut BTreeMap<RecordId, RecordId>,
) {
    if let Some(old) = prior.record(at)
        && ledger.state_of(old) == Some(RecordState::Claim(ClaimState::Promoted))
    {
        retiring.insert(new, old);
    }
}

fn build_content(record: &ParsedRecord, subject: EntityId) -> Result<Content, IngestError> {
    let is_hypothesis_id = record.id.starts_with('H');
    let has_hypothesis_section = record.section("Hypothesis").is_some();
    if is_hypothesis_id != has_hypothesis_section {
        return Err(IngestError::HypothesisSignalMismatch { record: record.id.clone() });
    }

    if is_hypothesis_id {
        let score_by = parse_date(record, "score_by")?.ok_or_else(|| IngestError::BadDate {
            record: record.id.clone(),
            key: "score_by".into(),
            value: String::new(),
        })?;
        return Ok(Content::Hypothesis(HypothesisContent {
            statement: record.section("Hypothesis").expect("checked").to_string(),
            falsifier: record.section("Falsifier").map(str::to_string),
            score_by,
        }));
    }
    if record.yaml.contains_key("score_by") {
        return Err(IngestError::StrayScoreBy { record: record.id.clone() });
    }

    let assertion = record.section("Assertion").unwrap_or_default();
    let trailing = |skip: &[&str]| -> String {
        record
            .sections
            .iter()
            .filter(|(label, _)| !skip.contains(&label.as_str()))
            .map(|(label, body)| format!("\n\n{label}. {body}"))
            .collect()
    };

    Ok(match record.section("Forces") {
        Some(forces) => Content::Claim(ClaimContent::Pattern {
            context: record.title.clone(),
            forces: split_forces(forces),
            solution: format!("{assertion}{}", trailing(&["Assertion", "Forces"])),
            about: vec![subject],
        }),
        None => Content::Claim(ClaimContent::Text {
            body: format!("{assertion}{}", trailing(&["Assertion"])),
            about: vec![subject],
        }),
    })
}

/// Semicolons are the author's own force separator. Sentence-splitting would
/// mis-fire on clauses ending in a quoted phrase, so this stays deliberately
/// crude — and the ingest reports what it produced, because the split is the
/// machine proposing, not deciding.
fn split_forces(paragraph: &str) -> Vec<String> {
    let parts: Vec<String> = paragraph
        .split("; ")
        .map(|p| p.trim().trim_end_matches('.').trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() { vec![paragraph.trim().to_string()] } else { parts }
}

fn parse_date(
    record: &ParsedRecord,
    key: &str,
) -> Result<Option<jiff::Timestamp>, IngestError> {
    let Some(raw) = record.yaml.get(key) else { return Ok(None) };
    let value = raw.split_whitespace().next().unwrap_or(raw);
    let date: Date = value.parse().map_err(|_| IngestError::BadDate {
        record: record.id.clone(),
        key: key.to_string(),
        value: value.to_string(),
    })?;
    let stamp = date
        .to_zoned(TimeZone::UTC)
        .map_err(|_| IngestError::BadDate {
            record: record.id.clone(),
            key: key.to_string(),
            value: value.to_string(),
        })?
        .timestamp();
    Ok(Some(stamp))
}

/// Resolve each evidence entry to a source entity, hard-erroring on anything
/// that does not exist on disk or escapes the repo. Labels are stored
/// repo-relative: absolute paths would carry the author's home directory into
/// the corpus.
fn resolve_evidence(
    ledger: &mut Ledger,
    record: &ParsedRecord,
    repo_root: &Path,
    report: &mut IngestReport,
) -> Result<Vec<Evidence>, IngestError> {
    let Some(list) = record.yaml.get("evidence") else { return Ok(Vec::new()) };
    let mut links = Vec::new();

    for entry in split_evidence(list) {
        let (raw_path, span) = split_entry(&entry);
        let relative = resolve_path(&raw_path, repo_root).ok_or_else(|| {
            IngestError::UnresolvableEvidence {
                record: record.id.clone(),
                entry: entry.clone(),
            }
        })?;
        if relative.starts_with("..") {
            return Err(IngestError::EvidenceEscapesRepo {
                record: record.id.clone(),
                entry: entry.clone(),
            });
        }
        let source = ledger.upsert_entity(tacit_core::SOURCE_KIND, &relative)?;
        if !report.sources.iter().any(|(label, _)| *label == relative) {
            report.sources.push((relative.clone(), source));
        }
        links.push(Evidence { source, span });
    }
    Ok(links)
}

/// `design/001-data-model.md §3` → (path, span). `this file — ...` is the
/// corpus referring to itself.
fn split_entry(entry: &str) -> (String, Option<String>) {
    if let Some(rest) = entry.strip_prefix("this file") {
        let span = rest.trim_start_matches([' ', '—']).trim();
        return (
            "DECISIONS.md".to_string(),
            (!span.is_empty()).then(|| span.to_string()),
        );
    }
    match entry.split_once(' ') {
        Some((path, span)) => (path.to_string(), Some(span.trim().to_string())),
        None => (entry.to_string(), None),
    }
}

/// Try `docs/<path>` then `<path>`, both under the repo root. Returns the
/// repo-relative form.
///
/// Absolute candidates are rejected before anything else: `Path::join`
/// *discards its base* when the argument is absolute, so `docs`.join("/etc/x")
/// is `/etc/x` — which would defeat both the prefix and the containment check
/// and store an absolute path as a source label. Containment is then confirmed
/// against canonical paths so a symlink cannot walk out of the repo either.
fn resolve_path(candidate: &str, repo_root: &Path) -> Option<String> {
    let cleaned = candidate.trim_end_matches('/');
    let as_path = Path::new(cleaned);
    if cleaned.is_empty() || as_path.is_absolute() {
        return None;
    }
    if as_path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }

    let root = repo_root.canonicalize().ok()?;
    for prefix in ["docs", ""] {
        let relative =
            if prefix.is_empty() { as_path.to_path_buf() } else { Path::new(prefix).join(as_path) };
        let full = repo_root.join(&relative);
        if !full.exists() {
            continue;
        }
        if !full.canonicalize().ok()?.starts_with(&root) {
            return None;
        }
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::Attestation;
    use tacit_core::{ClaimState, Projection, RecordState, StateFilter, ViewSpec};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn ingested() -> (Ledger, IngestReport) {
        let mut ledger = Ledger::new();
        let report = ingest_decisions(&mut ledger, &repo_root()).expect("corpus ingests");
        (ledger, report)
    }

    /// The document is expected to grow. These tests derive what to expect
    /// from an independent parse rather than freezing a census of it — a
    /// hardcoded count would turn every new decision record into a red suite.
    fn parsed() -> Vec<crate::parse::ParsedRecord> {
        let text = std::fs::read_to_string(repo_root().join("docs/DECISIONS.md")).unwrap();
        crate::parse::parse_corpus(&text).unwrap()
    }


    // ── U-29: a verdict transcribed from prose ──────────────────────────────

    fn verdicts_of<'a>(ledger: &'a Ledger, report: &IngestReport) -> Vec<&'a tacit_core::Record> {
        report.verdicts.iter().filter_map(|id| ledger.record(*id)).collect()
    }

    #[test]
    fn every_transcribed_verdict_says_how_its_author_is_known() {
        let (ledger, report) = ingested();
        let verdicts = verdicts_of(&ledger, &report);
        assert!(!verdicts.is_empty());
        for verdict in verdicts {
            let detail = verdict
                .envelope()
                .author()
                .detail
                .as_deref()
                .expect("a transcribed verdict states what backs it");
            // Parseability, not signedness: the working tree may well be dirty
            // while these tests run, and "these lines are not committed" is a
            // perfectly good thing for a record to say.
            assert!(
                Attestation::parse(detail).is_some(),
                "unreadable attestation {detail:?} on {}",
                verdict.id()
            );
        }
    }

    #[test]
    fn naming_the_signers_is_the_other_half_of_whose_signature_counts() {
        let key = "F63F9CB7003A73E3";
        let mine = |signer: &str| Attestation::Signed {
            commit: "d6eb4a8".into(),
            key: key.into(),
            signer: signer.into(),
        };
        let stranger_key = Attestation::UnknownKey {
            commit: "d6eb4a8".into(),
            key: "0000000000000000".into(),
            signer: "Greg Villa".into(),
        };

        // Asking only for a signature accepts any key this machine trusts,
        // whoever holds it.
        let any = Attest::RequireSignature;
        assert!(any.admits(&mine("Greg Villa")).is_ok());
        assert!(any.admits(&mine("A Colleague")).is_ok());

        let named = Attest::RequireSignatureFrom(["Greg Villa".to_string()].into());
        assert!(named.admits(&mine("Greg Villa")).is_ok());
        let why = named.admits(&mine("A Colleague")).unwrap_err();
        assert!(why.contains("not named as a bearer of verdicts"), "got {why}");
        // Exact: a loose match would admit every Gregory who ever signed
        // anything, and the error names what it saw so a mistype is fixable.
        assert!(Attest::RequireSignatureFrom(["Greg".to_string()].into())
            .admits(&mine("Greg Villa"))
            .is_err());
        assert!(named.admits(&mine("Greg Villa Jr")).is_err());

        // And the right name on a key nobody vouched for is still nothing: the
        // name is matched against the identity the *signature* binds, not
        // against a field anyone can type.
        assert!(named.admits(&stranger_key).is_err());
        assert!(any.admits(&stranger_key).is_err());

        // Observing asks for nothing, which is what makes it usable over a
        // document its author is still writing.
        assert!(Attest::Observe.admits(&stranger_key).is_ok());
    }

    #[test]
    fn a_signer_this_repository_never_had_carries_no_verdict() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let report = ingest_corpus_with(
            &mut ledger,
            &root,
            Attest::RequireSignatureFrom(["Nobody Of That Name".to_string()].into()),
        )
        .expect("ingest");

        // Whatever the state of the working tree, nothing survives a policy
        // that names a signer this repository has never had — and the corpus
        // still loads, every claim of it waiting on a person.
        assert!(report.verdicts.is_empty());
        assert!(!report.withheld.is_empty());
        assert_eq!(ledger.promoted_claims().count(), 0);
        assert!(!report.content_claims.is_empty(), "the claims are still ingested");
    }

    #[test]
    fn a_corpus_supplied_as_text_promotes_and_records_that_nothing_backed_it() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let report =
            ingest_text(&mut ledger, &decisions_doc(&[("D-0001", "Four forces.")]), None, &root)
                .expect("ingest");

        // It still promotes — the default is to observe, not to obstruct.
        assert_eq!(
            ledger.state_of(report.content_claim("D-0001").unwrap()),
            Some(RecordState::Claim(ClaimState::Promoted))
        );
        assert!(report.unattested.contains(&"D-0001".to_string()));
        assert!(report.withheld.is_empty());
        assert!(!report.quiet());

        // And the record says so itself, permanently, rather than the check
        // having happened once and left no trace.
        let detail = verdicts_of(&ledger, &report)[0]
            .envelope()
            .author()
            .detail
            .clone()
            .expect("detail");
        assert!(matches!(Attestation::parse(&detail), Some(Attestation::None { .. })));
        // Both claims — the content and the title — promoted by the one set
        // verdict, and both still counted by an audit that reads what the
        // verdict actually did rather than which variant it is.
        assert_eq!(crate::attest::unattested_promotions(&ledger).len(), 2);
        assert_eq!(ledger.ratification().in_sets.values().sum::<usize>(), 2);
        assert_eq!(ledger.ratification().individually, 0);
    }

    /// The whole of U-29: write access to the document was promotion authority.
    #[test]
    fn requiring_a_signature_leaves_the_claim_proposed_rather_than_promoting_on_prose() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let report = ingest_text_with(
            &mut ledger,
            &decisions_doc(&[("D-0001", "Four forces.")]),
            None,
            &root,
            &Attestations::none(Attest::RequireSignature),
        )
        .expect("ingest");

        // The record loads; nobody is promoted by prose. Failing in this
        // direction is the point — a claim left proposed is a claim awaiting a
        // person, which is where it should have been all along.
        let claim = report.content_claim("D-0001").expect("the claim is still ingested");
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Proposed)));
        assert_eq!(report.withheld.len(), 1);
        assert_eq!(report.withheld[0].0, "D-0001");
        assert!(report.verdicts.is_empty());
        assert!(crate::attest::unattested_promotions(&ledger).is_empty());
    }

    #[test]
    fn the_register_is_as_editable_as_the_decision_document() {
        let root = repo_root();
        let register = "## Room 2 · Known unknowns\n\n\
             | id | Question | Trigger | Notes |\n\
             |----|----------|---------|-------|\n\
             | U-1 | ~~Whether a query language is needed~~ **Resolved 2026-08-23** \
             in conversation | — | nothing here states the answer |\n\n\
             *Recorded 2026-08-23. Owner: Greg Villa.*\n";
        let mut ledger = Ledger::new();
        let report = ingest_text_with(
            &mut ledger,
            &decisions_doc(&[("D-0001", "Four forces.")]),
            Some(register),
            &root,
            &Attestations::none(Attest::RequireSignature),
        )
        .expect("ingest");

        // Marking a question resolved is a verdict too, and an agent that can
        // edit one document can edit the other.
        let gap = report.gap("U-1").expect("the gap is still registered");
        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Registered)));
        assert!(report.withheld.iter().any(|(id, _)| id == "U-1"));
    }

    #[test]
    fn an_attestation_survives_the_store() {
        let root = repo_root();
        let path = std::env::temp_dir().join(format!("tacit-attest-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let claim = {
            let mut ledger = Ledger::open(&path).expect("open").ledger;
            ingest_corpus(&mut ledger, &root).expect("ingest").content_claim("D-0001").unwrap()
        };

        // The attestation is envelope data, so it replays through the grammar
        // with everything else — a check that left no durable trace would be
        // no better than no check.
        let ledger = Ledger::open(&path).expect("reopen").ledger;
        let verdict = ledger.history(claim)[0];
        let detail = verdict.envelope().author().detail.as_deref().expect("detail survived");
        assert!(Attestation::parse(detail).is_some(), "got {detail:?}");

        let _ = std::fs::remove_file(&path);
    }

    // ── U-19: ingest is a sync, not a load ──────────────────────────────────

    /// A corpus small enough that every append is countable and every edit is
    /// deliberate. The real documents exercise scale; these exercise identity.
    fn decisions_doc(records: &[(&str, &str)]) -> String {
        let mut doc = String::new();
        for (id, assertion) in records {
            doc.push_str(&format!(
                "## {id} · The forces driving the build\n\n\
                 ```yaml\n\
                 id: {id}\n\
                 state: promoted\n\
                 author: Greg Villa\n\
                 source: founding-interview / round 1\n\
                 recorded: 2026-08-22\n\
                 valid_from: 2026-08-22\n\
                 ```\n\n\
                 **Assertion.** {assertion}\n\n"
            ));
        }
        doc
    }

    fn register_doc(note: &str) -> String {
        format!(
            "## Room 2 · Known unknowns\n\n\
             | id | Question | Trigger | Notes |\n\
             |----|----------|---------|-------|\n\
             | U-1 | Whether a query language is needed | first agent usage | {note} |\n\n\
             *Recorded 2026-08-23. Owner: Greg Villa.*\n"
        )
    }

    #[test]
    fn re_ingesting_an_unchanged_corpus_writes_nothing() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let first = ingest_corpus(&mut ledger, &root).expect("first ingest");
        let length = ledger.log().len();
        assert!(first.appended() > 0);
        assert_eq!(first.count(Disposition::Unchanged), 0, "a fresh ledger holds nothing");

        let second = ingest_corpus(&mut ledger, &root).expect("second ingest");
        assert_eq!(second.appended(), 0, "the same documents twice write nothing");
        assert_eq!(ledger.log().len(), length, "and the log is exactly as long");
        assert!(second.quiet());
        assert_eq!(second.count(Disposition::Fresh), 0);
        assert!(second.absent.is_empty() && second.drifted.is_empty());

        // The report still resolves, which is what the later phases need: they
        // must find the ids of records this run did not write.
        assert_eq!(second.content_claims.len(), first.content_claims.len());
        assert_eq!(second.gaps.len(), first.gaps.len());
        assert_eq!(second.content_claim("D-0001"), first.content_claim("D-0001"));
        assert_eq!(
            ledger.state_of(second.content_claim("D-0001").unwrap()),
            Some(RecordState::Claim(ClaimState::Promoted))
        );
    }

    #[test]
    fn an_edited_record_supersedes_its_predecessor_and_retires_it() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let before = ingest_text(
            &mut ledger,
            &decisions_doc(&[("D-0001", "Four forces jointly motivate the build.")]),
            None,
            &root,
        )
        .expect("first ingest");
        let old = before.content_claim("D-0001").unwrap();
        assert_eq!(ledger.state_of(old), Some(RecordState::Claim(ClaimState::Promoted)));

        let after = ingest_text(
            &mut ledger,
            &decisions_doc(&[("D-0001", "Five forces jointly motivate the build.")]),
            None,
            &root,
        )
        .expect("second ingest");
        let new = after.content_claim("D-0001").unwrap();

        assert_eq!(after.count(Disposition::Changed), 1);
        assert_ne!(new, old);
        assert_eq!(ledger.record(new).unwrap().envelope().supersedes(), Some(old));
        assert_eq!(ledger.state_of(new), Some(RecordState::Claim(ClaimState::Promoted)));
        assert_eq!(ledger.state_of(old), Some(RecordState::Claim(ClaimState::Retired)));

        // One editorial act, one verdict: promoting the new wording *is*
        // retiring the old (design/001 §3.1), not two decisions that happen to
        // agree.
        let promoting = ledger.history(new);
        assert_eq!(promoting.len(), 1);
        assert_eq!(promoting[0].id(), ledger.history(old)[1].id());
    }

    #[test]
    fn a_record_dropped_from_the_document_is_reported_not_retired() {
        let root = repo_root();
        let both = decisions_doc(&[("D-0001", "Four forces."), ("D-0002", "Two layers.")]);
        let one = decisions_doc(&[("D-0001", "Four forces.")]);

        let mut ledger = Ledger::new();
        let before = ingest_text(&mut ledger, &both, None, &root).expect("first ingest");
        let dropped = before.content_claim("D-0002").unwrap();

        let after = ingest_text(&mut ledger, &one, None, &root).expect("second ingest");
        assert_eq!(after.absent, vec!["D-0002".to_string()]);
        assert_eq!(after.appended(), 0);
        // Deleting a paragraph is not a person retiring a decision, and the
        // document no longer contains the words that would say so.
        assert_eq!(ledger.state_of(dropped), Some(RecordState::Claim(ClaimState::Promoted)));
        assert!(!after.quiet() || !after.absent.is_empty());
    }

    fn hypothesis_doc(statement: &str) -> String {
        format!(
            "## H-0001 · Success hypothesis\n\n\
             ```yaml\n\
             id: H-0001\n\
             state: registered\n\
             author: Greg Villa\n\
             source: founding-interview / round 3\n\
             recorded: 2026-08-22\n\
             score_by: 2027-02-22\n\
             ```\n\n\
             **Hypothesis.** {statement}\n\n"
        )
    }

    fn withdraw_reason(record: &tacit_core::Record) -> Option<WithdrawReason> {
        match record.content() {
            Content::Verdict(v) => match v.action {
                VerdictAction::Withdraw { reason, .. } => Some(reason),
                VerdictAction::Abandon { reason, .. } => Some(reason),
                _ => None,
            },
            _ => None,
        }
    }

    #[test]
    fn a_reworded_hypothesis_supersedes_the_prediction_it_replaces() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let before =
            ingest_text(&mut ledger, &hypothesis_doc("Within six months it self-hosts."), None, &root)
                .expect("first ingest");
        let old = before.content_claim("H-0001").unwrap();

        let after =
            ingest_text(&mut ledger, &hypothesis_doc("Within nine months it self-hosts."), None, &root)
                .expect("second ingest");
        let new = after.content_claim("H-0001").unwrap();

        assert!(after.drifted.is_empty(), "an open prediction is not history");
        assert_ne!(new, old);
        assert_eq!(ledger.record(new).unwrap().envelope().supersedes(), Some(old));
        assert_eq!(
            ledger.state_of(new),
            Some(RecordState::Hypothesis(HypothesisState::Registered))
        );
        assert_eq!(
            ledger.state_of(old),
            Some(RecordState::Hypothesis(HypothesisState::Abandoned)),
            "and abandoned is not the same as scored Falsified — the project stopped \
             making the prediction, it did not find it false"
        );
        assert_eq!(withdraw_reason(ledger.history(old)[0]), Some(WithdrawReason::Superseded));
    }

    /// The case that raised U-30: rewording a `registered` hypothesis leaves two
    /// proposed title claims, because the verdict that retires a predecessor
    /// can only retire one that reached promoted, and neither of these did.
    #[test]
    fn rewording_leaves_one_wording_in_the_inbox_not_two() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        ingest_text(&mut ledger, &hypothesis_doc("Within six months it self-hosts."), None, &root)
            .expect("first ingest");
        let before = ledger.pending_proposals().queued.len();

        ingest_text(&mut ledger, &hypothesis_doc("Within nine months it self-hosts."), None, &root)
            .expect("second ingest");
        let after = ledger.pending_proposals();

        assert_eq!(after.queued.len(), before, "a reviewer reads one wording, not two");
        assert!(!after.superseded.is_empty(), "and the one it replaced is still in the record");
        // Nothing was closed, because nothing was decided: the predecessor is
        // still proposed, and only a person can say otherwise.
        for record in &after.superseded {
            assert_eq!(
                ledger.state_of(record.id()),
                Some(RecordState::Claim(ClaimState::Proposed))
            );
        }
    }

    #[test]
    fn a_reworded_open_register_row_supersedes_the_wording_it_replaces() {
        let root = repo_root();
        let decisions = decisions_doc(&[("D-0001", "Four forces.")]);
        let mut ledger = Ledger::new();

        let before = ingest_text(
            &mut ledger,
            &decisions,
            Some(&register_doc("deferred, not rejected")),
            &root,
        )
        .expect("first ingest");
        let old = before.gap("U-1").unwrap();

        let after = ingest_text(
            &mut ledger,
            &decisions,
            Some(&register_doc("deferred until real agent usage exists")),
            &root,
        )
        .expect("second ingest");
        let new = after.gap("U-1").unwrap();

        assert!(after.drifted.is_empty());
        assert_ne!(new, old);
        assert_eq!(ledger.record(new).unwrap().envelope().supersedes(), Some(old));
        assert_eq!(ledger.state_of(new), Some(RecordState::Gap(GapState::Registered)));
        assert_eq!(ledger.state_of(old), Some(RecordState::Gap(GapState::Withdrawn)));
        // The point of the whole exercise: one live question where the document
        // asks one, and a record that says which kind of withdrawal it was.
        assert_eq!(ledger.registered_gaps().len(), 1);
        assert_eq!(withdraw_reason(ledger.history(old)[0]), Some(WithdrawReason::Superseded));
    }

    #[test]
    fn a_question_reworded_after_it_was_settled_is_left_as_history() {
        let root = repo_root();
        let decisions = decisions_doc(&[("D-0001", "Four forces.")]);
        let resolved = |note: &str| {
            format!(
                "## Room 2 · Known unknowns\n\n\
                 | id | Question | Trigger | Notes |\n\
                 |----|----------|---------|-------|\n\
                 | U-1 | ~~Whether a query language is needed~~ **Resolved 2026-08-23** \
                 → D-0001: the forces settle it | — | {note} |\n\n\
                 *Recorded 2026-08-23. Owner: Greg Villa.*\n"
            )
        };

        let mut ledger = Ledger::new();
        let before = ingest_text(&mut ledger, &decisions, Some(&resolved("as first written")), &root)
            .expect("first ingest");
        let gap = before.gap("U-1").unwrap();
        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Answered)));

        let after = ingest_text(&mut ledger, &decisions, Some(&resolved("tidied later")), &root)
            .expect("second ingest");
        assert_eq!(after.drifted, vec!["U-1".to_string()]);
        assert_eq!(after.appended(), 0);
        assert_eq!(after.gap("U-1"), Some(gap), "history is never rewritten");
    }

    #[test]
    fn a_resolved_row_naming_no_record_is_withdrawn_as_answered_elsewhere() {
        let root = repo_root();
        let register = "## Room 2 · Known unknowns\n\n\
             | id | Question | Trigger | Notes |\n\
             |----|----------|---------|-------|\n\
             | U-1 | ~~Whether a query language is needed~~ **Resolved 2026-08-23** \
             in conversation | — | nothing here states the answer |\n\n\
             *Recorded 2026-08-23. Owner: Greg Villa.*\n";
        let mut ledger = Ledger::new();
        let report = ingest_text(&mut ledger, &decisions_doc(&[("D-0001", "Four forces.")]), Some(register), &root)
            .expect("ingest");
        let gap = report.gap("U-1").unwrap();

        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Withdrawn)));
        // Not "we stopped asking": the row says it was resolved and names
        // nothing that resolved it, which is an answer this ledger does not
        // hold — the reason exists so a keeper can go looking for it.
        assert_eq!(
            withdraw_reason(ledger.history(gap)[0]),
            Some(WithdrawReason::AnsweredElsewhere)
        );
    }

    #[test]
    fn a_retired_claim_is_reported_not_resurrected() {
        let root = repo_root();
        let doc = decisions_doc(&[("D-0001", "Four forces.")]);
        let mut ledger = Ledger::new();
        let before = ingest_text(&mut ledger, &doc, None, &root).expect("first ingest");
        let claim = before.content_claim("D-0001").unwrap();

        // A person retires it in the ledger while the document still says
        // `state: promoted` — the two disagree, which is drift and not an error.
        ledger
            .append(Draft::new(
                Author::human("Greg Villa"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Retire {
                        target: claim,
                        reason: tacit_core::RetireReason::NoLongerTrue,
                    },
                    rationale: None,
                }),
            ))
            .expect("retire");

        let after = ingest_text(&mut ledger, &doc, None, &root).expect("second ingest");
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Retired)));
        assert!(
            after.refused.iter().any(|(id, state)| id == "D-0001" && state.contains("Retired")),
            "the sync reports the disagreement rather than re-promoting: {:?}",
            after.refused
        );
        assert!(!after.quiet());
    }

    #[test]
    fn a_store_this_sync_cannot_read_says_so_instead_of_duplicating_quietly() {
        let root = repo_root();
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity(DECISION_KIND, "D-0001").unwrap();
        // A record with provenance from somewhere else entirely — the shape a
        // store written before D-0021 has, and the shape an agent's own
        // proposals have.
        ledger
            .append(Draft::new(
                Author::agent("some-agent"),
                SourceRef {
                    channel: "conversation".into(),
                    reference: Some("docs/DECISIONS.md D-0001 recorded:2026-08-22".into()),
                },
                Content::Claim(ClaimContent::Text { body: "four forces".into(), about: vec![subject] }),
            ))
            .expect("append");

        let report = ingest_text(&mut ledger, &decisions_doc(&[("D-0001", "Four forces.")]), None, &root)
            .expect("ingest");
        assert!(report.unreadable_provenance);
        assert!(!report.quiet(), "a silent duplication is the failure being reported");

        // And a fresh ledger, which is the ordinary case, says nothing.
        let mut clean = Ledger::new();
        let ordinary =
            ingest_text(&mut clean, &decisions_doc(&[("D-0001", "Four forces.")]), None, &root)
                .expect("ingest");
        assert!(!ordinary.unreadable_provenance);
    }

    /// D-0057: a document fault is found before the first durable write. The
    /// state check lives in the third phase, after entities and claims are
    /// appended, which is exactly the fault that used to leave a partial
    /// store behind a refusal.
    #[test]
    fn a_refused_document_writes_nothing_durable() {
        let root = repo_root();
        let path = std::env::temp_dir().join(format!("tacit-atomic-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let good = decisions_doc(&[("D-0001", "First."), ("D-0002", "Second.")]);
        assert!(good.contains("state: promoted"), "the fixture writes promoted records");
        // The second record carries the fault, so a pass that appends as it
        // goes would have written D-0001 before refusing.
        let bad = good.replace("id: D-0002\nstate: promoted", "id: D-0002\nstate: proposed");
        assert_ne!(good, bad, "the fault was planted");

        {
            let mut ledger = Ledger::open(&path).expect("open").ledger;
            let err = ingest_text(&mut ledger, &bad, None, &root).expect_err("refused");
            assert!(matches!(err, IngestError::UnsupportedState { .. }), "{err}");
            assert!(ledger.log().is_empty(), "nothing appended in memory either");
        }
        let reopened = Ledger::open(&path).expect("reopen");
        assert_eq!(reopened.recovery.events_replayed, 0, "nothing reached the disk");

        // The same document, corrected, then ingests in full — the rehearsal
        // is a gate, not a tax on the corpus.
        let mut ledger = reopened.ledger;
        let report = ingest_text(&mut ledger, &good, None, &root).expect("ingests");
        assert_eq!(ledger.log().len(), report.appended());
        assert!(report.appended() > 0);

        let _ = std::fs::remove_file(&path);
    }

    /// The case U-19 was actually about: the store outlives the process, so the
    /// next run opens a ledger that already holds the corpus. This is what the
    /// MCP host does on every restart.
    #[test]
    fn a_durable_store_syncs_the_document_instead_of_duplicating_it() {
        let root = repo_root();
        let path = std::env::temp_dir().join(format!("tacit-sync-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let written = {
            let mut ledger = Ledger::open(&path).expect("open").ledger;
            ingest_corpus(&mut ledger, &root).expect("first ingest").appended()
        };

        let mut ledger = Ledger::open(&path).expect("reopen").ledger;
        assert_eq!(ledger.log().len(), written, "the whole corpus replayed");
        let report = ingest_corpus(&mut ledger, &root).expect("second ingest");
        assert_eq!(report.appended(), 0, "a restart is not a second corpus");
        assert_eq!(ledger.log().len(), written);
        assert_eq!(report.count(Disposition::Unchanged), report.dispositions.len());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_whole_corpus_ingests() {
        let (ledger, report) = ingested();
        let records = parsed();
        assert!(records.len() >= 16, "the parser found the corpus, not a fragment");

        assert_eq!(report.decisions.len(), records.len(), "one anchor per record");
        assert_eq!(report.content_claims.len(), records.len());
        assert_eq!(report.title_claims.len(), records.len());

        // One verdict per promoted record, covering its content claim and its
        // title together. It used to be two — the keeper split the record and
        // then charged the split back to the author as a second declaration,
        // which is the transcription cost U-20 recorded and D-0034 removed.
        let promoted = records.iter().filter(|r| r.yaml["state"] == "promoted").count();
        assert_eq!(report.verdicts.len(), promoted);

        // Exactly one hypothesis, and it is the registered one.
        let hypotheses = records.iter().filter(|r| r.id.starts_with('H')).count();
        assert_eq!(hypotheses, records.len() - promoted);

        assert_eq!(ledger.log().len(), report.appended(), "nothing appended off-report");
    }

    #[test]
    fn evidence_resolves_to_real_files_only() {
        let (ledger, report) = ingested();
        let expected: usize = parsed()
            .iter()
            .filter_map(|r| r.yaml.get("evidence"))
            .map(|list| crate::parse::split_evidence(list).len())
            .sum();
        assert_eq!(report.evidence_links, expected);
        assert!(report.evidence_links >= 10);

        for (label, id) in &report.sources {
            assert!(!label.starts_with('/'), "labels stay repo-relative: {label}");
            assert!(!label.contains(".."), "labels never escape the repo: {label}");
            assert!(repo_root().join(label).exists(), "{label} exists on disk");
            assert_eq!(ledger.entity(*id).unwrap().kind(), tacit_core::SOURCE_KIND);
        }
        // The corpus's self-reference resolves to the corpus.
        assert!(report.sources.iter().any(|(l, _)| l == "docs/DECISIONS.md"));
        // Sources are deduplicated: several records cite the same design doc.
        let unique: std::collections::BTreeSet<_> =
            report.sources.iter().map(|(l, _)| l.clone()).collect();
        assert_eq!(unique.len(), report.sources.len());
        assert!(report.sources.len() < report.evidence_links, "dedup actually happened");
    }

    #[test]
    fn promoted_records_reach_promoted_only_through_verdicts() {
        let (ledger, report) = ingested();
        let d1 = report.content_claim("D-0001").unwrap();
        assert_eq!(ledger.state_of(d1), Some(RecordState::Claim(ClaimState::Promoted)));
        assert_eq!(ledger.history(d1).len(), 1);
        let verdict = ledger.history(d1)[0];
        assert_eq!(verdict.envelope().author().kind, tacit_core::AuthorKind::Human);
    }

    #[test]
    fn the_hypothesis_is_registered_not_promoted() {
        let (ledger, report) = ingested();
        let h = report.content_claim("H-0001").unwrap();
        assert_eq!(
            ledger.state_of(h),
            Some(RecordState::Hypothesis(tacit_core::HypothesisState::Registered))
        );
        assert!(ledger.history(h).is_empty());
        let Content::Hypothesis(content) = ledger.record(h).unwrap().content() else {
            panic!("H-0001 is a hypothesis");
        };
        assert!(content.falsifier.is_some());
        assert_eq!(content.score_by.strftime("%Y-%m-%d").to_string(), "2027-02-23");
    }

    /// The content-shape rule is mechanical: a record becomes a Pattern
    /// exactly when it carries a Forces section.
    #[test]
    fn forces_bearing_records_become_patterns() {
        let (ledger, report) = ingested();
        let expected: Vec<String> = parsed()
            .iter()
            .filter(|r| r.section("Forces").is_some())
            .map(|r| r.id.clone())
            .collect();
        assert!(expected.len() >= 5, "the corpus still has forces-bearing records");

        let mut patterns = Vec::new();
        for (id, claim) in &report.content_claims {
            if let Content::Claim(ClaimContent::Pattern { context, forces, about, .. }) =
                ledger.record(*claim).unwrap().content()
            {
                patterns.push(id.clone());
                assert!(!context.is_empty(), "{id} pattern has a context");
                assert!(!forces.is_empty(), "{id} pattern has forces");
                assert_eq!(about.len(), 1, "{id} is about its own anchor");
            }
        }
        assert_eq!(patterns, expected);
    }

    /// The machine's reading of cross-references is proposed, never promoted:
    /// the default graph has no edges until a human ratifies them.
    #[test]
    fn mention_edges_stay_proposed() {
        let (ledger, report) = ingested();
        assert!(!report.mention_claims.is_empty());
        for (_, _, id) in &report.mention_claims {
            assert_eq!(ledger.state_of(*id), Some(RecordState::Claim(ClaimState::Proposed)));
        }

        let projection = Projection::rebuild(&ledger);
        assert!(projection.view(&ledger, ViewSpec::now()).edges().is_empty());
        let proposed = projection
            .view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
        assert_eq!(proposed.edges().len(), report.mention_claims.len());
    }

    #[test]
    fn the_projected_graph_carries_titles_and_prose() {
        let (ledger, report) = ingested();
        let projection = Projection::rebuild(&ledger);
        let view = projection.view(&ledger, ViewSpec::now());

        let d12 = report.decision("D-0012").unwrap();
        let node = view.node(d12).unwrap();
        let title = node.property("title").unwrap();
        assert_eq!(
            title.single().unwrap().value(),
            &tacit_core::Value::Text("Write-path: grammar in the engine, truth in the keeper".into())
        );
        assert_eq!(node.about().len(), 1, "its own promoted content claim");

        // H-0001's anchor is dark in the default view: nothing about it is
        // promoted, because a hypothesis is scored rather than promoted.
        let h = report.decision("H-0001").unwrap();
        assert!(view.node(h).unwrap().property("title").is_none());
        assert!(view.node(h).unwrap().about().is_empty());
    }

    /// Nothing is silently dropped: every prose section of every record
    /// survives into the stored content, and every yaml key was understood.
    #[test]
    fn ingest_is_lossless() {
        let (ledger, report) = ingested();
        for record in parsed() {
            let claim = report.content_claim(&record.id).expect("ingested");
            let content = ledger.record(claim).unwrap().content();

            // The Forces section is deliberately decomposed rather than
            // copied, so the property there is that every force is a slice of
            // the original paragraph — the machine may split, never invent.
            if let Content::Claim(ClaimContent::Pattern { forces, .. }) = content {
                let original = record.section("Forces").expect("pattern implies Forces");
                for force in forces {
                    assert!(
                        original.contains(force.as_str()),
                        "{}: force {force:?} is not a slice of its own paragraph",
                        record.id
                    );
                }
                let recovered: usize = forces.iter().map(|f| f.chars().count()).sum();
                assert!(
                    recovered * 100 / original.chars().count().max(1) > 90,
                    "{}: the split dropped more than a tenth of the forces text",
                    record.id
                );
            }

            let carried = match content {
                Content::Claim(ClaimContent::Pattern { solution, .. }) => solution.clone(),
                Content::Claim(ClaimContent::Text { body, .. }) => body.clone(),
                Content::Hypothesis(h) => {
                    format!("{} {}", h.statement, h.falsifier.clone().unwrap_or_default())
                }
                other => panic!("{} ingested as {other:?}", record.id),
            };
            for (label, body) in &record.sections {
                if label == "Forces" {
                    continue;
                }
                // A distinctive slice, not exact equality: assembly reflows
                // the document's hard-wrapped lines.
                let probe: String = body.chars().take(48).collect();
                assert!(
                    carried.contains(probe.trim()),
                    "{} lost its {label} section",
                    record.id
                );
            }
        }
    }

    fn ingested_corpus() -> (Ledger, IngestReport) {
        let mut ledger = Ledger::new();
        let report = ingest_corpus(&mut ledger, &repo_root()).expect("both documents ingest");
        (ledger, report)
    }

    fn register_rows() -> Vec<crate::register::ParsedUnknown> {
        let text = std::fs::read_to_string(repo_root().join("docs/REGISTER.md")).unwrap();
        crate::register::parse_register(&text).unwrap()
    }

    /// The whole point: the register's open questions become retrievable gaps,
    /// so the engine can say "that is a registered open question" instead of
    /// "nothing found".
    #[test]
    fn open_unknowns_become_registered_gaps() {
        let (ledger, report) = ingested_corpus();
        let rows = register_rows();
        assert!(rows.len() >= 20, "the register was found, not a fragment");
        assert_eq!(report.gaps.len(), rows.len());

        let open = rows.iter().filter(|u| u.resolved.is_none()).count();
        assert!(open > 0);
        assert_eq!(ledger.registered_gaps().len(), open);

        // A decisions-only ingest has none of this — the contrast is the point.
        let mut bare = Ledger::new();
        ingest_decisions(&mut bare, &repo_root()).unwrap();
        assert!(bare.registered_gaps().is_empty());
    }

    /// Resolved unknowns are answered by the very claims that settled them,
    /// and the engine refuses unless those claims are genuinely promoted.
    #[test]
    fn resolved_unknowns_are_answered_by_their_deciding_claims() {
        let (ledger, report) = ingested_corpus();
        let resolved: Vec<_> =
            register_rows().into_iter().filter(|u| u.resolved.is_some()).collect();
        assert!(resolved.len() >= 3, "U-1, U-2 and U-10 are resolved");

        for row in &resolved {
            let gap = report.gap(&row.id).expect("gap exists");
            let state = ledger.state_of(gap).expect("gap has state");
            assert_ne!(
                state,
                RecordState::Gap(tacit_core::GapState::Registered),
                "{} is resolved in the register",
                row.id
            );
        }

        // Each answer names a promoted decision claim.
        for (unknown, decision) in &report.answered {
            let claim = report.content_claim(decision).expect("decision ingested");
            assert_eq!(
                ledger.state_of(claim),
                Some(RecordState::Claim(ClaimState::Promoted)),
                "{unknown} is answered by {decision}, which must be promoted"
            );
        }
        assert!(report.answered.iter().any(|(u, d)| u == "U-1" && d == "D-0012"));
        assert!(report.answered.iter().any(|(u, d)| u == "U-2" && d == "D-0015"));
    }

    #[test]
    fn gaps_carry_their_triggers_and_territory() {
        let (ledger, report) = ingested_corpus();
        for row in register_rows().iter().filter(|u| u.resolved.is_none()) {
            let gap = report.gap(&row.id).expect("gap exists");
            let record = ledger.record(gap).expect("record");
            assert!(
                record.envelope().review_trigger().is_some(),
                "{} carries the trigger that forces it",
                row.id
            );
            let Content::Gap(content) = record.content() else { panic!("{} is a gap", row.id) };
            assert!(
                content.territory.contains(&report.unknown(&row.id).unwrap()),
                "{} covers its own anchor",
                row.id
            );
        }
    }

    /// Cross-references now resolve in both directions: a decision naming U-1
    /// and an unknown naming D-0012 both find an anchor.
    #[test]
    fn decisions_and_unknowns_cross_link() {
        let (ledger, report) = ingested_corpus();
        let projection = Projection::rebuild(&ledger);
        let view = projection
            .view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));

        let u1 = report.unknown("U-1").expect("U-1 anchor");
        let inbound: Vec<_> = view
            .node(u1)
            .unwrap()
            .in_edges()
            .iter()
            .map(|e| ledger.entity(e.subject()).unwrap().label().to_string())
            .collect();
        assert!(
            inbound.iter().any(|l| l == "D-0006"),
            "D-0006 names U-1 in its assertion; got {inbound:?}"
        );

        // The register's gap for U-1 reaches D-0012 through its territory.
        let gap = report.gap("U-1").expect("gap");
        let Content::Gap(content) = ledger.record(gap).unwrap().content() else { panic!() };
        assert!(content.territory.contains(&report.decision("D-0012").unwrap()));
    }

    #[test]
    fn the_corpus_does_not_contradict_itself() {
        let (ledger, _) = ingested();
        assert!(ledger.contradictions().is_empty());
        assert!(Projection::rebuild(&ledger).view(&ledger, ViewSpec::now()).conflicts().is_empty());
    }

    #[test]
    fn every_promoted_content_claim_carries_a_review_trigger() {
        let (ledger, report) = ingested();
        for (id, claim) in &report.content_claims {
            let record = ledger.record(*claim).unwrap();
            assert!(
                record.envelope().review_trigger().is_some(),
                "{id} carries a review trigger"
            );
        }

        let queue = ledger.review_queue(jiff::Timestamp::now());
        // The only promoted claims without a trigger are the title
        // transcriptions, which carry no editorial commitment of their own.
        let promoted_titles = report
            .title_claims
            .iter()
            .filter(|(_, r)| {
                ledger.state_of(*r) == Some(RecordState::Claim(ClaimState::Promoted))
            })
            .count();
        assert_eq!(queue.missing_trigger.len(), promoted_titles);
        assert!(queue.due.is_empty(), "every trigger is an event, not a date");
    }
}

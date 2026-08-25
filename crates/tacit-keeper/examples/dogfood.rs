//! Ingest the project's own decision records and interrogate the result.
//!
//! `cargo run -p tacit-keeper --example dogfood`

use std::collections::BTreeMap;
use std::path::PathBuf;
use tacit_core::{
    Author, ClaimContent, Content, CostSpec, CostTransform, Embedder, HashingEmbedder, Ledger,
    MeasurementTarget, MissingCost, Projection, Query, RecordState, StateFilter, TextIndex,
    VectorIndex, Via, ViewSpec,
};
use tacit_keeper::corpus::{DECISION_KIND, ingest_corpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ledger = Ledger::new();

    let before_ingest = jiff::Timestamp::now();
    let report = ingest_corpus(&mut ledger, &repo_root)?;

    rule("INGEST");
    println!(
        "  {} corpus records -> {} ledger records, {} entities",
        report.decisions.len(),
        ledger.log().len(),
        ledger.entities().count()
    );
    println!(
        "    content claims {:>3}   title claims {:>3}   mention edges {:>3}",
        report.content_claims.len(),
        report.title_claims.len(),
        report.mention_claims.len()
    );
    println!(
        "    register gaps  {:>3}   verdicts     {:>3}",
        report.gaps.len(),
        report.verdicts.len()
    );
    println!(
        "  {} evidence links across {} source files",
        report.evidence_links,
        report.sources.len()
    );

    rule("LEDGER STATE");
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    for record in ledger.records() {
        let state = match ledger.state_of(record.id()) {
            Some(RecordState::Verdict) => "verdict".to_string(),
            Some(state) => state.to_string(),
            None => "unknown".to_string(),
        };
        *by_state.entry(state).or_default() += 1;
    }
    for (state, count) in &by_state {
        println!("  {count:>3}  {state}");
    }
    println!(
        "  pending proposals {}   registered gaps {}   contradictions {}",
        ledger.pending_proposals().queued.len(),
        ledger.registered_gaps().len(),
        ledger.contradictions().len()
    );

    rule("PROVENANCE — \"why did Tacit choose embedded-first?\"");
    let d15 = report.content_claim("D-0015").expect("D-0015 ingested");
    let record = ledger.record(d15).expect("record exists");
    let envelope = record.envelope();
    println!("  state      {}", ledger.state_of(d15).expect("state"));
    println!("  author     {} ({:?})", envelope.author().name, envelope.author().kind);
    println!("  channel    {}", envelope.source().channel);
    println!("  valid from {}", envelope.valid_from().strftime("%Y-%m-%d"));
    println!("  trigger    {}", trigger_of(record));
    println!("  evidence:");
    for evidence in envelope.evidence() {
        let source = ledger.entity(evidence.source).expect("source entity");
        match &evidence.span {
            Some(span) => println!("    - {} ({})", source.label(), span),
            None => println!("    - {}", source.label()),
        }
    }
    for verdict in ledger.history(d15) {
        let Content::Verdict(v) = verdict.content() else { continue };
        println!(
            "  promoted by {} — {}",
            verdict.envelope().author().name,
            v.rationale.as_deref().unwrap_or("no rationale")
        );
    }
    if let Content::Claim(ClaimContent::Pattern { forces, .. }) = record.content() {
        println!("  forces the machine proposed and the transcribed verdict ratified:");
        for force in forces {
            println!("    · {}", truncate(force, 88));
        }
    }

    rule("RETRIEVAL — one plan: filter, rank, expand, abstain");
    let index = TextIndex::rebuild(&ledger);
    let projection_for_search = Projection::rebuild(&ledger);
    let embedder = HashingEmbedder::default();
    let vectors = VectorIndex::rebuild(&ledger, &embedder);
    let retriever = index
        .retriever(&ledger, &projection_for_search, ViewSpec::now())
        .with_vectors(&vectors, &embedder);
    println!(
        "  {} indexed records (verdicts contribute nothing), {} vectors from {}",
        index.documents(),
        vectors.len(),
        embedder.model_id()
    );

    for question in [
        "why is the runtime embedded rather than a server",
        "what storage engine does Tacit use",
        "how does the vector index handle sharding across regions",
    ] {
        let found = retriever.retrieve(&Query::text(question));
        println!("\n  ? {question}");
        println!("    tags: {}", found.tags().join(" + "));
        for item in found.items.iter().take(2) {
            let label = anchor_label(&ledger, item.record);
            let via = match &item.via {
                Via::Lexical => "lexical".to_string(),
                Via::Vector => "vector".to_string(),
                Via::Hybrid => "lexical+vector".to_string(),
                Via::Expanded { path, .. } => format!("expanded {} hop(s)", path.len()),
            };
            println!("    -> {label} (relevance {:.2}, {via})", item.relevance);
        }
        for gap in found.gaps.iter().take(2) {
            let Content::Gap(content) = gap.content() else { continue };
            let question = content.question.split("\n\n").next().unwrap_or_default();
            println!("    open question: {}", truncate(question, 78));
        }
        if found.is_abstention() && found.items.is_empty() && found.gaps.is_empty() {
            println!("    (the record has nothing, and says so)");
        }
    }
    println!(
        "\n  What to read here is the tags, not the ranking. `matches` means the record\n  \
         covers the question; `weak_matches` means the best hit did not, and\n  \
         `is_abstention()` reports it. `registered_gap` means the engine also raised\n  \
         an open question whose territory the query meets — an honest \"nobody has\n  \
         decided yet\", with a citation, beside whatever else was found.\n\n  \
         Two rankers feed one plan now: `lexical+vector` means both found it. The\n  \
         vector half buys robustness to spelling and morphology, not meaning — its\n  \
         similarity was measured to overlap between answerable and unanswerable\n  \
         questions, so it is allowed to raise a question and never to assert an\n  \
         answer (D-0020). The golden suite is what says whether any of this helps:\n  \
         cargo run -p tacit-keeper --example golden."
    );

    rule("RECORD-TIME TRAVEL — bitemporality over the real corpus");
    println!(
        "  D-0015 at t0 (before ingest)        {}",
        describe(ledger.state_of_at(d15, before_ingest))
    );
    println!("  D-0015 now                          {}", describe(ledger.state_of(d15)));
    println!(
        "  valid-from in the document          {}",
        envelope.valid_from().strftime("%Y-%m-%d")
    );
    println!(
        "  record-time in this ledger          {}",
        envelope.recorded_at().strftime("%Y-%m-%d %H:%M:%SZ")
    );
    println!("  (nothing was backdated: record-time is when this ledger learned it)");

    rule("THE PROJECTED GRAPH");
    let projection = Projection::rebuild(&ledger);
    let default = projection.view(&ledger, ViewSpec::now());
    let with_proposed =
        projection.view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
    println!(
        "  default view (promoted only)   {} nodes, {} edges",
        default.nodes().len(),
        default.edges().len()
    );
    println!(
        "  include-proposed view          {} nodes, {} edges",
        with_proposed.nodes().len(),
        with_proposed.edges().len()
    );
    println!(
        "  the ratchet is visible in the graph: the machine's reading of the corpus's\n  \
         cross-references is proposed, so no edge enters the default graph until a\n  \
         human ratifies it."
    );

    let d12 = report.decision("D-0012").expect("anchor");
    let node = with_proposed.node(d12).expect("node");
    println!("\n  node {} — {}", node.label(), title_of(&node));
    println!(
        "    mentions {} records, mentioned by {}",
        node.out_edges().len(),
        node.in_edges().len()
    );
    for edge in node.in_edges().iter().take(4) {
        let source = ledger.entity(edge.subject()).expect("entity");
        println!("      <- {} [{:?}]", source.label(), edge.state());
    }

    rule("THE INSTRUMENT PANEL — costs that move without a verdict");
    // A real corpus statistic, not invented data: how many records mention the
    // edge's target. Written to the panel, so it is machine-owned and mutable
    // in place — no verdict, no ceremony (D-0013).
    let mut inbound: BTreeMap<String, f64> = BTreeMap::new();
    for (_, target, _) in &report.mention_claims {
        *inbound.entry(target.clone()).or_default() += 1.0;
    }
    let updater = Author::agent("corpus-stats");
    for (_, target, edge) in &report.mention_claims {
        ledger.record_measurement(
            MeasurementTarget::Relation(*edge),
            "inbound_mentions",
            inbound.get(target).copied().unwrap_or(0.0),
            updater.clone(),
            jiff::Timestamp::now(),
        )?;
    }
    println!("  wrote {} measurements; the governed ledger is unchanged", report.mention_claims.len());
    println!("  log length still {} records", ledger.log().len());

    println!(
        "\n  the corpus's reference graph: {} proposed edges, a sample:",
        report.mention_claims.len()
    );
    for (from, to, _) in report.mention_claims.iter().take(6) {
        println!("    {from} -> {to}");
    }

    let cost = CostSpec {
        measurement: "inbound_mentions".into(),
        transform: CostTransform::Identity,
        missing: MissingCost::Exclude,
    };
    let from = report.decision("D-0010").expect("anchor");
    let to = report.decision("D-0002").expect("anchor");
    let view = projection.view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
    println!("\n  path D-0010 -> D-0002 (a genuine branch: direct, or via D-0003)");
    println!(
        "    fewest hops      {}",
        render_path(&view.shortest_path(from, to, &CostSpec::hops())?, &ledger)
    );
    println!(
        "    by measurement   {}",
        render_path(&view.shortest_path(from, to, &cost)?, &ledger)
    );

    // A what-if, labelled as one: an observer decides the direct route is
    // costly. No claim is proposed, no verdict is rendered, nothing is
    // appended — and the answer moves.
    let direct = report
        .mention_claims
        .iter()
        .find(|(f, t, _)| f == "D-0010" && t == "D-0002")
        .map(|(_, _, id)| *id)
        .expect("direct edge");
    let log_before = ledger.log().len();
    ledger.record_measurement(
        MeasurementTarget::Relation(direct),
        "inbound_mentions",
        10.0,
        updater,
        jiff::Timestamp::now(),
    )?;
    let view = projection.view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
    println!(
        "    after an observer raises the direct edge's cost to 10:\n                     {}",
        render_path(&view.shortest_path(from, to, &cost)?, &ledger)
    );
    println!(
        "  the graph learned: log length {} -> {}, zero claims proposed, zero verdicts.\n  \
         That is why weights live on the panel and not in the governed ledger.",
        log_before,
        ledger.log().len()
    );

    rule("THE BOUNDARY OF THE RECORD — what Tacit knows it does not know");
    let gaps = ledger.registered_gaps();
    println!("  {} registered gaps, from the register's Room 2", gaps.len());
    println!(
        "  {} resolved unknowns were answered by the very claims that settled them:",
        report.answered.len()
    );
    for (unknown, decision) in &report.answered {
        let gap = report.gap(unknown).expect("gap");
        println!(
            "    {unknown} -> {} (answered by {decision})",
            ledger.state_of(gap).expect("state")
        );
    }
    println!("\n  a sample of what remains open:");
    for gap in gaps.iter().take(3) {
        let Content::Gap(content) = gap.content() else { continue };
        let question = content.question.split("\n\n").next().unwrap_or_default();
        println!("    · {}", truncate(question, 86));
        if let Some(trigger) = gap.envelope().review_trigger().and_then(|t| t.on_event.clone()) {
            println!("      trigger: {}", truncate(&trigger, 78));
        }
    }
    println!(
        "\n  this is what abstention is made of: an agent asked about storage can say\n  \
         \"that is registered unknown U-5, triggered by the implementation phase\"\n  \
         rather than inventing an answer or returning nothing."
    );

    rule("WHAT BACKS EACH PROMOTION");
    let promoted = ledger.promoted_claims().count();
    let resting_on_nothing = tacit_keeper::attest::unattested_promotions(&ledger);
    println!("  Every promoted claim here reached promoted through a verdict this ingest");
    println!("  transcribed from `state: promoted` in a markdown file. So the honest");
    println!("  question is not whether a person declared it — the engine enforces that —");
    println!("  but how the keeper knows a person wrote the file (D-0025), and whose");
    println!("  signature counts when one did (D-0026).");
    println!();
    println!("  promoted claims                        {promoted}");
    println!("  resting on a verdict backed by nothing {}", resting_on_nothing.len());
    for (id, why) in resting_on_nothing.iter().take(3) {
        println!("    · {id} — {why}");
    }
    if let Some(sample) = ledger
        .promoted_claims()
        .find_map(|c| ledger.history(c.id()).first().copied())
        .and_then(|v| v.envelope().author().detail.clone())
    {
        println!("  a verdict's own account of itself:");
        println!("    {sample}");
    }
    let review = tacit_keeper::review_trust(&ledger, &repo_root);
    println!();
    println!("  And re-asked today, because a key can stop being trusted after it signs:");
    println!(
        "    {} verify as they did · {} weakened · {} strengthened · {} unverifiable · {} \
         nothing to re-ask",
        review.unchanged,
        review.weakened.len(),
        review.strengthened.len(),
        review.unverifiable.len(),
        review.nothing_to_recheck.len()
    );
    println!("  A weakening changes nothing in the record. Something happened in the world,");
    println!("  not in the ledger, and retiring what a revoked key signed is a person's");
    println!("  verdict to declare — so this is an alarm and never a write (D-0027).");
    println!();
    println!("  Run the host with --require-signature, or --signed-by NAME to say whose");
    println!("  signature carries a verdict, and a promotion that does not meet it is not");
    println!("  transcribed at all: the claim stays proposed, waiting for a person, which");
    println!("  is where an unbacked promotion always belonged.");
    println!();

    rule("KEEPER WORK QUEUE");
    let queue = ledger.review_queue(jiff::Timestamp::now());
    println!("  due for review        {}", queue.due.len());
    println!("  promoted, no trigger  {}", queue.missing_trigger.len());
    let pending = ledger.pending_proposals();
    println!("  awaiting a verdict    {}", pending.queued.len());
    if !pending.superseded.is_empty() {
        println!(
            "  replaced before read  {}  (still proposed — an author editing an unreviewed\n  \
             draft is not a verdict — but not queued twice)",
            pending.superseded.len()
        );
    }
    println!(
        "  ({} title transcriptions carry no trigger of their own; {} mention edges\n  \
         await ratification.)",
        queue.missing_trigger.len(),
        report.mention_claims.len()
    );

    rule("H-0001(a) — HONEST POSITION");
    println!("  claim: \"Tacit self-hosts its own decision corpus with envelopes and");
    println!("         lifecycle enforced by the engine.\"");
    println!("  met so far:");
    println!("    · every record carries a complete envelope — the engine rejects any that does not");
    println!("    · promoted state is reachable only through a human-authored verdict");
    println!("    · provenance, bitemporal reads, and the projected graph all answer over the corpus");
    println!("    · the record now survives the process (D-0019): run the MCP host with");
    println!("      --store <path> and the ledger is replayed from an append-only log,");
    println!("      re-validated through the same grammar an append runs");
    println!("    · and it stays current (D-0021): re-reading the documents is a sync, so an");
    println!("      edited record supersedes and retires its predecessor in one verdict,");
    println!("      while an unchanged one writes nothing at all");
    println!("  NOT yet met:");
    println!("    · this example still uses a scratch in-memory ledger — the durable path");
    println!("      is the host's, and this demo exercises the grammar rather than the store");
    println!("    · docs/DECISIONS.md stays the copy a person edits. That is now a decision");
    println!("      rather than a gap, and it has a cost worth naming: write access to that");
    println!("      file is promotion authority, since the ingest transcribes `state:");
    println!("      promoted` as a person\'s verdict and cannot know who typed it. There is");
    println!("      no promote tool on the MCP surface; this is the side door (U-29)");
    println!();
    println!("  H-0001(b) — \"MCP tools let an agent answer why Tacit chose X with");
    println!("  provenance, and honestly abstain on registered unknowns\":");
    println!("    · the capability exists and is tested end to end over stdio");
    println!("      (cargo run -p tacit-mcp -- .), with no promote tool on the surface");
    println!("    · how good those answers are is what (c) measures, below");
    println!();
    println!("  H-0001(c) — \"a small golden suite grades it, rewarding abstention at the");
    println!("  record's boundary\": docs/GOLDEN.md, run with");
    println!("    cargo run -p tacit-keeper --example golden");
    println!("  It scores abstention as a pass and names the room each failure came from.");
    println!("  Today: 11/14, four of those passes earned by declining to answer, and");
    println!("  three known shortfalls tracked against U-23 rather than hidden.");
    println!();
    println!("  Scored honestly: (a) durable, re-validated, and current, with the document");
    println!("  deliberately upstream and U-29 the price of that; (b) capability met;");
    println!("  (c) instrument exists and reports honestly.");
    println!("  Retrieval quality (U-23) is the open work, not the grading — and as of");
    println!("  D-0028 it is smaller than it looked: three of the four faults measured");
    println!("  were lexical and are fixed. What is left needs a model that sees meaning.");

    println!();
    Ok(())
}

fn rule(title: &str) {
    println!("\n\x1b[1m{title}\x1b[0m");
    println!("{}", "─".repeat(title.len().max(20)));
}

fn describe(state: Option<RecordState>) -> String {
    match state {
        Some(state) => state.to_string(),
        None => "not yet in the record".to_string(),
    }
}

fn trigger_of(record: &tacit_core::Record) -> String {
    record
        .envelope()
        .review_trigger()
        .and_then(|t| t.on_event.clone())
        .map(|t| truncate(&t, 76))
        .unwrap_or_else(|| "none".to_string())
}

fn title_of(node: &tacit_core::Node<'_>) -> String {
    node.property("title")
        .and_then(|p| p.single().map(|c| c.value().clone()))
        .map(|v| match v {
            tacit_core::Value::Text(t) => t,
            other => format!("{other:?}"),
        })
        .unwrap_or_else(|| "(untitled in this view)".to_string())
}

fn render_path(path: &Option<tacit_core::Path>, ledger: &Ledger) -> String {
    let Some(path) = path else { return "no path".to_string() };
    if path.edges.is_empty() {
        return "same node".to_string();
    }
    let mut hops = Vec::new();
    for edge in &path.edges {
        let record = ledger.record(*edge).expect("edge record");
        if let Content::Claim(ClaimContent::Relation { subject, object, .. }) = record.content() {
            if hops.is_empty() {
                hops.push(label_of(ledger, *subject));
            }
            hops.push(label_of(ledger, *object));
        }
    }
    format!("{}  (cost {:.0})", hops.join(" -> "), path.total_cost)
}

fn label_of(ledger: &Ledger, id: tacit_core::EntityId) -> String {
    ledger
        .entity(id)
        .filter(|e| e.kind() == DECISION_KIND)
        .map(|e| e.label().to_string())
        .unwrap_or_else(|| id.to_string())
}

/// The label of whatever a record is about, for readable output.
fn anchor_label(ledger: &Ledger, record: &tacit_core::Record) -> String {
    let entities = match record.content() {
        Content::Claim(claim) => claim.entity_refs(),
        Content::Gap(gap) => gap.territory.clone(),
        _ => Vec::new(),
    };
    for entity in entities {
        if let Some(e) = ledger.entity(entity)
            && (e.kind() == DECISION_KIND || e.kind() == "unknown")
        {
            return e.label().to_string();
        }
    }
    format!("{:?}", record.kind())
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let head: String = text.chars().take(width - 1).collect();
    format!("{head}…")
}

//! What the engine costs at a size the self-hosting corpus cannot reach.
//!
//! `cargo run --release -p tacit-keeper --example scale -- 2000 [--store PATH]`
//!
//! Every registered cost in this project — dead index slots (U-18), replay on
//! open (U-24), an fsync per append (U-25), an exact vector scan (U-26) — was
//! reasoned about from the shape of the code and never once observed, because
//! the only corpus available was fifty-four records about this project. This
//! runs the same engine over a generated one and reports numbers instead.

use std::path::PathBuf;
use std::time::Instant;
use tacit_core::{
    Embedder, HashingEmbedder, Ledger, Projection, Query, TextIndex, VectorIndex, ViewSpec,
};
use tacit_keeper::synthetic::{Shape, generate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut claims = 2000usize;
    let mut store: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--store" => {
                store = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            other => {
                claims = other.parse().unwrap_or(claims);
                index += 1;
            }
        }
    }

    let shape = Shape::of_size(claims);
    println!(
        "shape: {} topics x {} claims, seed {:#x}{}",
        shape.topics,
        shape.claims_per_topic,
        shape.seed,
        store.as_ref().map(|p| format!(", durable at {}", p.display())).unwrap_or_default()
    );

    let mut ledger = match &store {
        Some(path) => {
            let _ = std::fs::remove_file(path);
            Ledger::open(path)?.ledger
        }
        None => Ledger::new(),
    };

    let started = Instant::now();
    let corpus = generate(&mut ledger, shape)?;
    let built = started.elapsed();

    rule("WHAT WAS BUILT");
    println!("  records            {}", ledger.log().len());
    println!("  promoted claims    {}", corpus.promoted);
    println!("  proposed           {}", corpus.proposed);
    println!("  retired / rejected {} / {}", corpus.retired, corpus.rejected);
    println!(
        "  gaps               {} open, {} answered, {} withdrawn",
        corpus.gaps_open.len(),
        corpus.gaps_answered,
        corpus.gaps_withdrawn
    );
    println!("  contradictions     {} planted", corpus.contradictions.len());
    println!("  supersessions      {}", corpus.supersessions.len());

    rule("WHAT IT COST TO BUILD");
    let per = built.as_secs_f64() * 1e6 / ledger.log().len() as f64;
    println!("  append             {:>8.2?}  ({per:.0} us/record)", built);
    if store.is_some() {
        println!("  ^ one fsync per append (U-25); compare a run without --store");
    }

    rule("DERIVED VIEWS");
    let t = Instant::now();
    let projection = Projection::rebuild(&ledger);
    println!("  projection rebuild {:>8.2?}", t.elapsed());
    let t = Instant::now();
    let index = TextIndex::rebuild(&ledger);
    println!("  text index rebuild {:>8.2?}", t.elapsed());
    let embedder = HashingEmbedder::default();
    let t = Instant::now();
    let vectors = VectorIndex::rebuild(&ledger, &embedder);
    println!("  vector rebuild     {:>8.2?}  ({} vectors)", t.elapsed(), vectors.len());

    rule("RETRIEVAL, AGAINST KNOWN GROUND TRUTH");
    // Each topic's vocabulary appears in that topic and nowhere else, so the
    // right answer is not a matter of judgement.
    let retriever = index.retriever(&ledger, &projection, ViewSpec::now());
    let sample: Vec<_> = corpus.topics.iter().step_by(corpus.topics.len().div_ceil(20)).collect();
    let mut hits = 0usize;
    let t = Instant::now();
    for topic in &sample {
        let found = retriever.retrieve(&Query::text(topic.question()));
        if found
            .items
            .first()
            .is_some_and(|item| topic.promoted.contains(&item.record.id()))
        {
            hits += 1;
        }
    }
    let lexical_each = t.elapsed() / sample.len() as u32;
    println!("  lexical only       {:>8.2?} per query, top hit right {hits}/{}", lexical_each, sample.len());

    let hybrid = retriever.with_vectors(&vectors, &embedder as &dyn Embedder);
    let mut hits = 0usize;
    let t = Instant::now();
    for topic in &sample {
        let found = hybrid.retrieve(&Query::text(topic.question()));
        if found
            .items
            .first()
            .is_some_and(|item| topic.promoted.contains(&item.record.id()))
        {
            hits += 1;
        }
    }
    let hybrid_each = t.elapsed() / sample.len() as u32;
    println!("  with vectors       {:>8.2?} per query, top hit right {hits}/{}", hybrid_each, sample.len());
    println!(
        "  ^ the vector half is an exact scan over every admitted record (U-26): {:.1}x",
        hybrid_each.as_secs_f64() / lexical_each.as_secs_f64().max(f64::MIN_POSITIVE)
    );

    rule("WHERE A HYBRID QUERY'S TIME GOES");
    // The register assumed the per-candidate admission dominated. Assumptions
    // about where time goes are exactly what this corpus exists to replace.
    let view = projection.view(&ledger, ViewSpec::now());
    let ids: Vec<_> = vectors.iter().map(|(id, _)| *id).collect();
    let t = Instant::now();
    let admitted = ids.iter().filter(|id| view.admits_record(**id)).count();
    let admitting = t.elapsed();
    let probe = embedder.embed(&sample[0].question());
    let t = Instant::now();
    let mut sum = 0.0f32;
    for (_, embedded) in vectors.iter() {
        sum += tacit_core::similarity(&probe, &embedded.vector);
    }
    let arithmetic = t.elapsed();
    let q = Query::text(sample[0].question());
    let t = Instant::now();
    let (lex, vec) = hybrid.candidates(&q);
    let candidates = t.elapsed();
    let t = Instant::now();
    let fused = tacit_core::fuse(&[lex.clone(), vec.clone()], &Default::default());
    let fusing = t.elapsed();
    let t = Instant::now();
    let whole = hybrid.retrieve(&q);
    let retrieving = t.elapsed();
    println!("  both candidate lists      {:>8.2?}  ({} lexical, {} vector)", candidates, lex.len(), vec.len());
    println!("  fusing them               {:>8.2?}  ({} fused)", fusing, fused.len());
    println!("  the whole retrieve        {:>8.2?}  ({} returned)", retrieving, whole.items.len());
    println!("  admitting {} candidates {:>8.2?}", ids.len(), admitting);
    println!("  the similarity itself     {:>8.2?}  (checksum {sum:.1})", arithmetic);
    println!(
        "  {admitted} admitted; admission is {:.0}x the arithmetic",
        admitting.as_secs_f64() / arithmetic.as_secs_f64().max(f64::MIN_POSITIVE)
    );

    rule("THE INSTRUMENT PANEL");
    let t = Instant::now();
    let found = ledger.contradictions();
    println!("  contradictions     {:>8.2?}  ({} found, {} planted)", t.elapsed(), found.len(), corpus.contradictions.len());
    let t = Instant::now();
    let pending = ledger.pending_proposals();
    println!("  pending inbox      {:>8.2?}  ({} queued, {} superseded)", t.elapsed(), pending.queued.len(), pending.superseded.len());
    let t = Instant::now();
    let gaps = ledger.registered_gaps();
    println!("  open questions     {:>8.2?}  ({} found, {} left open)", t.elapsed(), gaps.len(), corpus.gaps_open.len());

    if let Some(path) = &store {
        rule("WHAT IT COSTS TO COME BACK (U-24)");
        let bytes = std::fs::metadata(path)?.len();
        drop(ledger);
        let t = Instant::now();
        let opened = Ledger::open(path)?;
        println!(
            "  replay             {:>8.2?}  ({} events, {:.1} MB on disk)",
            t.elapsed(),
            opened.recovery.events_replayed,
            bytes as f64 / 1e6
        );
        println!("  ^ every event re-validated through the grammar, never deserialized");
        let _ = std::fs::remove_file(path);
    }
    println!();
    Ok(())
}

fn rule(title: &str) {
    println!("\n\x1b[1m{title}\x1b[0m");
    println!("{}", "─".repeat(title.len().max(24)));
}

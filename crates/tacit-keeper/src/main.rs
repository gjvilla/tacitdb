//! `tacit-keeper` — the person's side of the ratchet, at the keyboard.
//!
//! The MCP host lets an agent propose and never promote. This binary is where
//! the promoting happens: it opens a store, shows what is waiting, and renders
//! one verdict at a time under a typed name (D-0055). It holds the store's
//! lock while it does, so it cannot write underneath a running host, and the
//! host cannot start underneath it.

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use tacit_core::{ClaimContent, Content, Ledger, RecordId};
use tacit_keeper::{Ruling, StoreLock, render, retire_reason};

const USAGE: &str = "\
tacit-keeper — render a person's verdict on a store

Usage:
  tacit-keeper pending --store <PATH>
  tacit-keeper promote --store <PATH> --as <NAME> --why <TEXT> <RECORD> [--retiring <RECORD>]
  tacit-keeper reject  --store <PATH> --as <NAME> --why <TEXT> <RECORD>
  tacit-keeper retire  --store <PATH> --as <NAME> --why <TEXT> --reason <REASON> <RECORD>
  tacit-keeper -h | --help

Arguments:
  <RECORD>               A record id as the host prints it: rec_01M1P58RMFAD6DC86Q3D6JV6H2

Options:
  --store <PATH>         The ledger the MCP host serves with --store. Required.
  --as <NAME>            Who is ruling. Recorded as the verdict's human author, and
                         recorded as asserted: nothing here verifies a name.
  --why <TEXT>           The rationale. Required — a verdict without a reason is the
                         thing the record exists to prevent.
  --retiring <RECORD>    promote only: a promoted claim this one replaces, retired by
                         the same verdict.
  --reason <REASON>      retire only: superseded | no-longer-true | promoted-in-error

The store is locked for the duration. If the host is serving it, stop the host
first; the verdict is in the log when it next starts.

Exit status: 0 rendered; 1 the ledger refused (its reason is printed); 2 usage.
";

struct Args {
    command: String,
    store: Option<PathBuf>,
    who: Option<String>,
    why: Option<String>,
    retiring: Option<String>,
    reason: Option<String>,
    record: Option<String>,
}

fn parse(raw: Vec<String>) -> Result<Args, String> {
    let mut it = raw.into_iter();
    let command = match it.next() {
        Some(c) if c == "-h" || c == "--help" => return Err(String::new()),
        Some(c) => c,
        None => return Err(String::new()),
    };
    let mut args = Args {
        command,
        store: None,
        who: None,
        why: None,
        retiring: None,
        reason: None,
        record: None,
    };
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| it.next().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "--store" => args.store = Some(PathBuf::from(value("--store")?)),
            "--as" => args.who = Some(value("--as")?),
            "--why" => args.why = Some(value("--why")?),
            "--retiring" => args.retiring = Some(value("--retiring")?),
            "--reason" => args.reason = Some(value("--reason")?),
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => {
                if args.record.replace(other.to_string()).is_some() {
                    return Err("one record per verdict".to_string());
                }
            }
        }
    }
    Ok(args)
}

fn record_id(text: &str) -> Result<RecordId, String> {
    RecordId::from_str(text).map_err(|e| format!("{e} — ids look like rec_01M1P58RMFAD6DC86Q3D6JV6H2"))
}

fn usage_error(message: &str) -> ExitCode {
    if !message.is_empty() {
        eprintln!("tacit-keeper: {message}");
        eprintln!();
    }
    eprint!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(message) => return usage_error(&message),
    };
    let Some(store) = args.store.clone() else { return usage_error("--store is required") };

    // The lock comes before the open: a store being served must not be read
    // and then written around, and the refusal names who has it.
    let lock = match StoreLock::acquire(&store, "tacit-keeper") {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("tacit-keeper: {error}");
            return ExitCode::from(1);
        }
    };
    if let Some(from) = &lock.took_over_from {
        eprintln!("tacit-keeper: took over the lock from {from}");
    }

    let mut ledger = match Ledger::open(&store) {
        Ok(opened) => {
            if opened.recovery.truncated_bytes > 0 {
                eprintln!(
                    "tacit-keeper: dropped {} bytes of a torn final write while replaying",
                    opened.recovery.truncated_bytes
                );
            }
            opened.ledger
        }
        Err(error) => {
            eprintln!("tacit-keeper: could not open {}: {error}", store.display());
            return ExitCode::from(1);
        }
    };

    match args.command.as_str() {
        "pending" => {
            pending(&ledger);
            ExitCode::SUCCESS
        }
        "promote" | "reject" | "retire" => match ruling(&args) {
            Ok((who, why, ruling)) => verdict(&mut ledger, &who, &why, &ruling),
            Err(message) => usage_error(&message),
        },
        other => usage_error(&format!("unknown command {other}")),
    }
}

fn ruling(args: &Args) -> Result<(String, String, Ruling), String> {
    let who = args.who.clone().filter(|w| !w.trim().is_empty()).ok_or("--as is required")?;
    let why = args.why.clone().filter(|w| !w.trim().is_empty()).ok_or("--why is required")?;
    let target = record_id(args.record.as_deref().ok_or("a record id is required")?)?;
    let ruling = match args.command.as_str() {
        "promote" => Ruling::Promote {
            target,
            retiring: args.retiring.as_deref().map(record_id).transpose()?,
        },
        "reject" => {
            if args.retiring.is_some() {
                return Err("--retiring belongs to promote".to_string());
            }
            Ruling::Reject { target }
        }
        "retire" => {
            let text = args.reason.as_deref().ok_or("--reason is required for retire")?;
            let reason = retire_reason(text).ok_or_else(|| {
                format!("unknown reason {text:?}: superseded | no-longer-true | promoted-in-error")
            })?;
            Ruling::Retire { target, reason }
        }
        other => return Err(format!("unknown command {other}")),
    };
    if args.reason.is_some() && args.command != "retire" {
        return Err("--reason belongs to retire".to_string());
    }
    Ok((who, why, ruling))
}

fn verdict(ledger: &mut Ledger, who: &str, why: &str, ruling: &Ruling) -> ExitCode {
    let target = ruling.target();
    let before = ledger.state_of(target);
    match render(ledger, who, why, ruling) {
        Ok(id) => {
            let after = ledger.state_of(target).map(|s| s.to_string()).unwrap_or_default();
            let before = before.map(|s| s.to_string()).unwrap_or_else(|| "absent".into());
            println!("{target}: {before} -> {after}");
            println!("verdict {id} by {who} (asserted, not verified) — {why}");
            if let Ruling::Promote { retiring: Some(retired), .. } = ruling {
                let state = ledger.state_of(*retired).map(|s| s.to_string()).unwrap_or_default();
                println!("{retired}: retired by the same verdict -> {state}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("tacit-keeper: the ledger refused: {error}");
            ExitCode::from(1)
        }
    }
}

fn pending(ledger: &Ledger) {
    let inbox = ledger.pending_proposals();
    if inbox.queued.is_empty() {
        println!("nothing is waiting for a verdict");
    }
    for record in &inbox.queued {
        let envelope = record.envelope();
        println!(
            "{}  {}  {} ({:?})  via {}",
            record.id(),
            envelope.recorded_at().strftime("%Y-%m-%d %H:%M"),
            envelope.author().name,
            envelope.author().kind,
            envelope.source().channel
        );
        println!("    {}", text_of(ledger, record.content()));
    }
    if !inbox.superseded.is_empty() {
        println!(
            "({} proposal(s) replaced by their author before review — still proposed, not queued)",
            inbox.superseded.len()
        );
    }
    if !inbox.identical.is_empty() {
        println!("({} byte-identical duplicate(s) folded behind their first witness)", inbox.identical.len());
    }
}

fn text_of(ledger: &Ledger, content: &Content) -> String {
    let label = |id| ledger.entity(id).map(|e| e.label().to_string()).unwrap_or_else(|| id.to_string());
    match content {
        Content::Claim(ClaimContent::Text { body, .. }) => body.clone(),
        Content::Claim(ClaimContent::Pattern { solution, .. }) => solution.clone(),
        Content::Claim(ClaimContent::Attribute { subject, name, value }) => {
            format!("{} {name} = {value:?}", label(*subject))
        }
        Content::Claim(ClaimContent::Relation { subject, predicate, object, .. }) => {
            format!("{} {predicate} {}", label(*subject), label(*object))
        }
        other => format!("{:?}", other.kind()),
    }
}

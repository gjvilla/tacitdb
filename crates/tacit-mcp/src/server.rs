//! The Tacit MCP tool surface.
//!
//! The ratchet is visible here as an absence: **there is no promote tool.**
//! An agent can propose a claim, register an open question, and read anything
//! the record holds, but no sequence of calls it can make will move a claim to
//! promoted. That transition needs a human-declared verdict, which this
//! surface does not offer (D-0012, invariants 5 and 6).

use crate::shapes::{RecordOut, VerdictOut, state_label};
use crate::store::Store;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tacit_core::{
    Author, ClaimContent, Content, Draft, EntityId, Expansion, GapContent, Query, RecordId,
    SourceRef, StateFilter, ViewSpec,
};

const INSTRUCTIONS: &str = "\
Tacit is a graph database of organizational knowledge. Every record carries a
provenance envelope: who said it, from what source, when it is true, what
evidence supports it, and what would trigger its review.

Two things to know before using these tools.

First, abstention is a correct answer. `tacit_search` reports an outcome of
`matches`, `weak_matches`, or `none`, and separately reports any `registered
gap` whose territory your question meets. A registered gap is a named open
question the organization has recorded but not answered. When a search comes
back weak or empty, say so and cite the open question if one was offered —
that is more useful than the nearest plausible paragraph.

Second, you can propose but you cannot promote. `tacit_propose_claim` and
`tacit_register_gap` add records in the `proposed` and `registered` states.
Nothing you can call moves a claim to `promoted`; that requires a human
verdict, by design. Proposing is how you contribute; a person decides what the
organization knows.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// The question, in natural language.
    pub query: String,
    /// Include claims that are proposed but not yet promoted. Default false:
    /// the default view is what the organization has actually agreed.
    #[serde(default)]
    pub include_proposed: bool,
    /// Follow this many hops out from what the matches are about, to pull in
    /// surrounding context. 0 disables expansion.
    #[serde(default)]
    pub expand_hops: u8,
    /// Maximum results. Defaults to 8.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchOutput {
    /// `matches`, `weak_matches`, or `none`, plus `registered_gap` when the
    /// question meets a recorded open question.
    pub tags: Vec<String>,
    /// True when the honest answer is "the record does not settle this".
    pub is_abstention: bool,
    pub items: Vec<SearchItem>,
    /// Open questions the record has registered on this territory.
    pub open_questions: Vec<RecordOut>,
    pub truncated: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchItem {
    pub relevance: f64,
    /// How this was found: `lexical` for a word match, `vector` for one
    /// reached by similarity alone, `lexical+vector` for both, or `expanded`
    /// for context reached by traversal.
    pub via: String,
    /// True when `text` is the window of a long record around your question
    /// rather than the whole record — the budget assembles k answers, not one
    /// document (U-43). Fetch the record by id for the full text.
    pub excerpted: bool,
    #[serde(flatten)]
    pub record: RecordOut,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordParams {
    /// A record id, as returned by `tacit_search` (`rec_...`).
    pub record_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryOutput {
    pub record: RecordOut,
    /// Every verdict that moved this record, oldest first.
    pub verdicts: Vec<VerdictOut>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AsOfParams {
    pub record_id: String,
    /// An RFC3339 instant, e.g. `2026-08-23T12:00:00Z`.
    pub at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AsOfOutput {
    pub record_id: String,
    pub at: String,
    /// What the record said at that instant.
    pub state_then: String,
    pub state_now: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RecordsOutput {
    pub count: usize,
    pub records: Vec<RecordOut>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PendingOutput {
    pub count: usize,
    pub records: Vec<RecordOut>,
    /// Proposals a later draft replaced before anyone ruled on them. Still in
    /// the record and still proposed — reported here rather than dropped, so
    /// the count above is never mistaken for everything unreviewed.
    pub superseded_and_not_queued: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProposeClaimParams {
    /// What you are claiming, in plain language.
    pub text: String,
    /// Your name, recorded as the authoring agent.
    pub agent: String,
    /// Where this came from — a conversation, a document, an observation.
    pub source: String,
    /// Entity ids (`ent_...`) this claim is about, if known.
    #[serde(default)]
    pub about: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RegisterGapParams {
    /// The open question, stated so a person could answer it.
    pub question: String,
    pub agent: String,
    pub source: String,
    #[serde(default)]
    pub territory: Vec<String>,
    /// The event that should force this question to be answered.
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProposedOutput {
    pub record_id: String,
    pub state: String,
    /// What has to happen for this to become something the organization knows.
    pub next_step: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AuditParams {
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AuditOutput {
    pub entries: Vec<crate::store::AuditEntry>,
    /// Entries aged out of the bounded log.
    pub dropped: usize,
}

#[derive(Clone)]
pub struct TacitServer {
    store: Arc<Mutex<Store>>,
    tool_router: ToolRouter<Self>,
}

fn bad_request(message: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(message.into(), None)
}

#[tool_router]
impl TacitServer {
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        Self { store, tool_router: Self::tool_router() }
    }

    fn parse_record(id: &str) -> Result<RecordId, ErrorData> {
        id.parse::<RecordId>().map_err(|e| bad_request(e.to_string()))
    }

    fn parse_entities(ids: &[String]) -> Result<Vec<EntityId>, ErrorData> {
        ids.iter()
            .map(|id| id.parse::<EntityId>().map_err(|e| bad_request(e.to_string())))
            .collect()
    }

    #[tool(
        name = "tacit_search",
        description = "Search the organization's record. Returns matching records with full \
                       provenance, and separately reports registered open questions your query \
                       meets. An outcome of `weak_matches` or `none` means the record does not \
                       settle the question — report that honestly rather than paraphrasing the \
                       closest hit."
    )]
    fn search(&self, Parameters(params): Parameters<SearchParams>) -> Json<SearchOutput> {
        let mut store = self.store.lock().expect("store lock");
        let spec = if params.include_proposed {
            ViewSpec::now().with_states(StateFilter::PromotedAndProposed)
        } else {
            ViewSpec::now()
        };

        let mut query = Query::text(&params.query);
        query.budget.k = params.limit.unwrap_or(8).clamp(1, 50);
        if params.expand_hops > 0 {
            query = query.expanding(Expansion::hops(params.expand_hops.min(3)));
        }

        let output = {
            let retriever = store
                .index
                .retriever(&store.ledger, &store.projection, spec)
                .with_vectors(&store.vectors, &store.embedder);
            let found = retriever.retrieve(&query);
            SearchOutput {
                tags: found.tags().iter().map(|t| t.to_string()).collect(),
                is_abstention: found.is_abstention(),
                items: found
                    .items
                    .iter()
                    .map(|item| {
                        let mut record = RecordOut::of(&store.ledger, item.record);
                        // The budget assembled a window, and the window is
                        // what this tool serves — handing back the full text
                        // anyway would spend the tokens the excerpt saved.
                        if let Some(excerpt) = &item.excerpt {
                            record.text = excerpt.clone();
                        }
                        SearchItem {
                            relevance: (item.relevance * 100.0).round() / 100.0,
                            via: match &item.via {
                                tacit_core::Via::Lexical => "lexical".to_string(),
                                tacit_core::Via::Vector => "vector".to_string(),
                                tacit_core::Via::Hybrid => "lexical+vector".to_string(),
                                tacit_core::Via::Expanded { path, .. } => {
                                    format!("expanded ({} edge(s))", path.len())
                                }
                            },
                            excerpted: item.excerpt.is_some(),
                            record,
                        }
                    })
                    .collect(),
                open_questions: found
                    .gaps
                    .iter()
                    .map(|g| RecordOut::of(&store.ledger, g))
                    .collect(),
                truncated: found.truncated,
            }
        };
        store.record_call(
            "tacit_search",
            params.query.chars().take(80).collect::<String>(),
            output.tags.join("+"),
        );
        Json(output)
    }

    #[tool(
        name = "tacit_get_record",
        description = "Fetch one record by id, with its envelope and evidence chain."
    )]
    fn get_record(
        &self,
        Parameters(params): Parameters<RecordParams>,
    ) -> Result<Json<RecordOut>, ErrorData> {
        let id = Self::parse_record(&params.record_id)?;
        let mut store = self.store.lock().expect("store lock");
        let out = store
            .ledger
            .record(id)
            .map(|record| RecordOut::of(&store.ledger, record))
            .ok_or_else(|| bad_request(format!("no record {id}")))?;
        store.record_call("tacit_get_record", params.record_id.clone(), out.state.clone());
        Ok(Json(out))
    }

    #[tool(
        name = "tacit_history",
        description = "Why a record is in the state it is: every verdict that moved it, who \
                       rendered it, and their stated reason."
    )]
    fn history(
        &self,
        Parameters(params): Parameters<RecordParams>,
    ) -> Result<Json<HistoryOutput>, ErrorData> {
        let id = Self::parse_record(&params.record_id)?;
        let mut store = self.store.lock().expect("store lock");
        let output = {
            let record = store
                .ledger
                .record(id)
                .ok_or_else(|| bad_request(format!("no record {id}")))?;
            HistoryOutput {
                record: RecordOut::of(&store.ledger, record),
                verdicts: store.ledger.history(id).into_iter().filter_map(VerdictOut::of).collect(),
            }
        };
        store.record_call(
            "tacit_history",
            params.record_id.clone(),
            format!("{} verdict(s)", output.verdicts.len()),
        );
        Ok(Json(output))
    }

    #[tool(
        name = "tacit_open_questions",
        description = "Every registered open question — things the organization knows it has not \
                       decided. Cite these when the record cannot answer something."
    )]
    fn open_questions(&self) -> Json<RecordsOutput> {
        let mut store = self.store.lock().expect("store lock");
        let records: Vec<RecordOut> = store
            .ledger
            .registered_gaps()
            .iter()
            .map(|r| RecordOut::of(&store.ledger, r))
            .collect();
        let output = RecordsOutput { count: records.len(), records };
        store.record_call("tacit_open_questions", "", format!("{} open", output.count));
        Json(output)
    }

    #[tool(
        name = "tacit_as_of",
        description = "What the record said about something at a past instant. Record-time \
                       travel: use it to answer \"what did we know when we decided this?\""
    )]
    fn as_of(&self, Parameters(params): Parameters<AsOfParams>) -> Result<Json<AsOfOutput>, ErrorData> {
        let id = Self::parse_record(&params.record_id)?;
        let at: jiff::Timestamp = params
            .at
            .parse()
            .map_err(|_| bad_request(format!("{:?} is not an RFC3339 instant", params.at)))?;
        let mut store = self.store.lock().expect("store lock");
        let output = AsOfOutput {
            record_id: id.to_string(),
            at: at.to_string(),
            state_then: state_label(store.ledger.state_of_at(id, at)),
            state_now: state_label(store.ledger.state_of(id)),
        };
        store.record_call("tacit_as_of", params.at.clone(), output.state_then.clone());
        Ok(Json(output))
    }

    #[tool(
        name = "tacit_contradictions",
        description = "Promoted claims that contradict each other — the same subject and \
                       attribute, both true at once. Surfaced, never silently resolved."
    )]
    fn contradictions(&self) -> Json<RecordsOutput> {
        let mut store = self.store.lock().expect("store lock");
        let records: Vec<RecordOut> = store
            .ledger
            .contradictions()
            .iter()
            .flat_map(|c| [c.a, c.b])
            .map(|r| RecordOut::of(&store.ledger, r))
            .collect();
        let output = RecordsOutput { count: records.len() / 2, records };
        store.record_call("tacit_contradictions", "", format!("{} pair(s)", output.count));
        Json(output)
    }

    #[tool(
        name = "tacit_pending_proposals",
        description = "Claims awaiting a human verdict — the keeper's inbox, including anything \
                       agents have proposed. A draft its own author has already replaced is \
                       counted separately rather than listed twice."
    )]
    fn pending_proposals(&self) -> Json<PendingOutput> {
        let mut store = self.store.lock().expect("store lock");
        let pending = store.ledger.pending_proposals();
        let superseded_and_not_queued = pending.superseded.len();
        let records: Vec<RecordOut> =
            pending.queued.iter().map(|r| RecordOut::of(&store.ledger, r)).collect();
        let output =
            PendingOutput { count: records.len(), records, superseded_and_not_queued };
        store.record_call(
            "tacit_pending_proposals",
            "",
            format!("{} pending, {superseded_and_not_queued} superseded", output.count),
        );
        Json(output)
    }

    #[tool(
        name = "tacit_propose_claim",
        description = "Propose something for the record. It lands as `proposed` and stays there \
                       until a person promotes it — you cannot promote it yourself, and no tool \
                       here can. Use this to contribute what you learned; a human decides \
                       whether it becomes something the organization knows."
    )]
    fn propose_claim(
        &self,
        Parameters(params): Parameters<ProposeClaimParams>,
    ) -> Result<Json<ProposedOutput>, ErrorData> {
        let about = Self::parse_entities(&params.about)?;
        let mut store = self.store.lock().expect("store lock");
        let draft = Draft::new(
            Author::agent(&params.agent),
            SourceRef {
                channel: "mcp".into(),
                reference: Some(params.source.clone()),
            },
            Content::Claim(ClaimContent::Text { body: params.text.clone(), about }),
        );
        let id = store.ledger.append(draft).map_err(|e| bad_request(e.to_string()))?;
        store.refresh();
        store.record_call(
            "tacit_propose_claim",
            params.text.chars().take(80).collect::<String>(),
            id.to_string(),
        );
        Ok(Json(ProposedOutput {
            record_id: id.to_string(),
            state: "proposed".into(),
            next_step: "A person must render a promote verdict before this counts as something \
                        the organization knows."
                .into(),
        }))
    }

    #[tool(
        name = "tacit_register_gap",
        description = "Register an open question the record cannot answer. This is how a gap \
                       becomes citable later instead of being rediscovered. Prefer this to \
                       guessing when a search comes back weak or empty."
    )]
    fn register_gap(
        &self,
        Parameters(params): Parameters<RegisterGapParams>,
    ) -> Result<Json<ProposedOutput>, ErrorData> {
        let territory = Self::parse_entities(&params.territory)?;
        let mut store = self.store.lock().expect("store lock");
        let mut draft = Draft::new(
            Author::agent(&params.agent),
            SourceRef { channel: "mcp".into(), reference: Some(params.source.clone()) },
            Content::Gap(GapContent { question: params.question.clone(), territory }),
        );
        draft.review_trigger = params.trigger.clone().map(|event| tacit_core::ReviewTrigger {
            due_at: None,
            on_event: Some(event),
        });
        let id = store.ledger.append(draft).map_err(|e| bad_request(e.to_string()))?;
        store.refresh();
        store.record_call(
            "tacit_register_gap",
            params.question.chars().take(80).collect::<String>(),
            id.to_string(),
        );
        Ok(Json(ProposedOutput {
            record_id: id.to_string(),
            state: "registered".into(),
            next_step: "The question is now citable. It closes when a person answers it with a \
                        promoted claim, or withdraws it."
                .into(),
        }))
    }

    #[tool(
        name = "tacit_audit",
        description = "What has been asked of this record and by whom — the tool-call log."
    )]
    fn audit(&self, Parameters(params): Parameters<AuditParams>) -> Json<AuditOutput> {
        let store = self.store.lock().expect("store lock");
        let (entries, dropped) = store.audit(params.limit.unwrap_or(50).clamp(1, 500));
        Json(AuditOutput { entries, dropped })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TacitServer {
    fn get_info(&self) -> ServerInfo {
        let store = self.store.lock().expect("store lock");
        let summary = format!(
            "{} records, {} entities, {} open questions.",
            store.ledger.log().len(),
            store.ledger.entities().count(),
            store.ledger.registered_gaps().len()
        );
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tacit", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!("{INSTRUCTIONS}\n\nThis store holds {summary}"))
    }
}

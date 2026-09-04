//! End-to-end tests against the real binary over stdio. These spawn the host
//! and speak MCP to it, because a tool surface that compiles is not the same
//! as one that answers.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Host {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Host {
    /// Start the host over this repository's own corpus.
    fn start() -> Self {
        let repo = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_tacit-mcp"))
            .arg(repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("host starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut host = Self { child, stdin, stdout, next_id: 0 };
        host.initialize();
        host
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");

        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read");
        let response: Value = serde_json::from_str(&line).expect("json response");
        assert_eq!(response["id"], id, "responses are read in order for one request at a time");
        assert!(response.get("error").is_none(), "unexpected error: {response}");
        response["result"].clone()
    }

    fn notify(&mut self, method: &str) {
        let message = json!({"jsonrpc": "2.0", "method": method});
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
    }

    fn initialize(&mut self) -> Value {
        let result = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "tacit-tests", "version": "1"}
            }),
        );
        self.notify("notifications/initialized");
        result
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        result
            .get("structuredContent")
            .cloned()
            .or_else(|| {
                result["content"][0]["text"]
                    .as_str()
                    .and_then(|t| serde_json::from_str(t).ok())
            })
            .unwrap_or(result)
    }

    fn tool_names(&mut self) -> Vec<String> {
        let result = self.request("tools/list", json!({}));
        result["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_string())
            .collect()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_host_speaks_mcp_and_loads_the_corpus() {
    let mut host = Host::start();
    let info = host.request("tools/list", json!({}));
    assert!(!info["tools"].as_array().expect("tools").is_empty());
}

/// The ratchet as an executable claim: no sequence of tool calls an agent can
/// make promotes anything, because the surface offers no way to.
#[test]
fn there_is_no_promote_tool() {
    let mut host = Host::start();
    let names = host.tool_names();
    assert!(!names.is_empty());
    for forbidden in ["promote", "verdict", "retire", "reject", "approve"] {
        assert!(
            !names.iter().any(|n| n.contains(forbidden)),
            "the agent surface must not expose {forbidden}: {names:?}"
        );
    }
    assert!(names.contains(&"tacit_propose_claim".to_string()));
}

#[test]
fn search_answers_with_provenance() {
    let mut host = Host::start();
    let out = host.call(
        "tacit_search",
        json!({"query": "why is the runtime embedded rather than a server"}),
    );
    assert!(out["tags"].as_array().unwrap().iter().any(|t| t == "matches"));
    assert_eq!(out["is_abstention"], false);

    let first = &out["items"][0];
    assert!(first["author"].as_str().is_some_and(|a| !a.is_empty()));
    assert!(first["author_kind"].as_str().is_some());
    assert!(first["state"].as_str().is_some_and(|s| s.contains("Promoted")));
    assert!(first["source_channel"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(first["recorded_at"].as_str().is_some());
}

/// R-10 over the wire: a question the record does not settle comes back
/// labelled, not paraphrased.
#[test]
fn search_abstains_on_what_the_record_does_not_cover() {
    let mut host = Host::start();
    let out = host.call(
        "tacit_search",
        json!({"query": "how does sharding across geographic regions work"}),
    );
    assert_eq!(out["is_abstention"], true);
    let tags: Vec<&str> = out["tags"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    assert!(!tags.contains(&"matches"), "got {tags:?}");
}

#[test]
fn open_questions_are_citable() {
    let mut host = Host::start();
    let out = host.call("tacit_open_questions", json!({}));
    // The floor was fifteen until 2026-08-31, when the register's open rows
    // dropped beneath it honestly — thirty-plus resolutions in nine days.
    // The assertion is that the register is *loaded*, not that it stays big.
    assert!(out["count"].as_u64().unwrap() >= 8, "the register's unknowns are loaded");
    let first = &out["records"][0];
    assert!(first["text"].as_str().is_some_and(|t| !t.is_empty()));
}

/// U-12, at the tool surface: the second identical proposal is written — a
/// witness keeps its envelope — and told what it is, and the inbox folds it
/// rather than queueing one wording twice.
#[test]
fn an_identical_proposal_is_disclosed_and_folded_not_refused() {
    let mut host = Host::start();
    let text = "the deploy runbook lives in the operations wiki";
    let first = host.call(
        "tacit_propose_claim",
        json!({"text": text, "agent": "agent-one", "source": "test"}),
    );
    assert!(first.get("identical_to").is_none(), "the first of anything is no duplicate");

    let second = host.call(
        "tacit_propose_claim",
        json!({"text": text, "agent": "agent-two", "source": "test"}),
    );
    assert_eq!(second["identical_to"], first["record_id"]);
    assert!(second["identical_state"].as_str().unwrap().contains("Proposed"));
    assert_ne!(second["record_id"], first["record_id"], "both records exist");

    let pending = host.call("tacit_pending_proposals", json!({"limit": 200}));
    assert_eq!(pending["identical_and_folded"], 1);
    let queued_texts = pending["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["text"] == text)
        .count();
    assert_eq!(queued_texts, 1, "one wording, one queue slot");
}

/// An agent may contribute, and what it contributes stays proposed.
#[test]
fn an_agent_can_propose_but_what_it_proposes_stays_proposed() {
    let mut host = Host::start();
    let proposed = host.call(
        "tacit_propose_claim",
        json!({
            "text": "a claim contributed by the test agent",
            "agent": "test-agent",
            "source": "integration test"
        }),
    );
    assert_eq!(proposed["state"], "proposed");
    let id = proposed["record_id"].as_str().expect("id").to_string();

    let fetched = host.call("tacit_get_record", json!({"record_id": id}));
    assert_eq!(fetched["author_kind"], "agent");
    assert!(fetched["state"].as_str().unwrap().contains("Proposed"));

    // It is in the keeper's inbox, waiting on a person — and the default
    // window keeps the newest, so a fresh proposal is visible without asking
    // for the whole queue (D-0049).
    let pending = host.call("tacit_pending_proposals", json!({}));
    let mine = pending["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["author"] == "test-agent")
        .count();
    assert_eq!(mine, 1);
    // The count is the queue's truth, not the window's length: a bounded
    // listing must never be mistaken for everything unreviewed (U-30's rule,
    // restated for a limit). Asking for one record leaves the count whole.
    let bounded = host.call("tacit_pending_proposals", json!({"limit": 1}));
    assert_eq!(bounded["records"].as_array().unwrap().len(), 1);
    assert_eq!(
        bounded["count"].as_u64().unwrap(),
        pending["count"].as_u64().unwrap()
    );
    assert!(pending["superseded_and_not_queued"].is_number());

    // And it has no verdict history, because no verdict is reachable from here.
    let history = host.call("tacit_history", json!({"record_id": id}));
    assert!(history["verdicts"].as_array().unwrap().is_empty());
}

#[test]
fn an_agent_can_register_an_open_question() {
    let mut host = Host::start();
    let before = host.call("tacit_open_questions", json!({}))["count"].as_u64().unwrap();
    let registered = host.call(
        "tacit_register_gap",
        json!({
            "question": "which embedding model should supply vector candidates?",
            "agent": "test-agent",
            "source": "integration test",
            "trigger": "before the golden suite"
        }),
    );
    assert_eq!(registered["state"], "registered");
    let after = host.call("tacit_open_questions", json!({}))["count"].as_u64().unwrap();
    assert_eq!(after, before + 1, "the question is immediately citable");
}

/// Record-time travel over the wire.
#[test]
fn as_of_reads_the_past() {
    let mut host = Host::start();
    let out = host.call("tacit_search", json!({"query": "runtime shape embedded server"}));
    let id = out["items"][0]["id"].as_str().expect("an id").to_string();

    let past = host.call("tacit_as_of", json!({"record_id": id, "at": "2020-01-01T00:00:00Z"}));
    assert_eq!(past["state_then"], "not in the record");
    assert!(past["state_now"].as_str().unwrap().contains("Promoted"));
}

#[test]
fn every_call_is_audited() {
    let mut host = Host::start();
    host.call("tacit_search", json!({"query": "provenance"}));
    host.call("tacit_open_questions", json!({}));
    let audit = host.call("tacit_audit", json!({"limit": 10}));
    let tools: Vec<&str> = audit["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["tool"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"tacit_search"));
    assert!(tools.contains(&"tacit_open_questions"));
}

#[test]
fn a_bad_id_is_a_clean_error_not_a_panic() {
    let mut host = Host::start();
    host.next_id += 1;
    let id = host.next_id;
    let message = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": "tacit_get_record", "arguments": {"record_id": "not-an-id"}}
    });
    writeln!(host.stdin, "{message}").expect("write");
    host.stdin.flush().expect("flush");
    let mut line = String::new();
    host.stdout.read_line(&mut line).expect("read");
    let response: Value = serde_json::from_str(&line).expect("json");
    let text = response.to_string();
    assert!(text.contains("not a valid"), "expected a clean parse error, got {text}");

    // The host is still serving.
    let names = host.tool_names();
    assert!(!names.is_empty());
}

/// `--help` is the one thing a stranger types first. It answers on stdout,
/// exits clean, and names every option the parser actually accepts — checked
/// here so the help cannot drift from the code it describes.
#[test]
fn help_names_every_option_and_exits_clean() {
    let output = Command::new(env!("CARGO_BIN_EXE_tacit-mcp"))
        .arg("--help")
        .output()
        .expect("host runs");
    assert!(output.status.success(), "help exits 0");
    let text = String::from_utf8(output.stdout).expect("utf8");
    for option in ["--store", "--require-signature", "--signed-by", "--help"] {
        assert!(text.contains(option), "help mentions {option}");
    }
    assert!(text.contains("docs/DECISIONS.md"), "help says where the corpus lives");
}

/// An unknown option still fails, and now points at the help instead of
/// leaving the reader to open main.rs.
#[test]
fn an_unknown_option_points_at_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_tacit-mcp"))
        .arg("--halp")
        .output()
        .expect("host runs");
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stderr).expect("utf8");
    assert!(text.contains("--help"), "stderr was: {text}");
}

/// With a store, the host holds the store's lock for as long as it runs and
/// releases it on exit — so a keyboard verdict cannot land underneath it
/// (D-0055). Checked against the real binary because a lock the host forgot
/// to take would fail silently in exactly the case it exists for.
#[test]
fn a_served_store_is_held_and_released() {
    let dir = std::env::temp_dir().join(format!("tacit-host-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.log");
    let lock = dir.join("store.lock");

    let repo = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_tacit-mcp"))
        .arg("--store")
        .arg(&store)
        .arg(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("host starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Once it answers initialize, it has opened the store — and taken the lock.
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},
            "clientInfo":{"name":"lock-test","version":"1"}}})
    )
    .unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert!(line.contains("\"result\""), "initialize answered: {line}");

    let held = std::fs::read_to_string(&lock).expect("lock file exists while serving");
    assert!(held.contains("tacit-mcp"), "lock names its holder: {held:?}");
    assert_eq!(held.split_whitespace().next().unwrap(), child.id().to_string());

    drop(stdin);
    child.wait().expect("host exits when stdin closes");
    assert!(!lock.exists(), "lock released on exit");
}

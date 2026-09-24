//! Provider usage, read as the provider reported it, and the one normalisation every
//! comparison uses.
//!
//! Providers do not agree on what their fields mean. Anthropic reports three disjoint input
//! quantities (uncached, written to the cache, read from the cache); OpenAI reports one input
//! total with the cached part *inside* it. Both are kept in their native names in the run
//! record ([`EconomicsRequest`]), and [`totals`] maps them onto one shape where
//! `total_input = uncached + cache write + cache read` holds for both.
//!
//! What a session said is never kept. [`read_stream`] reads a Claude Code `stream-json`
//! transcript and keeps numbers and tool *names*; text, tool inputs and tool results are
//! dropped as they are read, so a run record cannot leak a prompt, a file or a secret.
//!
//! ```
//! use majordomus_cli::economics::usage::read_stream;
//! let stream = concat!(
//!     r#"{"type":"assistant","message":{"id":"m1","model":"claude-sonnet-5","content":[{"type":"text","text":"secret"}],"usage":{"input_tokens":3,"cache_creation_input_tokens":100,"cache_read_input_tokens":0,"output_tokens":7}}}"#, "\n",
//!     r#"{"type":"assistant","message":{"id":"m1","model":"claude-sonnet-5","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/x"}}],"usage":{"input_tokens":3,"cache_creation_input_tokens":100,"cache_read_input_tokens":0,"output_tokens":7}}}"#, "\n",
//!     r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0.25}"#, "\n",
//! );
//! let s = read_stream(stream).unwrap();
//! assert_eq!(s.requests.len(), 1, "one message, two content blocks, one request");
//! assert_eq!(s.tools.get("Read"), Some(&1));
//! assert_eq!(s.reported_cost_microusd, Some(250_000));
//! assert!(!serde_json::to_string(&s.requests).unwrap().contains("secret"));
//! ```

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::model::{
    EconomicsModelTotal, EconomicsOrientation, EconomicsRequest, EconomicsRun, EconomicsSession,
    EconomicsUsage,
};

/// Tools that change files: the first call to one ends a session's orientation.
const EDITS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];
/// Tools that read a file.
const READS: &[&str] = &["Read", "NotebookRead"];
/// Tools that search.
const SEARCHES: &[&str] = &["Grep", "Glob", "LS", "WebSearch"];
/// Shell commands that are searches when they lead a `Bash` call.
const SHELL_SEARCHES: &[&str] = &["grep", "rg", "find", "ls", "git grep", "fd"];

/// What [`read_stream`] recovers from one session's transcript: the numeric and structural
/// parts of an [`EconomicsSession`]. The caller adds what the transcript does not know
/// (index, prompt digest, wall clock).
///
/// ```
/// use majordomus_cli::economics::usage::{read_stream, StreamFacts};
/// let line = concat!(
///     r#"{"type":"assistant","message":{"id":"m1","model":"m","content":[],"#,
///     r#""usage":{"input_tokens":4,"output_tokens":1}}}"#,
/// );
/// let facts: StreamFacts = read_stream(line).unwrap();
/// assert_eq!(facts.requests[0].input_tokens, 4);
/// assert_eq!(facts.ended, "no_result", "no closing `result` event: the session never finished");
/// assert!(facts.is_error);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamFacts {
    /// Requests, deduplicated by message id, in first-seen order. An event that carries no
    /// message id is a request of its own: nothing says it repeats another.
    pub requests: Vec<EconomicsRequest>,
    /// The harness's per-model totals from its closing `result` event.
    pub models: Vec<EconomicsModelTotal>,
    /// The harness's `total_cost_usd`, in millionths of a dollar.
    pub reported_cost_microusd: Option<u64>,
    /// Tool calls by name.
    pub tools: BTreeMap<String, u64>,
    /// Orientation before the first edit.
    pub orientation: EconomicsOrientation,
    /// Turns.
    pub turns: Option<u64>,
    /// How the session ended.
    pub ended: String,
    /// Whether the harness reported an error.
    pub is_error: bool,
    /// Tools the harness offered, from its `init` event.
    pub tools_offered: Vec<String>,
    /// MCP servers the harness connected, from its `init` event.
    pub mcp_servers: Vec<String>,
}

fn u(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn microusd(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_f64)
        .filter(|x| x.is_finite() && *x >= 0.0)
        .map(|x| (x * 1_000_000.0).round() as u64)
}

/// Read an Anthropic `usage` object into a request, native field names kept.
///
/// ```
/// use majordomus_cli::economics::usage::anthropic_request;
/// let r = anthropic_request("m", &serde_json::json!({
///     "input_tokens": 2, "cache_creation_input_tokens": 10, "cache_read_input_tokens": 90,
///     "output_tokens": 5, "output_tokens_details": {"thinking_tokens": 3}
/// }));
/// assert_eq!((r.input_tokens, r.cache_read_input_tokens, r.thinking_tokens), (2, 90, Some(3)));
/// ```
pub fn anthropic_request(model: &str, usage: &Value) -> EconomicsRequest {
    EconomicsRequest {
        model: model.to_string(),
        input_tokens: u(usage, "input_tokens"),
        cache_creation_input_tokens: u(usage, "cache_creation_input_tokens"),
        cache_read_input_tokens: u(usage, "cache_read_input_tokens"),
        output_tokens: u(usage, "output_tokens"),
        thinking_tokens: usage
            .get("output_tokens_details")
            .and_then(|d| d.get("thinking_tokens"))
            .and_then(Value::as_u64),
    }
}

/// Read an OpenAI `usage` object into the same shape. OpenAI's `input_tokens` *includes*
/// the cached part (`input_tokens_details.cached_tokens`), where Anthropic's excludes it;
/// the cached part is moved out so that the three input fields are disjoint for both.
/// OpenAI bills no cache writes, so that field is zero, which is what it is.
///
/// ```
/// use majordomus_cli::economics::usage::openai_request;
/// let r = openai_request("gpt-5", &serde_json::json!({
///     "input_tokens": 1000, "input_tokens_details": {"cached_tokens": 800},
///     "output_tokens": 50, "output_tokens_details": {"reasoning_tokens": 30}
/// }));
/// assert_eq!((r.input_tokens, r.cache_read_input_tokens, r.cache_creation_input_tokens), (200, 800, 0));
/// assert_eq!(r.thinking_tokens, Some(30));
/// ```
pub fn openai_request(model: &str, usage: &Value) -> EconomicsRequest {
    let input = u(usage, "input_tokens").max(u(usage, "prompt_tokens"));
    let cached = usage
        .get("input_tokens_details")
        .or_else(|| usage.get("prompt_tokens_details"))
        .map(|d| u(d, "cached_tokens"))
        .unwrap_or(0)
        .min(input);
    EconomicsRequest {
        model: model.to_string(),
        input_tokens: input - cached,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: cached,
        output_tokens: u(usage, "output_tokens").max(u(usage, "completion_tokens")),
        thinking_tokens: usage
            .get("output_tokens_details")
            .or_else(|| usage.get("completion_tokens_details"))
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(Value::as_u64),
    }
}

fn request_total(r: &EconomicsRequest) -> u64 {
    r.input_tokens + r.cache_creation_input_tokens + r.cache_read_input_tokens + r.output_tokens
}

/// What a shell command does to the working tree, as far as its text shows. Agents edit
/// through the shell as often as through edit tools (`cat > f`, `sed -i`, a heredoc script
/// that opens a file for writing), so orientation cannot end only at an `Edit` call.
///
/// ```
/// use majordomus_cli::economics::usage::{shell_effect, ShellEffect};
/// let effects = ["rg todo src", "head -5 a.rs", "cargo test", "cp a.rs b.rs"].map(shell_effect);
/// assert_eq!(
///     effects,
///     [ShellEffect::Search, ShellEffect::Read, ShellEffect::Other, ShellEffect::Edit]
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEffect {
    /// Writes a file.
    Edit,
    /// Searches (grep, find, ls, git ls-files).
    Search,
    /// Reads a file (cat, head, tail, sed -n).
    Read,
    /// Anything else: running tests, git status, ...
    Other,
}

/// The command with every quoted span replaced by one placeholder word, so that a `>` or a
/// `;` inside an argument (`grep '>' f`, `echo "a; rm b"`) is not read as shell syntax,
/// while a quoted redirection target (`> 'out.txt'`) still leaves a target behind. A quote
/// left open runs to the end of the command, as the shell would read it.
fn unquoted(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut chars = cmd.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                // a single-quoted span ends at the next single quote; nothing escapes in it
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                }
                out.push('_');
            }
            '"' => {
                // a double-quoted span ends at the next unescaped double quote
                while let Some(q) = chars.next() {
                    match q {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
                out.push('_');
            }
            _ => out.push(c),
        }
    }
    out
}

fn writes(cmd: &str) -> bool {
    const MARKERS: &[&str] = &[
        "sed -i",
        "perl -i",
        "| tee ",
        " tee -a ",
        "git apply",
        "patch -p",
        ".write_text(",
        ".write(",
        "open(p, \"w\")",
        "open(p,\"w\")",
        "\"w\")",
        "'w')",
        "\"a\")",
        "'a')",
    ];
    if MARKERS.iter().any(|m| cmd.contains(m)) {
        return true;
    }
    let bare = unquoted(cmd);
    let tokens: Vec<&str> = bare.split_whitespace().collect();
    // a file-changing command word, in command position: first, or after ; && || |
    let command_position = |i: usize| {
        i == 0
            || tokens
                .get(i - 1)
                .is_some_and(|p| p.ends_with(';') || ["&&", "||", "|", ";"].contains(p))
    };
    if tokens
        .iter()
        .enumerate()
        .any(|(i, t)| ["cp", "mv", "rm", "touch", "mkdir"].contains(t) && command_position(i))
    {
        return true;
    }
    tokens.iter().enumerate().any(|(i, t)| {
        let target = if *t == ">" || *t == ">>" {
            tokens.get(i + 1).copied()
        } else {
            t.strip_prefix(">>").or_else(|| t.strip_prefix('>'))
        };
        target.is_some_and(|x| !x.is_empty() && !x.starts_with("/dev/") && !x.starts_with('&'))
    })
}

/// Classify one shell command by its text alone; nothing is run. A command that writes a
/// file is an edit whatever else it does, so a write outranks the read or search it starts
/// with; otherwise the leading command decides, and a pipe into `grep` makes a search. A
/// redirection to `/dev/null` or to another descriptor (`2>&1`) writes nothing and is not an
/// edit, and neither is a `>` inside a quoted argument, which is text, not a redirection.
///
/// ```
/// use majordomus_cli::economics::usage::{shell_effect, ShellEffect};
/// assert_eq!(shell_effect("cat > tests/test_x.py <<'EOF'"), ShellEffect::Edit);
/// assert_eq!(shell_effect("sed -i '' 's/a/b/' f.py"), ShellEffect::Edit);
/// assert_eq!(shell_effect("python3 -m unittest -q 2>&1 | tail -3"), ShellEffect::Other);
/// assert_eq!(shell_effect("grep -rn vat ledgerlite"), ShellEffect::Search);
/// assert_eq!(shell_effect("cat ledgerlite/tax.py"), ShellEffect::Read);
/// assert_eq!(shell_effect("cat a.py > b.py"), ShellEffect::Edit, "the write outranks the read");
/// assert_eq!(shell_effect("ls src > /dev/null"), ShellEffect::Search);
/// assert_eq!(shell_effect("grep -rn \"total > limit\" ledgerlite"), ShellEffect::Search);
/// assert_eq!(shell_effect("echo done > 'notes.md'"), ShellEffect::Edit, "a quoted target");
/// ```
pub fn shell_effect(cmd: &str) -> ShellEffect {
    let c = cmd.trim_start();
    if writes(c) {
        return ShellEffect::Edit;
    }
    let first = |s: &str| c == s || c.starts_with(&format!("{s} "));
    if SHELL_SEARCHES.iter().any(|s| first(s)) || c.contains("git ls-files") || c.contains("| grep")
    {
        return ShellEffect::Search;
    }
    if ["cat", "head", "tail", "less", "sed -n", "nl", "wc"]
        .iter()
        .any(|s| first(s))
    {
        return ShellEffect::Read;
    }
    ShellEffect::Other
}

fn is_search(name: &str, input: &Value) -> bool {
    if SEARCHES.contains(&name) {
        return true;
    }
    name == "Bash"
        && shell_effect(input.get("command").and_then(Value::as_str).unwrap_or(""))
            == ShellEffect::Search
}

fn is_read(name: &str, input: &Value) -> bool {
    READS.contains(&name)
        || (name == "Bash"
            && shell_effect(input.get("command").and_then(Value::as_str).unwrap_or(""))
                == ShellEffect::Read)
}

fn is_edit(name: &str, input: &Value) -> bool {
    EDITS.contains(&name)
        || (name == "Bash"
            && shell_effect(input.get("command").and_then(Value::as_str).unwrap_or(""))
                == ShellEffect::Edit)
}

/// Read a Claude Code `--output-format stream-json --verbose` transcript. Lines that are not
/// JSON objects are skipped; a transcript with neither a request nor a `result` event is an
/// error, because a session that reported nothing must be recorded as having reported
/// nothing, not as having used zero tokens.
///
/// ```
/// use majordomus_cli::economics::usage::read_stream;
/// assert!(read_stream("").is_err(), "a silent transcript is not zero tokens");
/// let stream = concat!(
///     r#"{"type":"system","subtype":"init","tools":["Read","Bash"],"#,
///     r#""mcp_servers":[{"name":"majordomus"}]}"#, "\n",
///     "not json\n",
///     r#"{"type":"result","subtype":"error_max_turns","is_error":true,"num_turns":9}"#,
/// );
/// let facts = read_stream(stream).unwrap();
/// assert_eq!(facts.tools_offered, ["Read", "Bash"]);
/// assert_eq!(facts.mcp_servers, ["majordomus"]);
/// let ending = (facts.ended.as_str(), facts.is_error, facts.turns);
/// assert_eq!(ending, ("error_max_turns", true, Some(9)));
/// assert!(facts.requests.is_empty() && facts.reported_cost_microusd.is_none());
/// ```
pub fn read_stream(text: &str) -> Result<StreamFacts, String> {
    let mut facts = StreamFacts::default();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut saw_result = false;
    let mut edited = false;
    let mut before_edit_requests: BTreeSet<String> = BTreeSet::new();
    for (line_no, line) in text.lines().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event.get("type").and_then(Value::as_str) {
            Some("system") if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                facts.tools_offered = strings(event.get("tools"));
                facts.mcp_servers = event
                    .get("mcp_servers")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.get("name").and_then(Value::as_str))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
            }
            Some("assistant") => {
                let Some(message) = event.get("message") else {
                    continue;
                };
                // A message id is what joins the events one request was split into. An
                // event without one is a request of its own, keyed by its line, so that two
                // of them are never collapsed into one, and a later one never takes the
                // place of an earlier one inside or outside the orientation.
                let id = match message.get("id").and_then(Value::as_str) {
                    Some(id) if !id.is_empty() => id.to_string(),
                    _ => format!("#{}", line_no + 1),
                };
                let model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                if let Some(usage) = message.get("usage") {
                    let request = anthropic_request(model, usage);
                    match index.get(&id) {
                        Some(&i) => facts.requests[i] = request,
                        None => {
                            index.insert(id.clone(), facts.requests.len());
                            facts.requests.push(request);
                        }
                    }
                }
                if !edited {
                    before_edit_requests.insert(id.clone());
                }
                let blocks = message.get("content").and_then(Value::as_array);
                for block in blocks.into_iter().flatten() {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    *facts.tools.entry(name.to_string()).or_insert(0) += 1;
                    if edited {
                        continue;
                    }
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    if is_edit(name, &input) {
                        edited = true;
                        facts.orientation.edited = true;
                        continue;
                    }
                    facts.orientation.tool_calls += 1;
                    if is_read(name, &input) {
                        facts.orientation.reads += 1;
                    }
                    if is_search(name, &input) {
                        facts.orientation.searches += 1;
                    }
                }
            }
            Some("result") => {
                saw_result = true;
                facts.turns = event.get("num_turns").and_then(Value::as_u64);
                facts.is_error = event
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                facts.ended = event
                    .get("subtype")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                facts.reported_cost_microusd = microusd(event.get("total_cost_usd"));
                if let Some(models) = event.get("modelUsage").and_then(Value::as_object) {
                    facts.models = models
                        .iter()
                        .map(|(model, m)| EconomicsModelTotal {
                            model: model.clone(),
                            input_tokens: u(m, "inputTokens"),
                            cache_creation_input_tokens: u(m, "cacheCreationInputTokens"),
                            cache_read_input_tokens: u(m, "cacheReadInputTokens"),
                            output_tokens: u(m, "outputTokens"),
                            cost_microusd: microusd(m.get("costUSD")),
                        })
                        .collect();
                    facts.models.sort_by(|a, b| a.model.cmp(&b.model));
                }
            }
            _ => {}
        }
    }
    if facts.requests.is_empty() && !saw_result {
        return Err(
            "the transcript holds no request and no result: the provider reported nothing".into(),
        );
    }
    if !saw_result {
        facts.ended = "no_result".into();
        facts.is_error = true;
    }
    // an event that carried no usage names no request, and is not counted as one
    facts.orientation.requests = before_edit_requests
        .iter()
        .filter(|id| index.contains_key(*id))
        .count() as u64;
    facts.orientation.tokens = facts
        .requests
        .iter()
        .zip(index_ids(&index, facts.requests.len()))
        .filter(|(_, id)| before_edit_requests.contains(id))
        .map(|(r, _)| request_total(r))
        .sum();
    Ok(facts)
}

fn index_ids(index: &BTreeMap<String, usize>, n: usize) -> Vec<String> {
    let mut ids = vec![String::new(); n];
    for (id, &i) in index {
        ids[i] = id.clone();
    }
    ids
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A session's totals. The harness's per-model totals are preferred, because they include
/// the calls the harness makes on its own account (titles, summaries) that the transcript
/// does not show as messages; the per-request sum is used only when they are absent. `None`
/// when the session reported neither: absence stays absence.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsModelTotal, EconomicsSession};
/// use majordomus_cli::economics::usage::{anthropic_request, session_totals};
/// # let session = |requests| EconomicsSession {
/// #     index: 1, prompt_sha256: String::new(), duration_ms: 0, turns: None,
/// #     ended: "success".into(), is_error: false, requests, models: vec![],
/// #     reported_cost_microusd: None, tools: Default::default(), orientation: Default::default(),
/// # };
/// let usage = serde_json::json!({"input_tokens": 5, "cache_read_input_tokens": 90,
///     "output_tokens": 10});
/// let mut s = session(vec![anthropic_request("m", &usage)]);
/// assert_eq!(session_totals(&s).map(|t| (t.total_input, t.total)), Some((95, 105)));
/// s.models.push(EconomicsModelTotal { model: "m".into(), input_tokens: 25,
///     cache_creation_input_tokens: 0, cache_read_input_tokens: 90, output_tokens: 12,
///     cost_microusd: None });
/// assert_eq!(session_totals(&s).unwrap().total, 127, "the harness's totals win");
/// s.models.clear();
/// s.requests.clear();
/// assert_eq!(session_totals(&s), None, "absence stays absence");
/// ```
pub fn session_totals(s: &EconomicsSession) -> Option<EconomicsUsage> {
    let mut t = EconomicsUsage::default();
    if !s.models.is_empty() {
        for m in &s.models {
            t.input_uncached += m.input_tokens;
            t.cache_write += m.cache_creation_input_tokens;
            t.cache_read += m.cache_read_input_tokens;
            t.output += m.output_tokens;
        }
    } else if !s.requests.is_empty() {
        for r in &s.requests {
            t.input_uncached += r.input_tokens;
            t.cache_write += r.cache_creation_input_tokens;
            t.cache_read += r.cache_read_input_tokens;
            t.output += r.output_tokens;
        }
    } else {
        return None;
    }
    t.reasoning = s
        .requests
        .iter()
        .map(|r| r.thinking_tokens)
        .sum::<Option<u64>>()
        .filter(|_| !s.requests.is_empty());
    t.total_input = t.input_uncached + t.cache_write + t.cache_read;
    t.total = t.total_input + t.output;
    t.requests = s.requests.len() as u64;
    t.tool_calls = s.tools.values().sum();
    t.cost_microusd = s.reported_cost_microusd;
    Some(t)
}

/// A run's totals: the sum of its sessions'. `None` when any session reported nothing,
/// because a partial sum would understate the run.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsRun, EconomicsSession};
/// use majordomus_cli::economics::usage::{anthropic_request, totals};
/// # let mut run: EconomicsRun = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-run/v1", "id": "t--control--r1", "suite": "pilot",
/// #     "suite_version": 1, "methodology": 1, "task": "t", "variant": "control",
/// #     "repetition": 1, "provider": "anthropic",
/// #     "harness": {"name": "claude-code", "version": "0"}, "model_requested": "m",
/// #     "models_reported": ["m"], "repository": {"commit": "0", "dirty": false},
/// #     "majordomus_version": "0", "fixture_digest": "", "inputs_digest": "",
/// #     "configuration_digest": "", "started_at": "", "finished_at": "", "sessions": [],
/// #     "outcome": {"completed": true, "checks": [], "changed_files": []}
/// # })).unwrap();
/// # let session = |input: u64, output: u64| EconomicsSession {
/// #     index: 1, prompt_sha256: String::new(), duration_ms: 0, turns: None,
/// #     ended: "success".into(), is_error: false, models: vec![],
/// #     reported_cost_microusd: None, tools: Default::default(), orientation: Default::default(),
/// #     requests: vec![anthropic_request("m", &serde_json::json!({
/// #         "input_tokens": input, "output_tokens": output}))],
/// # };
/// // two sessions, of (input, output) tokens (100, 20) and (50, 10)
/// run.sessions = vec![session(100, 20), session(50, 10)];
/// assert_eq!(totals(&run).map(|t| (t.total, t.requests)), Some((180, 2)));
/// run.sessions[1].requests.clear();
/// assert_eq!(totals(&run), None, "one silent session voids the sum rather than shrinking it");
/// ```
pub fn totals(run: &EconomicsRun) -> Option<EconomicsUsage> {
    let mut out: Option<EconomicsUsage> = None;
    for s in &run.sessions {
        let t = session_totals(s)?;
        out = Some(match out {
            None => t,
            Some(acc) => add(&acc, &t),
        });
    }
    out
}

/// The field-wise sum of two totals. Reasoning and cost stay known only when both are: an
/// unknown on either side makes the sum unknown, never a smaller number.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsUsage;
/// use majordomus_cli::economics::usage::add;
/// let a = EconomicsUsage { total: 10, output: 2, cost_microusd: Some(40), ..Default::default() };
/// let b = EconomicsUsage { total: 5, output: 1, cost_microusd: None, ..Default::default() };
/// let sum = add(&a, &b);
/// assert_eq!((sum.total, sum.output), (15, 3));
/// assert_eq!(sum.cost_microusd, None, "a cost one side lacks is unknown for the sum");
/// assert_eq!(add(&a, &a).cost_microusd, Some(80));
/// ```
pub fn add(a: &EconomicsUsage, b: &EconomicsUsage) -> EconomicsUsage {
    EconomicsUsage {
        input_uncached: a.input_uncached + b.input_uncached,
        cache_write: a.cache_write + b.cache_write,
        cache_read: a.cache_read + b.cache_read,
        output: a.output + b.output,
        reasoning: a.reasoning.zip(b.reasoning).map(|(x, y)| x + y),
        total_input: a.total_input + b.total_input,
        total: a.total + b.total,
        requests: a.requests + b.requests,
        tool_calls: a.tool_calls + b.tool_calls,
        cost_microusd: a.cost_microusd.zip(b.cost_microusd).map(|(x, y)| x + y),
    }
}

/// The input tokens of a session's first request: everything the session was given before
/// the model had done anything (system prompt, instruction files, hook output, the prompt).
/// The three disjoint input fields are summed; the request's output is not input and is
/// left out. `None` when the session made no request.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsSession;
/// use majordomus_cli::economics::usage::{anthropic_request, first_request_input};
/// # let session = |requests| EconomicsSession {
/// #     index: 1, prompt_sha256: String::new(), duration_ms: 0, turns: None,
/// #     ended: "success".into(), is_error: false, requests, models: vec![],
/// #     reported_cost_microusd: None, tools: Default::default(), orientation: Default::default(),
/// # };
/// let first = serde_json::json!({"input_tokens": 4, "cache_creation_input_tokens": 12000,
///     "cache_read_input_tokens": 0, "output_tokens": 300});
/// let second = serde_json::json!({"input_tokens": 9, "cache_read_input_tokens": 12000});
/// let mut s = session(vec![anthropic_request("m", &first), anthropic_request("m", &second)]);
/// assert_eq!(first_request_input(&s), Some(12_004), "output and later requests excluded");
/// s.requests.clear();
/// assert_eq!(first_request_input(&s), None);
/// ```
pub fn first_request_input(s: &EconomicsSession) -> Option<u64> {
    s.requests
        .first()
        .map(|r| r.input_tokens + r.cache_creation_input_tokens + r.cache_read_input_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant(id: &str, blocks: &str, usage: (u64, u64, u64, u64)) -> String {
        format!(
            r#"{{"type":"assistant","message":{{"id":"{id}","model":"claude-sonnet-5","content":[{blocks}],"usage":{{"input_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{},"output_tokens":{}}}}}}}"#,
            usage.0, usage.1, usage.2, usage.3
        )
    }

    const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":4,"total_cost_usd":0.5,"modelUsage":{"claude-sonnet-5":{"inputTokens":6,"outputTokens":30,"cacheReadInputTokens":300,"cacheCreationInputTokens":200,"costUSD":0.45},"claude-haiku-4-5":{"inputTokens":50,"outputTokens":5,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"costUSD":0.05}}}"#;

    fn stream() -> String {
        [
            r#"{"type":"system","subtype":"init","tools":["Read","Edit","Bash"],"mcp_servers":[]}"#.to_string(),
            assistant("m1", r#"{"type":"text","text":"let me look"}"#, (2, 200, 0, 10)),
            assistant("m1", r#"{"type":"tool_use","name":"Read","input":{"file_path":"/repo/secret.env"}}"#, (2, 200, 0, 10)),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"API_KEY=sk-live-123"}]}}"#.to_string(),
            assistant("m2", r#"{"type":"tool_use","name":"Bash","input":{"command":"grep -rn vat ."}}"#, (2, 0, 150, 10)),
            assistant("m3", r#"{"type":"tool_use","name":"Edit","input":{"file_path":"a.py"}}"#, (2, 0, 150, 10)),
            RESULT.to_string(),
        ]
        .join("\n")
    }

    fn session(facts: StreamFacts) -> EconomicsSession {
        EconomicsSession {
            index: 1,
            prompt_sha256: String::new(),
            duration_ms: 0,
            turns: facts.turns,
            ended: facts.ended,
            is_error: facts.is_error,
            requests: facts.requests,
            models: facts.models,
            reported_cost_microusd: facts.reported_cost_microusd,
            tools: facts.tools,
            orientation: facts.orientation,
        }
    }

    #[test]
    fn a_message_split_across_content_blocks_is_one_request() {
        let f = read_stream(&stream()).unwrap();
        assert_eq!(f.requests.len(), 3, "m1 appears twice and is counted once");
        let summed: u64 = f.requests.iter().map(request_total).sum();
        assert_eq!(summed, 212 + 162 + 162);
    }

    #[test]
    fn orientation_ends_at_the_first_edit() {
        let f = read_stream(&stream()).unwrap();
        assert!(f.orientation.edited);
        assert_eq!(f.orientation.tool_calls, 2, "Read and Bash, not the Edit");
        assert_eq!(f.orientation.reads, 1);
        assert_eq!(
            f.orientation.searches, 1,
            "a Bash call leading with grep is a search"
        );
        assert_eq!(
            f.orientation.requests, 3,
            "the request that made the edit is included"
        );
    }

    #[test]
    fn an_edit_made_through_the_shell_ends_orientation_too() {
        let text = [
            assistant("m1", r#"{"type":"tool_use","name":"Bash","input":{"command":"cat ledgerlite/tax.py; grep -rn vat ."}}"#, (1, 1, 1, 1)),
            assistant("m2", r#"{"type":"tool_use","name":"Bash","input":{"command":"python3 - <<'EOF'\np='a.py'\ns=open(p).read()\nopen(p,\"w\").write(s)\nEOF"}}"#, (1, 1, 1, 1)),
            assistant("m3", r#"{"type":"tool_use","name":"Bash","input":{"command":"cat a.py"}}"#, (1, 1, 1, 1)),
            RESULT.to_string(),
        ]
        .join("\n");
        let f = read_stream(&text).unwrap();
        assert!(f.orientation.edited);
        assert_eq!(f.orientation.tool_calls, 1);
        assert_eq!(
            f.orientation.reads, 1,
            "a command that reads first is a read"
        );
        assert_eq!(f.orientation.requests, 2);
    }

    #[test]
    fn redirections_to_nowhere_are_not_edits() {
        for c in [
            "python3 -m unittest 2>&1 | tail -3",
            "ls > /dev/null",
            "git status 2>/dev/null",
            "echo hi >&2",
            "grep -rn 'long term ' docs",
        ] {
            assert_ne!(shell_effect(c), ShellEffect::Edit, "{c}");
        }
        for c in [
            "echo x > a.txt",
            "printf 'y' >>notes.md",
            "cat > tests/t.py <<'EOF'",
            "cd a && rm b.py",
            "mv a.py b.py",
        ] {
            assert_eq!(shell_effect(c), ShellEffect::Edit, "{c}");
        }
    }

    #[test]
    fn shell_syntax_inside_quotes_is_text() {
        for c in [
            r#"grep -rn "total > limit" ledgerlite"#,
            "grep -rn '>' docs",
            "grep -c ' >>x' notes",
            r#"echo "a; rm b.py""#,
            r#"git log --format="%h > %s" -3"#,
            r#"grep -n "say \"a > b\"" f"#,
            "grep 'unclosed > quote",
        ] {
            assert_ne!(shell_effect(c), ShellEffect::Edit, "{c}");
        }
        for c in [
            "echo done > 'notes.md'",
            r#"printf "%s" x >"out file.txt""#,
            "echo '>' > a.txt",
        ] {
            assert_eq!(shell_effect(c), ShellEffect::Edit, "{c}");
        }
        assert_eq!(unquoted(r#"a 'b > c' "d \" > e" f"#), "a _ _ f");
    }

    #[test]
    fn requests_without_a_message_id_are_neither_collapsed_nor_misattributed() {
        let anonymous = |blocks: &str, usage: (u64, u64, u64, u64)| {
            assistant("", blocks, usage).replace(r#""id":"","#, "")
        };
        let text = [
            anonymous(
                r#"{"type":"tool_use","name":"Read","input":{"file_path":"a.py"}}"#,
                (10, 0, 0, 1),
            ),
            anonymous(
                r#"{"type":"tool_use","name":"Edit","input":{"file_path":"a.py"}}"#,
                (20, 0, 0, 2),
            ),
            anonymous(r#"{"type":"text","text":"done"}"#, (40, 0, 0, 4)),
            assistant(
                "",
                r#"{"type":"text","text":"an empty id is no id"}"#,
                (80, 0, 0, 8),
            ),
            RESULT.to_string(),
        ]
        .join("\n");
        assert!(
            !text.lines().next().unwrap().contains(r#""id""#),
            "the first event has no id"
        );
        let f = read_stream(&text).unwrap();
        let inputs: Vec<u64> = f.requests.iter().map(|r| r.input_tokens).collect();
        assert_eq!(
            inputs,
            [10, 20, 40, 80],
            "four requests, none collapsed into another"
        );
        assert_eq!(
            f.orientation.requests, 2,
            "the read and the request that made the edit"
        );
        assert_eq!(
            f.orientation.tokens,
            11 + 22,
            "nothing after the edit is orientation"
        );
    }

    #[test]
    fn an_event_without_usage_is_not_an_orientation_request() {
        let text = [
            r#"{"type":"assistant","message":{"id":"m0","model":"m","content":[]}}"#.to_string(),
            assistant(
                "m1",
                r#"{"type":"tool_use","name":"Edit","input":{"file_path":"a.py"}}"#,
                (1, 0, 0, 1),
            ),
            RESULT.to_string(),
        ]
        .join("\n");
        let f = read_stream(&text).unwrap();
        assert_eq!(f.requests.len(), 1);
        assert_eq!(
            f.orientation.requests, 1,
            "m0 reported no usage and is no request"
        );
    }

    #[test]
    fn the_harness_totals_win_over_the_request_sum_because_they_include_its_own_calls() {
        let t = session_totals(&session(read_stream(&stream()).unwrap())).unwrap();
        assert_eq!(t.input_uncached, 56);
        assert_eq!(t.cache_write, 200);
        assert_eq!(t.cache_read, 300);
        assert_eq!(t.output, 35);
        assert_eq!(t.total_input, 556);
        assert_eq!(t.total, 591);
        assert_eq!(t.cost_microusd, Some(500_000));
        assert_eq!(t.tool_calls, 3);
    }

    #[test]
    fn nothing_a_session_said_or_read_survives_into_the_record() {
        let s = session(read_stream(&stream()).unwrap());
        let json = serde_json::to_string(&s).unwrap();
        for leaked in [
            "secret.env",
            "sk-live-123",
            "let me look",
            "grep -rn",
            "a.py",
        ] {
            assert!(
                !json.contains(leaked),
                "{leaked} leaked into the record: {json}"
            );
        }
    }

    #[test]
    fn a_silent_provider_is_an_error_not_zero_tokens() {
        assert!(read_stream("").is_err());
        assert!(read_stream("not json\n{\"type\":\"user\"}\n").is_err());
    }

    #[test]
    fn a_transcript_cut_off_before_its_result_is_recorded_as_an_error() {
        let cut = assistant("m1", r#"{"type":"text","text":"x"}"#, (1, 1, 1, 1));
        let f = read_stream(&cut).unwrap();
        assert_eq!(f.ended, "no_result");
        assert!(f.is_error);
    }

    #[test]
    fn a_session_with_no_usage_has_no_totals() {
        let s = session(StreamFacts::default());
        assert!(session_totals(&s).is_none());
    }

    #[test]
    fn reasoning_is_known_only_when_every_request_reported_it() {
        let mut s = session(read_stream(&stream()).unwrap());
        assert_eq!(session_totals(&s).unwrap().reasoning, None);
        for r in &mut s.requests {
            r.thinking_tokens = Some(4);
        }
        assert_eq!(session_totals(&s).unwrap().reasoning, Some(12));
    }
}

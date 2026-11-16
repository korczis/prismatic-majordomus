//! The `instances` module: live MCP instances that announce themselves in instance registries.
//!
//! An instance registry is a directory of owner-only JSON files, one per running instance,
//! each written by the instance itself when its MCP server starts and removed when it stops
//! (schema `mcp-instance/v1`: name, kind, owner_pid, registered, endpoint, the path of a 0600
//! token file, never the token). A cluster of instances started by different launchers is then
//! discoverable without any launcher knowing about discovery. This module reads such
//! directories; it writes and deletes nothing, because the records are the instances' own.
//!
//! It is neither the mesh nor a web surface. The mesh admits signed Majordomus nodes; a web
//! surface is a mount this executable serves. An instance here is a third-party MCP server on
//! this machine that said where it is.
//!
//! Three rules keep a record from being more than a pointer:
//! - a record that carries a token (`agent_token`, `token`) is refused and nothing of it is
//!   echoed, so the registry never widens who can call the instance;
//! - a stale record (its owner process is gone) is reported as stale and left in place;
//! - a probe (opt-in) reads the token from the record's token file in memory only, refuses an
//!   endpoint that is not loopback or that differs from the token file's own, and never puts
//!   the token in the answer.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::get;

/// The URI under which `instances.list` is read as an MCP resource.
pub const INSTANCES_URI: &str = "majordomus://instances";

/// The environment variable naming the registries, colon-separated.
pub const REGISTRIES_ENV: &str = "MAJORDOMUS_INSTANCE_REGISTRIES";

/// The record schema this module reads.
pub const RECORD_SCHEMA: &str = "mcp-instance/v1";

/// The answer's own schema.
pub const LIST_SCHEMA: &str = "instance-list/v1";

/// Fields a record may never carry: a registry points at credentials, it does not hold them.
const SECRET_FIELDS: [&str; 2] = ["agent_token", "token"];

/// How long a probe waits to connect and to read.
const PROBE_BUDGET: Duration = Duration::from_secs(2);

/// The most a probe reads of an answer.
const PROBE_READ_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
/// What to ask of `instances.list`.
pub struct InstancesInput {
    /// The registry directories to read; when empty, the colon-separated MAJORDOMUS_INSTANCE_REGISTRIES.
    #[serde(default)]
    pub registries: Vec<String>,
    /// Ping each instance's loopback MCP endpoint with the token from its token file (read in memory, never echoed).
    #[serde(default)]
    pub probe: bool,
}

impl BenchmarkCases for InstancesInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // The configured registries, unprobed: what a reader asks most, and it touches no network.
        vec![NamedCase::new("configured", InstancesInput::default())]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
/// Whether a record's owner is still running.
pub enum InstanceState {
    /// Its owner process exists.
    Live,
    /// Its owner process is gone; the record is left where it is, its owner's to remove.
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
/// Where a registry directory was named.
pub enum RegistrySource {
    /// The capability's input.
    Input,
    /// MAJORDOMUS_INSTANCE_REGISTRIES.
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One registry directory as read.
pub struct RegistryView {
    /// The directory.
    pub path: String,
    /// Where it was named.
    pub source: RegistrySource,
    /// Whether it exists; an absent registry is a fact, not an error.
    pub exists: bool,
    /// How many JSON files it holds.
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One instance, as its record says and as this machine sees it now.
pub struct Instance {
    /// The registry it was read from.
    pub registry: String,
    /// The record's file name.
    pub file: String,
    /// The instance's name, unique in its registry.
    pub name: String,
    /// What kind of process it is, as the record says (for example server, gui, panel).
    pub kind: String,
    /// Live or stale.
    pub state: InstanceState,
    /// The process that owns the record.
    pub owner_pid: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The record's schema, when it names one.
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One line saying what it is.
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When it registered, Unix seconds.
    pub registered: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Its MCP endpoint.
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The path of its token file (the file's contents are never read except by a probe, never returned).
    pub token_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A base URL, for a record that is not an MCP endpoint (a control panel).
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What wrote the record.
    pub producer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Every other field of the record, as written (never one of the secret fields, which refuse the record).
    pub extra: Option<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// With probe: whether the endpoint answered an MCP ping.
    pub reachable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// With probe: why it was not probed, or what the probe met.
    pub probe_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// A file of a registry that is not listed, and why.
pub struct Refused {
    /// The registry.
    pub registry: String,
    /// The file name.
    pub file: String,
    /// Why it is not listed.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Every instance of every registry the configuration names.
pub struct InstanceList {
    /// The answer's schema.
    pub schema: String,
    /// The registries read, in the order named.
    pub registries: Vec<RegistryView>,
    /// The instances, registry by registry, file name order within each.
    pub instances: Vec<Instance>,
    /// The files that are not listed, with the reason.
    pub refused: Vec<Refused>,
    /// Whether the instances were probed.
    pub probed: bool,
}

/// The registries to read: the input's, else the environment's, with where each was named.
fn registries(input: &InstancesInput, env: Option<String>) -> Vec<(PathBuf, RegistrySource)> {
    if !input.registries.is_empty() {
        return input
            .registries
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| (PathBuf::from(p), RegistrySource::Input))
            .collect();
    }
    env.unwrap_or_default()
        .split(':')
        .filter(|p| !p.is_empty())
        .map(|p| (PathBuf::from(p), RegistrySource::Environment))
        .collect()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// One record file into an instance, or the reason it is refused.
fn read_record(registry: &Path, file: &str, bytes: &[u8]) -> Result<Instance, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("not JSON: {e}"))?;
    let Some(obj) = v.as_object() else {
        return Err("not a JSON object".into());
    };
    if let Some(field) = SECRET_FIELDS.iter().find(|f| obj.contains_key(**f)) {
        return Err(format!(
            "carries a token ({field}): a registry record names its token file, never the token"
        ));
    }
    let name = text(&v, "name").ok_or("no name")?;
    let owner_pid = v
        .get("owner_pid")
        .and_then(Value::as_u64)
        .ok_or("no owner_pid")?;
    const KNOWN: [&str; 10] = [
        "schema",
        "name",
        "kind",
        "role",
        "owner_pid",
        "registered",
        "endpoint",
        "token_file",
        "url",
        "producer",
    ];
    let extra: BTreeMap<String, Value> = obj
        .iter()
        .filter(|(k, _)| !KNOWN.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let alive = u32::try_from(owner_pid).is_ok_and(crate::lease::alive);
    Ok(Instance {
        registry: registry.display().to_string(),
        file: file.to_string(),
        name,
        kind: text(&v, "kind").unwrap_or_else(|| "unknown".into()),
        state: if alive {
            InstanceState::Live
        } else {
            InstanceState::Stale
        },
        owner_pid,
        schema: text(&v, "schema"),
        role: text(&v, "role"),
        registered: v.get("registered").and_then(Value::as_f64),
        endpoint: text(&v, "endpoint"),
        token_file: text(&v, "token_file"),
        url: text(&v, "url"),
        producer: text(&v, "producer"),
        extra: (!extra.is_empty()).then_some(extra),
        reachable: None,
        probe_error: None,
    })
}

/// Read every registry: views, instances and refusals. Writes and deletes nothing.
fn read_registries(
    named: &[(PathBuf, RegistrySource)],
) -> (Vec<RegistryView>, Vec<Instance>, Vec<Refused>) {
    let (mut views, mut instances, mut refused) = (Vec::new(), Vec::new(), Vec::new());
    for (dir, source) in named {
        // File-name order through a BTreeMap: the directory's own order is the filesystem's.
        let mut files: BTreeMap<String, PathBuf> = BTreeMap::new();
        let exists = dir.is_dir();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") && !name.starts_with('.') {
                    files.insert(name, e.path());
                }
            }
        }
        views.push(RegistryView {
            path: dir.display().to_string(),
            source: *source,
            exists,
            files: files.len(),
        });
        for (file, path) in files {
            match std::fs::read(&path)
                .map_err(|e| format!("unreadable: {e}"))
                .and_then(|b| read_record(dir, &file, &b))
            {
                Ok(i) => instances.push(i),
                Err(reason) => refused.push(Refused {
                    registry: dir.display().to_string(),
                    file,
                    reason,
                }),
            }
        }
    }
    (views, instances, refused)
}

/// The socket address of a loopback `http://host:port/...` endpoint, and its path.
fn loopback(endpoint: &str) -> Result<(SocketAddr, String, String), String> {
    let rest = endpoint
        .strip_prefix("http://")
        .ok_or("not an http:// endpoint")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let (host, port) = authority.rsplit_once(':').ok_or("no port")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let ip: IpAddr = if host == "localhost" {
        IpAddr::from([127, 0, 0, 1])
    } else {
        host.parse().map_err(|_| "not an IP literal or localhost")?
    };
    if !ip.is_loopback() {
        return Err("not a loopback endpoint: a probe never sends a token off this machine".into());
    }
    let port: u16 = port.parse().map_err(|_| "not a port")?;
    Ok((SocketAddr::new(ip, port), authority.to_string(), path))
}

/// An MCP `ping` to a loopback endpoint with a bearer token: true when it answers a result.
fn ping(addr: SocketAddr, authority: &str, path: &str, token: &str) -> Result<bool, String> {
    let mut stream = TcpStream::connect_timeout(&addr, PROBE_BUDGET)
        .map_err(|e| format!("no answer: {}", e.kind()))?;
    let _ = stream.set_read_timeout(Some(PROBE_BUDGET));
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;
    let request = format!(
        "POST {path} HTTP/1.0\r\nHost: {authority}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("could not send: {}", e.kind()))?;
    let mut answer = String::new();
    let _ = stream.take(PROBE_READ_LIMIT).read_to_string(&mut answer);
    let status = answer.split_whitespace().nth(1).unwrap_or("");
    if !status.starts_with('2') {
        return Err(format!(
            "answered HTTP {}",
            if status.is_empty() { "nothing" } else { status }
        ));
    }
    Ok(answer.contains("\"result\""))
}

/// Probe one instance; the token stays in this function.
fn probe(i: &mut Instance) {
    let outcome = (|| -> Result<bool, String> {
        let endpoint = i.endpoint.as_deref().ok_or("no endpoint")?;
        let token_file = i.token_file.as_deref().ok_or("no token file")?;
        let (addr, authority, path) = loopback(endpoint)?;
        let cfg: Value = serde_json::from_slice(
            &std::fs::read(token_file)
                .map_err(|e| format!("token file unreadable: {}", e.kind()))?,
        )
        .map_err(|_| "token file is not JSON")?;
        if cfg.get("endpoint").and_then(Value::as_str) != Some(endpoint) {
            return Err("the token file names another endpoint: not probed".into());
        }
        let token = cfg
            .get("agent_token")
            .and_then(Value::as_str)
            .ok_or("token file has no agent_token")?;
        ping(addr, &authority, &path, token)
    })();
    match outcome {
        Ok(r) => i.reachable = Some(r),
        Err(e) => {
            // A loopback endpoint that was probed and did not answer is unreachable; one that
            // was not probed at all says why and claims nothing.
            if e.starts_with("no answer")
                || e.starts_with("answered")
                || e.starts_with("could not send")
            {
                i.reachable = Some(false);
            }
            i.probe_error = Some(e);
        }
    }
}

/// Every instance of every registry, from an explicit environment value (testable without
/// touching the process environment).
fn list(input: &InstancesInput, env: Option<String>) -> InstanceList {
    let named = registries(input, env);
    let (registries, mut instances, refused) = read_registries(&named);
    if input.probe {
        for i in instances
            .iter_mut()
            .filter(|i| i.state == InstanceState::Live)
        {
            probe(i);
        }
    }
    InstanceList {
        schema: LIST_SCHEMA.into(),
        registries,
        instances,
        refused,
        probed: input.probe,
    }
}

fn instances_list(_ctx: &Context, input: InstancesInput) -> Result<InstanceList, CapabilityError> {
    Ok(list(&input, std::env::var(REGISTRIES_ENV).ok()))
}

/// The `instances` module: live MCP instances from instance registries.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "instances",
        title: "MCP instances from instance registries",
        description: "Third-party MCP servers on this machine that announce themselves by writing a record (mcp-instance/v1) to a shared registry directory: listed with whether their owner is alive, refused when a record is malformed or carries a token, never deleted, and probed with an MCP ping only on request and only on loopback.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "instances.list",
                title: "MCP instances from instance registries",
                description: "Every record of every registry directory named by the input's registries or MAJORDOMUS_INSTANCE_REGISTRIES (colon-separated): name, kind, owner pid and whether it is alive (live or stale; a stale record is left in place), endpoint, the path of its token file, and the record's other fields. A malformed file, or a record that carries agent_token or token, is refused with the reason and nothing of it is echoed. With probe, each live instance's loopback endpoint is sent an MCP ping with the token read from its token file in memory; a non-loopback endpoint or one that differs from the token file's is not probed. Writes nothing.",
                input: InstancesInput,
                output: InstanceList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_instances".into()),
                        resource: Some(McpResource { uri: INSTANCES_URI.into(), name: "instances".into() }),
                    }),
                    http: get("/api/v1/instances"),
                    cli: None,
                },
                tags: ["instances", "discovery", "mcp", "introspection"],
                handler: instances_list,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn record(dir: &Path, file: &str, body: &str) {
        std::fs::write(dir.join(file), body).unwrap();
    }

    fn gone_pid() -> u64 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        u64::from(pid)
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "instances");
        let exposure = &m.capabilities[0].capability.exposure;
        let mcp = exposure.mcp.as_ref().expect("an MCP exposure");
        assert_eq!(mcp.tool.as_deref(), Some("majordomus_instances"));
        assert_eq!(
            mcp.resource.as_ref().map(|r| r.uri.as_str()),
            Some(INSTANCES_URI)
        );
        assert_eq!(
            exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/instances")
        );
    }

    #[test]
    fn live_stale_and_refused_records_are_told_apart_and_nothing_is_deleted() {
        let t = tempfile::tempdir().unwrap();
        let me = std::process::id();
        record(
            t.path(),
            "a.json",
            &format!(
                r#"{{"schema":"mcp-instance/v1","name":"a","kind":"server","owner_pid":{me},"registered":1,"endpoint":"http://127.0.0.1:9/mcp","game_port":3979}}"#
            ),
        );
        record(
            t.path(),
            "b.json",
            &format!(r#"{{"name":"b","kind":"gui","owner_pid":{}}}"#, gone_pid()),
        );
        record(t.path(), "c.json", r#"{"name": "#);
        record(
            t.path(),
            "d.json",
            &format!(r#"{{"name":"d","owner_pid":{me},"agent_token":"SECRET-d"}}"#),
        );
        record(t.path(), ".e.tmp.json", "half a record");
        let input = InstancesInput {
            registries: vec![t.path().display().to_string()],
            probe: false,
        };
        let out = list(&input, None);
        let names: Vec<(&str, InstanceState)> = out
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i.state))
            .collect();
        assert_eq!(
            names,
            [("a", InstanceState::Live), ("b", InstanceState::Stale)]
        );
        let refused: Vec<&str> = out.refused.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(refused, ["c.json", "d.json"]);
        assert!(out.refused[1].reason.starts_with("carries a token"));
        assert_eq!(
            out.instances[0]
                .extra
                .as_ref()
                .and_then(|e| e.get("game_port")),
            Some(&serde_json::json!(3979))
        );
        let text = serde_json::to_string(&out).unwrap();
        assert!(
            !text.contains("SECRET-d"),
            "a refused record's token is echoed: {text}"
        );
        assert!(
            t.path().join("b.json").exists(),
            "a stale record was deleted"
        );
        assert_eq!(
            out.registries[0].files, 4,
            "a dot-file is a write in progress, not a record"
        );
    }

    #[test]
    fn the_input_names_the_registries_before_the_environment_does() {
        let input = InstancesInput {
            registries: vec!["/x".into()],
            probe: false,
        };
        let r = registries(&input, Some("/a::/b".into()));
        assert_eq!(r, [(PathBuf::from("/x"), RegistrySource::Input)]);
        let r = registries(&InstancesInput::default(), Some("/a::/b".into()));
        assert_eq!(
            r,
            [
                (PathBuf::from("/a"), RegistrySource::Environment),
                (PathBuf::from("/b"), RegistrySource::Environment)
            ]
        );
        let out = list(
            &InstancesInput {
                registries: vec!["/no/such/registry".into()],
                probe: false,
            },
            None,
        );
        assert!(!out.registries[0].exists && out.instances.is_empty() && out.refused.is_empty());
    }

    #[test]
    fn only_loopback_endpoints_are_probed() {
        assert!(loopback("http://127.0.0.1:8731/mcp").is_ok());
        assert!(loopback("http://[::1]:8731/mcp").is_ok());
        assert!(loopback("http://localhost:8731/mcp").is_ok());
        assert!(loopback("http://192.0.2.1:8731/mcp")
            .unwrap_err()
            .starts_with("not a loopback endpoint"));
        assert!(
            loopback("http://example.com:80/mcp").is_err(),
            "a name is never resolved"
        );
        assert!(loopback("https://127.0.0.1:1/mcp").is_err());
    }

    #[test]
    fn a_probe_pings_with_the_token_and_never_returns_it() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let ok = req.contains("Authorization: Bearer TOKEN-4621")
                && req.contains("\"method\":\"ping\"");
            let body = if ok {
                r#"{"jsonrpc":"2.0","id":1,"result":{}}"#
            } else {
                r#"{"jsonrpc":"2.0","id":1,"error":{}}"#
            };
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        let t = tempfile::tempdir().unwrap();
        let endpoint = format!("http://127.0.0.1:{port}/mcp");
        let token = t.path().join("token.json");
        std::fs::write(
            &token,
            format!(r#"{{"agent_token":"TOKEN-4621","endpoint":"{endpoint}"}}"#),
        )
        .unwrap();
        record(
            t.path(),
            "live.json",
            &format!(
                r#"{{"name":"live","owner_pid":{},"endpoint":"{endpoint}","token_file":"{}"}}"#,
                std::process::id(),
                token.display()
            ),
        );
        let out = list(
            &InstancesInput {
                registries: vec![t.path().display().to_string()],
                probe: true,
            },
            None,
        );
        server.join().unwrap();
        assert_eq!(
            out.instances[0].reachable,
            Some(true),
            "{:?}",
            out.instances[0].probe_error
        );
        assert!(!serde_json::to_string(&out).unwrap().contains("TOKEN-4621"));
    }

    #[test]
    fn a_token_file_naming_another_endpoint_is_not_used() {
        let t = tempfile::tempdir().unwrap();
        let token = t.path().join("token.json");
        std::fs::write(
            &token,
            r#"{"agent_token":"T","endpoint":"http://127.0.0.1:1/other"}"#,
        )
        .unwrap();
        record(
            t.path(),
            "x.json",
            &format!(
                r#"{{"name":"x","owner_pid":{},"endpoint":"http://127.0.0.1:1/mcp","token_file":"{}"}}"#,
                std::process::id(),
                token.display()
            ),
        );
        let out = list(
            &InstancesInput {
                registries: vec![t.path().display().to_string()],
                probe: true,
            },
            None,
        );
        assert_eq!(out.instances[0].reachable, None);
        assert_eq!(
            out.instances[0].probe_error.as_deref(),
            Some("the token file names another endpoint: not probed")
        );
    }
}

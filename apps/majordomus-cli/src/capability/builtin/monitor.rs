//! The `monitor` module: everything that happens around this repository, followed in one
//! stream (ADR 0133). The handler gathers each source's read and [`crate::monitor`] folds and
//! pages them; nothing here decides what a signal is.

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::monitor::{
    assemble, executor, github, mesh, Cursor, MonitorPage, MonitorSource, MonitorSourceHealth,
    Reader, Reads, SourceRead,
};
use crate::{capability, module};

use super::{get, mcp};

/// How many signals a page carries when the caller does not say.
pub(crate) const DEFAULT_LIMIT: u64 = 50;

/// The most signals one page carries.
pub(crate) const MAX_LIMIT: u64 = 500;

/// How far back a first page reaches when the caller does not say, in seconds.
pub(crate) const DEFAULT_SINCE: u64 = 3600;

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `monitor`: where the reader stands, how much it wants, from which sources,
/// and who it is.
pub struct MonitorInput {
    /// The `cursor` of the previous page; without it, the page is the newest signals of the
    /// last `since` seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// At most this many signals (default 50, at most 500).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// A comma-separated subset of `mesh`, `executor` and `github` (default: all three).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<String>,
    /// Comma-separated names of the reader — a worker (`1b`), a role, a login — so that what
    /// is addressed to it is marked `for_you`. An MCP caller is also its own board session.
    #[serde(default, rename = "for", skip_serializing_if = "Option::is_none")]
    pub reader: Option<String>,
    /// How far back a first page reaches, in seconds (default 3600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
}

impl BenchmarkCases for MonitorInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        cases()
    }
}

/// The benchmark cases: every parameter has an example, and GitHub — the network — is never
/// asked, so a benchmark measures this machine.
fn cases() -> Vec<NamedCase<MonitorInput>> {
    {
        vec![
            NamedCase::new(
                "local-first-page",
                MonitorInput {
                    after: None,
                    limit: Some(50),
                    sources: Some("mesh,executor".into()),
                    reader: Some("1b".into()),
                    since: Some(DEFAULT_SINCE),
                },
            ),
            NamedCase::new(
                "local-resumed",
                MonitorInput {
                    after: Some("x0".into()),
                    limit: Some(20),
                    sources: Some("mesh,executor".into()),
                    reader: Some("1b,cd".into()),
                    since: Some(600),
                },
            ),
        ]
    }
}

/// The sources a comma list names; every name must be one.
pub(crate) fn wanted(list: Option<&str>) -> Result<Vec<MonitorSource>, CapabilityError> {
    let Some(list) = list.filter(|l| !l.trim().is_empty()) else {
        return Ok(MonitorSource::ALL.to_vec());
    };
    list.split(',')
        .filter(|n| !n.trim().is_empty())
        .map(|name| {
            MonitorSource::parse(name).ok_or_else(|| {
                CapabilityError::InvalidInput(format!(
                    "unknown source {:?}: the sources are mesh, executor and github",
                    name.trim()
                ))
            })
        })
        .collect()
}

/// The runtime key (`<node>-<runtime>`) of a stream (`<node>-<runtime>-<instance>`): what a
/// review names as its reviewer.
pub(crate) fn runtime_of(stream: &str) -> String {
    stream.rsplit_once('-').map_or(stream, |(runtime, _)| runtime).to_string()
}

fn skipped<P>() -> Result<SourceRead<P>, (MonitorSourceHealth, String)> {
    Err((MonitorSourceHealth::Skipped, "not asked for".into()))
}

fn monitor(ctx: &Context, input: MonitorInput) -> Result<MonitorPage, CapabilityError> {
    let previous = match input.after.as_deref() {
        Some(token) => Cursor::parse(token).map_err(CapabilityError::InvalidInput)?,
        None => Cursor::default(),
    };
    let resuming = input.after.is_some();
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
    let sources = wanted(input.sources.as_deref())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let since = now.saturating_sub(input.since.unwrap_or(DEFAULT_SINCE));
    let mut reader = Reader::named(input.reader.as_deref());
    if let Some(peer) = &ctx.caller {
        reader.sessions.push(format!("board-{}", peer.as_str()));
    }
    let root = Path::new(&ctx.index.repository.root);

    let mesh = if !sources.contains(&MonitorSource::Mesh) {
        skipped()
    } else if let Some(c) = ctx.mesh.cooperation() {
        // a review addressed to this runtime is addressed to whoever reads here
        reader
            .sessions
            .push(runtime_of(c.journal().own_stream().as_str()));
        let events = c.journal().events_after(0, usize::MAX);
        let machines: HashMap<String, String> = ctx
            .mesh
            .nodes()
            .into_iter()
            .map(|n| (n.node_id.as_str().to_string(), n.display_name))
            .collect();
        let fold_ctx = mesh::Context {
            reader: &reader,
            machines: &machines,
            since,
        };
        Ok(mesh::fold(&events, previous.mesh.as_ref(), &fold_ctx))
    } else {
        Err((
            MonitorSourceHealth::Absent,
            ctx.mesh
                .cooperation_status()
                .reason
                .unwrap_or_else(|| "the mesh is not running in this server".into()),
        ))
    };

    let executor = if !sources.contains(&MonitorSource::Executor) {
        skipped()
    } else {
        let trail = crate::integration::drain::events(root);
        if trail.is_empty() {
            Err((
                MonitorSourceHealth::Absent,
                "the executor has recorded nothing in this repository".into(),
            ))
        } else {
            Ok(executor::fold(&trail, previous.executor, since, &reader))
        }
    };

    let github = if !sources.contains(&MonitorSource::Github) {
        skipped()
    } else {
        github::fetch(root).map(|f| github::fold(&f, previous.github, since, &reader))
    };

    Ok(assemble(
        Reads {
            mesh,
            executor,
            github,
        },
        &previous,
        resuming,
        limit,
    ))
}

/// The `monitor` module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "monitor",
        title: "The monitor",
        description: "Everything that happens around this repository, followed in one stream: the mesh journal (sessions on every linked machine, this checkout's board among them, their claims, handovers and reviews), the executor's trail (merges, refreshes, repairs, closures, failures) and GitHub (pull requests opened, merged and closed, finished workflow runs), folded so that a session re-announcing itself unchanged and a step that decided nothing are counted rather than shown, with what is addressed to the reader marked (ADR 0133).",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "monitor.stream",
                title: "Follow everything in one stream",
                description: "A page of signals after a cursor, oldest first, or without one the newest of the last `since` seconds: each with its time, source, kind, severity, machine, session, actor, pull request, branch, issue, paths, a one-line summary and whether it is for the reader. Re-projections and undecided steps are counted as `suppressed`; a source that cannot be read is listed with its reason, never answered as empty. The page's `cursor` is the next page's `after`, and nothing on this page comes back. Reads only.",
                input: MonitorInput,
                output: MonitorPage,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_monitor"), http: get("/api/v1/monitor"), cli: None },
                tags: ["monitor", "mesh", "coordination", "observability"],
                handler: monitor,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_all_by_default_and_an_unknown_one_is_refused() {
        assert_eq!(wanted(None).unwrap(), MonitorSource::ALL.to_vec());
        assert_eq!(wanted(Some(" ")).unwrap(), MonitorSource::ALL.to_vec());
        assert_eq!(
            wanted(Some("mesh, github")).unwrap(),
            vec![MonitorSource::Mesh, MonitorSource::Github]
        );
        let e = wanted(Some("mesh,slack")).unwrap_err();
        assert!(e.to_string().contains("unknown source \"slack\""), "{e}");
    }

    #[test]
    fn a_runtime_is_its_stream_without_the_instance() {
        assert_eq!(runtime_of("aa-bb-cc"), "aa-bb");
        assert_eq!(runtime_of("plain"), "plain");
    }

    #[test]
    fn the_benchmark_cases_never_reach_the_network() {
        let cases = cases();
        assert_eq!(cases.len(), 2);
        for case in cases {
            let sources = wanted(case.input.sources.as_deref()).unwrap();
            assert!(!sources.contains(&MonitorSource::Github), "{}", case.name);
        }
    }
}

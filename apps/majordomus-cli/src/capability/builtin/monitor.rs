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
    stream
        .rsplit_once('-')
        .map_or(stream, |(runtime, _)| runtime)
        .to_string()
}

/// Why the mesh source is absent when the mesh runtime gives no reason of its own.
const NOT_RUNNING: &str = "the mesh is not running in this server";

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
        let mut machines: HashMap<String, String> = ctx
            .mesh
            .nodes()
            .into_iter()
            .map(|n| (n.node_id.as_str().to_string(), n.display_name))
            .collect();
        // the registry holds the nodes this one has heard, never itself
        if let Some(me) = ctx.mesh.status().identity {
            machines.insert(me.node_id.as_str().to_string(), me.display_name);
        }
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
                .unwrap_or(NOT_RUNNING.to_string()),
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
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
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

    use std::sync::Arc;

    use serde_json::json;

    use crate::monitor::github::{self, Fetched};
    use crate::monitor::testing::{context, cooperation, session};

    fn input(sources: &str) -> MonitorInput {
        MonitorInput {
            after: None,
            limit: None,
            sources: Some(sources.into()),
            reader: Some("1b".into()),
            since: Some(10 * 365 * 86_400),
        }
    }

    fn state(page: &MonitorPage, source: MonitorSource) -> (MonitorSourceHealth, Option<String>) {
        let s = page.sources.iter().find(|s| s.source == source).unwrap();
        (s.state, s.reason.clone())
    }

    /// Every source answers through the capability: the mesh from the cooperation's journal,
    /// the executor from the trail in the repository, GitHub from its fetch. What names the
    /// reader is for them, and the page resumes from its cursor with nothing repeated.
    #[test]
    fn every_source_answers_through_the_capability_and_a_page_resumes() {
        let _cache = github::CACHE_TESTS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = context(&repo);
        let c = cooperation();
        session(&c, "s1", "3d", "the runtime train");
        c.request_review("s1", "1b, read #885".into(), vec![], None, None, None)
            .unwrap();
        ctx.mesh.attach_cooperation(Arc::clone(&c));
        let trail = crate::integration::events_path(repo.root()).unwrap();
        std::fs::create_dir_all(trail.parent().unwrap()).unwrap();
        std::fs::write(
            &trail,
            format!(
                "{}\n",
                json!({"at": "2026-10-10T20:00:00Z", "actor": "cd (pid 7 on lundra)",
                       "action": "merge_succeeded", "pr": 9, "master_before": null,
                       "head_sha": null, "master_after": null, "reasons": [], "detail": ""})
            ),
        )
        .unwrap();
        // the context's root is the canonical one (/private/var, not /var, on macOS)
        github::seed(
            Path::new(&ctx.index.repository.root),
            Fetched {
                prs: json!([{"number": 5, "title": "1b's fix", "state": "OPEN",
                             "createdAt": "2026-10-10T20:05:00Z", "headRefName": "fix/x",
                             "author": {"login": "korczis"}}]),
                runs: json!([]),
            },
        );

        let page = monitor(&ctx, input("mesh,executor,github")).unwrap();
        for source in MonitorSource::ALL {
            assert_eq!(
                state(&page, source).0,
                MonitorSourceHealth::Ok,
                "{source:?}"
            );
        }
        let kinds: Vec<&str> = page.signals.iter().map(|s| s.kind.as_str()).collect();
        for kind in ["session_joined", "review_requested", "merged", "pr_opened"] {
            assert!(kinds.contains(&kind), "{kind} is missing from {kinds:?}");
        }
        assert_eq!(
            page.for_you, 2,
            "the review and the pull request name 1b: {kinds:?}"
        );

        let mut next = input("mesh,executor,github");
        next.after = Some(page.cursor.clone());
        let resumed = monitor(&ctx, next).unwrap();
        assert!(resumed.signals.is_empty(), "{:?}", resumed.signals);
    }

    /// A source not asked for is skipped, a mesh not running is absent with its reason, an
    /// empty trail is absent, and a bad cursor or source is refused as invalid input.
    #[test]
    fn what_cannot_answer_says_why_and_bad_input_is_refused() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = context(&repo);
        let page = monitor(&ctx, input("mesh,executor")).unwrap();
        let (mesh, reason) = state(&page, MonitorSource::Mesh);
        assert_eq!(mesh, MonitorSourceHealth::Absent);
        assert!(
            reason
                .as_deref()
                .is_some_and(|r| r.starts_with("not active")),
            "the mesh runtime's own reason: {reason:?}"
        );
        assert_eq!(
            state(&page, MonitorSource::Executor).0,
            MonitorSourceHealth::Absent
        );
        assert_eq!(
            state(&page, MonitorSource::Github).0,
            MonitorSourceHealth::Skipped
        );

        let mut bad = input("mesh");
        bad.after = Some("m12".into());
        assert!(matches!(
            monitor(&ctx, bad),
            Err(CapabilityError::InvalidInput(_))
        ));
        assert!(matches!(
            monitor(&ctx, input("mesh,slack")),
            Err(CapabilityError::InvalidInput(_))
        ));
    }

    /// A mesh that is active and has heard another node names both machines, this one
    /// included, and a source not asked for is skipped whichever it is.
    #[test]
    fn the_machines_are_the_nodes_heard_and_this_one_and_what_is_not_asked_is_skipped() {
        use crate::mesh::config::MeshConfig;
        use crate::mesh::identity::NodeIdentity;
        use crate::mesh::protocol::advertise;

        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = context(&repo);
        // this machine's own key is listed: a machine its trust list does not name keeps its
        // mesh closed and hears nobody
        let this = NodeIdentity::ephemeral().unwrap();
        let config: MeshConfig = serde_json::from_value(json!({
            "schema": "mesh/v1", "kind": "mesh", "id": "majordomus", "enabled": true,
            "multicast": { "enabled": false },
            "trust": { "allow": [this.public.public_key.clone()] },
        }))
        .unwrap();
        ctx.mesh
            .activate_as(
                &config,
                Arc::new(this),
                "0000000000000001",
                vec![],
                vec!["root".into()],
                "test",
            )
            .unwrap();
        let other = NodeIdentity::ephemeral().unwrap();
        let envelope = advertise(&other, 1, &["127.0.0.1:1".into()], &[], &[], "test");
        ctx.mesh
            .register(&serde_json::to_value(&envelope).unwrap(), "test");
        assert_eq!(ctx.mesh.nodes().len(), 1, "the other node was heard");
        let c = cooperation();
        session(&c, "s1", "3d", "the runtime train");
        ctx.mesh.attach_cooperation(Arc::clone(&c));

        let page = monitor(&ctx, input("mesh")).unwrap();
        assert_eq!(state(&page, MonitorSource::Mesh).0, MonitorSourceHealth::Ok);
        assert_eq!(
            state(&page, MonitorSource::Executor).0,
            MonitorSourceHealth::Skipped
        );
        let page = monitor(&ctx, input("executor")).unwrap();
        assert_eq!(
            state(&page, MonitorSource::Mesh).0,
            MonitorSourceHealth::Skipped
        );
        ctx.mesh.stop();
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

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
use crate::monitor::stale::{self, Listing, RunStanding, RunVerdict};
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

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `monitor.stale-runs`: which runs to judge, and against what.
pub struct StaleRunsInput {
    /// Judge only the runs of this branch (default: every run in flight).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// A repository-relative JSON file holding a recorded listing — `runs`, `pull_requests`
    /// and `heads` — to judge in place of asking the forge: what a test, an example and a
    /// second reader of one verdict use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

impl BenchmarkCases for StaleRunsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a recorded listing, so that a benchmark never asks the network
        vec![
            NamedCase::new(
                "recorded",
                StaleRunsInput {
                    branch: None,
                    snapshot: Some(STALE_RUNS_EXAMPLE.into()),
                },
            ),
            NamedCase::new(
                "recorded-one-branch",
                StaleRunsInput {
                    branch: Some("master".into()),
                    snapshot: Some(STALE_RUNS_EXAMPLE.into()),
                },
            ),
        ]
    }
}

/// The recorded listing the examples and the benchmark judge.
pub(crate) const STALE_RUNS_EXAMPLE: &str = "test/fixtures/stale-runs/listing.json";

/// Every run in flight with its verdict: what `monitor.stale-runs` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StaleRuns {
    /// Whether the listing could be read.
    pub state: MonitorSourceHealth,
    /// Why not, when it could not: never an empty answer that reads as "nothing is stale".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How many runs in flight may be cancelled.
    pub stale: usize,
    /// How many stay.
    pub kept: usize,
    /// Every run in flight, stale ones first, each in the order the forge listed it.
    pub runs: Vec<RunVerdict>,
}

/// The recorded listing at `path` under `root`, or why it cannot be judged.
fn recorded(root: &Path, path: &str) -> Result<Listing, (MonitorSourceHealth, String)> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err((
            MonitorSourceHealth::Error,
            format!("{path} is not a path inside the repository"),
        ));
    }
    let text = std::fs::read_to_string(root.join(relative)).map_err(|e| {
        (
            MonitorSourceHealth::Absent,
            format!("no recorded listing at {path}: {e}"),
        )
    })?;
    serde_json::from_str(&text).map_err(|e| {
        (
            MonitorSourceHealth::Error,
            format!("{path} is not a recorded listing: {e}"),
        )
    })
}

fn stale_runs(ctx: &Context, input: StaleRunsInput) -> Result<StaleRuns, CapabilityError> {
    let root = Path::new(&ctx.index.repository.root);
    let listing = match input.snapshot.as_deref() {
        Some(path) => recorded(root, path),
        None => stale::fetch(root),
    };
    let listing = match listing {
        Ok(listing) => listing,
        Err((state, reason)) => {
            return Ok(StaleRuns {
                state,
                reason: Some(reason),
                stale: 0,
                kept: 0,
                runs: Vec::new(),
            })
        }
    };
    let mut runs: Vec<RunVerdict> = stale::judge(&listing)
        .into_iter()
        .filter(|v| input.branch.as_deref().is_none_or(|b| v.branch == b))
        .collect();
    // stable: within each standing the forge's order stays
    runs.sort_by_key(|v| v.standing != RunStanding::Stale);
    let stale = runs
        .iter()
        .filter(|v| v.standing == RunStanding::Stale)
        .count();
    Ok(StaleRuns {
        state: MonitorSourceHealth::Ok,
        reason: None,
        stale,
        kept: runs.len() - stale,
        runs,
    })
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
            capability! {
                id: "monitor.stale-runs",
                title: "Judge which workflow runs in flight nobody will read",
                description: "Every workflow run in flight with one verdict and its reason, stale ones first. A run is stale when the pull request of its branch is merged or closed, when its commit is no longer the head of its branch, when its branch is gone, or when an older run of the same workflow already judges the same commit; a stale run carries the command that cancels it. A release run, a deployment in flight, the trunk's only run of a commit, a run somebody dispatched and a run on a current head are kept, and so is a run whose branch could not be read: not knowing never cancels. A forge that cannot be asked is a state with its reason, never an empty answer. Cancels nothing (project.an-expedited-landing-is-the-owners-word, clause 7).",
                input: StaleRunsInput,
                output: StaleRuns,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_monitor_stale_runs"), http: get("/api/v1/monitor/stale-runs"), cli: None },
                tags: ["monitor", "delivery", "ci", "coordination"],
                handler: stale_runs,
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

    /// The recorded listing of the repository, through the capability: the stale runs come
    /// first with their commands, the kept ones carry none, one branch can be asked for, and
    /// a listing that is not there or not inside the repository is a state with its reason.
    #[test]
    fn stale_runs_are_judged_from_a_recorded_listing() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = crate::monitor::testing::context(&repo);
        let root = Path::new(&ctx.index.repository.root);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(STALE_RUNS_EXAMPLE);
        let at = root.join(STALE_RUNS_EXAMPLE);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::copy(fixture, &at).unwrap();

        let ask = |branch: Option<&str>, snapshot: &str| {
            stale_runs(
                &ctx,
                StaleRunsInput {
                    branch: branch.map(str::to_string),
                    snapshot: Some(snapshot.into()),
                },
            )
            .unwrap()
        };
        let all = ask(None, STALE_RUNS_EXAMPLE);
        assert_eq!(all.state, MonitorSourceHealth::Ok);
        assert_eq!((all.stale, all.kept), (3, 4), "{all:?}");
        assert!(all.runs[..3]
            .iter()
            .all(|v| v.standing == RunStanding::Stale && v.cancel.is_some()));
        assert!(all.runs[3..]
            .iter()
            .all(|v| v.standing == RunStanding::Kept && v.cancel.is_none()));
        assert_eq!(all.runs[0].cancel.as_deref(), Some("gh run cancel 38392595807"));

        let trunk = ask(Some("master"), STALE_RUNS_EXAMPLE);
        assert_eq!((trunk.stale, trunk.kept), (1, 3), "{trunk:?}");
        assert!(trunk.runs.iter().all(|v| v.branch == "master"));

        let absent = ask(None, "test/fixtures/stale-runs/none.json");
        assert_eq!(absent.state, MonitorSourceHealth::Absent);
        assert!(absent.reason.unwrap().contains("no recorded listing"));
        assert!(absent.runs.is_empty());
        let outside = ask(None, "../outside.json");
        assert_eq!(outside.state, MonitorSourceHealth::Error);
        assert!(outside.reason.unwrap().contains("not a path inside"));
        std::fs::write(&at, "[").unwrap();
        let broken = ask(None, STALE_RUNS_EXAMPLE);
        assert_eq!(broken.state, MonitorSourceHealth::Error);
        assert!(broken.reason.unwrap().contains("not a recorded listing"));
        assert_eq!(StaleRunsInput::benchmark_cases(&CaseContext::of(&ctx)).len(), 2);
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

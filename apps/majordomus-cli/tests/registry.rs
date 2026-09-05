//! The registry's invariants: one canonical identity, explicit collisions, explicit
//! refusals, every party's provenance named, and the same registry twice.

mod common;

use majordomus_cli::capability::handler::handler;
use majordomus_cli::capability::{
    CanonicalSchema, Capability, CapabilityId, CapabilityKind, CapabilityRegistry, CliExposure,
    Executable, Exposure, HttpExposure, HttpMethod, McpExposure, McpResource, Provenance,
    RegistryError, Stability,
};
use majordomus_cli::discovery::{Sources, VcsIndex};
use majordomus_cli::git::GitState;
use majordomus_cli::metadata::KindSchema;
use majordomus_cli::{Index, Repository};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct In {
    #[allow(dead_code)]
    name: String,
}
#[derive(Serialize, JsonSchema)]
struct Out {
    echo: String,
}

fn query(id: &str, exposure: Exposure, stability: Stability, module: &str) -> Executable {
    Executable {
        capability: Capability {
            id: CapabilityId::parse(id).unwrap_or_else(|_| unchecked(id)),
            kind: CapabilityKind::Query,
            title: format!("Fixture {id}"),
            description: "A fixture capability.".into(),
            input: CanonicalSchema::of::<In>(),
            output: CanonicalSchema::of::<Out>(),
            provenance: Provenance::Builtin {
                module: module.into(),
            },
            exposure,
            stability,
            tags: vec![],
        },
        handler: handler::<serde_json::Value, Out, _>(|_, v| {
            Ok(Out {
                echo: v.to_string(),
            })
        }),
    }
}

/// Bypass validation the way a descriptor written in code does: the registry validates.
fn unchecked(id: &str) -> CapabilityId {
    serde_json::from_value(json!(id)).unwrap()
}

fn tool(name: &str) -> Exposure {
    Exposure {
        mcp: Some(McpExposure {
            tool: Some(name.into()),
            resource: None,
        }),
        http: None,
        cli: None,
    }
}
fn route(path: &str) -> Exposure {
    Exposure {
        mcp: None,
        http: Some(HttpExposure {
            method: HttpMethod::Get,
            path: path.into(),
        }),
        cli: None,
    }
}

fn errors_of(execs: Vec<Executable>) -> Vec<RegistryError> {
    CapabilityRegistry::builder()
        .with_builtin(execs)
        .build()
        .expect_err("the registry refuses")
}

#[test]
fn a_valid_set_builds_and_iterates_by_id() {
    let r = CapabilityRegistry::builder()
        .with_builtin(vec![
            query("zeta.one", tool("zeta_one"), Stability::Implemented, "m"),
            query(
                "alpha.one",
                route("/api/v1/alpha"),
                Stability::Implemented,
                "m",
            ),
        ])
        .build()
        .unwrap();
    assert_eq!(
        r.iter().map(|c| c.id.to_string()).collect::<Vec<_>>(),
        ["alpha.one", "zeta.one"]
    );
    assert_eq!(r.by_mcp_tool("zeta_one").unwrap().id.as_str(), "zeta.one");
    assert_eq!(
        r.by_http(HttpMethod::Get, "/api/v1/alpha")
            .unwrap()
            .id
            .as_str(),
        "alpha.one"
    );
    assert!(r.by_http(HttpMethod::Post, "/api/v1/alpha").is_none());
    let s = r.summary();
    assert_eq!(
        (s.total, s.builtin, s.mcp_tools, s.http_routes),
        (2, 2, 1, 1)
    );
}

#[test]
fn duplicate_ids_name_both_provenances_rust_and_rust() {
    let errs = errors_of(vec![
        query("dup.one", tool("a"), Stability::Implemented, "mod_a"),
        query("dup.one", tool("b"), Stability::Implemented, "mod_b"),
    ]);
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateId { id, first, second }] if id == "dup.one" && first == "builtin mod_a" && second == "builtin mod_b"),
        "{errs:?}"
    );
}

#[test]
fn duplicate_ids_rust_and_declarative() {
    let f = common::Fixture::new();
    let repo = Repository::discover(&f.root()).unwrap();
    let sources = Sources::load(&repo).unwrap();
    let index = Index::build(
        &repo,
        &sources,
        &KindSchema::embedded().unwrap(),
        &VcsIndex,
        GitState::Unavailable {
            reason: "test".into(),
        },
    )
    .unwrap();
    let errs = CapabilityRegistry::builder()
        .with_builtin(vec![query(
            "rule.project.alpha@1",
            tool("alpha"),
            Stability::Implemented,
            "mod_a",
        )])
        .with_index(&index)
        .build()
        .err()
        .unwrap();
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateId { id, first, second }] if id == "rule.project.alpha@1" && first == ".ai/repo/rules/project/alpha.v1.md" && second == "builtin mod_a"),
        "{errs:?}"
    );
}

#[test]
fn every_projection_collision_is_explicit() {
    let errs = errors_of(vec![
        query("a.one", tool("same"), Stability::Implemented, "m"),
        query("a.two", tool("same"), Stability::Implemented, "m"),
    ]);
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateMcpName { name, first, second }] if name == "same" && first == "a.one" && second == "a.two"),
        "{errs:?}"
    );

    let errs = errors_of(vec![
        query("a.one", route("/api/v1/x"), Stability::Implemented, "m"),
        query("a.two", route("/api/v1/x"), Stability::Implemented, "m"),
    ]);
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateHttpRoute { method, path, .. }] if method == "GET" && path == "/api/v1/x"),
        "{errs:?}"
    );

    let cli = |w: &str| Exposure {
        mcp: None,
        http: None,
        cli: Some(CliExposure {
            path: vec![w.into()],
        }),
    };
    let errs = errors_of(vec![
        query("a.one", cli("x"), Stability::Implemented, "m"),
        query("a.two", cli("x"), Stability::Implemented, "m"),
    ]);
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateCliPath { path, .. }] if path == "x"),
        "{errs:?}"
    );

    let res = |uri: &str| Exposure {
        mcp: Some(McpExposure {
            tool: None,
            resource: Some(McpResource {
                uri: uri.into(),
                name: "n".into(),
            }),
        }),
        http: None,
        cli: None,
    };
    let errs = errors_of(vec![
        query("a.one", res("majordomus://x"), Stability::Implemented, "m"),
        query("a.two", res("majordomus://x"), Stability::Implemented, "m"),
    ]);
    assert!(
        matches!(&errs[..], [RegistryError::DuplicateMcpUri { uri, .. }] if uri == "majordomus://x"),
        "{errs:?}"
    );
}

#[test]
fn invalid_ids_routes_and_names_are_refused_with_provenance() {
    let errs = errors_of(vec![query("NoDot", tool("t"), Stability::Implemented, "m")]);
    assert!(
        matches!(&errs[..], [RegistryError::InvalidId { id, provenance, .. }] if id == "NoDot" && provenance == "builtin m"),
        "{errs:?}"
    );

    let errs = errors_of(vec![query(
        "a.one",
        route("/objects"),
        Stability::Implemented,
        "m",
    )]);
    assert!(
        matches!(&errs[..], [RegistryError::InvalidExposure { projection, reason, .. }] if projection == "HTTP" && reason.contains("/api/v1/")),
        "{errs:?}"
    );

    let errs = errors_of(vec![query(
        "a.one",
        tool("Bad-Name"),
        Stability::Implemented,
        "m",
    )]);
    assert!(
        matches!(&errs[..], [RegistryError::InvalidExposure { projection, .. }] if projection == "MCP"),
        "{errs:?}"
    );

    let errs = errors_of(vec![query(
        "a.one",
        Exposure {
            mcp: None,
            http: None,
            cli: Some(CliExposure {
                path: vec!["".into()],
            }),
        },
        Stability::Implemented,
        "m",
    )]);
    assert!(
        matches!(&errs[..], [RegistryError::InvalidExposure { projection, .. }] if projection == "CLI"),
        "{errs:?}"
    );
}

#[test]
fn a_planned_capability_is_listed_but_never_executable() {
    let errs = errors_of(vec![query(
        "future.thing",
        tool("future_thing"),
        Stability::Planned,
        "m",
    )]);
    assert!(
        matches!(&errs[..], [RegistryError::NotExecutable { id, stability, projection, .. }] if id == "future.thing" && stability == "planned" && projection == "MCP tool"),
        "{errs:?}"
    );
    // with no executable exposure it is fine: visible to introspection, callable by nobody
    let r = CapabilityRegistry::builder()
        .with_builtin(vec![query(
            "future.thing",
            Exposure::default(),
            Stability::Planned,
            "m",
        )])
        .build()
        .unwrap();
    assert_eq!(r.get("future.thing").unwrap().stability, Stability::Planned);
    assert_eq!(r.summary().by_stability["planned"], 1);
}

#[test]
fn errors_are_collected_not_stopped_at_the_first() {
    let errs = errors_of(vec![
        query("a.one", tool("same"), Stability::Implemented, "m"),
        query("a.two", tool("same"), Stability::Implemented, "m"),
        query("bad", tool("x"), Stability::Implemented, "m"),
    ]);
    assert_eq!(errs.len(), 2, "{errs:?}");
}

#[test]
fn the_same_inputs_build_the_same_registry() {
    let f = common::Fixture::new();
    let a = common::load_app(&f);
    let b = common::load_app(&f);
    let ids = |r: &CapabilityRegistry| {
        r.iter()
            .map(|c| serde_json::to_string(c).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(a.registry()), ids(b.registry()));
    assert_eq!(a.registry().summary(), b.registry().summary());
    assert!(a.registry().get("rule.project.alpha@1").is_some());
    assert!(a.registry().get("repository.info").is_some());
}

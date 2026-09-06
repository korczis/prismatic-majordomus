//! Property-based guarantees over generated inputs, driven by the registry: for every
//! executable, one strategy (its input type's benchmark cases widened with domain values)
//! feeds every property. Cache semantics (off, cold and warm agree; a hit runs no
//! handler), canonical input normalisation (key order never matters), transport
//! equivalence (direct, HTTP and MCP answer the same data for the same input, or fail
//! the same way), projection determinism (two registries over one repository project the
//! same bytes), and duplicate detection (any composition that repeats an id is refused,
//! naming both parties). No property names a capability.

mod common;

use std::sync::Arc;

use majordomus_cli::bench::BenchmarkProjection;
use majordomus_cli::capability::executor::canonical_json;
use majordomus_cli::capability::{
    builtin, CapabilityError, CapabilityKind, CapabilityRegistry, CaseContext, Context, Provenance,
    RegistryError,
};
use majordomus_cli::http::{openapi, Request, Router};
use majordomus_cli::mcp::{Server, Surface};
use majordomus_cli::peers::Transport;
use majordomus_cli::perf::COUNTERS;
use majordomus_cli::synthetic::{Shape, SyntheticRepository};
use proptest::prelude::*;
use serde_json::{json, Map, Value};

/// One context shared by the properties of this file: a synthetic repository of a fixed
/// shape (the properties concern the executor and the projections, not the layer).
fn shared() -> Arc<Context> {
    static CTX: std::sync::OnceLock<(SyntheticRepository, Arc<Context>)> =
        std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let repo = SyntheticRepository::new(Shape {
            rules: 12,
            prompts: 2,
            documents: 4,
            body_lines: 6,
        })
        .expect("synthetic repository");
        let ctx = repo.context().expect("context");
        (repo, ctx)
    })
    .1
    .clone()
}

/// The counters are process-wide and the context is shared; the properties take turns.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A strategy over inputs of one executable: its benchmark cases, plus domain values for
/// the fields the shipped input types have (`query`, `kind`, `limit`, `uri`, `id`, `tag`,
/// `exposure`, `intent`, `scope`), plus shuffled key order.
fn input_strategy(ctx: &Arc<Context>, id: &str) -> BoxedStrategy<Value> {
    let cases: Vec<Value> = ctx
        .registry
        .cases(id)
        .map(|p| {
            p(&CaseContext { index: &ctx.index })
                .into_iter()
                .map(|c| c.input)
                .collect()
        })
        .unwrap_or_default();
    let kinds: Vec<String> = ctx.index.kinds().keys().map(|k| k.to_string()).collect();
    let uris: Vec<String> = ctx.index.objects.iter().map(|o| o.uri.clone()).collect();
    let ids: Vec<String> = ctx.registry.iter().map(|c| c.id.to_string()).collect();
    let (props, _) = ctx.registry.get(id).expect("executable").input.properties();
    let names: Vec<String> = props.into_iter().map(|(n, _)| n).collect();
    let field = move |name: String| -> BoxedStrategy<Option<Value>> {
        let kinds = kinds.clone();
        let uris = uris.clone();
        let ids = ids.clone();
        match name.as_str() {
            "query" => prop_oneof![
                3 => "[a-z]{1,8}".prop_map(Value::from),
                1 => Just(Value::from("rule")),
                1 => Just(Value::from("  ")),
            ]
            .prop_map(Some)
            .boxed(),
            "kind" => prop_oneof![
                2 => Just(None),
                2 => proptest::sample::select(kinds).prop_map(|k| Some(Value::from(k))),
                1 => "[a-z]{2,6}".prop_map(|k| Some(Value::from(k))),
            ]
            .boxed(),
            "limit" => prop_oneof![2 => Just(None), 3 => (0u64..300).prop_map(|l| Some(Value::from(l)))].boxed(),
            "uri" => prop_oneof![
                3 => proptest::sample::select(uris).prop_map(|u| Some(Value::from(u))),
                1 => "majordomus://[a-z]{3,6}/[a-z]{1,5}".prop_map(|u| Some(Value::from(u))),
            ]
            .boxed(),
            "id" => prop_oneof![
                3 => proptest::sample::select(ids).prop_map(|i| Some(Value::from(i))),
                1 => "[a-z]{2,5}\\.[a-z]{1,5}".prop_map(|i| Some(Value::from(i))),
            ]
            .boxed(),
            "tag" => prop_oneof![2 => Just(None), 1 => Just(Some(Value::from("synthetic"))), 1 => "[a-z]{1,5}".prop_map(|t| Some(Value::from(t)))].boxed(),
            "exposure" => prop_oneof![2 => Just(None), 1 => proptest::sample::select(vec!["mcp", "http", "cli"]).prop_map(|e| Some(Value::from(e)))].boxed(),
            "intent" => "[a-z ]{1,20}".prop_map(|t| Some(Value::from(t))).boxed(),
            "scope" => proptest::collection::vec("[a-z/]{1,10}", 0..3).prop_map(|s| Some(json!(s))).boxed(),
            _ => Just(None).boxed(),
        }
    };
    let generated = names
        .iter()
        .map(|n| {
            field(n.clone()).prop_map({
                let n = n.clone();
                move |v| (n.clone(), v)
            })
        })
        .collect::<Vec<_>>();
    let generated = generated.prop_map(|pairs| {
        let mut m = Map::new();
        for (k, v) in pairs {
            if let Some(v) = v {
                m.insert(k, v);
            }
        }
        Value::Object(m)
    });
    let strategy: BoxedStrategy<Value> = if cases.is_empty() {
        generated.boxed()
    } else {
        prop_oneof![1 => proptest::sample::select(cases), 3 => generated].boxed()
    };
    // key order never matters: sometimes reverse it
    (strategy, any::<bool>())
        .prop_map(|(v, reverse)| match (v, reverse) {
            (Value::Object(m), true) => {
                let mut r = Map::new();
                for (k, v) in m.iter().rev() {
                    r.insert(k.clone(), v.clone());
                }
                Value::Object(r)
            }
            (v, _) => v,
        })
        .boxed()
}

/// Every executable of the shared context with its strategy.
fn executables() -> Vec<(String, BoxedStrategy<Value>)> {
    let ctx = shared();
    ctx.registry
        .iter()
        .filter(|c| c.kind.is_executable())
        .map(|c| (c.id.to_string(), input_strategy(&ctx, c.id.as_str())))
        .collect()
}

/// Errors compared by class, not by message.
fn class(e: &CapabilityError) -> &'static str {
    match e {
        CapabilityError::InvalidInput(_) => "invalid_input",
        CapabilityError::NotFound(_) => "not_found",
        CapabilityError::Refused(_) => "refused",
        CapabilityError::Internal(_) => "internal",
    }
}

/// `perf.counters` answers live numbers; every other output is a pure function of the
/// input and the repository.
fn deterministic(id: &str) -> bool {
    id != "perf.counters" && id != "peers.list" && id != "peers.announce"
}

#[test]
fn cache_off_cold_and_warm_agree_and_a_hit_runs_no_handler() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = shared();
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: 48,
        ..Default::default()
    });
    for (id, strategy) in executables() {
        if !ctx.registry.get(&id).unwrap().cache.is_enabled() {
            continue;
        }
        let ctx2 = Arc::clone(&ctx);
        runner
            .run(&strategy, |input| {
                let uncached = Arc::new(Context::new(ctx2.index.clone(), ctx2.registry.clone()));
                let off = uncached.execute(&id, input.clone());
                ctx2.executor.clear();
                let cold = ctx2.execute(&id, input.clone());
                let before = COUNTERS.snapshot();
                let warm = ctx2.execute(&id, input.clone());
                let after = COUNTERS.snapshot();
                match (&off, &cold, &warm) {
                    (Ok(a), Ok(b), Ok(c)) => {
                        prop_assert_eq!(a, b, "{}: cold equals off", id);
                        prop_assert_eq!(b, c, "{}: warm equals cold", id);
                        prop_assert_eq!(
                            after.handler_invocations,
                            before.handler_invocations,
                            "{}: a hit runs no handler",
                            id
                        );
                        prop_assert_eq!(after.cache_hits, before.cache_hits + 1);
                    }
                    (Err(a), Err(b), Err(c)) => {
                        prop_assert_eq!(class(a), class(b));
                        prop_assert_eq!(class(b), class(c));
                        prop_assert_eq!(
                            after.handler_invocations,
                            before.handler_invocations + 1,
                            "{}: an error is never cached",
                            id
                        );
                    }
                    other => prop_assert!(false, "{}: mixed outcomes {:?}", id, other),
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

#[test]
fn canonical_json_is_invariant_under_key_order_and_round_trips() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut runner = proptest::test_runner::TestRunner::default();
    let ctx = shared();
    for (id, strategy) in executables() {
        let _ = &ctx;
        runner
            .run(&strategy, |input| {
                let Value::Object(m) = &input else {
                    return Ok(());
                };
                let mut reversed = Map::new();
                for (k, v) in m.iter().rev() {
                    reversed.insert(k.clone(), v.clone());
                }
                prop_assert_eq!(
                    canonical_json(&input),
                    canonical_json(&Value::Object(reversed))
                );
                let back: Value = serde_json::from_str(&canonical_json(&input)).unwrap();
                prop_assert_eq!(back, input);
                Ok(())
            })
            .unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

#[test]
fn direct_http_and_mcp_answer_the_same_data_or_fail_the_same_way() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = shared();
    let router = Router::new(Arc::clone(&ctx), "test");
    let surface = Surface::new(Arc::clone(&ctx));
    let peer = ctx.peers.attach(Transport::Stdio);
    let mcp = std::cell::RefCell::new(Server::new(surface.for_peer(peer.clone()), "test"));
    let direct_ctx = Arc::new(ctx.for_caller(peer));
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: 32,
        ..Default::default()
    });
    for (id, strategy) in executables() {
        if !deterministic(&id) {
            continue;
        }
        let c = ctx.registry.get(&id).unwrap().clone();
        let tool = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone());
        let route = c.exposure.http.clone();
        runner
            .run(&strategy, |input| {
                let direct = direct_ctx.execute(&id, input.clone());
                if let Some(tool) = &tool {
                    let frame = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": input } });
                    let answer = mcp
                        .borrow_mut()
                        .handle(frame)
                        .expect("a response")
                        .into_value();
                    match &direct {
                        Ok(v) => {
                            prop_assert!(answer["result"]["isError"] == false, "{}: {}", id, answer);
                            prop_assert_eq!(&answer["result"]["structuredContent"], v, "{}: MCP equals direct", id);
                        }
                        Err(CapabilityError::Internal(_)) => prop_assert!(answer.get("error").is_some()),
                        Err(_) => prop_assert!(answer["result"]["isError"] == true, "{}: a refusal is a result with isError", id),
                    }
                }
                if let Some(route) = &route {
                    if route.method == majordomus_cli::capability::HttpMethod::Get {
                        let query: Vec<String> = input
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .filter(|(_, v)| !v.is_null())
                                    .map(|(k, v)| {
                                        let text = match v {
                                            Value::String(s) => s.clone(),
                                            other => other.to_string(),
                                        };
                                        format!("{}={}", encode(k), encode(&text))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let target = if query.is_empty() {
                            route.path.clone()
                        } else {
                            format!("{}?{}", route.path, query.join("&"))
                        };
                        let response = router.handle(&Request::parse_target("GET", &target, vec![]));
                        match &direct {
                            Ok(v) => {
                                prop_assert_eq!(response.status, 200, "{} {}: {}", id, target, response.body);
                                let body: Value = serde_json::from_str(&response.body.text()).unwrap();
                                prop_assert_eq!(&body, v, "{}: HTTP equals direct", id);
                            }
                            Err(e) => {
                                let expected = match e {
                                    CapabilityError::InvalidInput(_) => 400,
                                    CapabilityError::NotFound(_) => 404,
                                    CapabilityError::Refused(_) => 422,
                                    CapabilityError::Internal(_) => 500,
                                };
                                prop_assert_eq!(response.status, expected, "{} {}: {}", id, target, response.body);
                            }
                        }
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b':'
            | b'@' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[test]
fn projections_of_one_repository_are_deterministic_across_registries() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let repo = SyntheticRepository::small().unwrap();
    let a = repo.context().unwrap();
    let b = repo.context().unwrap();
    assert_eq!(a.registry.fingerprint(), b.registry.fingerprint());
    // without examples: the examples are the repository's own benchmark cases, and this
    // is about the builtin projections
    assert_eq!(
        openapi::document(&a.registry, "t", None).unwrap(),
        openapi::document(&b.registry, "t", None).unwrap()
    );
    assert_eq!(
        *Surface::new(a.clone()).tools_json(),
        *Surface::new(b.clone()).tools_json()
    );
    assert_eq!(
        *Surface::new(a.clone()).resources_json(),
        *Surface::new(b.clone()).resources_json()
    );
    assert_eq!(
        BenchmarkProjection::from_context(&a),
        BenchmarkProjection::from_context(&b)
    );
    let ga =
        majordomus_cli::generate::context_artifacts(&a, "t", majordomus_cli::generate::Target::ALL)
            .unwrap();
    let gb =
        majordomus_cli::generate::context_artifacts(&b, "t", majordomus_cli::generate::Target::ALL)
            .unwrap();
    assert_eq!(ga, gb);
    // a change to the repository moves the fingerprint and the resource listing, and
    // nothing of the builtin projections
    repo.touch_rule(0, "An extra line.").unwrap();
    let c = repo.context().unwrap();
    assert_ne!(c.registry.fingerprint(), a.registry.fingerprint());
    assert_eq!(
        openapi::document(&c.registry, "t", None).unwrap(),
        openapi::document(&a.registry, "t", None).unwrap()
    );
    assert_eq!(
        *Surface::new(c.clone()).tools_json(),
        *Surface::new(a.clone()).tools_json()
    );
}

#[test]
fn any_composition_that_repeats_an_id_is_refused_naming_both_parties() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut runner = proptest::test_runner::TestRunner::default();
    let n = builtin::all().len();
    runner
        .run(
            &(
                proptest::collection::vec(0..n, 1..=n),
                proptest::collection::vec(0..n, 0..=n),
            ),
            |(first, second)| {
                // compose the chosen executables, some possibly twice
                let pick = |indexes: &[usize]| -> Vec<_> {
                    let mut all = builtin::all();
                    let mut chosen = Vec::new();
                    let mut sorted = indexes.to_vec();
                    sorted.sort_unstable_by(|a, b| b.cmp(a));
                    sorted.dedup();
                    for i in sorted {
                        chosen.push(all.remove(i));
                    }
                    chosen
                };
                let a = pick(&first);
                let b = pick(&second);
                let expected_duplicates: std::collections::BTreeSet<String> = a
                    .iter()
                    .filter(|x| b.iter().any(|y| y.capability.id == x.capability.id))
                    .map(|x| x.capability.id.to_string())
                    .collect();
                let result = CapabilityRegistry::builder()
                    .with_builtin(a)
                    .with_builtin(b)
                    .build();
                match result {
                    Ok(registry) => {
                        prop_assert!(expected_duplicates.is_empty());
                        prop_assert!(!registry.is_empty());
                    }
                    Err(errors) => {
                        let mut found = std::collections::BTreeSet::new();
                        for e in &errors {
                            match e {
                                RegistryError::DuplicateId { id, first, second } => {
                                    prop_assert!(
                                        first.contains("builtin") && second.contains("builtin"),
                                        "{:?}",
                                        e
                                    );
                                    found.insert(id.clone());
                                }
                                other => prop_assert!(false, "unexpected error {:?}", other),
                            }
                        }
                        prop_assert_eq!(found, expected_duplicates);
                    }
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn commands_are_never_cached_and_queries_are_the_only_kind_the_cache_ever_sees() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = shared();
    for c in ctx.registry.iter() {
        if c.kind == CapabilityKind::Command {
            assert!(!c.cache.is_enabled(), "{}", c.id);
        }
        if c.kind == CapabilityKind::Resource {
            assert!(!c.cache.is_enabled(), "{}", c.id);
        }
    }
}

/// Graph invariants, over every derivation and every shape of repository the generator
/// makes. These are the guarantees a renderer relies on: a node it can look up for every
/// edge end, one node per id, and the same bytes for the same tree — which is what lets a
/// graph be cached, committed, snapshotted or diffed.
#[test]
fn every_derived_graph_holds_its_invariants_on_every_repository() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let shapes = [
        Shape {
            rules: 0,
            prompts: 0,
            documents: 0,
            body_lines: 1,
        },
        Shape {
            rules: 1,
            prompts: 1,
            documents: 1,
            body_lines: 2,
        },
        Shape {
            rules: 25,
            prompts: 4,
            documents: 9,
            body_lines: 4,
        },
    ];
    for shape in shapes {
        let repo = SyntheticRepository::new(shape).expect("synthetic repository");
        let ctx = repo.context().expect("context");
        for id in majordomus_cli::graph::ids() {
            let g = majordomus_cli::graph::derive(id, &ctx.registry, &ctx.index)
                .unwrap_or_else(|| panic!("{id} is listed and does not derive"));

            // one node per id
            let ids: std::collections::BTreeSet<&str> =
                g.nodes.iter().map(|n| n.id.as_str()).collect();
            assert_eq!(ids.len(), g.nodes.len(), "{id}: a node id appears twice");

            // every edge end is a node of this graph, and every kind is declared
            for e in &g.edges {
                assert!(
                    ids.contains(e.source.as_str()),
                    "{id}: edge from {}",
                    e.source
                );
                assert!(
                    ids.contains(e.target.as_str()),
                    "{id}: edge to {}",
                    e.target
                );
                assert!(
                    g.edge_kinds.contains_key(&e.kind),
                    "{id}: undeclared edge kind {}",
                    e.kind
                );
            }
            for n in &g.nodes {
                assert!(
                    g.node_kinds.contains_key(&n.kind),
                    "{id}: undeclared node kind {}",
                    n.kind
                );
            }

            // the metadata is the graph, counted and judged, not a second claim
            assert_eq!(g.metadata.nodes, g.nodes.len(), "{id}");
            assert_eq!(g.metadata.edges, g.edges.len(), "{id}");
            assert_eq!(
                g.metadata.acyclic,
                majordomus_cli::graph::is_acyclic(&g.nodes, &g.edges),
                "{id}"
            );
            assert!(
                g.nodes.len() <= majordomus_cli::graph::MAX_NODES,
                "{id}: past the node limit without saying so"
            );

            // sorted, so two derivations of one tree are byte-equal and a diff is readable
            let mut sorted = g.nodes.clone();
            sorted.sort();
            assert_eq!(sorted, g.nodes, "{id}: nodes are not sorted");

            // no absolute path and no clock: the same tree and executable, the same bytes
            let text = serde_json::to_string(&g).expect("a graph serialises");
            assert!(
                !text.contains(repo.root().to_str().unwrap()),
                "{id} carries the checkout path"
            );

            // derived twice from two registries over one tree: identical
            let other = repo.context().expect("a second context");
            let again = majordomus_cli::graph::derive(id, &other.registry, &other.index)
                .expect("the same graph");
            assert_eq!(again, g, "{id} is not deterministic");
        }
    }
}

/// The registry graph is the executable describing itself, so it must describe all of
/// itself: every builtin capability is a node, and every projection it declares is an
/// edge. A capability added to a module reaches the graph without the derivation knowing
/// its name.
#[test]
fn the_registry_graph_holds_every_builtin_capability_and_its_projections() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = shared();
    let g = majordomus_cli::graph::derive("registry", &ctx.registry, &ctx.index)
        .expect("the registry graph");
    let node = |id: &str| g.nodes.iter().any(|n| n.id == id);
    let edge = |from: &str, to: &str| g.edges.iter().any(|e| e.source == from && e.target == to);

    for c in ctx.registry.iter() {
        if !matches!(c.provenance, Provenance::Builtin { .. }) {
            continue;
        }
        let capability = format!("capability:{}", c.id);
        assert!(node(&capability), "{} is not a node", c.id);
        assert!(
            edge(&format!("module:{}", c.module), &capability),
            "{} is not composed by its module",
            c.id
        );
        assert!(
            edge(
                &capability,
                &format!("source:{}", c.provenance.source_path())
            ),
            "{} does not name the file it was declared in",
            c.id
        );
        for (declared, projection) in [
            (c.exposure.mcp.is_some(), "projection:mcp"),
            (c.exposure.http.is_some(), "projection:http"),
            (c.exposure.cli.is_some(), "projection:cli"),
        ] {
            assert_eq!(
                edge(&capability, projection),
                declared,
                "{} and {projection} disagree",
                c.id
            );
        }
    }
}

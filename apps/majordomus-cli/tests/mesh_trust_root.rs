//! A working tree may narrow the mesh's trust and never widen it (I2135, ADR 0050 rule 6).
//!
//! The server read the mesh declaration from the checkout's working tree, so a branch — or an
//! uncommitted edit — could add a trusted key, point the runtime at a host of its author's
//! choosing, or switch the policy to trust on first use, and the runtime of that checkout
//! obeyed. These tests hold the rule that replaced it: the declaration in force is the
//! working tree's with every widening of the trunk's committed copy removed, each removal is
//! named by `mesh doctor`, and trust on first use is honoured only from a committed copy.

mod common;

use common::Fixture;
use majordomus_cli::mesh::root::resolve;
use majordomus_cli::mesh::trust::TrustPolicy;
use serde_json::{json, Value};

const PATH: &str = ".ai/repo/mesh/majordomus.yaml";
const A: &str = "946e8aa593fe16c58d5eb43493338810896cd10bc6bdd44ffa8e9bfebd37ca1f";
const B: &str = "3f7b357805aa15ecdb3c1ace7e7ac02de263e3d0def8aafda9ad280d3af7c813";

fn declaration(policy: &str, allow: &[&str], endpoints: &[&str]) -> String {
    let mut text = format!(
        "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: {policy}\n"
    );
    if !allow.is_empty() {
        text.push_str("  allow:\n");
        for key in allow {
            text.push_str(&format!("    - {key}\n"));
        }
    }
    if !endpoints.is_empty() {
        text.push_str("rendezvous:\n  endpoints:\n");
        for e in endpoints {
            text.push_str(&format!("    - {e}\n"));
        }
    }
    text
}

/// The declaration in force, as a freshly loaded process of the fixture resolves it.
fn in_force(f: &Fixture) -> majordomus_cli::mesh::root::Resolved {
    let app = common::load_app(f);
    let working = app
        .context
        .index
        .objects
        .iter()
        .find(|o| o.kind == majordomus_cli::mesh::KIND)
        .cloned();
    resolve(&f.root(), working.as_ref())
}

fn doctor(f: &Fixture) -> Value {
    let app = common::load_app(f);
    let report = app
        .context
        .execute("mesh.doctor", json!({}))
        .expect("mesh.doctor answers");
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "trust-root")
        .cloned()
        .expect("a trust-root check")
}

#[test]
fn a_working_tree_edit_that_widens_trust_is_not_obeyed() {
    let f = Fixture::new();
    f.write(PATH, &declaration("deny_unknown", &[A], &[]));
    f.commit("the mesh, as reviewed");
    // an uncommitted edit: one more key, a looser policy and a host of the editor's choosing
    f.write(
        PATH,
        &declaration("tofu", &[A, B], &["http://198.51.100.7:8791"]),
    );

    let resolved = in_force(&f);
    let config = resolved.declaration.unwrap().unwrap();
    assert!(resolved.committed);
    assert_eq!(config.trust.allow, vec![A.to_string()]);
    assert_eq!(config.trust.policy, TrustPolicy::DenyUnknown);
    assert!(config.rendezvous.endpoints.is_empty());
    assert_eq!(resolved.narrowed.len(), 3, "{:?}", resolved.narrowed);

    let check = doctor(&f);
    assert_eq!(check["ok"], json!(false), "{check}");
    let detail = check["detail"].as_str().unwrap();
    assert!(
        detail.contains(B) && detail.contains("198.51.100.7") && detail.contains("tofu"),
        "{detail}"
    );
}

#[test]
fn a_branch_that_commits_a_widening_is_held_to_the_trunk() {
    let f = Fixture::new();
    f.write(PATH, &declaration("deny_unknown", &[A], &[]));
    f.commit("the mesh, as reviewed");
    // a contributor's branch, checked out by a reviewer: committed is not enough
    f.git(&["checkout", "-q", "-b", "contribution"]);
    f.write(
        PATH,
        &declaration("deny_unknown", &[A, B], &["http://198.51.100.7:8791"]),
    );
    f.commit("trust one more machine");

    let resolved = in_force(&f);
    let config = resolved.declaration.unwrap().unwrap();
    assert_ne!(
        resolved.trunk.as_deref(),
        Some("contribution"),
        "the branch is not the trunk"
    );
    assert_eq!(config.trust.allow, vec![A.to_string()]);
    assert!(config.rendezvous.endpoints.is_empty());
    assert_eq!(resolved.narrowed.len(), 2, "{:?}", resolved.narrowed);
}

#[test]
fn trust_on_first_use_is_honoured_only_from_a_committed_copy() {
    let f = Fixture::new();
    f.write(PATH, &declaration("tofu", &[], &[]));
    // staged, so the index holds it, and on no trunk yet
    f.git(&["add", PATH]);
    let uncommitted = in_force(&f);
    assert!(!uncommitted.committed);
    assert_eq!(
        uncommitted.declaration.unwrap().unwrap().trust.policy,
        TrustPolicy::DenyUnknown
    );
    assert_eq!(uncommitted.narrowed.len(), 1);

    f.commit("tofu, decided on the trunk");
    let committed = in_force(&f);
    assert!(committed.committed);
    assert_eq!(
        committed.declaration.unwrap().unwrap().trust.policy,
        TrustPolicy::Tofu
    );
    assert!(committed.narrowed.is_empty());
}

#[test]
fn a_working_tree_that_narrows_trust_is_obeyed_as_written() {
    let f = Fixture::new();
    f.write(
        PATH,
        &declaration("deny_unknown", &[A, B], &["http://192.0.2.1:8791"]),
    );
    f.commit("two machines and a hub");
    f.write(PATH, &declaration("deny_unknown", &[A], &[]));

    let resolved = in_force(&f);
    let config = resolved.declaration.unwrap().unwrap();
    assert_eq!(config.trust.allow, vec![A.to_string()]);
    assert!(config.rendezvous.endpoints.is_empty());
    assert!(resolved.narrowed.is_empty(), "{:?}", resolved.narrowed);
    assert_eq!(doctor(&f)["ok"], json!(true));
}

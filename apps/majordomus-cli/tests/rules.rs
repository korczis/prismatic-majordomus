//! The rule proof graph, through the public API.
//!
//! The unit tests beside `majordomus_cli::rules` prove the derivation against inputs they
//! construct. This proves the same module the way a caller reaches it: build a repository,
//! read it through `definitions` and `report`, and run the capabilities that project it —
//! so that a change which keeps the internals correct and breaks the public surface is
//! still caught.

use majordomus_cli::capability::builtin::rules::module;
use majordomus_cli::evidence::Ledger;
use majordomus_cli::rules::{definitions, report, Class, Mode, RuleState};
use majordomus_cli::synthetic::SyntheticRepository;

/// Every rule of the tree is discovered, in the crate's canonical order, with the tallies
/// counted from what was derived rather than from anything written down.
#[test]
fn the_corpus_is_read_from_the_tree_and_ordered_canonically() {
    let repo = SyntheticRepository::small().unwrap();
    let index = repo.index().unwrap();

    let defs = definitions(&index);
    assert_eq!(defs.len(), repo.shape.rules, "a rule was not discovered");
    assert!(
        majordomus_cli::order::is_canonical(&defs),
        "definitions are not in the crate's canonical order"
    );

    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    assert_eq!(r.coverage.rules, defs.len());
    assert_eq!(r.coverage.advisory, defs.len());
    assert_eq!(r.coverage.blocking, 0);
    assert!(majordomus_cli::order::is_canonical(&r.rules));

    // the same tree twice is the same report: discovery order is the filesystem's business
    // and must not reach the answer
    assert_eq!(r, report(&index, &ledger));
}

/// A rule that names nothing is `unproven` and, while it is advisory, not a finding. The
/// two facts are separate and the report keeps them separate.
#[test]
fn a_rule_naming_no_proof_is_unproven_without_being_a_finding() {
    let repo = SyntheticRepository::small().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&repo.index().unwrap(), &ledger);

    assert!(r.rules.iter().all(|p| p.state == RuleState::Unproven));
    assert!(r.rules.iter().all(|p| p.rule.class == Class::Advisory));
    assert!(r
        .rules
        .iter()
        .all(|p| p.rule.enforcement.mode == Mode::Declarative));
    assert!(
        r.findings.is_empty(),
        "an advisory rule owes no executable proof"
    );
    assert!(r.satisfied());
    assert_eq!(r.coverage.named_proof, 0);
    assert_eq!(r.coverage.review_only, 0);
}

/// A blocking rule naming a case that is not in the tree is `dangling` and a finding: it
/// reads as enforced on every surface that checks only the name, which is the defect the
/// whole graph exists to catch.
#[test]
fn a_blocking_rule_naming_a_missing_case_is_a_finding() {
    let repo = SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-0.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  tests: [test/cases/99_deleted.sh]",
        ),
    )
    .unwrap();

    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&repo.index().unwrap(), &ledger);
    let p = r
        .rules
        .iter()
        .find(|p| p.rule.id == "project.rule-0")
        .unwrap();

    assert_eq!(p.rule.class, Class::Blocking);
    assert_eq!(p.rule.enforcement.mode, Mode::Gated);
    assert_eq!(p.state, RuleState::Dangling);
    assert!(!p.satisfied);
    assert!(!r.satisfied());

    let f = r
        .findings
        .iter()
        .find(|f| f.rule == "project.rule-0")
        .unwrap();
    assert!(f.reason.contains("not in the tree"));
    assert!(
        !f.reproduce.is_empty(),
        "a finding names how to see it again"
    );
}

/// A rule that declares, with its reason, that no program can express it is `reviewed`:
/// counted apart, never a finding, and never counted as proof.
#[test]
fn a_declared_exemption_is_counted_apart_from_every_proof() {
    let repo = SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-1.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  reviewed_because: the rule is about the \
             provenance of code, which no program in this tree reads",
        ),
    )
    .unwrap();

    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&repo.index().unwrap(), &ledger);
    let p = r
        .rules
        .iter()
        .find(|p| p.rule.id == "project.rule-1")
        .unwrap();

    assert_eq!(p.rule.enforcement.mode, Mode::Reviewed);
    assert_eq!(p.state, RuleState::Reviewed);
    assert!(
        p.satisfied,
        "a declared, reasoned exemption is not a finding"
    );
    assert!(!p.state.passing(), "review is not a recorded pass");
    assert_eq!(r.coverage.review_only, 1);
    assert_eq!(
        r.coverage.named_proof, 0,
        "review names no executable proof"
    );
    assert!(r.findings.is_empty());
}

/// The module's declaration is the only place its projections exist; every surface derives
/// from it, so an exposure dropped in a refactor fails here.
#[test]
fn the_module_declares_every_projection_it_claims() {
    let m = module();
    assert_eq!(m.id.as_str(), "rules");
    for e in &m.capabilities {
        let id = e.capability.id.as_str();
        let x = &e.capability.exposure;
        assert!(e.capability.kind.is_read_only(), "{id} writes");
        assert!(x.mcp.is_some(), "{id} is not reachable over MCP");
        assert!(x.http.is_some(), "{id} is not reachable over HTTP");
        assert!(x.cli.is_some(), "{id} has no command line");
        assert!(
            !e.capability.cache.is_enabled(),
            "{id} caches, but the ledger changes outside this process"
        );
    }
}

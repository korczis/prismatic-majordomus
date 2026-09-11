//! Unit proof for the rule proof graph.
//!
//! The subject is the derivation, not the corpus: every test here builds the front matter
//! or the state it is about, so that a rule added to this repository tomorrow changes the
//! report and not these assertions. The behaviour against the real corpus is proven by
//! `test/cases/125_rule_proof.sh` and `test/cases/129_rules_graph.sh`, which run the tool
//! against a repository on disk.

use super::*;
use serde_json::json;

// ---------------------------------------------------------------- the front matter

/// The mode is decided by what the block names, never by a word it could also declare. A
/// block that names a validator is dispatched even when it also names cases; one that names
/// only cases is gated; one that names neither is declarative whatever else it carries.
#[test]
fn the_mode_is_what_the_block_names() {
    let dispatched = enforcement_of(&json!({"x-majordomus": {
        "validator": "lib/checks/scope.sh", "category": "scope", "exit_code": 12,
        "enforced_by": "commit", "tests": ["test/cases/07_scope.sh"]
    }}));
    assert_eq!(dispatched.mode, Mode::Dispatched);
    assert_eq!(dispatched.validator.as_deref(), Some("lib/checks/scope.sh"));
    assert_eq!(dispatched.exit_code, Some(12));
    assert_eq!(dispatched.enforced_by.as_deref(), Some("commit"));
    assert_eq!(dispatched.tests, ["test/cases/07_scope.sh"]);

    let gated = enforcement_of(&json!({"x-majordomus": {"tests": ["test/cases/07_scope.sh"]}}));
    assert_eq!(gated.mode, Mode::Gated);
    assert!(gated.validator.is_none());

    // a half-declaration — metadata without either half of a proof — is declarative, and
    // the finding that follows says so; it is never silently read as enforced
    let half = enforcement_of(&json!({"x-majordomus": {"category": "scope"}}));
    assert_eq!(half.mode, Mode::Declarative);
    assert!(half.tests.is_empty());

    assert_eq!(enforcement_of(&json!({})).mode, Mode::Declarative);
    // an empty list is not a proof
    assert_eq!(
        enforcement_of(&json!({"x-majordomus": {"tests": []}})).mode,
        Mode::Declarative
    );
}

/// A class is read, never defaulted: guessing here is how a blocking rule quietly becomes
/// advice.
#[test]
fn a_class_is_read_and_never_guessed() {
    assert_eq!(Class::parse("blocking"), Class::Blocking);
    assert_eq!(Class::parse(" advisory "), Class::Advisory);
    assert_eq!(Class::parse("strict"), Class::Unknown);
    assert_eq!(Class::parse(""), Class::Unknown);
    for c in [Class::Blocking, Class::Advisory, Class::Unknown] {
        assert!(!c.label().is_empty());
    }
}

/// Front matter is YAML, and a scalar arrives as a string, a number or a bool. A reader
/// that only understands strings drops `version` and `exit_code` silently.
#[test]
fn a_scalar_field_reads_whatever_yaml_made_of_it() {
    let m = json!({"a": "x", "b": 12, "c": true, "d": "", "e": "  y  ", "f": "'z'", "g": []});
    assert_eq!(field(&m, "a"), Some("x".into()));
    assert_eq!(field(&m, "b"), Some("12".into()));
    assert_eq!(field(&m, "c"), Some("true".into()));
    assert_eq!(field(&m, "d"), None);
    assert_eq!(field(&m, "e"), Some("y".into()));
    assert_eq!(field(&m, "f"), Some("z".into()));
    assert_eq!(field(&m, "g"), None);
    assert_eq!(field(&m, "missing"), None);
}

/// A list field accepts the flow list the rules use and the single scalar a hand-written
/// rule occasionally is, and refuses everything else rather than inventing an entry.
#[test]
fn a_list_field_accepts_a_list_or_one_scalar() {
    assert_eq!(list(&json!({"t": ["a", "b"]}), "t"), ["a", "b"]);
    assert_eq!(list(&json!({"t": "a"}), "t"), ["a"]);
    assert_eq!(list(&json!({"t": [" a ", "", "b"]}), "t"), ["a", "b"]);
    assert!(list(&json!({"t": ""}), "t").is_empty());
    assert!(list(&json!({"t": 3}), "t").is_empty());
    assert!(list(&json!({}), "t").is_empty());
}

// ---------------------------------------------------------------- the states

/// The ranking a report sorts by, and the guarantee that a rule's state is the weakest of
/// its parts: one dangling case makes the rule dangling however many others pass.
#[test]
fn the_states_rank_from_strongest_to_weakest() {
    assert!(RuleState::Proven < RuleState::InputsUnchanged);
    assert!(RuleState::InputsUnchanged < RuleState::Stale);
    assert!(RuleState::Stale < RuleState::Gated);
    assert!(RuleState::Gated < RuleState::Failing);
    assert!(RuleState::Failing < RuleState::NotRun);
    assert!(RuleState::NotRun < RuleState::Reviewed);
    assert!(RuleState::Reviewed < RuleState::Unrunnable);
    assert!(RuleState::Unrunnable < RuleState::Dangling);
    assert!(RuleState::Dangling < RuleState::Unproven);
    // the weakest of a set is its max, which is how a rule's state is taken
    let worst = [RuleState::Proven, RuleState::Dangling, RuleState::Proven]
        .into_iter()
        .max()
        .unwrap();
    assert_eq!(worst, RuleState::Dangling);
}

/// Only the three states with a passing run behind them are passing, and a stale pass is
/// one of them: it did pass, which is a different fact from proving the tree in front of
/// you, and a finding is what separates the two.
#[test]
fn only_a_recorded_pass_is_passing() {
    assert!(RuleState::Proven.passing());
    assert!(RuleState::InputsUnchanged.passing());
    assert!(RuleState::Stale.passing());
    assert!(!RuleState::Gated.passing(), "a gate is a mechanism, not a recorded pass");
    assert!(!RuleState::Failing.passing());
    assert!(!RuleState::NotRun.passing());
    assert!(!RuleState::Reviewed.passing());
    assert!(!RuleState::Unrunnable.passing());
    assert!(!RuleState::Dangling.passing());
    assert!(!RuleState::Unproven.passing());
}

/// Every state says what it means, so that no surface has to invent a gloss and the Cockpit
/// badge and the command line cannot disagree.
#[test]
fn every_state_words_itself_once() {
    for s in [
        RuleState::Proven,
        RuleState::InputsUnchanged,
        RuleState::Stale,
        RuleState::Gated,
        RuleState::Failing,
        RuleState::NotRun,
        RuleState::Reviewed,
        RuleState::Unrunnable,
        RuleState::Dangling,
        RuleState::Unproven,
    ] {
        assert!(!s.label().is_empty(), "a state with no word");
        assert!(!s.meaning().is_empty(), "{} has no meaning", s.label());
    }
    for m in [Mode::Dispatched, Mode::Gated, Mode::Reviewed, Mode::Declarative] {
        assert!(!m.label().is_empty());
    }
    for k in [ArtifactKind::Case, ArtifactKind::Gate, ArtifactKind::Unknown] {
        assert!(!k.label().is_empty());
    }
}

/// A test's evidence state maps onto the rule vocabulary, and a missing file overrides
/// whatever the ledger says: a rule naming a deleted case reads as proven on every surface
/// that checks only the name, which is the defect this state exists to name.
#[test]
fn a_missing_file_beats_whatever_the_ledger_says() {
    let proof = |present, state| TestProof {
        path: "test/cases/07_scope.sh".into(),
        kind: ArtifactKind::Case,
        present,
        test: Some("suite:07_scope".into()),
        state,
        meaning: state.meaning().into(),
        execution: None,
        reproduce: None,
        gates: vec![],
    };
    assert_eq!(
        rule_state_of(&proof(false, ProofState::Proven)),
        RuleState::Dangling
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::Proven)),
        RuleState::Proven
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::InputsUnchanged)),
        RuleState::InputsUnchanged
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::Stale)),
        RuleState::Stale
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::Failing)),
        RuleState::Failing
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::NotRun)),
        RuleState::NotRun
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::Unrunnable)),
        RuleState::Unrunnable
    );
    assert_eq!(
        rule_state_of(&proof(true, ProofState::NoTest)),
        RuleState::Unproven
    );
}

// ---------------------------------------------------------------- the findings

/// A blocking rule claims a gate refuses work that violates it; every state without a live
/// proof makes that claim false. An advisory rule claims nothing executable, so only a name
/// that does not resolve is a finding against it — a dangling path reads as proof at any
/// class, which is why it is a finding at every class.
#[test]
fn a_blocking_rule_is_held_to_its_proof_and_an_advisory_one_is_not() {
    assert!(unsupported(Class::Blocking, RuleState::Unproven).is_some());
    assert!(unsupported(Class::Blocking, RuleState::Failing).is_some());
    assert!(unsupported(Class::Blocking, RuleState::Unrunnable).is_some());
    assert!(unsupported(Class::Blocking, RuleState::Dangling).is_some());
    assert!(unsupported(Class::Blocking, RuleState::Proven).is_none());
    assert!(unsupported(Class::Blocking, RuleState::InputsUnchanged).is_none());
    assert!(unsupported(Class::Blocking, RuleState::Stale).is_none());
    // a gate refuses violations, which is what a blocking rule claims; the missing verdict
    // is a weaker state, not a broken claim
    assert!(unsupported(Class::Blocking, RuleState::Gated).is_none());

    for state in [
        RuleState::Unproven,
        RuleState::Failing,
        RuleState::NotRun,
        RuleState::Unrunnable,
    ] {
        assert!(
            unsupported(Class::Advisory, state).is_none(),
            "advisory must not be held to a live proof: {}",
            state.label()
        );
    }
    // at every class, a name that resolves to nothing is a finding
    for class in [Class::Blocking, Class::Advisory, Class::Unknown] {
        assert!(unsupported(class, RuleState::Dangling).is_some());
    }
}

// ---------------------------------------------------------------- the gates

/// A gate's binding to a path is read from the gate's own command. No gate id is written
/// down: a gate renamed in the CI model is renamed in this answer.
#[test]
fn a_gate_is_found_by_the_command_it_runs() {
    let commands = vec![
        ("rule-proof".to_string(), "scripts/ci/rule-proof-check".to_string()),
        ("shell-suite".to_string(), "MJ_TEST_JOBS=4 bash test/run.sh".to_string()),
        (
            "rust-integration".to_string(),
            "scripts/rust-check --integration".to_string(),
        ),
        ("site-build".to_string(), "scripts/site build".to_string()),
    ];
    assert_eq!(
        gates_for("test/cases/07_scope.sh", &commands),
        ["shell-suite"]
    );
    assert_eq!(
        gates_for("apps/majordomus-cli/tests/product.rs", &commands),
        ["rust-integration"]
    );
    // a validator wired as its own gate is found by name
    assert_eq!(
        gates_for("scripts/ci/rule-proof-check", &commands),
        ["rule-proof"]
    );
    // a path nothing runs gets nothing, rather than the nearest plausible gate
    assert!(gates_for("lib/checks/scope.sh", &commands).is_empty());
    assert!(gates_for("test/cases/07_scope.sh", &[]).is_empty());
}

/// The CI model is parsed from the file, so a gate declared without a command, or a file
/// that is not there at all, yields no binding rather than a panic.
#[test]
fn the_ci_model_is_read_from_the_tree_or_not_at_all() {
    let dir = std::env::temp_dir().join(format!("majordomus-rules-gates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".ai/repo/ci")).unwrap();
    // nothing written yet: no gates, no panic
    assert!(gate_commands(&dir).is_empty());
    std::fs::write(
        dir.join(GATES_PATH),
        "gates:\n  - id: alpha\n    always: true\n    runs: scripts/a\n  - id: beta\n    \
         summary: declared with no command\n  - id: gamma\n    runs: scripts/c --flag\n",
    )
    .unwrap();
    let got = gate_commands(&dir);
    assert_eq!(
        got,
        vec![
            ("alpha".to_string(), "scripts/a".to_string()),
            ("gamma".to_string(), "scripts/c --flag".to_string()),
        ],
        "a gate that declares no command binds to no path"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------- the whole report

/// The report is derived from the index, in canonical id order, with every tally counted
/// from what was derived. The synthetic repository's rules are advisory and name no proof,
/// which is exactly the shape that must produce `unproven` without producing a finding.
#[test]
fn a_corpus_with_no_proof_is_unproven_and_not_a_finding() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);

    assert_eq!(r.coverage.rules, repo.shape.rules, "every rule discovered");
    assert_eq!(r.coverage.advisory, repo.shape.rules);
    assert_eq!(r.coverage.blocking, 0);
    assert_eq!(r.coverage.named_proof, 0);
    assert_eq!(r.coverage.gated, 0);
    assert_eq!(r.coverage.satisfied, r.coverage.rules);
    assert!(r.rules.iter().all(|p| p.state == RuleState::Unproven));
    assert_eq!(r.states.get("unproven"), Some(&repo.shape.rules));
    assert!(r.findings.is_empty(), "advisory rules owe no executable proof");
    assert!(r.satisfied());

    // canonical order, from the crate's one comparator, so that two runs over one tree
    // agree whatever order the filesystem handed the discovery
    assert!(
        crate::order::is_canonical(&r.rules),
        "rules are not reported in canonical order"
    );

    // and the derivation is a function of the tree: the same tree twice, the same report
    let again = report(&index, &ledger);
    assert_eq!(r, again);
}

/// An empty corpus is an answer, not a crash, and its tallies are zero rather than absent —
/// an empty denominator that reads as 100% is how a governance number stops meaning
/// anything.
#[test]
fn a_repository_with_no_rules_reports_zero_and_not_success_by_vacancy() {
    let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape {
        rules: 0,
        ..Default::default()
    })
    .unwrap();
    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    assert_eq!(r.coverage.rules, 0);
    assert!(r.rules.is_empty());
    assert!(r.states.is_empty());
    assert!(r.findings.is_empty());
    assert_eq!(r.coverage, Coverage::default());
}

/// The converse of `depends_on`, which no rule declares and which every reader wants: if A
/// depends on B, B is required by A, and the relation is derived once rather than restated
/// in each rule.
#[test]
fn the_dependency_relation_is_readable_in_both_directions() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    // rule 1 depends on rule 0; nothing else changes
    let rel = ".ai/repo/rules/project/rule-1.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace("depends_on: []", "depends_on: [project.rule-0]"),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let zero = r.rules.iter().find(|p| p.rule.id == "project.rule-0").unwrap();
    let one = r.rules.iter().find(|p| p.rule.id == "project.rule-1").unwrap();
    assert_eq!(one.rule.depends_on, ["project.rule-0"]);
    assert_eq!(zero.required_by, ["project.rule-1"]);
    assert!(one.required_by.is_empty());
}

/// A blocking rule that names a case which is not in the tree is the defect the whole graph
/// exists to catch: it reads as enforced, and nothing it names can run.
#[test]
fn a_blocking_rule_naming_a_missing_case_is_dangling_and_a_finding() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-2.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  tests: [test/cases/99_deleted_yesterday.sh]",
        ),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-2").unwrap();
    assert_eq!(p.rule.class, Class::Blocking);
    assert_eq!(p.rule.enforcement.mode, Mode::Gated);
    assert_eq!(p.state, RuleState::Dangling);
    assert!(!p.satisfied);
    assert_eq!(p.tests.len(), 1);
    assert!(!p.tests[0].present);
    assert_eq!(r.coverage.blocking, 1);
    assert_eq!(r.coverage.named_proof, 1);
    assert_eq!(r.coverage.artifacts_present, 0);

    let f = r.findings.iter().find(|f| f.rule == "project.rule-2").unwrap();
    assert_eq!(f.state, RuleState::Dangling);
    assert!(f.reason.contains("not in the tree"));
    assert!(!f.reproduce.is_empty(), "a finding carries how to reproduce it");
    assert!(!r.satisfied());
}

/// A blocking rule that names nothing at all is the other half of the same defect, and the
/// one no other check in this repository can express: nothing else knows that a blocking
/// rule is supposed to name anything.
#[test]
fn a_blocking_rule_naming_nothing_is_unproven_and_a_finding() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-3.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace("class: advisory", "class: blocking"),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-3").unwrap();
    assert_eq!(p.state, RuleState::Unproven);
    assert!(!p.satisfied);
    assert!(p.gates.is_empty());
    let f = r.findings.iter().find(|f| f.rule == "project.rule-3").unwrap();
    assert!(f.reason.contains("neither a validator nor a test"));
}

/// A gate is a mechanism and a case is a verdict, and the report must not read one as the
/// other. A rule proved by a check wired as its own CI gate is `gated`: violations are
/// refused, and nothing here says what the last run decided. Reading that as `proven` would
/// be a green badge for a file existing; reading it as `unrunnable` would call a working
/// gate broken.
#[test]
fn a_check_wired_as_a_gate_is_a_mechanism_and_not_a_verdict() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    std::fs::create_dir_all(repo.root().join(".ai/repo/ci")).unwrap();
    std::fs::write(
        repo.root().join(GATES_PATH),
        "gates:\n  - id: alpha-check\n    runs: scripts/ci/alpha-check\n  - id: shell-suite\n    \
         runs: bash test/run.sh\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.root().join("scripts/ci")).unwrap();
    std::fs::write(repo.root().join("scripts/ci/alpha-check"), "#!/bin/sh\n").unwrap();

    let rel = ".ai/repo/rules/project/rule-4.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  tests: [scripts/ci/alpha-check]",
        ),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-4").unwrap();
    assert_eq!(p.tests[0].kind, ArtifactKind::Gate);
    assert_eq!(p.tests[0].gates, ["alpha-check"]);
    assert_eq!(p.state, RuleState::Gated);
    assert!(p.satisfied, "a gate refuses violations, which is the claim");
    assert!(!p.state.passing(), "a gate is not a recorded pass");
    assert_eq!(r.coverage.mechanism_only, 1);
    assert_eq!(r.coverage.passing, 0);
    assert_eq!(r.coverage.gated, 1);
    assert!(r.findings.is_empty());

    // the same path, with no gate running it, is proof in prose only
    std::fs::write(
        repo.root().join(GATES_PATH),
        "gates:\n  - id: shell-suite\n    runs: bash test/run.sh\n",
    )
    .unwrap();
    let r = report(&repo.index().unwrap(), &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-4").unwrap();
    assert_eq!(p.tests[0].kind, ArtifactKind::Unknown);
    assert_eq!(p.state, RuleState::Unrunnable);
    assert!(!p.satisfied, "a blocking rule nothing runs is a finding");
}

/// A rule may name more than one thing, and the report takes the weakest: a rule with a
/// live case and a deleted one is dangling, because the half that does not resolve is the
/// half a reader would have trusted.
#[test]
fn a_rule_is_only_as_proven_as_its_weakest_part() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    std::fs::create_dir_all(repo.root().join("test/cases")).unwrap();
    std::fs::write(repo.root().join("test/cases/01_alive.sh"), "#!/bin/sh\n").unwrap();
    let rel = ".ai/repo/rules/project/rule-5.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  tests: [test/cases/01_alive.sh, \
             test/cases/02_deleted.sh]",
        ),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-5").unwrap();
    assert_eq!(p.tests.len(), 2);
    assert!(p.tests[0].present && !p.tests[1].present);
    assert_eq!(p.state, RuleState::Dangling);
    assert!(!p.satisfied);
}

/// A declared, reasoned exemption is a mode; an undeclared one is the debt it replaces.
///
/// This is the difference between "nobody can automate this, and here is why" and "nobody
/// got round to it", which a single `unproven` bucket cannot express. The reason is not
/// decoration: without it the declaration falls through to declarative, so an exemption
/// cannot be claimed by writing one word.
#[test]
fn review_enforcement_is_the_reason_or_it_is_not_declared() {
    let with_reason = enforcement_of(&json!({"x-majordomus": {
        "reviewed_because": "the rule is about the provenance of code, which no program reads"
    }}));
    assert_eq!(with_reason.mode, Mode::Reviewed);
    assert!(with_reason.reviewed_because.is_some());

    // an empty reason is not a reason, and so not a declaration
    assert_eq!(
        enforcement_of(&json!({"x-majordomus": {"reviewed_because": "  "}})).mode,
        Mode::Declarative
    );
    // and naming the dispatching commands is not one either: that field says which commands
    // run a validator, which is a different claim entirely
    assert_eq!(
        enforcement_of(&json!({"x-majordomus": {"enforced_by": ["check", "watch"]}})).mode,
        Mode::Declarative
    );

    // an executable proof always wins: a rule that acquires a case stops being
    // review-enforced without anyone remembering to delete the declaration
    let both = enforcement_of(&json!({"x-majordomus": {
        "reviewed_because": "r", "tests": ["test/cases/07_scope.sh"]
    }}));
    assert_eq!(both.mode, Mode::Gated);
}

/// A blocking rule that declares review is not a finding — and is not counted as proven
/// either. The number that must be visible is how many rules only a reader enforces, and
/// it is counted apart from every executable mode so that a summary cannot fold it into a
/// total and make governance look complete.
#[test]
fn a_reviewed_rule_is_counted_apart_and_never_as_proof() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-6.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  reviewed_because: the rule is about the \
             provenance of code, which no program reads",
        ),
    )
    .unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-6").unwrap();
    assert_eq!(p.rule.enforcement.mode, Mode::Reviewed);
    assert_eq!(p.state, RuleState::Reviewed);
    assert!(p.satisfied, "a declared, reasoned exemption is not a finding");
    assert!(!p.state.passing());
    assert_eq!(r.coverage.review_only, 1);
    assert_eq!(
        r.coverage.named_proof, 0,
        "a reviewed rule names no executable proof and must not be counted as if it did"
    );
    assert_eq!(r.coverage.passing, 0);
    assert_eq!(r.coverage.gated, 0);
    assert!(r.findings.is_empty());
}

/// A validator is a shell function, not a path, and reading it as one is the defect this
/// test keeps fixed.
///
/// `validator: adr` names `mj_validate_adr`, which the doctrine dispatcher calls and which
/// `lib/` defines. Resolving it against the filesystem makes every dispatched rule in the
/// vendored package report as naming something that is not in the tree — the opposite of
/// true, because those are the best-enforced rules the repository has. `test/cases/133`
/// found it; this is the unit half.
#[test]
fn a_validator_is_a_function_in_lib_and_not_a_path() {
    let dir = std::env::temp_dir().join(format!("majordomus-rules-val-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(
        dir.join("lib/doctrine.sh"),
        "#!/usr/bin/env bash\nmj_validate_scope() { :; }\n  mj_validate_indented() { :; }\n",
    )
    .unwrap();
    // a file that only mentions the name is not a definition
    std::fs::write(
        dir.join("lib/other.sh"),
        "# see mj_validate_scope() for the shape\necho mj_validate_ghost\n",
    )
    .unwrap();
    // and a file that is not shell is not scanned
    std::fs::write(dir.join("lib/notes.md"), "mj_validate_ghost() { :; }\n").unwrap();

    assert_eq!(
        validator_defined_in(&dir, "scope").as_deref(),
        Some("lib/doctrine.sh")
    );
    // leading whitespace is still a definition
    assert_eq!(
        validator_defined_in(&dir, "indented").as_deref(),
        Some("lib/doctrine.sh")
    );
    assert_eq!(validator_defined_in(&dir, "ghost"), None);
    assert_eq!(validator_defined_in(&dir, "absent"), None);
    // a tree with no lib/ at all answers, rather than panicking
    assert_eq!(
        validator_defined_in(&dir.join("nowhere"), "scope"),
        None
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A dispatched rule whose validator nothing defines is dangling — the same finding as a
/// deleted case, because a dispatcher that cannot find its function enforces nothing.
#[test]
fn a_dispatched_rule_whose_validator_is_undefined_is_dangling() {
    let repo = crate::synthetic::SyntheticRepository::small().unwrap();
    let rel = ".ai/repo/rules/project/rule-7.v1.md";
    let text = std::fs::read_to_string(repo.root().join(rel)).unwrap();
    std::fs::write(
        repo.root().join(rel),
        text.replace(
            "class: advisory",
            "class: blocking\n\nx-majordomus:\n  validator: nothing_defines_this\n  category: x\n  \
             exit_code: 10\n  enforced_by: [check]\n  tests: [test/cases/01_alive.sh]",
        ),
    )
    .unwrap();
    std::fs::create_dir_all(repo.root().join("test/cases")).unwrap();
    std::fs::write(repo.root().join("test/cases/01_alive.sh"), "#!/bin/sh\n").unwrap();

    let index = repo.index().unwrap();
    let ledger = Ledger::load(repo.root()).unwrap();
    let r = report(&index, &ledger);
    let p = r.rules.iter().find(|p| p.rule.id == "project.rule-7").unwrap();
    assert_eq!(p.rule.enforcement.mode, Mode::Dispatched);
    let v = p.validator.as_ref().expect("a dispatched rule carries its validator");
    assert_eq!(v.function, "mj_validate_nothing_defines_this");
    assert!(!v.present);
    assert!(v.defined_in.is_none());
    assert_eq!(p.state, RuleState::Dangling);
    assert!(!p.satisfied);
}

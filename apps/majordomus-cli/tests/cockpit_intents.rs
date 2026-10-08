//! The Cockpit's intent pages through the served executable: the page of one intent says why
//! its verdict is what it is and shows the review of its plan, the list says the verdict, and
//! every word of them is the word the command line answers on the same repository.

// claims: intent-cockpit-pages

mod common;

use common::{run_in, Fixture, Served};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use serde_json::Value;

const PAGE: &str = "/cockpit/intents/fixture-intent";

fn json_of(f: &Fixture, args: &[&str]) -> Value {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (_, out, err) = run_in(&f.root(), &argv, "");
    serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"))
}

/// The page a fresh server answers for `target`, with its status.
fn page(f: &Fixture, target: &str) -> (u16, String) {
    let served = Served::start(&f.root(), &[]);
    let (status, headers, body) = served.request("GET", target, None);
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "content-type" && v.starts_with("text/html")),
        "{target} is not HTML: {headers:?}"
    );
    (status, body)
}

/// Record one run of `test` at HEAD over a clean tree, and commit the ledger.
fn record(f: &Fixture, test: &str, outcome: Outcome) {
    let id = TestId::of(test).unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let mut ledger = Ledger::load(&f.root()).unwrap_or_else(|_| Ledger::empty());
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest: digest_of(&source),
        at: "2026-10-08T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("record a run");
}

/// The fixture with a guard on its intent and an optional criterion nobody ran.
fn guarded() -> Fixture {
    let f = Fixture::new();
    f.write("test/cases/01_guard.sh", ". \"$ROOT/test/lib.sh\"\ntrue\n");
    f.write("test/cases/02_never.sh", ". \"$ROOT/test/lib.sh\"\ntrue\n");
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &(common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            "    ref: test/cases/00_x.sh\n  - id: nice-to-have\n    criterion: It would be nice\n    evidence: test\n    ref: test/cases/02_never.sh\n    optional: true\n",
        ) + "guards:\n  - id: stays-valid\n    invariant: The fixture stays a valid repository\n    evidence: test\n    ref: test/cases/01_guard.sh\n"),
    );
    f.commit("a guard and an optional criterion");
    f
}

/// The words of the first `n` badges after `anchor`: what a row is read in, in order.
fn badges_after<'a>(html: &'a str, anchor: &str, n: usize) -> Vec<&'a str> {
    let from = html
        .find(anchor)
        .unwrap_or_else(|| panic!("no {anchor} in the page"));
    html[from..]
        .split("mj-badge-dot\" aria-hidden=\"true\"></span>")
        .skip(1)
        .take(n)
        .map(|rest| rest.split('<').next().unwrap())
        .collect()
}

fn badge_after<'a>(html: &'a str, anchor: &str) -> &'a str {
    badges_after(html, anchor, 1)[0]
}

/// The word of the first tag after `anchor`.
fn tag_after<'a>(html: &'a str, anchor: &str) -> &'a str {
    let from = html
        .find(anchor)
        .unwrap_or_else(|| panic!("no {anchor} in the page"));
    html[from..]
        .split("class=\"mj-tag\">")
        .nth(1)
        .unwrap_or_else(|| panic!("no tag after {anchor}"))
        .split('<')
        .next()
        .unwrap()
}

/// Criterion `verdict-is-explained-on-the-page`: the verdict's reasons, the judged runs, the
/// optional marker and the guards are on the page, each in the word `intent explain` answers.
#[test]
fn the_intent_page_explains_the_verdict() {
    let f = guarded();
    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    record(&f, "test/cases/01_guard.sh", Outcome::Fail);
    let head = f.git(&["rev-parse", "HEAD~1"]).trim().to_string();

    let explained = json_of(&f, &["intent", "explain", "fixture-intent"]);
    let intent = &explained["intent"];
    assert_eq!(intent["verdict"]["state"], "unsatisfied", "{intent}");
    assert_eq!(intent["guards"][0]["standing"], "violated");

    let (status, html) = page(&f, PAGE);
    assert_eq!(status, 200);
    // the verdict, in the word the command line answers, and what holds it back
    assert!(html.contains("Why the verdict is what it is"), "{html}");
    assert_eq!(
        badge_after(&html, ">Verdict</dt>"),
        intent["verdict"]["state"].as_str().unwrap()
    );
    assert!(html.contains("href=\"#guard-stays-valid\""), "{html}");
    assert!(
        !html.contains("href=\"#criterion-nice-to-have\""),
        "an optional criterion is no reason: {html}"
    );
    // each criterion in the state the command line answers, with whether it is required
    for c in intent["satisfaction"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let anchor = format!("id=\"criterion-{id}\"");
        assert_eq!(
            tag_after(&html, &anchor),
            if c["optional"] == true {
                "optional"
            } else {
                "required"
            },
            "{id}"
        );
        let state = c["state"].as_str().unwrap().replace('_', " ");
        assert!(
            badge_after(&html, &anchor).starts_with(&state),
            "{id}: the page does not say {state}"
        );
    }
    // the run the met criterion was judged by
    let judged = intent["satisfaction"][0]["evaluation"]["commit"]
        .as_str()
        .unwrap();
    assert!(html.contains(&judged[..10]), "{html}");
    assert!(head.starts_with(&judged[..10]) || !judged.is_empty());
    // the guard, in the standing the engine answers
    assert_eq!(
        badge_after(&html, "id=\"guard-stays-valid\""),
        intent["guards"][0]["standing"].as_str().unwrap()
    );
    // what the record declares beside its criteria
    assert!(
        html.contains("The fixture stays a valid repository"),
        "{html}"
    );
    assert!(html.contains("rule:project.alpha"), "{html}");

    // the guard passes: it holds, the page says so, and nothing holds the verdict back
    record(&f, "test/cases/01_guard.sh", Outcome::Pass);
    let (_, html) = page(&f, PAGE);
    assert_eq!(badge_after(&html, "id=\"guard-stays-valid\""), "holds");
    assert_eq!(badge_after(&html, ">Verdict</dt>"), "satisfied");
    assert!(
        html.contains("Every required criterion has current evidence"),
        "{html}"
    );

    // an intent without a guard has no card for guards
    let plain = Fixture::new();
    let (_, html) = page(&plain, PAGE);
    assert!(!html.contains("Guards: what must stay true"), "{html}");
}

/// Criterion `review-is-on-the-page`: the review's state against the plan, the disposition,
/// the stamp and every finding with its resolution, as `intent oppose` answers them.
#[test]
fn the_intent_page_shows_the_review() {
    let f = Fixture::new();
    let (_, html) = page(&f, PAGE);
    assert!(html.contains("The review of its plan"), "{html}");
    assert!(
        html.contains("No critique of this plan is recorded"),
        "{html}"
    );
    assert_eq!(badge_after(&html, ">Review</dt>"), "none");

    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        "intent: fixture-intent\nreviewed_at: HEAD\nreviewed_by: a reviewer\nfindings:\n  - id: thin\n    class: insufficient_work\n    subject: fixture-intent#the-case-passes\n    finding: One case is not enough\n    source: a reviewer\n    blocking: true\n    resolution:\n      state: open\n",
    );
    f.commit("a critique with an open blocker");
    let opposed = json_of(&f, &["intent", "oppose", "fixture-intent"]);
    assert_eq!(opposed["disposition"], "reject", "{opposed}");
    let (status, html) = page(&f, PAGE);
    assert_eq!(status, 200);
    assert_eq!(
        badge_after(&html, ">Review</dt>"),
        opposed["review"]["state"]
            .as_str()
            .unwrap()
            .replace('_', " ")
    );
    assert_eq!(
        badge_after(&html, ">Disposition</dt>"),
        opposed["disposition"].as_str().unwrap()
    );
    assert!(html.contains("mj-badge--bad"), "{html}");
    assert!(html.contains("What rejects the plan"), "{html}");
    for line in opposed["rejecting"].as_array().unwrap() {
        assert!(html.contains(line.as_str().unwrap()), "{line}\n{html}");
    }
    assert!(html.contains("One case is not enough"), "{html}");
    assert!(html.contains("a reviewer"), "{html}");

    // stamped: the review is current
    let (code, _, err) = run_in(
        &f.root(),
        &["intent", "stamp", "fixture-intent", "--by", "the test"],
        "",
    );
    assert_eq!(code, 0, "{err}");
    f.commit("the review is stamped");
    let (_, html) = page(&f, PAGE);
    assert_eq!(badge_after(&html, ">Review</dt>"), "current");
    // the stamp is on the page: who reviewed, and the tool that stamped it
    let stamped = json_of(&f, &["intent", "oppose", "fixture-intent"]);
    for field in ["reviewed_by", "reviewed_at", "reviewed_with"] {
        let value = stamped["review"][field].as_str().unwrap();
        assert!(!value.is_empty(), "{field}: {stamped}");
        assert!(
            html.contains(&format!("<code class=\"mj-mono\">{value}</code>")),
            "{field} {value}\n{html}"
        );
    }
}

/// Criterion `list-says-the-verdict`: the list shows each intent's verdict beside its stage,
/// the one `intent_realization.work` answers, and links the intent's page.
#[test]
fn the_intent_list_shows_the_verdict() {
    let f = Fixture::new();
    let work = json_of(&f, &["intent", "realization"]);
    let view = work["intents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["intent"] == "fixture-intent")
        .unwrap_or_else(|| panic!("{work}"));
    let listed = json_of(&f, &["intent", "list"]);
    let verdict = view["verdict"].as_str().unwrap();
    assert!(
        listed
            .to_string()
            .contains(&format!("\"state\":\"{verdict}\"")),
        "the realization and the intent list disagree: {listed}"
    );

    let (status, html) = page(&f, "/cockpit/intents");
    assert_eq!(status, 200);
    assert!(html.contains(">Verdict</th>"), "{html}");
    let row = "href=\"/cockpit/intents/fixture-intent\"";
    // the stage badge, then the verdict's
    assert_eq!(badges_after(&html, row, 2)[1], verdict, "{html}");

    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    let (_, html) = page(&f, "/cockpit/intents");
    assert_eq!(badges_after(&html, row, 2)[1], "satisfied");
}

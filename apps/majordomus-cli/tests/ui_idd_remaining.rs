//! Every milestone of the open intent says what remains before it is DONE (I2808), over a
//! real socket.
//!
//! `devtask.issue` is asked of every open issue, so the screen draws a placeholder naming its
//! own route and the script fills it. That route names, per milestone of the intent, each issue
//! not complete with its readiness and the reason, the evidence its record requires and does
//! not hold, and its validation — the capabilities' words, as they answer them — and says that
//! nothing remains once every issue is done.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#a-milestone-says-what-remains`.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};

const REMAINING: &str = "/ui/panels/remaining?intent=fixture-intent";

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[test]
fn every_milestone_names_its_open_issues_and_the_evidence_they_owe() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains(&format!("data-panel-src=\"{}\"", escaped(REMAINING))),
        "the first paint leaves what remains to its own route: {page}"
    );

    let (status, task) = s.get("/api/v1/devtask/issue?issue=I0001");
    assert_eq!(status, 200, "{task}");
    let (status, _, panel) = s.request("GET", REMAINING, None);
    assert_eq!(status, 200, "{panel}");
    assert!(
        panel.contains("fixture-milestone"),
        "the panel names the intent's milestone"
    );
    assert!(panel.contains("0 of 1 issue(s) complete"), "{panel}");
    assert!(
        panel.contains("id=\"remaining-I0001\""),
        "the open issue is listed: {panel}"
    );
    for pointer in ["/readiness/state", "/readiness/reason"] {
        let value = task
            .pointer(pointer)
            .and_then(|v| v.as_str())
            .expect("devtask answers it");
        assert!(
            panel.contains(&escaped(value)),
            "the panel does not show {pointer} `{value}` as devtask answers it"
        );
    }
    // the token the record requires and does not hold, and the validation it names
    assert!(
        panel.contains(">proof<"),
        "the uncovered token is not named: {panel}"
    );
    assert!(
        panel.contains(">true<"),
        "the validation command is not named: {panel}"
    );
    assert!(
        panel.contains("devtask.milestone"),
        "the panel names its source"
    );

    // a route that names no intent says what it needs
    let (status, _, empty) = s.request("GET", "/ui/panels/remaining", None);
    assert_eq!(status, 200);
    assert!(empty.contains("Name an intent"), "{empty}");
}

#[test]
fn a_milestone_whose_issues_are_all_done_says_nothing_remains() {
    let f = Fixture::new();
    // the fixture's one issue, its one token covered, then moved through start, verify and
    // done by the capability that moves an issue, so its stamps carry their events
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &format!(
            "{}evidence:\n  - covers: proof\n    type: test\n    command: \"true\"\n    result: \"pass\"\n    commit: \"0000000000000000000000000000000000000000\"\n    recorded_at: 2026-10-17T10:09:00Z\n",
            common::ISSUE
        ),
    );
    for transition in ["start", "verify", "done"] {
        let out = common::majordomus()
            .args(["run", "plan.transition", "--input"])
            .arg(format!(
                r#"{{"issue":"I0001","transition":"{transition}"}}"#
            ))
            .current_dir(f.root())
            .env("MAJORDOMUS_SHARE", common::dist_share())
            .output()
            .expect("run plan.transition");
        assert!(
            out.status.success(),
            "{transition} was refused: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = Served::start(&f.root(), &[]);

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (status, _, panel) = s.request("GET", REMAINING, None);
        assert_eq!(status, 200, "{panel}");
        if panel.contains("No issue remains") {
            assert!(panel.contains("1 of 1 issue(s) complete"), "{panel}");
            assert!(
                !panel.contains("id=\"remaining-I0001\""),
                "a done issue is listed: {panel}"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the panel never said nothing remains: {panel}"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

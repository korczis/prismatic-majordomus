//! The intent workspace folds the scope conflicts among its issues to the first few and lists
//! every one of them on its own panel route (I3840), over a real socket.
//!
//! An intent of many issues has scope conflicts by the thousand; drawn inline they were most of
//! /ui/idd and kept it over its 100 ms budget. The route answers them all, and refuses to
//! guess an intent it was not given.

mod common;

use common::{Fixture, Served, ISSUE};

const PANEL: &str = "/ui/panels/shared-scope";

#[test]
fn the_shared_scope_route_lists_every_pair_of_the_intents_issues() {
    let f = Fixture::new();
    // a second issue of the intent's milestone in the same scope as I0001
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &ISSUE
            .replace("id: I0001", "id: I0002")
            .replace("slug: issue-I0001", "slug: issue-I0002"),
    );
    f.commit("a second issue in I0001's scope");
    let s = Served::start(&f.root(), &[]);

    let (status, _, panel) = s.request("GET", &format!("{PANEL}?intent=fixture-intent"), None);
    assert_eq!(status, 200, "{panel}");
    assert!(
        panel.contains("Which of its issues share a scope?"),
        "{panel}"
    );
    for id in ["I0001", "I0002"] {
        assert!(
            panel.contains(&format!("issue={id}")),
            "{id} is not a link: {panel}"
        );
    }
    assert!(panel.contains("lib"), "the shared path is said: {panel}");

    // the workspace folds the same pair, and one pair needs no link to the rest
    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("Shared scope: "), "{page}");
    assert!(
        !page.contains(PANEL),
        "a fold under its bound links nowhere: {page}"
    );

    // no intent, an unknown one, and one no id could be: the route says what it needs
    for query in ["", "?intent=no-such-intent", "?intent=x%22%3E%3Cscript%3E"] {
        let (status, _, empty) = s.request("GET", &format!("{PANEL}{query}"), None);
        assert_eq!(status, 200, "{query}: {empty}");
        assert!(empty.contains("Name an intent"), "{query}: {empty}");
        assert!(!empty.contains("x\"><script>"), "{query}: {empty}");
    }
}

#[test]
fn an_intent_whose_issues_share_no_scope_says_so() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, panel) = s.request("GET", &format!("{PANEL}?intent=fixture-intent"), None);
    assert_eq!(status, 200, "{panel}");
    assert!(
        panel.contains("No two issues of this intent share a scope."),
        "{panel}"
    );
}

//! Every intent, milestone and issue inspector of the UI leads to the intent workspace of the
//! intent it belongs to (I2811), over a real socket.
//!
//! Which intent a milestone or an issue belongs to is the plan's own relation (`serves`); the
//! links are derived from it and lead to pages the server answers. Through them the published
//! snapshot, which follows links, reaches every intent's workspace.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#every-inspector-leads-to-its-workspace`.

mod common;

use common::{Fixture, Served};

const WORKSPACE: &str = "/ui/idd?intent=fixture-intent";

#[test]
fn every_inspector_leads_to_the_workspace_of_its_intent() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // the intent's inspector on the roadmap
    let (status, _, page) = s.request("GET", "/ui/roadmap?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains(&format!("href=\"{WORKSPACE}\"")) && page.contains("Open its workspace"),
        "the intent's inspector does not lead to its workspace"
    );

    // the milestone's inspector: the workspace of every intent it serves
    let (status, _, page) = s.request("GET", "/ui/roadmap?milestone=fixture-milestone", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains(&format!("href=\"{WORKSPACE}\"")),
        "the milestone's inspector does not lead to the workspace of the intent it serves"
    );

    // the issue's detail: its move, in the workspace of the intent it serves
    let (status, _, page) = s.request("GET", "/ui/prs?issue=I0001", None);
    assert_eq!(status, 200, "{page}");
    let to_move = "/ui/idd?intent=fixture-intent&amp;issue=I0001#move";
    assert!(
        page.contains(&format!("href=\"{to_move}\"")),
        "the issue does not lead to its move in its intent's workspace"
    );

    // every one of those links is a page the server answers, the move panel among them
    // a browser keeps the fragment to itself: the server is asked for the page alone
    let page_of_move = to_move.replace("&amp;", "&");
    let page_of_move = page_of_move.split('#').next().expect("a path");
    let (status, _, workspace) = s.request("GET", page_of_move, None);
    assert_eq!(status, 200);
    assert!(
        workspace.contains("id=\"move\""),
        "the move panel is not where the link leads"
    );
    let (status, _, workspace) = s.request("GET", WORKSPACE, None);
    assert_eq!(status, 200);
    assert!(
        !workspace.contains("Open its workspace"),
        "the workspace's own inspector leads to itself"
    );
}

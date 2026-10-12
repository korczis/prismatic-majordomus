//! `majordomus ui`, the primary surface (ADR 0135), over a real socket.
//!
//! The server a person starts serves the UI at `/ui` beside the Cockpit. These tests hold
//! the contract a reader relies on:
//! - every screen is complete HTML under the Cockpit's security headers;
//! - every value names the capability it was read from;
//! - the slow pull-request queue is a route of its own, never something the first paint
//!   waits for;
//! - the topology lists the UI as a public surface.

mod common;

use common::{Fixture, Served};

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_dashboard_is_served_at_ui_under_the_cockpits_policy() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, headers, page) = s.request("GET", "/ui", None);
    assert_eq!(status, 200, "{page}");
    assert!(header(&headers, "content-type").is_some_and(|v| v.starts_with("text/html")));
    let (_, cockpit_headers, _) = s.request("GET", "/cockpit", None);
    assert_eq!(
        header(&headers, "content-security-policy"),
        header(&cockpit_headers, "content-security-policy"),
        "the UI and the Cockpit carry one policy"
    );
    assert_eq!(header(&headers, "x-content-type-options"), Some("nosniff"));

    // the frame, the screen and its sources
    assert!(page.contains("<title>Dashboard · majordomus ui</title>"));
    assert!(
        page.contains("href=\"/cockpit\""),
        "the UI links down to the Cockpit"
    );
    for capability in [
        "health.report",
        "release.version",
        "distribution.releases",
        "plan.status",
        "plan.roadmap",
        "mesh.peers",
    ] {
        assert!(
            page.contains(&format!("/cockpit/capabilities/{capability}")),
            "the dashboard does not cite {capability}"
        );
    }
    // the queue is a link to its own route, not something this page waited for
    assert!(page.contains("data-panel-src=\"/ui/panels/pull-requests\""));
    assert!(!page.contains("/cockpit/capabilities/integration.queue"));
}

#[test]
fn the_queue_panel_is_a_page_and_a_fragment() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/panels/pull-requests", None);
    assert_eq!(status, 200);
    assert!(
        page.starts_with("<!doctype html>"),
        "without ?fragment it is a whole page"
    );
    assert!(page.contains("integration.queue"));

    let (status, _, fragment) = s.request("GET", "/ui/panels/pull-requests?fragment", None);
    assert_eq!(status, 200);
    assert!(
        fragment.starts_with("<section"),
        "the fragment is the panel alone: {fragment}"
    );
    assert!(!fragment.contains("<html"));
}

#[test]
fn the_ui_is_a_public_surface_and_answers_only_reads() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, surfaces) = s.get("/api/v1/web/surfaces");
    assert_eq!(status, 200);
    let text = surfaces.to_string();
    assert!(
        text.contains("\"ui\""),
        "the topology does not list the UI: {text}"
    );

    let (status, _, _) = s.request("POST", "/ui", Some("{}"));
    assert_eq!(status, 405);
    let (status, _, page) = s.request("GET", "/ui/nowhere", None);
    assert_eq!(status, 404);
    assert!(page.contains("/ui/nowhere"));
    let (status, _, css) = s.request("GET", "/ui/assets/ui.css", None);
    assert_eq!(status, 200);
    assert!(!css.is_empty());
}

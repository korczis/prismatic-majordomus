//! No link on `majordomus ui` is dead (I2482).
//!
//! A real server over a fixture: every screen the navigation lists is read, every link it
//! draws inside the server is followed — the UI's own routes, its inspectors and the technical
//! views of objects in the Cockpit — and each must answer 200. A link the UI draws to a page
//! that answers 404 is the dead end ADR 0135 rules out, and this is the test that finds one.

mod common;

use std::collections::{BTreeSet, VecDeque};

use common::{Fixture, Served};

/// How many pages the crawl reads before it stops: the screens, their inspectors and the
/// objects they name, with room to spare. A crawl that reaches it fails, so a UI that grows a
/// link per row of an unbounded list is seen rather than read forever.
const PAGES: usize = 400;

/// Every `href="…"` of a page, with the entities the renderer escapes undone.
fn hrefs(page: &str) -> Vec<String> {
    page.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(|h| {
            h.replace("&amp;", "&")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
        })
        .collect()
}

/// Whether a link stays inside the server and is one this test follows: a UI route or the
/// Cockpit's page of one object. Assets are the asset audit's, other Cockpit pages the
/// Cockpit's own tests', and an address elsewhere is not this server's to answer.
fn followed(href: &str) -> bool {
    (href == "/ui" || href.starts_with("/ui/") || href.starts_with("/ui?"))
        && !href.starts_with("/ui/assets/")
        || href.starts_with("/cockpit/object?")
}

/// The page a link names: without its fragment, which the server never sees.
fn page_of(href: &str) -> String {
    href.split('#').next().unwrap_or(href).to_string()
}

#[test]
fn every_link_the_ui_draws_inside_the_server_answers() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, front) = s.request("GET", "/ui", None);
    assert_eq!(status, 200, "{front}");
    let screens: BTreeSet<String> = hrefs(&front)
        .into_iter()
        .filter(|h| followed(h) && !h.contains('?') && !h.starts_with("/cockpit/"))
        .map(|h| page_of(&h))
        .collect();
    assert!(
        screens.len() >= 13,
        "the navigation lists every screen: {screens:?}"
    );

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<(String, String)> = screens
        .iter()
        .map(|s| (s.clone(), "/ui".to_string()))
        .collect();
    let mut dead = Vec::new();
    let mut objects = 0usize;
    while let Some((target, from)) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }
        assert!(
            seen.len() <= PAGES,
            "the crawl passed {PAGES} pages; a list draws a link per row without bound"
        );
        let (status, _, page) = s.request("GET", &target, None);
        if status != 200 {
            dead.push(format!("{target} (linked from {from}) answered {status}"));
            continue;
        }
        if target.starts_with("/cockpit/") {
            objects += 1;
            // the technical view is followed for its own answer, not crawled further
            continue;
        }
        for h in hrefs(&page) {
            if followed(&h) {
                let p = page_of(&h);
                if !seen.contains(&p) {
                    queue.push_back((p, target.clone()));
                }
            }
        }
    }
    assert!(
        dead.is_empty(),
        "dead links on the UI:\n{}",
        dead.join("\n")
    );
    for screen in &screens {
        assert!(seen.contains(screen), "{screen} was never read");
    }
    // the fixture's objects are named somewhere: the crawl reached at least one technical view
    assert!(
        objects > 0,
        "no screen linked an object's technical view; read {} pages",
        seen.len()
    );
}

#[test]
fn the_crawl_reads_hrefs_and_follows_only_the_servers_own_pages() {
    let page = r#"<a href="/ui/roadmap?milestone=M1&amp;x=1#milestone">m</a>
        <a href="https://example.test/">e</a><a href="/ui/assets/ui.css">a</a>
        <a href="/cockpit/object?uri=majordomus%3A%2F%2Ftest%2Fa">o</a><a href="/cockpit">c</a>"#;
    let all = hrefs(page);
    assert_eq!(all[0], "/ui/roadmap?milestone=M1&x=1#milestone");
    let kept: Vec<&String> = all.iter().filter(|h| followed(h)).collect();
    assert_eq!(
        kept,
        [
            "/ui/roadmap?milestone=M1&x=1#milestone",
            "/cockpit/object?uri=majordomus%3A%2F%2Ftest%2Fa"
        ]
    );
    assert_eq!(page_of(&all[0]), "/ui/roadmap?milestone=M1&x=1");
}

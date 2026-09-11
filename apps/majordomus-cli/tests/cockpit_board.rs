//! The Cockpit's board over a real socket: `/cockpit/board`, the page that shows who else
//! is working in this repository and where this checkout's server stands.
//!
//! The behavioural claim these tests exist for: a session that attaches to this server and
//! says what it is working on appears on a page a person can open, and when two sessions
//! claim ground that meets, the page says so in words rather than leaving it to be found
//! afterwards in the history of a branch. Nothing here plants a fixture on the board —
//! every peer is attached over MCP at `/mcp` the way a real client attaches, and every
//! announcement is made with the tool a real client calls.
//!
//! The page is `majordomus_cli::cockpit::pages::board`, laid out from `peers.list` and
//! `server.status` and from nothing else.

mod common;

use common::{Fixture, Served};

/// Attach an MCP session over HTTP and answer with its session id, which is how every
/// later call on that session is recognised — and which is what makes it a *peer* rather
/// than an anonymous HTTP caller.
fn attach(served: &Served, client: &str, version: &str) -> String {
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"{client}","version":"{version}"}}}}}}"#
    );
    let (status, headers, answer) = served.request("POST", "/mcp", Some(&body));
    assert_eq!(status, 200, "initialize: {answer}");
    headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("initialize answered no session id: {headers:?}"))
}

/// Announce an intent and a scope on an attached session, with the tool a client calls.
fn announce(served: &Served, session: &str, intent: &str, scope: &[&str]) -> String {
    announce_claim(served, session, None, intent, scope)
}

/// The same, under a name: one session that fans work out to several workers holds one
/// named claim per worker, and every one of them stands beside the others.
fn announce_claim(
    served: &Served,
    session: &str,
    claim: Option<&str>,
    intent: &str,
    scope: &[&str],
) -> String {
    let claims: Vec<String> = scope.iter().map(|s| format!("\"{s}\"")).collect();
    let named = match claim {
        Some(c) => format!(r#","claim":"{c}""#),
        None => String::new(),
    };
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"majordomus_announce","arguments":{{"intent":"{intent}","scope":[{}]{named}}}}}}}"#,
        claims.join(",")
    );
    let (status, _, answer) =
        served.request_with("POST", "/mcp", Some(&body), &[("Mcp-Session-Id", session)]);
    assert_eq!(status, 200, "announce: {answer}");
    assert!(!answer.contains("\"error\""), "announce failed: {answer}");
    answer
}

/// End an attached session. A shared server outlives the client that started it while any
/// peer is still attached — that is the point of it — so a test that attaches one and then
/// only closes stdin waits for the session to time out. Ending them is what a client does
/// when it exits, and it is what makes this file take seconds rather than minutes.
fn detach(served: &Served, session: &str) {
    let (status, _, body) =
        served.request_with("DELETE", "/mcp", None, &[("Mcp-Session-Id", session)]);
    assert_eq!(status, 204, "the session did not end: {body}");
}

fn page(served: &Served, target: &str) -> String {
    let (status, headers, body) = served.request("GET", target, None);
    assert_eq!(status, 200, "{target}: {body}");
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "content-type" && v.starts_with("text/html")),
        "{target} is not HTML: {headers:?}"
    );
    assert!(body.starts_with("<!doctype html>"), "{target}");
    assert!(body.trim_end().ends_with("</html>"), "{target}");
    body
}

#[test]
fn the_board_names_every_session_and_what_it_announced() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let claude = attach(&served, "claude-code", "2.1.268");
    announce(
        &served,
        &claude,
        "the ordering rule",
        &["apps/majordomus-cli/src/order.rs"],
    );
    let codex = attach(&served, "codex", "0.31");
    announce(&served, &codex, "the installer", &["site"]);
    // a session that attached and never said anything is on the board too: a silent
    // worker is exactly what a board exists to make visible
    let _quiet = attach(&served, "gemini-cli", "0.4");

    let body = page(&served, "/cockpit/board");

    // every client is named by the name it gave in its own initialize
    for client in ["claude-code", "codex", "gemini-cli"] {
        assert!(body.contains(client), "the board does not name {client}");
    }
    // and by the id the board gave it
    for id in ["p1", "p2", "p3"] {
        assert!(body.contains(&format!(">{id}<")), "no peer {id}: {body}");
    }
    // what each said it is doing, and the ground it claimed
    assert!(body.contains("the ordering rule"), "no intent on the board");
    assert!(
        body.contains("apps/majordomus-cli/src/order.rs"),
        "no claim on the board"
    );
    assert!(body.contains("the installer"), "no second intent");
    assert!(
        body.contains("said nothing"),
        "a session that announced nothing is not shown as one"
    );
    // three sessions have claimed nothing in common, and the page says so rather than
    // leaving the reader to infer it from an absence
    assert!(
        body.contains("No two sessions on this board have claimed the same ground"),
        "the empty collision state is not stated"
    );

    for session in [&claude, &codex, &_quiet] {
        detach(&served, session);
    }
}

#[test]
fn two_sessions_that_claim_ground_that_meets_are_shown_as_a_collision() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let first = attach(&served, "claude-code", "2.1.268");
    announce(&served, &first, "the whole crate", &["apps/majordomus-cli"]);
    let second = attach(&served, "codex", "0.31");
    announce(
        &served,
        &second,
        "one page of the cockpit",
        &["apps/majordomus-cli/src/cockpit/pages.rs"],
    );

    let body = page(&served, "/cockpit/board");

    // both sides of the pair are named, which the answer alone does not do: `peers.list`
    // names only the far peer of each overlap, and the near one is resolved out of the
    // announcements in the same answer
    assert!(
        body.contains("p2 codex and p1 claude-code"),
        "the collision does not name both sessions: {body}"
    );
    // both intents, so a reader knows what the two of them think they are doing
    assert!(body.contains("the whole crate"), "no first intent");
    assert!(body.contains("one page of the cockpit"), "no second intent");
    // the claims that meet, one pair per row
    assert!(
        body.contains("apps/majordomus-cli/src/cockpit/pages.rs"),
        "the meeting claim is not shown"
    );
    // and it is an alert, not a row in a table somebody has to notice
    assert!(
        body.contains("mj-alert--fail") && body.contains("claim(s) meet"),
        "the collision is not raised as an alert: {body}"
    );
    assert!(
        body.contains("mj-badge--blocked"),
        "a collision between two attached sessions is not marked as one"
    );

    detach(&served, &first);
    detach(&served, &second);
}

#[test]
fn the_board_shows_where_this_checkouts_server_stands() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let body = page(&served, "/cockpit/board");

    // the server answering this very request holds the lease, and says so with the
    // standing `server.status` decided, not a word this page chose
    assert!(
        body.contains("The lease this process holds"),
        "the answering process does not name its own lease"
    );
    assert!(
        body.contains(">ready<"),
        "a server answering its own Cockpit does not call itself ready: {body}"
    );
    // the address it is bound to is on the page, and it is the one the test is talking to
    assert!(
        body.contains(&served.address),
        "the board does not show the address this request arrived on"
    );
    // and the wide answer: this checkout among the checkouts of the repository
    assert!(
        body.contains("Every checkout of this repository"),
        "the wide answer is missing"
    );
    assert!(
        body.contains(&f.root().display().to_string()),
        "this checkout is not among them"
    );
    assert!(
        body.contains("Would serve"),
        "what the standing is measured against is not shown"
    );
}

#[test]
fn the_board_is_reached_from_every_page_because_it_declares_itself_an_area() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let session = attach(&served, "claude-code", "2.1.268");

    // derived, not listed: the sidebar is built from the areas that declare themselves, so
    // a page reachable from the overview is a page the probe's crawl and every other
    // reader finds without being told
    for from in ["/cockpit", "/cockpit/health", "/cockpit/board"] {
        let body = page(&served, from);
        assert!(
            body.contains("href=\"/cockpit/board\""),
            "{from} does not link the board"
        );
        assert!(body.contains(">Board<"), "{from} does not label it");
    }
    // the entry carries how many sessions are attached, which is a fact and not decoration
    let overview = page(&served, "/cockpit");
    assert!(
        overview.contains("mj-nav-count"),
        "the navigation carries no counts at all"
    );

    detach(&served, &session);
}

#[test]
fn a_board_nobody_is_on_still_renders_and_says_what_it_is() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let body = page(&served, "/cockpit/board");

    assert!(
        body.contains("Nothing is attached"),
        "an empty board does not say it is empty: {body}"
    );
    assert!(
        body.contains("No two sessions on this board have claimed the same ground"),
        "an empty board does not say there is no collision"
    );
    // and it is still the board: the page renders whole, with the shell around it
    assert!(body.contains("mj-sidebar"), "no navigation");
    assert!(body.contains("Majordomus Cockpit"), "no title");
}

#[test]
fn nothing_on_the_board_is_ever_interpreted_as_markup() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let session = attach(&served, "claude-code", "2.1.268");
    // an intent and a claim are text a client wrote; the board renders both and neither
    // reaches the reader as an element
    announce(
        &served,
        &session,
        "<img src=x onerror=alert(1)>",
        &["docs/<script>alert(2)</script>"],
    );

    let body = page(&served, "/cockpit/board");
    assert!(!body.contains("<img src=x"), "an intent became markup");
    assert!(
        !body.contains("<script>alert(2)"),
        "a claim became markup: {body}"
    );
    assert!(
        body.contains("&lt;img src=x"),
        "the intent was not rendered at all"
    );

    detach(&served, &session);
}

#[test]
fn a_session_that_fans_work_out_shows_every_claim_it_holds_and_not_only_the_last() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let session = attach(&served, "claude-code", "2.1.268");

    // one connection, three workers: a fanned-out session shares its MCP session with its
    // subagents, so the board holds three claims for one peer. Rendering only the most
    // recent would show the third and hide the first two — the exact defect named claims
    // were added to fix, and the exact thing this page must not reintroduce.
    announce_claim(
        &served,
        &session,
        Some("worker-a"),
        "the cockpit board",
        &["apps/majordomus-cli/src/cockpit"],
    );
    announce_claim(
        &served,
        &session,
        Some("worker-b"),
        "the provider adapters",
        &["share/providers.yaml"],
    );
    announce_claim(
        &served,
        &session,
        Some("worker-c"),
        "the entry rule",
        &["docs/ENTRY.md"],
    );

    let body = page(&served, "/cockpit/board");
    for (name, intent, claim) in [
        (
            "worker-a",
            "the cockpit board",
            "apps/majordomus-cli/src/cockpit",
        ),
        ("worker-b", "the provider adapters", "share/providers.yaml"),
        ("worker-c", "the entry rule", "docs/ENTRY.md"),
    ] {
        assert!(
            body.contains(name),
            "the claim named {name} is not on the board"
        );
        assert!(
            body.contains(intent),
            "the intent of {name} is not on the board"
        );
        assert!(
            body.contains(claim),
            "the ground {name} claimed is not on the board"
        );
    }
    // one peer, three claims: the count of sessions is still one
    assert!(
        body.contains("sessions attached"),
        "the board lost its statistics"
    );

    detach(&served, &session);
}

#[test]
fn a_collision_says_which_named_claim_of_a_fanned_out_session_met_the_other() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let one = attach(&served, "claude-code", "2.1.268");
    announce_claim(
        &served,
        &one,
        Some("the-crate"),
        "the whole crate",
        &["apps/majordomus-cli"],
    );
    announce_claim(&served, &one, Some("the-site"), "the site", &["site"]);
    let two = attach(&served, "codex", "0.31");
    announce_claim(
        &served,
        &two,
        Some("one-page"),
        "one page of the cockpit",
        &["apps/majordomus-cli/src/cockpit/pages.rs"],
    );

    let body = page(&served, "/cockpit/board");
    assert!(
        body.contains("p2 codex and p1 claude-code"),
        "the collision does not name both sessions: {body}"
    );
    // and which piece of work each side collided with, so the reader knows it is the crate
    // claim that met the page claim and not the site one
    assert!(
        body.contains("one-page") && body.contains("the-crate"),
        "the collision does not name the claims that met: {body}"
    );
    assert!(
        !body.contains("the-site claims"),
        "a claim that met nothing is named as if it had: {body}"
    );

    detach(&served, &one);
    detach(&served, &two);
}

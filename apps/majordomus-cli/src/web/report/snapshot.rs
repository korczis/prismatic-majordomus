//! The UI as a static snapshot: `majordomus ui`, read once at a pinned commit and written
//! as files a publication can carry (ADR 0141).
//!
//! The UI (ADR 0135) is a projection a running process answers; a publication has no
//! process. So the snapshot asks the UI itself — the same [`crate::ui::Ui`] the server
//! routes to, in-process, over one reading of the repository — for every screen and panel,
//! and then for the pages those link to, breadth first and up to a bound. Nothing is
//! rendered here: each page is the UI's own HTML. What changes is only what a file cannot
//! do that a route can:
//!
//! - **a link** to another page of the UI becomes a relative link to that page's file, so
//!   the directory works at any mount and from disk. An inspector's query (`?rule=…`) is a
//!   directory of its own; a query of more than one pair, or a page past the bound, is not
//!   in the snapshot and its link says so (below);
//! - **a link out of the UI** stays as it is when a publication holds what it names (the
//!   topology says which surfaces are published), and otherwise leads to the snapshot's one
//!   page about the live UI, carrying the path it named, because only a running process
//!   answers it: never a link to nothing;
//! - **the script** is told it is in a snapshot (`data-snapshot` on the body), so it reads
//!   nothing again and fetches no panel: a static page that refreshed itself would replace
//!   what was read with whatever the host answers for a query it ignores;
//! - **every page says what it is**: read at which commit, when, and by which executable,
//!   and how to see it live.
//!
//! A value the UI could not read when the snapshot was taken reads as the UI says it — none,
//! unknown or not answered — and never as zero, because the UI's own rule (`ui-derived-state`)
//! holds for every page this copies.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{write, write_json, Origin};
use crate::cockpit::html::el;
use crate::error::{Error, Result};
use crate::http::router::Request;
use crate::ui::{Ui, ASSETS_PREFIX, PREFIX, SURFACE};
use crate::web::discover::{Declaration, DECLARATION_FILE, DECLARATION_SCHEMA, GENERATED_ROOT};
use crate::web::model::{Availability, Category, Provenance, SurfaceKind, Topology, Visibility};

/// The snapshot's identity in the web topology.
pub const ID: &str = "ui-snapshot";

/// What produces it, as a reader would run it.
pub const PRODUCER: &str = "majordomus web report ui-snapshot";

/// The pages a snapshot holds at most when the caller names no bound: every screen and
/// panel, and the inspectors they link to, until this many pages are written.
pub const DEFAULT_LIMIT: usize = 400;

/// The page, relative to the snapshot's root, that explains a link only a running process
/// answers.
pub const LIVE_PAGE: &str = "live/index.html";

/// The data attribute that tells the UI's script it is in a snapshot.
pub const SNAPSHOT_ATTRIBUTE: &str = "data-snapshot";

/// What a snapshot run wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// The directory, absolute.
    #[serde(skip)]
    pub dir: PathBuf,
    /// Where it came from.
    pub origin: Origin,
    /// Every page written, as the UI target it was read from and the file it is in.
    pub pages: BTreeMap<String, String>,
    /// The asset files copied beside the pages.
    pub assets: Vec<String>,
    /// UI pages a page links to that are not in the snapshot: past the bound, or a query no
    /// file can stand for. Each link to one leads to the live page.
    pub left_out: Vec<String>,
    /// The bound the run was given.
    pub limit: usize,
}

/// Read the UI and write it under the generated web root, with the declaration that makes
/// the directory a surface of every publication.
pub fn render(root: &Path, ui: &Ui, topology: &Topology, limit: usize) -> Result<Snapshot> {
    let dir = root.join(GENERATED_ROOT).join(ID);
    if dir.exists() {
        // a snapshot is one reading: pages of an earlier one must not survive into it
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    let origin = Origin::read(root);
    let outside = Outside::from(topology);

    // breadth first from the screens and panels, so that a bound cuts the deepest pages
    let mut queue: VecDeque<String> = crate::ui::SCREENS
        .iter()
        .map(|s| s.route.to_string())
        .chain(crate::ui::PANELS.iter().map(|p| p.to_string()))
        .collect();
    let mut seen: BTreeSet<String> = queue.iter().cloned().collect();
    let mut pages: BTreeMap<String, String> = BTreeMap::new();
    let mut rendered: Vec<(String, String)> = Vec::new();
    let mut assets: BTreeSet<String> = BTreeSet::new();
    while let Some(target) = queue.pop_front() {
        if pages.len() >= limit.max(1) {
            break;
        }
        let Some(file) = page_file(&target) else {
            continue;
        };
        let response = ui.handle(&Request::parse_target("GET", &target, Vec::new()));
        if response.status != 200 {
            continue;
        }
        let html = response.body.text().into_owned();
        for link in links(&html) {
            let link = unescape(&link);
            if let Some(name) = asset_name(&link) {
                assets.insert(name);
            } else if page_file(&link).is_some() {
                let key = canonical(&link);
                if seen.insert(key.clone()) {
                    queue.push_back(key);
                }
            }
        }
        pages.insert(target.clone(), file.clone());
        rendered.push((file, html));
    }

    // the pages a link may lead to are exactly the ones written: decided before any rewrite
    let written: BTreeMap<String, String> = pages.clone();
    let mut left_out: BTreeSet<String> = BTreeSet::new();
    let mut frame: Option<String> = None;
    for (file, html) in &rendered {
        let page = rewrite(html, file, &written, &outside, &origin, &mut left_out);
        write(&dir, file, &page)?;
        if file == "index.html" {
            frame = Some(page);
        }
    }
    let mut copied = Vec::new();
    for name in &assets {
        let response = ui.handle(&Request::parse_target(
            "GET",
            &format!("{ASSETS_PREFIX}{name}"),
            Vec::new(),
        ));
        if response.status != 200 {
            continue; // the page's own fallback stands: the tables beside a graph, unstyled markup
        }
        let path = dir.join("assets").join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::io(parent.display().to_string(), e))?;
        }
        std::fs::write(&path, response.body.as_bytes())
            .map_err(|e| Error::io(path.display().to_string(), e))?;
        copied.push(format!("assets/{name}"));
    }
    let live = live_page(frame.as_deref(), &origin);
    write(&dir, LIVE_PAGE, &live)?;

    let snapshot = Snapshot {
        dir: dir.clone(),
        origin,
        pages,
        assets: copied,
        left_out: left_out.into_iter().collect(),
        limit,
    };
    write_json(&dir, "snapshot.json", &snapshot)?;
    declare(&dir, &snapshot.origin)?;
    Ok(snapshot)
}

/// The declaration beside the pages: a surface of every publication at the UI's own mount,
/// which no process serves, because a process serves the UI itself there.
fn declare(dir: &Path, origin: &Origin) -> Result<()> {
    let declaration = Declaration {
        schema: DECLARATION_SCHEMA.into(),
        id: ID.into(),
        mount: PREFIX.into(),
        title: Some("The UI, as it was read at one commit".into()),
        index: Some("index.html".into()),
        producer: Some(PRODUCER.into()),
        availability: Some(Availability::PublishedOnly),
        category: Some(Category::Interface),
        visibility: Some(Visibility::Public),
        built_from: origin.revision.clone(),
    };
    let mut body =
        serde_json::to_string_pretty(&declaration).map_err(|e| Error::InvalidSurface {
            surface: ID.into(),
            reason: format!("its declaration cannot be serialised: {e}"),
        })?;
    body.push('\n');
    write(dir, DECLARATION_FILE, &body)
}

/// Where a link that leaves the UI goes in a publication.
struct Outside {
    /// Mounts a publication holds, so a link under one keeps its path.
    kept: Vec<crate::web::model::Mount>,
    /// Mounts a process serves from the application's own source (the documentation build),
    /// whose pages a publication holds at the application's root instead.
    rebased: Vec<crate::web::model::Mount>,
}

impl Outside {
    fn from(topology: &Topology) -> Self {
        let from_site = |s: &crate::web::model::Surface| {
            matches!(s.provenance.get("kind"), Some(Provenance::SiteConfig { .. }))
        };
        let published_root = topology.surfaces.iter().any(|s| {
            s.mount.is_root() && s.availability.is_published() && from_site(s)
        });
        Outside {
            kept: topology
                .surfaces
                .iter()
                .filter(|s| !s.mount.is_root() && s.availability.is_published())
                .filter(|s| s.id != SURFACE && s.id != ID)
                .map(|s| s.mount.clone())
                .collect(),
            rebased: topology
                .surfaces
                .iter()
                .filter(|s| published_root && !s.availability.is_published())
                .filter(|s| s.kind == SurfaceKind::StaticDirectory && from_site(s))
                .map(|s| s.mount.clone())
                .collect(),
        }
    }

    /// The link a publication answers for `path`, or none when only a process does.
    fn resolve(&self, path: &str) -> Option<String> {
        if path == "/" {
            return Some("/".into());
        }
        if self.kept.iter().any(|m| m.owns(path)) {
            return Some(path.into());
        }
        self.rebased.iter().find(|m| m.owns(path)).map(|m| {
            let rest = path[m.as_str().len()..].trim_start_matches('/');
            format!("/{rest}")
        })
    }
}

/// The file a UI target is written to, relative to the snapshot's root; none for a target
/// that is not a page of the UI, or that no file can stand for.
///
/// ```text
/// /ui                       index.html
/// /ui/roadmap               roadmap/index.html
/// /ui/roadmap?milestone=M1  roadmap/_milestone/M1/index.html
/// /ui/issues?a=1&b=2        none: two pairs are a query, not a page
/// ```
fn page_file(target: &str) -> Option<String> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = path.split('#').next().unwrap_or(path);
    if path.starts_with(ASSETS_PREFIX) {
        return None;
    }
    let rest = if path == PREFIX {
        ""
    } else {
        path.strip_prefix(&format!("{PREFIX}/"))?
    };
    let rest = rest.trim_end_matches('/');
    if rest.split('/').any(|s| s.is_empty() && !rest.is_empty() || s == "." || s == "..") {
        return None;
    }
    let base = if rest.is_empty() {
        String::new()
    } else {
        format!("{rest}/")
    };
    let pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .filter(|(k, _)| k != crate::ui::FRAGMENT)
        .collect();
    match pairs.as_slice() {
        [] => Some(format!("{base}index.html")),
        [(k, v)] if !k.is_empty() && !v.is_empty() => {
            Some(format!("{base}_{}/{}/index.html", slug(k), slug(v)))
        }
        _ => None,
    }
}

/// One spelling per page, so that two links to one page are one page.
fn canonical(target: &str) -> String {
    let target = target.split('#').next().unwrap_or(target);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = if path.len() > PREFIX.len() {
        path.trim_end_matches('/')
    } else {
        path
    };
    let query: Vec<&str> = query
        .split('&')
        .filter(|p| !p.is_empty() && *p != crate::ui::FRAGMENT)
        .collect();
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

/// A value as a path segment of its own: the characters a URL and every file system carry
/// unchanged, and every other byte as `_` and its hex, so two values never share a file.
fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'.' {
            out.push(b as char);
        } else {
            out.push_str(&format!("_{b:02x}"));
        }
    }
    if out.starts_with('.') {
        out.replace_range(0..1, "_2e");
    }
    out
}

/// A percent-decoded query component.
fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(hi), Some(lo)) => {
                out.push(hi * 16 + lo);
                i += 3;
            }
            (b'+', _, _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The name of a UI asset a link names, without its digest.
fn asset_name(link: &str) -> Option<String> {
    let rest = link.strip_prefix(ASSETS_PREFIX)?;
    let name = rest.split(['?', '#']).next().unwrap_or(rest);
    (!name.is_empty() && !name.split('/').any(|s| s.is_empty() || s == ".."))
        .then(|| name.to_string())
}

/// The attributes whose value is a URL a reader or the script follows.
const URL_ATTRIBUTES: &str = "href|src|action|data-graph-lib|data-panel-src";

fn attribute_pattern() -> regex::Regex {
    regex::Regex::new(&format!(r#"\b({URL_ATTRIBUTES})="([^"]*)""#))
        .expect("the attribute pattern compiles")
}

/// The link a graph's node carries, inside its JSON in an attribute.
fn graph_href_pattern() -> regex::Regex {
    regex::Regex::new(r#"&quot;href&quot;:&quot;(.*?)&quot;"#).expect("the graph pattern compiles")
}

/// Every URL a page names, as written (still escaped).
fn links(html: &str) -> Vec<String> {
    let mut out: Vec<String> = attribute_pattern()
        .captures_iter(html)
        .map(|c| c[2].to_string())
        .collect();
    out.extend(
        graph_href_pattern()
            .captures_iter(html)
            .map(|c| c[1].to_string()),
    );
    out
}

fn unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The path from a file of the snapshot back to its root.
fn up(file: &str) -> String {
    "../".repeat(file.matches('/').count())
}

/// Where one link of the page in `file` leads in the snapshot.
fn relink(
    link: &str,
    file: &str,
    written: &BTreeMap<String, String>,
    outside: &Outside,
    left_out: &mut BTreeSet<String>,
) -> String {
    if link.is_empty() || link.starts_with('#') || !link.starts_with('/') || link.starts_with("//")
    {
        return link.to_string(); // the page itself, another origin, or a scheme: unchanged
    }
    let fragment = link.find('#').map(|i| &link[i..]).unwrap_or("");
    if let Some(name) = asset_name(link) {
        return format!("{}assets/{name}", up(file));
    }
    if page_file(link).is_some() || link == PREFIX || link.starts_with(&format!("{PREFIX}/")) {
        if let Some(target) = written.get(&canonical(link)) {
            return format!("{}{target}{fragment}", up(file));
        }
        let missing = canonical(link);
        left_out.insert(missing.clone());
        return format!("{}{LIVE_PAGE}#{}", up(file), missing);
    }
    let path = link.split(['?', '#']).next().unwrap_or(link);
    match outside.resolve(path) {
        Some(resolved) => format!("{resolved}{}", &link[path.len()..]),
        None => format!("{}{LIVE_PAGE}#{}", up(file), link.split('#').next().unwrap_or(link)),
    }
}

/// One page of the UI as a file of the snapshot.
fn rewrite(
    html: &str,
    file: &str,
    written: &BTreeMap<String, String>,
    outside: &Outside,
    origin: &Origin,
    left_out: &mut BTreeSet<String>,
) -> String {
    let attributes = attribute_pattern();
    let page = attributes.replace_all(html, |c: &regex::Captures<'_>| {
        let name = &c[1];
        let link = unescape(&c[2]);
        let target = if name == "action" {
            // a form asks the process: in a snapshot it leads to what says so
            let path = link.split(['?', '#']).next().unwrap_or(&link).to_string();
            format!("{}{LIVE_PAGE}#{path}", up(file))
        } else {
            relink(&link, file, written, outside, left_out)
        };
        format!(r#"{name}="{}""#, escape(&target))
    });
    let graphs = graph_href_pattern();
    let page = graphs.replace_all(&page, |c: &regex::Captures<'_>| {
        let target = relink(&unescape(&c[1]), file, written, outside, left_out);
        format!("&quot;href&quot;:&quot;{}&quot;", escape(&target))
    });
    let page = page.replacen("<body ", &format!("<body {SNAPSHOT_ATTRIBUTE} "), 1);
    let banner = banner(origin, &format!("{}{LIVE_PAGE}", up(file))).render();
    match page.find("<main") {
        Some(start) => match page[start..].find('>') {
            Some(end) => {
                let at = start + end + 1;
                format!("{}{banner}{}", &page[..at], &page[at..])
            }
            None => page,
        },
        None => page,
    }
}

/// What every page of a snapshot says about itself.
fn banner(origin: &Origin, live: &str) -> crate::cockpit::html::El {
    let commit = origin.revision.as_deref().unwrap_or("an unknown commit");
    el("div")
        .class("mj-alert mj-alert--info")
        .attr("role", "note")
        .attr("data-snapshot-of", origin.revision.as_deref().unwrap_or(""))
        .text(format!(
            "A snapshot: read at commit {commit} on {} by majordomus {}. Nothing on this page is read again. ",
            origin.rendered_at, origin.version
        ))
        .child(
            el("a")
                .class("mj-ui-link")
                .attr("href", live)
                .text("See it live"),
        )
}

/// The one page about the live UI: what a link to a running process meant, and how to get
/// one. Drawn in the UI's own frame, taken from the dashboard page the snapshot already read.
fn live_page(frame: Option<&str>, origin: &Origin) -> String {
    let body = el("div")
        .class("mj-ui-stack")
        .child(el("h1").class("mj-ui-title").text("The live UI"))
        .child(el("p").text(
            "This link leads to something only a running process answers: a screen read again every minute, a query, a panel past the snapshot's bound, the Cockpit or the API. Run the server in a checkout and open the same path there:",
        ))
        .child(crate::cockpit::view::pre(format!(
            "majordomus serve\n# then open http://localhost:<port>{PREFIX}"
        )))
        .child(
            el("p")
                .text("The path this link named: ")
                .child(el("code").attr("data-snapshot-path", "").text("(the part of this page's address after #)")),
        )
        .child(el("p").text(format!(
            "The snapshot was read at commit {} on {}.",
            origin.revision.as_deref().unwrap_or("an unknown commit"),
            origin.rendered_at
        )));
    let bare = || crate::cockpit::html::document("The live UI · majordomus ui", Vec::new(), body.clone());
    // the frame of the dashboard as the snapshot wrote it, at the root: its <main> is replaced
    let Some(html) = frame else { return bare() };
    let (Some(open), Some(close)) = (html.find("<main"), html.rfind("</main>")) else {
        return bare();
    };
    let Some(open_end) = html[open..].find('>').map(|e| open + e + 1) else {
        return bare();
    };
    // the dashboard is at the root and this page one directory down: every relative link of
    // the frame gains one step up, and nothing in the frame is the current screen here
    let relocate = |part: &str| {
        attribute_pattern()
            .replace_all(part, |c: &regex::Captures<'_>| {
                let v = &c[2];
                if v.is_empty() || v.starts_with('/') || v.starts_with('#') || v.contains("://") {
                    c[0].to_string()
                } else {
                    format!(r#"{}="../{v}""#, &c[1])
                }
            })
            .replace(r#" aria-current="page""#, "")
    };
    let head = relocate(&html[..open_end]);
    let head = match (head.find("<title>"), head.find("</title>")) {
        (Some(a), Some(b)) if a < b => format!(
            "{}<title>The live UI · majordomus ui{}",
            &head[..a],
            &head[b..]
        ),
        _ => head,
    };
    format!("{head}{}{}", body.render(), relocate(&html[close..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_a_file_when_one_file_can_stand_for_it() {
        assert_eq!(page_file("/ui").as_deref(), Some("index.html"));
        assert_eq!(page_file("/ui/").as_deref(), Some("index.html"));
        assert_eq!(page_file("/ui/roadmap").as_deref(), Some("roadmap/index.html"));
        assert_eq!(
            page_file("/ui/roadmap?milestone=M001").as_deref(),
            Some("roadmap/_milestone/M001/index.html")
        );
        assert_eq!(
            page_file("/ui/panels/pull-requests?fragment").as_deref(),
            Some("panels/pull-requests/index.html")
        );
        assert_eq!(
            page_file("/ui/governance?rule=project.a%2Fb").as_deref(),
            Some("governance/_rule/project.a_2fb/index.html")
        );
        assert_eq!(page_file("/ui/issues?status=READY&q=x"), None, "two pairs");
        assert_eq!(page_file("/ui/assets/ui.css?v=1"), None, "an asset");
        assert_eq!(page_file("/cockpit"), None, "another surface");
        assert_eq!(page_file("/uix"), None, "only shares letters");
        assert_eq!(page_file("/ui/../etc"), None, "walks out");
    }

    #[test]
    fn two_values_never_share_a_file() {
        assert_ne!(slug("a/b"), slug("a_b"));
        assert_ne!(slug("a b"), slug("a+b"));
        assert_eq!(slug(".hidden"), "_2ehidden");
        assert_eq!(decode("a%2Fb+c"), "a/b c");
        assert_eq!(decode("100%"), "100%");
    }

    #[test]
    fn a_link_leads_to_a_written_page_a_published_surface_or_the_live_page() {
        let mut written = BTreeMap::new();
        written.insert("/ui".to_string(), "index.html".to_string());
        written.insert(
            "/ui/roadmap?milestone=M1".to_string(),
            "roadmap/_milestone/M1/index.html".to_string(),
        );
        let outside = Outside {
            kept: vec![crate::web::model::Mount::parse("/rustdoc").unwrap()],
            rebased: vec![crate::web::model::Mount::parse("/docs").unwrap()],
        };
        let mut left = BTreeSet::new();
        let file = "governance/index.html";
        let go = |l: &str, left: &mut BTreeSet<String>| relink(l, file, &written, &outside, left);
        assert_eq!(go("/ui", &mut left), "../index.html");
        assert_eq!(
            go("/ui/roadmap?milestone=M1#x", &mut left),
            "../roadmap/_milestone/M1/index.html#x"
        );
        assert_eq!(go("/ui/assets/ui.css?v=12", &mut left), "../assets/ui.css");
        assert_eq!(go("/rustdoc/majordomus_cli/", &mut left), "/rustdoc/majordomus_cli/");
        assert_eq!(go("/docs/getting-started/", &mut left), "/getting-started/");
        assert_eq!(go("/docs", &mut left), "/");
        assert_eq!(go("/", &mut left), "/");
        assert_eq!(go("/cockpit/x", &mut left), "../live/index.html#/cockpit/x");
        assert_eq!(go("https://example.org/a", &mut left), "https://example.org/a");
        assert_eq!(go("#main", &mut left), "#main");
        assert!(left.is_empty());
        assert_eq!(go("/ui/mesh", &mut left), "../live/index.html#/ui/mesh");
        assert!(left.contains("/ui/mesh"), "a UI page not written is reported");
    }

    #[test]
    fn a_page_is_marked_stamped_and_relinked() {
        let origin = Origin {
            revision: Some("abc1234".into()),
            rendered_at: "2026-10-12T00:00:00Z".into(),
            version: "0.0.0-test".into(),
        };
        let mut written = BTreeMap::new();
        written.insert("/ui".to_string(), "index.html".to_string());
        let outside = Outside {
            kept: Vec::new(),
            rebased: Vec::new(),
        };
        let html = r#"<!doctype html><html><body class="mj-body"><main id="main" class="m"><a href="/ui">d</a><form action="/cockpit/search"></form><div data-graph="{&quot;href&quot;:&quot;/ui&quot;}"></div></main></body></html>"#;
        let mut left = BTreeSet::new();
        let page = rewrite(html, "index.html", &written, &outside, &origin, &mut left);
        assert!(page.contains(r#"<body data-snapshot class="mj-body">"#), "{page}");
        assert!(page.contains(r#"<a href="index.html">d</a>"#), "{page}");
        assert!(page.contains(r#"action="live/index.html#/cockpit/search""#), "{page}");
        assert!(page.contains("&quot;href&quot;:&quot;index.html&quot;"), "{page}");
        assert!(page.contains("read at commit abc1234"), "{page}");
        let banner_at = page.find("A snapshot").unwrap();
        assert!(page.find("<main").unwrap() < banner_at, "the banner is inside main");
    }

    #[test]
    fn the_snapshot_writes_every_screen_with_no_link_to_nothing() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let ui = Ui::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let root = tempfile::tempdir().unwrap();
        let topology = crate::web::discover::discover(root.path(), crate::web::discover::Runtime::full())
            .expect("a topology");
        let snapshot = render(root.path(), &ui, &topology, 60).expect("a snapshot");
        for screen in crate::ui::SCREENS {
            assert!(
                snapshot.pages.contains_key(screen.route),
                "{} is not in the snapshot",
                screen.route
            );
        }
        assert!(snapshot.pages.len() <= 60);
        let dir = &snapshot.dir;
        assert!(dir.join(DECLARATION_FILE).is_file());
        assert!(dir.join(LIVE_PAGE).is_file());
        // every relative link of every page names a file the snapshot holds
        for file in snapshot.pages.values().chain(std::iter::once(&LIVE_PAGE.to_string())) {
            let html = std::fs::read_to_string(dir.join(file)).unwrap();
            assert!(html.contains(SNAPSHOT_ATTRIBUTE) || file == LIVE_PAGE, "{file}");
            for link in links(&html) {
                let link = unescape(&link);
                assert!(
                    !link.starts_with("/ui"),
                    "{file} keeps an absolute UI link {link}"
                );
                if link.starts_with('/') || link.starts_with('#') || link.contains("://") {
                    continue;
                }
                let path = link.split(['#', '?']).next().unwrap();
                let base = Path::new(file).parent().unwrap_or(Path::new(""));
                let target = dir.join(base).join(path);
                assert!(target.exists(), "{file} links {link}, which is not in the snapshot");
            }
        }
        let declared: Declaration =
            serde_json::from_str(&std::fs::read_to_string(dir.join(DECLARATION_FILE)).unwrap())
                .unwrap();
        assert_eq!(declared.mount, PREFIX);
        assert_eq!(declared.availability, Some(Availability::PublishedOnly));
    }
}

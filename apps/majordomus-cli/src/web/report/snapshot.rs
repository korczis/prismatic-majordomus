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
//! - **a slow panel** a screen defers to its script is read by the UI before the page is
//!   written, because every page is asked with [`crate::ui::SNAPSHOT_HEADER`]: the published
//!   screen shows what the live one shows once its script has run, not a link to it;
//! - **every page says what it is**: read at which commit, when, and by which executable,
//!   and how to see it live;
//! - **no page names the machine**: the live surface shows the checkout's root and paths
//!   under it, which on a publication would be the disk of whoever built it, so every
//!   spelling of the root and of the home directory reads `<repo>` and `<home>` (I1012).
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
use crate::http::router::Response;
use crate::web::discover::{Declaration, DECLARATION_FILE, DECLARATION_SCHEMA, GENERATED_ROOT};
use crate::web::model::{Availability, Category, Provenance, SurfaceKind, Topology, Visibility};

/// The surface a snapshot is taken of: what tells the UI's snapshot from the Cockpit's.
///
/// One exporter reads both. Everything that differs between them is a value of this type,
/// so that a third surface is a constant here and not a second exporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subject {
    /// The snapshot's identity in the web topology, and its directory under the web root.
    pub id: &'static str,
    /// What produces it, as a reader would run it.
    pub producer: &'static str,
    /// The identity of the surface a process serves at the same mount.
    pub served: &'static str,
    /// The mount: the snapshot is published where the process serves the surface.
    pub prefix: &'static str,
    /// The prefix of the surface's assets.
    pub assets: &'static str,
    /// The surface in a sentence: "UI", "Cockpit".
    pub name: &'static str,
    /// The declaration's title.
    pub title: &'static str,
    /// The title of the page about the live surface.
    pub live_title: &'static str,
    /// A query key that asks for a part of a page and is no page of its own; empty for none.
    pub fragment: &'static str,
    /// Whether a page keeps its scripts. The UI's one script knows a snapshot and reads
    /// nothing again in one. The Cockpit's scripts open sockets, poll and post to the
    /// process, so a snapshot of it carries none: every page is complete without them.
    pub scripts: bool,
    /// Routes a publication never holds, whatever links to them: a page about the machine
    /// that renders it and not about the repository. Such a route is no page of the
    /// snapshot, so it is never read, and a link to it leads to the live page.
    pub withheld: &'static [&'static str],
    /// Names the process answers with a redirect, each with the route it leads to. A file
    /// cannot redirect, so the snapshot writes one page per name that sends a reader on, and
    /// only when the page it leads to is in the snapshot: never a redirect to nothing.
    pub redirects: &'static [(&'static str, &'static str)],
}

/// The UI's snapshot (ADR 0141).
pub const UI: Subject = Subject {
    id: "ui-snapshot",
    producer: "majordomus web report ui-snapshot",
    served: crate::ui::SURFACE,
    prefix: crate::ui::PREFIX,
    assets: crate::ui::ASSETS_PREFIX,
    name: "UI",
    title: "The UI, as it was read at one commit",
    live_title: "The live UI · majordomus ui",
    fragment: crate::ui::FRAGMENT,
    scripts: true,
    // `/ui/peers` names every worker's intent, worktree and machine as it is now: a
    // publication holds no page of it until its public-safe form exists (I2984).
    withheld: &[crate::ui::peers::ROUTE],
    redirects: crate::ui::observatory::REDIRECTS,
};

/// The Cockpit's snapshot (ADR 0147).
pub const COCKPIT: Subject = Subject {
    id: "cockpit-snapshot",
    producer: "majordomus web report cockpit-snapshot",
    served: crate::cockpit::SURFACE,
    prefix: crate::cockpit::PREFIX,
    assets: crate::cockpit::assets::PREFIX,
    name: "Cockpit",
    title: "The Cockpit, as it was read at one commit",
    live_title: "The live Cockpit · majordomus",
    fragment: "",
    scripts: false,
    // `/cockpit/continuity` reads `.ai/local/`: the open episodes and handovers of one
    // checkout, which name a disk. It is the one page of the Cockpit with no counterpart in
    // a publication (ADR 0014, docs/COCKPIT.md).
    withheld: &["/cockpit/continuity"],
    redirects: &[],
};

/// Every subject there is a snapshot of.
pub const SUBJECTS: &[&Subject] = &[&UI, &COCKPIT];

/// The pages a snapshot holds at most when the caller names no bound: every screen and
/// panel, and the inspectors they link to, until this many pages are written.
pub const DEFAULT_LIMIT: usize = 240;

/// How long a run reads inspectors for when the caller names no budget, in seconds. The
/// screens and panels are read whatever they cost; what they link to is read until the page
/// bound or this is reached, whichever is first, because an inspector's cost is its source's
/// (a claim's history, a pull request's checks) and a publication must not wait on the sum.
pub const DEFAULT_BUDGET_SECONDS: u64 = 180;

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
    /// The mount it is published at: the surface's own.
    pub mount: String,
    /// Every page written, as the target it was read from and the file it is in.
    pub pages: BTreeMap<String, String>,
    /// The asset files copied beside the pages.
    pub assets: Vec<String>,
    /// UI pages a page links to that are not in the snapshot: past the bound, or a query no
    /// file can stand for. Each link to one leads to the live page.
    pub left_out: Vec<String>,
    /// The bound the run was given.
    pub limit: usize,
    /// The seconds the run was given for the pages the screens link to.
    pub budget_seconds: u64,
    /// What ended the reading: `complete` (every linked page was written), `limit` or `budget`.
    pub stopped_by: &'static str,
    /// The pages written for a name the process redirects, as the name and the file: each
    /// sends its reader to a page of [`Snapshot::pages`] and is no page of the surface itself.
    pub redirects: BTreeMap<String, String>,
}

/// Read a surface and write it under the generated web root, with the declaration that makes
/// the directory a surface of every publication.
///
/// `handle` is the surface itself, asked in-process: the same function the server routes a
/// request to. `seeds` are the pages read first and whatever they cost, in order; what they
/// link to is read after, until the bound or the budget.
pub fn render(
    root: &Path,
    subject: &Subject,
    handle: &dyn Fn(&Request) -> Response,
    seeds: Vec<String>,
    topology: &Topology,
    limit: usize,
    budget_seconds: u64,
) -> Result<Snapshot> {
    let started = std::time::Instant::now();
    let budget = std::time::Duration::from_secs(budget_seconds);
    let mut stopped_by = "limit";
    let dir = root.join(GENERATED_ROOT).join(subject.id);
    if dir.exists() {
        // a snapshot is one reading: pages of an earlier one must not survive into it
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    let origin = Origin::read(root);
    let outside = Outside::from(root, subject, topology);

    // The screens and panels first, in the navigation's order. What they link to is then
    // taken one page per family in turn (a family is a route and the one key it is asked
    // with: every rule's inspector, every issue's), so that a bound leaves each family the
    // same share instead of the first long list all of it.
    let mut seeds: VecDeque<String> = seeds.into();
    let mut seen: BTreeSet<String> = seeds.iter().cloned().collect();
    let mut families: Vec<(String, VecDeque<String>)> = Vec::new();
    let mut turn = 0usize;
    let mut pages: BTreeMap<String, String> = BTreeMap::new();
    let mut rendered: Vec<(String, String)> = Vec::new();
    let mut assets: BTreeSet<String> = BTreeSet::new();
    while pages.len() < limit.max(1) {
        let target = match seeds.pop_front() {
            Some(seed) => seed,
            None => {
                // the next family that still has a page, starting after the last one served
                let waiting = families.len();
                let next = (0..waiting)
                    .map(|i| (turn + i) % waiting)
                    .find(|i| !families[*i].1.is_empty());
                let Some(i) = next else {
                    stopped_by = "complete";
                    break;
                };
                if started.elapsed() >= budget {
                    stopped_by = "budget";
                    break;
                }
                turn = i + 1;
                match families[i].1.pop_front() {
                    Some(target) => target,
                    None => break,
                }
            }
        };
        let Some(file) = subject.page_file(&target) else {
            continue;
        };
        let asked = std::time::Instant::now();
        // asked as a snapshot: a screen answers with its slow panels already in place
        let mut request = Request::parse_target("GET", &target, Vec::new());
        request
            .headers
            .push((crate::ui::SNAPSHOT_HEADER.to_string(), "1".to_string()));
        let response = handle(&request);
        tracing::debug!(target = %target, ms = asked.elapsed().as_millis() as u64, status = response.status, "snapshot page");
        if response.status != 200 {
            continue;
        }
        let html = response.body.text().into_owned();
        for link in links(&html) {
            let link = unescape(&link);
            if let Some(name) = subject.asset_name(&link) {
                assets.insert(name);
            } else if subject.page_file(&link).is_some() {
                let key = subject.canonical(&link);
                if seen.insert(key.clone()) {
                    let family = family(&key);
                    match families.iter_mut().find(|(name, _)| *name == family) {
                        Some((_, queue)) => queue.push_back(key),
                        None => families.push((family, VecDeque::from([key]))),
                    }
                }
            }
        }
        pages.insert(target.clone(), file.clone());
        rendered.push((file, html));
    }

    // the assets first: a page names only the ones the snapshot holds
    let mut copied = Vec::new();
    for name in &assets {
        let response = handle(&Request::parse_target(
            "GET",
            &format!("{}{name}", subject.assets),
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
        copied.push(name.clone());
    }
    let held: BTreeSet<String> = copied.iter().cloned().collect();
    // the pages a link may lead to are exactly the ones written: decided before any rewrite
    let written: BTreeMap<String, String> = pages.clone();
    let mut left_out: BTreeSet<String> = BTreeSet::new();
    let mut frame: Option<String> = None;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for (file, html) in &rendered {
        let page = rewrite(
            subject,
            html,
            file,
            &written,
            &held,
            &outside,
            &origin,
            &mut left_out,
        );
        let page = without_machine_paths(&page, root, home.as_deref());
        write(&dir, file, &page)?;
        if file == "index.html" {
            frame = Some(page);
        }
    }
    let live = live_page(subject, frame.as_deref(), &origin);
    write(&dir, LIVE_PAGE, &live)?;

    // a name the process redirects is a page that sends its reader on, when the snapshot
    // holds the page it leads to
    let mut redirects: BTreeMap<String, String> = BTreeMap::new();
    for (name, to) in subject.redirects {
        let (Some(file), Some(target)) = (subject.page_file(name), written.get(*to)) else {
            continue;
        };
        if written.values().any(|f| *f == file) {
            continue;
        }
        write(&dir, &file, &redirect_page(subject, &file, target, to))?;
        redirects.insert((*name).to_string(), file);
    }

    let snapshot = Snapshot {
        dir: dir.clone(),
        origin,
        mount: subject.prefix.to_string(),
        pages,
        assets: copied.iter().map(|n| format!("assets/{n}")).collect(),
        left_out: left_out.into_iter().collect(),
        limit,
        budget_seconds,
        stopped_by,
        redirects,
    };
    write_json(&dir, "snapshot.json", &snapshot)?;
    declare(subject, &dir, &snapshot.origin)?;
    // the snapshots written before this one led their links into this surface to their live
    // page, because this one did not exist yet: now it does, and they lead to its pages
    for other in SUBJECTS.iter().filter(|other| other.id != subject.id) {
        relink_into(root, other, subject, &snapshot.pages)?;
    }
    Ok(snapshot)
}

/// Lead the links of an earlier snapshot (`from`) into a surface that has a snapshot now
/// (`to`, holding `pages`): every link `from` led to its live page because `to` had none is
/// led to the page itself when `to` holds it, and left where it was when it does not.
///
/// Two snapshots link into each other, and one of them is written first. Without this the
/// first would be read twice, once to learn what it links to and once to link it; with it,
/// each surface is read once, in either order, and the files say the same thing. A snapshot
/// that is not there has nothing to correct. Returns how many links were led.
fn relink_into(
    root: &Path,
    from: &Subject,
    to: &Subject,
    pages: &BTreeMap<String, String>,
) -> Result<usize> {
    let Some(files) = pages_of(root, from) else {
        return Ok(0);
    };
    let dir = root.join(GENERATED_ROOT).join(from.id);
    let led = regex::Regex::new(&format!(
        r##"(?:\.\./)*{}#({}(?:/[^"#]*)?)"##,
        regex::escape(LIVE_PAGE),
        regex::escape(to.prefix)
    ))
    .expect("the pattern of a link led to the live page compiles");
    let mut count = 0;
    let mut names: Vec<String> = files.into_values().collect();
    crate::order::canonical_strings(&mut names);
    for name in names {
        let path = dir.join(&name);
        let Ok(html) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut changed = 0;
        let page = led.replace_all(&html, |c: &regex::Captures<'_>| {
            match pages.get(&to.canonical(&unescape(&c[1]))) {
                Some(file) => {
                    changed += 1;
                    let directory = file.strip_suffix("index.html").unwrap_or(file);
                    escape(&format!("{}/{directory}", to.prefix))
                }
                None => c[0].to_string(),
            }
        });
        if changed > 0 {
            std::fs::write(&path, page.as_bytes())
                .map_err(|e| Error::io(path.display().to_string(), e))?;
            count += changed;
        }
    }
    Ok(count)
}

/// The pages the Cockpit's snapshot reads first: every static route but the one page about
/// this machine, then every Cockpit page the UI's snapshot under `root` wanted to link to.
///
/// The page about this machine is the subject's to withhold ([`Subject::withheld`]), so it
/// is no seed and no link reaches it either. The UI's links come from the UI's own
/// snapshot, where a link into a Cockpit that was not yet published was led to the live
/// page with the path it meant: those paths are what a reader of the UI will follow first,
/// so they are read before the bound can leave them out. No list of them is kept anywhere.
pub fn cockpit_seeds(root: &Path) -> Vec<String> {
    let mut seeds: Vec<String> = crate::cockpit::static_routes()
        .iter()
        .map(|(path, _)| path.to_string())
        .filter(|path| COCKPIT.page_file(path).is_some())
        .collect();
    let wanted = regex::Regex::new(&format!(
        r##"{}#({}/[^"#&<> ]*)"##,
        regex::escape(LIVE_PAGE),
        regex::escape(COCKPIT.prefix)
    ))
    .expect("the pattern of a link the UI led to its live page compiles");
    let ui = root.join(GENERATED_ROOT).join(UI.id);
    let mut names: Vec<String> = pages_of(root, &UI)
        .unwrap_or_default()
        .into_values()
        .collect();
    crate::order::canonical_strings(&mut names);
    for file in names.into_iter().map(|name| ui.join(name)) {
        let Ok(html) = std::fs::read_to_string(&file) else {
            continue;
        };
        for found in wanted.captures_iter(&html) {
            let target = COCKPIT.canonical(&unescape(&found[1]));
            if COCKPIT.page_file(&target).is_some() && !seeds.contains(&target) {
                seeds.push(target);
            }
        }
    }
    seeds
}

/// The declaration beside the pages: a surface of every publication at the surface's own
/// mount, which no process serves, because a process serves the surface itself there.
fn declare(subject: &Subject, dir: &Path, origin: &Origin) -> Result<()> {
    let declaration = Declaration {
        schema: DECLARATION_SCHEMA.into(),
        id: subject.id.into(),
        mount: subject.prefix.into(),
        title: Some(subject.title.into()),
        index: Some("index.html".into()),
        producer: Some(subject.producer.into()),
        availability: Some(Availability::PublishedOnly),
        category: Some(Category::Interface),
        visibility: Some(Visibility::Public),
        built_from: origin.revision.clone(),
    };
    let mut body =
        serde_json::to_string_pretty(&declaration).map_err(|e| Error::InvalidSurface {
            surface: subject.id.into(),
            reason: format!("its declaration cannot be serialised: {e}"),
        })?;
    body.push('\n');
    write(dir, DECLARATION_FILE, &body)
}

/// Where a link that leaves the surface goes in a publication.
struct Outside {
    /// Mounts a publication holds, so a link under one keeps its path.
    kept: Vec<crate::web::model::Mount>,
    /// Mounts a process serves from the application's own source (the documentation build),
    /// whose pages a publication holds at the application's root instead.
    rebased: Vec<crate::web::model::Mount>,
    /// The other snapshots under the same web root, each with the pages it holds. A link
    /// into one leads to the page when the snapshot has it and to the live page when it
    /// does not: a snapshot is bounded, so its mount being published says nothing about one
    /// page of it.
    snapshots: Vec<(&'static Subject, BTreeMap<String, String>)>,
}

/// The pages another snapshot under `root` holds, read from its own account of itself;
/// none when it has not been written.
fn pages_of(root: &Path, subject: &Subject) -> Option<BTreeMap<String, String>> {
    let path = root
        .join(GENERATED_ROOT)
        .join(subject.id)
        .join("snapshot.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let pages = value.get("pages")?.as_object()?;
    Some(
        pages
            .iter()
            .filter_map(|(target, file)| Some((target.clone(), file.as_str()?.to_string())))
            .collect(),
    )
}

impl Outside {
    fn from(root: &Path, subject: &Subject, topology: &Topology) -> Self {
        let from_site = |s: &crate::web::model::Surface| {
            matches!(
                s.provenance.get("kind"),
                Some(Provenance::SiteConfig { .. })
            )
        };
        let published_root = topology
            .surfaces
            .iter()
            .any(|s| s.mount.is_root() && s.availability.is_published() && from_site(s));
        Outside {
            kept: topology
                .surfaces
                .iter()
                .filter(|s| !s.mount.is_root() && s.availability.is_published())
                // a surface a snapshot stands for is never kept by its mount alone
                .filter(|s| !SUBJECTS.iter().any(|x| s.id == x.served || s.id == x.id))
                .map(|s| s.mount.clone())
                .collect(),
            rebased: topology
                .surfaces
                .iter()
                .filter(|s| published_root && !s.availability.is_published())
                .filter(|s| s.kind == SurfaceKind::StaticDirectory && from_site(s))
                .map(|s| s.mount.clone())
                .collect(),
            snapshots: SUBJECTS
                .iter()
                .filter(|other| other.id != subject.id)
                .filter_map(|other| Some((*other, pages_of(root, other)?)))
                .collect(),
        }
    }

    /// Where a link into another snapshot leads: `Some(Some(href))` for a page it holds,
    /// `Some(None)` for a page of that surface it does not hold, and `None` for a link that
    /// is not into another snapshot's surface at all. `link` carries its query and no
    /// fragment.
    fn snapshot(&self, link: &str) -> Option<Option<String>> {
        let path = link.split('?').next().unwrap_or(link);
        let (other, pages) = self.snapshots.iter().find(|(other, _)| {
            path == other.prefix || path.starts_with(&format!("{}/", other.prefix))
        })?;
        Some(pages.get(&other.canonical(link)).map(|file| {
            let directory = file.strip_suffix("index.html").unwrap_or(file);
            format!("{}/{directory}", other.prefix)
        }))
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

impl Subject {
    /// The file a target is written to, relative to the snapshot's root; none for a target
    /// that is not a page of the surface, or that no file can stand for.
    ///
    /// ```text
    /// /ui                       index.html
    /// /ui/roadmap               roadmap/index.html
    /// /ui/roadmap?milestone=M1  roadmap/_milestone/M1/index.html
    /// /ui/issues?a=1&b=2        none: two pairs are a query, not a page
    /// ```
    fn page_file(&self, target: &str) -> Option<String> {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let path = path.split('#').next().unwrap_or(path);
        let route = path.trim_end_matches('/');
        if self
            .withheld
            .iter()
            .any(|w| route == *w || route.starts_with(&format!("{w}/")))
        {
            return None;
        }
        if path.starts_with(self.assets) {
            return None;
        }
        let rest = if path == self.prefix {
            ""
        } else {
            path.strip_prefix(&format!("{}/", self.prefix))?
        };
        let rest = rest.trim_end_matches('/');
        if rest
            .split('/')
            .any(|s| s.is_empty() && !rest.is_empty() || s == "." || s == "..")
        {
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
            .filter(|(k, _)| self.fragment.is_empty() || k != self.fragment)
            .collect();
        match pairs.as_slice() {
            [] => Some(format!("{base}index.html")),
            [(k, v)] if !k.is_empty() && !v.is_empty() => {
                Some(format!("{base}_{}/{}/index.html", slug(k), slug(v)))
            }
            _ => None,
        }
    }
}

/// The family of a page: its route and the key it is asked with, without the value.
fn family(target: &str) -> String {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let key = query.split('=').next().unwrap_or("");
    format!("{path}?{key}")
}

impl Subject {
    /// One spelling per page, so that two links to one page are one page.
    fn canonical(&self, target: &str) -> String {
        let target = target.split('#').next().unwrap_or(target);
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let path = if path.len() > self.prefix.len() {
            path.trim_end_matches('/')
        } else {
            path
        };
        let query: Vec<&str> = query
            .split('&')
            .filter(|p| !p.is_empty() && (self.fragment.is_empty() || *p != self.fragment))
            .collect();
        if query.is_empty() {
            path.to_string()
        } else {
            format!("{path}?{}", query.join("&"))
        }
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
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
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

impl Subject {
    /// The name of an asset of the surface a link names, without its digest.
    fn asset_name(&self, link: &str) -> Option<String> {
        let rest = link.strip_prefix(self.assets)?;
        let name = rest.split(['?', '#']).next().unwrap_or(rest);
        (!name.is_empty() && !name.split('/').any(|s| s.is_empty() || s == ".."))
            .then(|| name.to_string())
    }
}

/// The attributes whose value is a URL a reader or the script follows.
const URL_ATTRIBUTES: &str = "href|src|action|data-graph-lib|data-observatory-lib|\
     data-observatory-companion|data-p5-lib|data-panel-src|data-compute-worker|\
     data-compute-module";

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
    subject: &Subject,
    link: &str,
    file: &str,
    written: &BTreeMap<String, String>,
    held: &BTreeSet<String>,
    outside: &Outside,
    left_out: &mut BTreeSet<String>,
) -> String {
    if link.is_empty() || link.starts_with('#') || !link.starts_with('/') || link.starts_with("//")
    {
        return link.to_string(); // the page itself, another origin, or a scheme: unchanged
    }
    let fragment = link.find('#').map(|i| &link[i..]).unwrap_or("");
    if let Some(name) = subject.asset_name(link) {
        // an asset the distribution did not have is named nowhere: the page's own fallback
        // stands (the tables beside a graph), and no link leads to nothing
        return if held.contains(&name) {
            format!("{}assets/{name}", up(file))
        } else {
            String::new()
        };
    }
    if subject.page_file(link).is_some()
        || link == subject.prefix
        || link.starts_with(&format!("{}/", subject.prefix))
    {
        if let Some(target) = written.get(&subject.canonical(link)) {
            return format!("{}{target}{fragment}", up(file));
        }
        let missing = subject.canonical(link);
        left_out.insert(missing.clone());
        return format!("{}{LIVE_PAGE}#{}", up(file), missing);
    }
    let bare = link.split('#').next().unwrap_or(link);
    // a page of another snapshot: the page itself when that snapshot holds it, and what
    // says only a process answers it when it does not
    if let Some(held) = outside.snapshot(bare) {
        return match held {
            Some(href) => format!("{href}{fragment}"),
            None => format!("{}{LIVE_PAGE}#{bare}", up(file)),
        };
    }
    let path = link.split(['?', '#']).next().unwrap_or(link);
    match outside.resolve(path) {
        Some(resolved) => format!("{resolved}{}", &link[path.len()..]),
        None => format!(
            "{}{LIVE_PAGE}#{}",
            up(file),
            link.split('#').next().unwrap_or(link)
        ),
    }
}

/// One page of the surface as a file of the snapshot.
#[allow(clippy::too_many_arguments)]
fn rewrite(
    subject: &Subject,
    html: &str,
    file: &str,
    written: &BTreeMap<String, String>,
    held: &BTreeSet<String>,
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
            relink(subject, &link, file, written, held, outside, left_out)
        };
        format!(r#"{name}="{}""#, escape(&target))
    });
    let graphs = graph_href_pattern();
    let page = graphs.replace_all(&page, |c: &regex::Captures<'_>| {
        let target = relink(
            subject,
            &unescape(&c[1]),
            file,
            written,
            held,
            outside,
            left_out,
        );
        format!("&quot;href&quot;:&quot;{}&quot;", escape(&target))
    });
    let page = page.replacen("<body ", &format!("<body {SNAPSHOT_ATTRIBUTE} "), 1);
    // a surface whose scripts talk to the process carries none of them in a snapshot: a
    // script that opened a socket or posted a form would ask a host that answers neither
    let page = if subject.scripts {
        page
    } else {
        script_pattern().replace_all(&page, "").into_owned()
    };
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

/// A script a page loads from a file: `<script … src="…"></script>`, with nothing inside.
fn script_pattern() -> regex::Regex {
    regex::Regex::new(r#"<script\b[^>]*\bsrc="[^"]*"[^>]*>\s*</script>"#)
        .expect("the script pattern compiles")
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

/// The one page about the live surface: what a link to a running process meant, and how to
/// get one. Drawn in the surface's own frame, taken from the first page the snapshot read.
/// The page a redirected name is in a publication: it sends the reader to `target`, the file
/// of the route the process redirects that name to, by a refresh a browser follows without a
/// script, and says so in a link for a reader whose browser does not.
/// A page as a publication may carry it (I1012): every spelling of the repository's root and
/// of the home directory replaced as [`crate::redaction::normalise_machine_paths`] replaces
/// it, by a name in angle brackets escaped here, because the page is HTML and the bare name
/// would read as an element. The live surface names the checkout it serves; a published page
/// says where a thing is in the repository, never where the machine that read it keeps it.
fn without_machine_paths(page: &str, root: &Path, home: Option<&Path>) -> String {
    crate::redaction::normalise_machine_paths(page, root, home)
        .replace("<repo>", "&lt;repo&gt;")
        .replace("<home>", "&lt;home&gt;")
}

fn redirect_page(subject: &Subject, file: &str, target: &str, to: &str) -> String {
    let href = format!("{}{target}", up(file));
    let head = vec![
        el("meta")
            .attr("http-equiv", "refresh")
            .attr("content", format!("0; url={href}")),
        el("link")
            .attr("rel", "canonical")
            .attr("href", href.clone()),
    ];
    let body = el("body").child(
        el("p")
            .text(format!("This address of the {} is now ", subject.name))
            .child(el("a").attr("href", href).text(to)),
    );
    crate::cockpit::html::document(&format!("{to} · {}", subject.name), head, body)
}

fn live_page(subject: &Subject, frame: Option<&str>, origin: &Origin) -> String {
    let body = el("div")
        .class("mj-ui-stack")
        .child(
            el("h1")
                .class("mj-ui-title")
                .text(format!("The live {}", subject.name)),
        )
        .child(el("p").text(
            "This link leads to something only a running process answers: a page read again as the repository changes, a query, a form, a page past the snapshot's bound, another surface of the server or its API. Run the server in a checkout and open the same path there:",
        ))
        .child(crate::cockpit::view::pre(format!(
            "majordomus serve\n# then open http://localhost:<port>{}",
            subject.prefix
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
    let bare = || crate::cockpit::html::document(subject.live_title, Vec::new(), body.clone());
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
        (Some(a), Some(b)) if a < b => {
            format!("{}<title>{}{}", &head[..a], subject.live_title, &head[b..])
        }
        _ => head,
    };
    format!("{head}{}{}", body.render(), relocate(&html[close..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_a_file_when_one_file_can_stand_for_it() {
        assert_eq!(UI.page_file("/ui").as_deref(), Some("index.html"));
        assert_eq!(UI.page_file("/ui/").as_deref(), Some("index.html"));
        assert_eq!(
            UI.page_file("/ui/roadmap").as_deref(),
            Some("roadmap/index.html")
        );
        assert_eq!(
            UI.page_file("/ui/roadmap?milestone=M001").as_deref(),
            Some("roadmap/_milestone/M001/index.html")
        );
        assert_eq!(
            UI.page_file("/ui/panels/pull-requests?fragment").as_deref(),
            Some("panels/pull-requests/index.html")
        );
        assert_eq!(
            UI.page_file("/ui/governance?rule=project.a%2Fb").as_deref(),
            Some("governance/_rule/project.a_2fb/index.html")
        );
        assert_eq!(
            UI.page_file("/ui/issues?status=READY&q=x"),
            None,
            "two pairs"
        );
        assert_eq!(UI.page_file("/ui/assets/ui.css?v=1"), None, "an asset");
        assert_eq!(UI.page_file("/cockpit"), None, "another surface");
        assert_eq!(UI.page_file("/uix"), None, "only shares letters");
        assert_eq!(UI.page_file("/ui/../etc"), None, "walks out");
    }

    #[test]
    fn two_values_never_share_a_file() {
        assert_eq!(family("/ui/prs?issue=I0001"), family("/ui/prs?issue=I0002"));
        assert_ne!(family("/ui/prs?issue=I0001"), family("/ui/prs?pr=1"));
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
            snapshots: Vec::new(),
        };
        let mut left = BTreeSet::new();
        let file = "governance/index.html";
        let held: BTreeSet<String> = ["ui.css".to_string()].into();
        let go = |l: &str, left: &mut BTreeSet<String>| {
            relink(&UI, l, file, &written, &held, &outside, left)
        };
        assert_eq!(go("/ui", &mut left), "../index.html");
        assert_eq!(
            go("/ui/roadmap?milestone=M1#x", &mut left),
            "../roadmap/_milestone/M1/index.html#x"
        );
        assert_eq!(go("/ui/assets/ui.css?v=12", &mut left), "../assets/ui.css");
        assert_eq!(
            go("/ui/assets/vendor/absent.js", &mut left),
            "",
            "not held: named nowhere"
        );
        assert_eq!(
            go("/rustdoc/majordomus_cli/", &mut left),
            "/rustdoc/majordomus_cli/"
        );
        assert_eq!(go("/docs/getting-started/", &mut left), "/getting-started/");
        assert_eq!(go("/docs", &mut left), "/");
        assert_eq!(go("/", &mut left), "/");
        assert_eq!(go("/cockpit/x", &mut left), "../live/index.html#/cockpit/x");
        assert_eq!(
            go("https://example.org/a", &mut left),
            "https://example.org/a"
        );
        assert_eq!(go("#main", &mut left), "#main");
        assert!(left.is_empty());
        assert_eq!(go("/ui/mesh", &mut left), "../live/index.html#/ui/mesh");
        assert!(
            left.contains("/ui/mesh"),
            "a UI page not written is reported"
        );
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
            snapshots: Vec::new(),
        };
        let html = r#"<!doctype html><html><body class="mj-body"><main id="main" class="m"><a href="/ui">d</a><form action="/cockpit/search"></form><div data-graph="{&quot;href&quot;:&quot;/ui&quot;}"></div></main></body></html>"#;
        let mut left = BTreeSet::new();
        let page = rewrite(
            &UI,
            html,
            "index.html",
            &written,
            &BTreeSet::new(),
            &outside,
            &origin,
            &mut left,
        );
        assert!(
            page.contains(r#"<body data-snapshot class="mj-body">"#),
            "{page}"
        );
        assert!(page.contains(r#"<a href="index.html">d</a>"#), "{page}");
        assert!(
            page.contains(r#"action="live/index.html#/cockpit/search""#),
            "{page}"
        );
        assert!(
            page.contains("&quot;href&quot;:&quot;index.html&quot;"),
            "{page}"
        );
        assert!(page.contains("read at commit abc1234"), "{page}");
        let banner_at = page.find("A snapshot").unwrap();
        assert!(
            page.find("<main").unwrap() < banner_at,
            "the banner is inside main"
        );
    }

    #[test]
    fn the_snapshot_writes_every_screen_with_no_link_to_nothing() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let ui = crate::ui::Ui::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let ask = |req: &Request| ui.handle(req);
        let seeds = || -> Vec<String> {
            crate::ui::SCREENS
                .iter()
                .map(|s| s.route.to_string())
                .chain(crate::ui::PANELS.iter().map(|p| p.to_string()))
                .collect()
        };
        let root = tempfile::tempdir().unwrap();
        let topology =
            crate::web::discover::discover(root.path(), crate::web::discover::Runtime::full())
                .expect("a topology");
        let snapshot =
            render(root.path(), &UI, &ask, seeds(), &topology, 60, 600).expect("a snapshot");
        let published = || {
            crate::ui::SCREENS
                .iter()
                .filter(|s| UI.page_file(s.route).is_some())
        };
        for screen in published() {
            assert!(
                snapshot.pages.contains_key(screen.route),
                "{} is not in the snapshot",
                screen.route
            );
        }
        // a screen the subject withholds is no page, and the navigation's link to it leads to
        // the live page with the path it meant
        assert!(!snapshot.pages.contains_key(crate::ui::peers::ROUTE));
        assert!(snapshot
            .left_out
            .iter()
            .any(|p| p == crate::ui::peers::ROUTE));
        let overview = std::fs::read_to_string(snapshot.dir.join("index.html")).unwrap();
        assert!(
            overview.contains(&format!("{LIVE_PAGE}#{}", crate::ui::peers::ROUTE)),
            "the navigation's link to the withheld screen leads to the live page"
        );
        assert!(snapshot.pages.len() <= 60);
        // no budget at all still writes every screen and panel, and says what stopped it
        let hurried =
            render(root.path(), &UI, &ask, seeds(), &topology, 60, 0).expect("a snapshot");
        assert_eq!(hurried.stopped_by, "budget");
        assert_eq!(
            hurried.pages.len(),
            published().count() + crate::ui::PANELS.len(),
            "the screens and panels, and nothing they link to"
        );
        let snapshot =
            render(root.path(), &UI, &ask, seeds(), &topology, 60, 600).expect("a snapshot");
        let dir = &snapshot.dir;
        assert!(dir.join(DECLARATION_FILE).is_file());
        assert!(dir.join(LIVE_PAGE).is_file());
        // every relative link of every page names a file the snapshot holds
        for file in snapshot
            .pages
            .values()
            .chain(std::iter::once(&LIVE_PAGE.to_string()))
        {
            let html = std::fs::read_to_string(dir.join(file)).unwrap();
            assert!(
                html.contains(SNAPSHOT_ATTRIBUTE) || file == LIVE_PAGE,
                "{file}"
            );
            for link in links(&html) {
                let link = unescape(&link);
                assert!(
                    !link.starts_with("/ui"),
                    "{file} keeps an absolute UI link {link}"
                );
                if link.is_empty()
                    || link.starts_with('/')
                    || link.starts_with('#')
                    || link.contains("://")
                {
                    continue;
                }
                let path = link.split(['#', '?']).next().unwrap();
                let base = Path::new(file).parent().unwrap_or(Path::new(""));
                let target = dir.join(base).join(path);
                assert!(
                    target.exists(),
                    "{file} links {link}, which is not in the snapshot"
                );
            }
        }
        let declared: Declaration =
            serde_json::from_str(&std::fs::read_to_string(dir.join(DECLARATION_FILE)).unwrap())
                .unwrap();
        assert_eq!(declared.mount, UI.prefix);
        assert_eq!(declared.availability, Some(Availability::PublishedOnly));
    }
    /// Every relative link of every page of a snapshot under `dir` names a file it holds,
    /// and no page keeps an absolute link into the surface itself.
    fn holds_what_it_links(subject: &Subject, snapshot: &Snapshot) {
        let dir = &snapshot.dir;
        for file in snapshot
            .pages
            .values()
            .chain(std::iter::once(&LIVE_PAGE.to_string()))
        {
            let html = std::fs::read_to_string(dir.join(file)).unwrap();
            for link in links(&html) {
                let link = unescape(&link);
                assert!(
                    link != subject.prefix && !link.starts_with(&format!("{}/", subject.prefix)),
                    "{file} keeps an absolute link into its own surface: {link}"
                );
                if link.is_empty()
                    || link.starts_with('/')
                    || link.starts_with('#')
                    || link.contains("://")
                {
                    continue;
                }
                let path = link.split(['#', '?']).next().unwrap();
                let base = Path::new(file).parent().unwrap_or(Path::new(""));
                assert!(
                    dir.join(base).join(path).exists(),
                    "{file} links {link}, which is not in the snapshot"
                );
            }
        }
    }

    /// The Cockpit is read by the same exporter: every static route but the one page about
    /// this machine, no script on any page, every page stamped, no link to nothing, and a
    /// declaration at the Cockpit's own mount.
    #[test]
    fn the_cockpit_snapshot_holds_its_routes_without_scripts_or_the_page_about_this_machine() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let cockpit =
            crate::cockpit::Cockpit::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let root = tempfile::tempdir().unwrap();
        let topology =
            crate::web::discover::discover(root.path(), crate::web::discover::Runtime::full())
                .expect("a topology");
        let seeds = cockpit_seeds(root.path());
        assert!(seeds.contains(&COCKPIT.prefix.to_string()));
        assert!(
            !seeds.iter().any(|s| s == "/cockpit/continuity"),
            "the page about this machine is no seed"
        );
        let snapshot = render(
            root.path(),
            &COCKPIT,
            &|request| cockpit.handle(request),
            seeds,
            &topology,
            80,
            600,
        )
        .expect("a snapshot");
        assert_eq!(snapshot.mount, "/cockpit");
        assert!(snapshot.pages.contains_key("/cockpit"));
        assert!(snapshot.pages.contains_key("/cockpit/capabilities"));
        assert!(
            !snapshot
                .pages
                .keys()
                .any(|p| p.starts_with("/cockpit/continuity")),
            "the page about this machine is not published: {:?}",
            snapshot.pages.keys().collect::<Vec<_>>()
        );
        for file in snapshot.pages.values() {
            let html = std::fs::read_to_string(snapshot.dir.join(file)).unwrap();
            assert!(html.contains(SNAPSHOT_ATTRIBUTE), "{file} is not stamped");
            assert!(
                html.contains("data-snapshot-of"),
                "{file} does not say what it was read at"
            );
            assert!(
                !script_pattern().is_match(&html),
                "{file} carries a script that would talk to a process"
            );
        }
        holds_what_it_links(&COCKPIT, &snapshot);
        // the page is withheld by the subject, so a link to it says only a process answers it
        assert_eq!(COCKPIT.page_file("/cockpit/continuity"), None);
        assert_eq!(COCKPIT.page_file("/cockpit/continuity/"), None);
        assert_eq!(COCKPIT.page_file("/cockpit/continuity/episode"), None);
        assert!(COCKPIT
            .page_file("/cockpit/capabilities/continuity.state")
            .is_some());
        let overview = std::fs::read_to_string(snapshot.dir.join("index.html")).unwrap();
        assert!(
            overview.contains(&format!("{LIVE_PAGE}#/cockpit/continuity")),
            "the navigation's link to the withheld page leads to the live page"
        );
        assert!(snapshot.left_out.iter().any(|p| p == "/cockpit/continuity"));
        let declared: Declaration = serde_json::from_str(
            &std::fs::read_to_string(snapshot.dir.join(DECLARATION_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(declared.id, "cockpit-snapshot");
        assert_eq!(declared.mount, COCKPIT.prefix);
        assert_eq!(declared.availability, Some(Availability::PublishedOnly));
    }

    /// The live overview names the checkout's root, and no page of the snapshot does (I1012):
    /// the publication of 2026-10-21 carried the runner's absolute root on every Cockpit page.
    #[test]
    fn no_page_of_a_snapshot_names_the_root_it_was_read_from() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let cockpit =
            crate::cockpit::Cockpit::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let root = repo.root();
        let spellings: Vec<String> = [Some(root.to_path_buf()), std::fs::canonicalize(root).ok()]
            .into_iter()
            .flatten()
            .map(|p| p.to_string_lossy().trim_end_matches('/').to_string())
            .collect();
        let live = cockpit.handle(&Request::parse_target("GET", "/cockpit", Vec::new()));
        let live = live.body.text().into_owned();
        assert!(
            spellings.iter().any(|s| live.contains(s.as_str())),
            "the live overview no longer names its root, so this proves nothing"
        );
        let topology = crate::web::discover::discover(root, crate::web::discover::Runtime::full())
            .expect("a topology");
        let snapshot = render(
            root,
            &COCKPIT,
            &|request| cockpit.handle(request),
            cockpit_seeds(root),
            &topology,
            40,
            600,
        )
        .expect("a snapshot");
        for file in snapshot
            .pages
            .values()
            .chain(std::iter::once(&LIVE_PAGE.to_string()))
        {
            let html = std::fs::read_to_string(snapshot.dir.join(file)).unwrap();
            for s in &spellings {
                assert!(!html.contains(s.as_str()), "{file} names the root {s}");
            }
        }
        let overview = std::fs::read_to_string(snapshot.dir.join("index.html")).unwrap();
        assert!(
            overview.contains("&lt;repo&gt;"),
            "the overview's root reads as the repository"
        );
        assert!(
            !overview.contains("<repo>"),
            "the name is escaped, not an element"
        );
    }

    /// Two snapshots under one root link into each other by the pages each holds: the one
    /// written first is corrected by the one written after it, a page the other does not
    /// hold still leads to the live page, and the second one's seeds are what the first
    /// wanted to link to.
    #[test]
    fn two_snapshots_link_only_the_pages_the_other_holds() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let ui = crate::ui::Ui::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let cockpit =
            crate::cockpit::Cockpit::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let root = tempfile::tempdir().unwrap();
        let topology =
            crate::web::discover::discover(root.path(), crate::web::discover::Runtime::full())
                .expect("a topology");
        let screens: Vec<String> = crate::ui::SCREENS
            .iter()
            .map(|s| s.route.to_string())
            .collect();
        let first = render(
            root.path(),
            &UI,
            &|request| ui.handle(request),
            screens,
            &topology,
            40,
            0,
        )
        .expect("the UI's snapshot");
        let cockpit_links =
            |dir: &Path, pages: &BTreeMap<String, String>| -> (Vec<String>, usize) {
                let mut held = Vec::new();
                let mut live = 0;
                for file in pages.values() {
                    let html = std::fs::read_to_string(dir.join(file)).unwrap();
                    for link in links(&html) {
                        let link = unescape(&link);
                        if link.starts_with("/cockpit") {
                            held.push(link);
                        } else if link.contains(&format!("{LIVE_PAGE}#/cockpit")) {
                            live += 1;
                        }
                    }
                }
                (held, live)
            };
        let (held, live) = cockpit_links(&first.dir, &first.pages);
        assert!(held.is_empty(), "no Cockpit snapshot exists yet: {held:?}");
        assert!(live > 0, "the UI's source lines lead to the Cockpit");

        // the Cockpit's seeds are what the UI wanted to link to
        let seeds = cockpit_seeds(root.path());
        assert!(
            seeds
                .iter()
                .any(|s| s.starts_with("/cockpit/capabilities/")),
            "the capabilities the UI cites are read first: {seeds:?}"
        );
        let second = render(
            root.path(),
            &COCKPIT,
            &|request| cockpit.handle(request),
            seeds,
            &topology,
            400,
            600,
        )
        .expect("the Cockpit's snapshot");

        // the UI's files were corrected: a link into the Cockpit names a page it holds
        let (held, _) = cockpit_links(&first.dir, &first.pages);
        assert!(
            !held.is_empty(),
            "the UI's links were led into the Cockpit's snapshot"
        );
        for link in &held {
            let path = link.split(['#', '?']).next().unwrap();
            let rest = path
                .strip_prefix("/cockpit")
                .unwrap()
                .trim_start_matches('/');
            assert!(
                second.dir.join(rest).join("index.html").is_file(),
                "the UI links {link}, which the Cockpit's snapshot does not hold"
            );
        }
        // and a page of another snapshot it does not hold is the live page, never a guess
        let outside = Outside::from(root.path(), &UI, &topology);
        assert_eq!(
            outside.snapshot("/cockpit/capabilities"),
            Some(Some("/cockpit/capabilities/".to_string()))
        );
        assert_eq!(outside.snapshot("/cockpit/no/such/page"), Some(None));
        assert_eq!(
            outside.snapshot("/rustdoc/x"),
            None,
            "not a snapshot's surface"
        );
    }
}

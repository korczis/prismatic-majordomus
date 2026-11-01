//! `majordomus release site` against a repository with a recorded release and a real history
//! (`project.a-release-leaves-the-site-better`): the verdict the release workflow asks before
//! it builds anything, run as the executable it is, with each exit code it promises.

mod common;

use common::{majordomus, Fixture};

/// The checkout's own design, so the contrast measures are taken on what ships.
fn copy_design(f: &Fixture) {
    let share = common::dist_share();
    for rel in [
        "design/tokens.yaml",
        "design/primitives.css",
        "design/kit.css",
        "design/base.css",
        "cockpit/src/cockpit.css",
    ] {
        let text = std::fs::read_to_string(share.join(rel)).expect("the shipped design");
        f.write(&format!("share/{rel}"), &text);
    }
}

/// A repository whose release v0.1.0 is recorded at the commit that built its first site.
fn released() -> (Fixture, String) {
    let f = Fixture::new();
    f.write(
        ".ai/repo/ci/gates.yaml",
        "version: 1\ngates:\n  - id: site-build\n    job: site\n    runs: \"true\"\n    summary: s\nclasses:\n  - id: site\n    paths: [site/**]\n    gates: [site-build]\n  - id: design\n    paths: [share/design/**]\n    gates: [site-build]\n",
    );
    f.write(
        ".ai/repo/ci/pages.yaml",
        "version: 1\nrelease:\n  classes:\n    - site\n    - design\n  scopes:\n    - site\n  kinds:\n    - feat\n    - fix\n",
    );
    f.write(".gitattributes", "site/content/** merge=derived\n");
    copy_design(&f);
    f.write("site/content/a.md", "generated\n");
    f.write("site/content-src/b.md", "by hand\n");
    f.write("site/templates/page.html", "<main></main>\n");
    f.commit("feat(site): the first site");
    let built = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.write(
        ".ai/repo/releases/v0.1.0.yaml",
        &format!("schema: release/v1\nversion: \"0.1.0\"\ntag: v0.1.0\nchannel: stable\ncommit: {built}\npublished_at: \"2026-10-16T00:00:00Z\"\nartifacts: []\n"),
    );
    f.commit("chore(release): record v0.1.0");
    (f, built)
}

fn site(f: &Fixture, extra: &[&str]) -> (i32, String) {
    let root = f.root();
    let out = majordomus()
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .args(["release", "site", "--repo", root.to_str().unwrap()])
        .args(extra)
        .output()
        .expect("the executable runs");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if extra.contains(&"json") {
        // the verdict alone, which is what a reader of the JSON parses
        return (out.status.code().unwrap_or(-1), stdout);
    }
    let text = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), text)
}

#[test]
fn release_site_refuses_a_window_without_an_improvement_and_passes_one_with() {
    let (f, built) = released();

    // only a hand edit of generated output since the release: refused, and named
    f.write("site/content/a.md", "edited by hand\n");
    f.commit("fix(site): a typo in the generated page");
    let (code, text) = site(&f, &[]);
    assert_eq!(code, 10, "{text}");
    assert!(text.contains("FAIL site-not-improved"), "{text}");
    assert!(text.contains("changes only generated output"), "{text}");

    // an improvement of a site source: passes, every measure held
    f.write("site/templates/page.html", "<main class=\"page\"></main>\n");
    f.commit("fix(site): the template marks its page");
    let (code, text) = site(&f, &[]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("improves"), "{text}");
    assert!(
        text.contains("OK   the release leaves the site better"),
        "{text}"
    );
    let (code, json) = site(&f, &["--since", "v0.1.0", "--format", "json"]);
    assert_eq!(code, 0, "{json}");
    let v: serde_json::Value = serde_json::from_str(&json).expect("the verdict is JSON");
    assert_eq!(v["previous"]["tag"], "v0.1.0");
    assert_eq!(v["previous"]["commit"], built.as_str());
    assert_eq!(v["improvements"], 1);
    assert_eq!(v["passes"], true);

    // a page written by hand: the measure regresses, named with its delta
    f.write("site/content-src/c.md", "another by hand\n");
    f.commit("feat(site): another page by hand");
    let (code, text) = site(&f, &[]);
    assert_eq!(code, 10, "{text}");
    assert!(
        text.contains("site.pages-by-hand (site page files written by hand rather than generated) went from 1 to 2 (+1)"),
        "{text}"
    );

    // a baseline named by a commit rather than a record
    let (code, text) = site(&f, &["--since", &built, "--head", "HEAD~1"]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn release_site_without_a_baseline_or_a_declaration_is_unreadable() {
    let (f, built) = released();
    // the commit before the recorded one: nothing recorded precedes it
    let (code, text) = site(&f, &["--head", "HEAD~2"]);
    assert_eq!(code, 12, "{text}");
    assert!(
        text.contains("no recorded stable release is an ancestor"),
        "{text}"
    );
    // a ref this clone does not have
    let (code, text) = site(&f, &["--since", "v9.9.9"]);
    assert_eq!(code, 12, "{text}");
    assert!(text.contains("names no commit"), "{text}");
    // a class the CI model does not declare
    f.write(
        ".ai/repo/ci/pages.yaml",
        "version: 1\nrelease:\n  classes:\n    - nowhere\n",
    );
    let (code, text) = site(&f, &["--since", &built]);
    assert_eq!(code, 12, "{text}");
    assert!(text.contains("names the class `nowhere`"), "{text}");
}

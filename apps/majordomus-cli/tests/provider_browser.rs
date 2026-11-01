//! I3829: a provider declares whether its client drives a browser beside this repository's
//! server, and preflight says when this machine has no bridge for it.
//!
//! The declaration is the distribution's own `share/providers.yaml`; the machine is a
//! temporary home the test lays out, so no real browser profile is read.

mod common;

use std::path::{Path, PathBuf};

use majordomus_cli::environment::preflight::{
    browser_observations, derive, native_host_dirs, native_host_manifest, Observations, Verdict,
};
use majordomus_cli::generate::providers_markdown;
use majordomus_cli::share::{BrowserSource, ProviderDeclarations, ProviderOffers, Share};
use serde_json::json;

const HOST: &str = "com.anthropic.claude_code_browser_extension";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn declared() -> ProviderDeclarations {
    let repo = repo();
    Share::locate(Some(&repo.join("share")), &repo)
        .expect("the repository's share")
        .providers()
        .expect("share/providers.yaml reads")
}

/// A share of its own: the repository's kinds file beside a providers file the test writes.
fn share_with(providers: &str) -> (tempfile::TempDir, Result<ProviderDeclarations, String>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        repo().join("share/kinds.yaml"),
        dir.path().join("kinds.yaml"),
    )
    .unwrap();
    std::fs::write(dir.path().join("providers.yaml"), providers).unwrap();
    let read = Share::locate(Some(dir.path()), dir.path())
        .and_then(|s| s.providers())
        .map_err(|e| e.to_string());
    (dir, read)
}

#[test]
fn claude_code_declares_its_bridge_and_every_provider_cites_its_browser_answer() {
    let d = declared();
    let claude = d.providers.iter().find(|p| p.id == "claude-code").unwrap();
    assert_eq!(claude.offers.browser, BrowserSource::Bridge);
    assert_eq!(claude.offers.browser_host.as_deref(), Some(HOST));
    assert!(claude
        .offers
        .browser_setup
        .as_deref()
        .unwrap_or("")
        .contains("/chrome"));
    for p in d.providers.iter().filter(|p| p.declared) {
        assert!(
            p.offers.browser_evidence.len() >= 24,
            "{} cites nothing for browser: {:?}",
            p.id,
            p.offers.browser_evidence
        );
    }
}

#[test]
fn a_browser_word_outside_the_vocabulary_fails_the_read_naming_the_file() {
    let (_dir, read) =
        share_with("version: 1\nproviders:\n  x:\n    offers:\n      browser: \"extension\"\n");
    let err = read.expect_err("a word outside the vocabulary must not read as none");
    assert!(err.contains("providers.yaml"), "{err}");
    let (_dir, read) = share_with(
        "version: 1\nproviders:\n  x:\n    offers:\n      browser: \"bridge\"\n      browser_evidence: \"a vendor page\"\n",
    );
    let x = read
        .unwrap()
        .providers
        .into_iter()
        .find(|p| p.id == "x")
        .unwrap();
    assert_eq!(x.offers.browser, BrowserSource::Bridge);
    assert_eq!(x.offers.browser_host, None, "a host is never invented");
}

/// An `offers:` block that gives no `browser_evidence`, or a blank one, is an answer with no
/// citation, and it fails the read like a word outside the vocabulary does: before, the
/// missing value read as "" and only the evidence gate, if anyone ran it, noticed. So does a
/// block that leaves the `browser` word out, which used to read as a silent `none`.
#[test]
fn an_offers_block_without_browser_evidence_fails_the_read_naming_the_file() {
    let (_dir, read) =
        share_with("version: 1\nproviders:\n  x:\n    offers:\n      browser: \"none\"\n");
    let err = read.expect_err("a browser answer without its citation must not read");
    assert!(
        err.contains("providers.yaml") && err.contains("'x'") && err.contains("browser_evidence"),
        "{err}"
    );
    let (_dir, read) = share_with(
        "version: 1\nproviders:\n  x:\n    offers:\n      browser: \"none\"\n      browser_evidence: \"  \"\n",
    );
    let err = read.expect_err("a blank citation is no citation");
    assert!(
        err.contains("providers.yaml") && err.contains("browser_evidence"),
        "{err}"
    );
    // a block that answers other axes and says nothing about the browser at all
    let (_dir, read) = share_with(
        "version: 1\nproviders:\n  x:\n    offers:\n      episode: \"hooks\"\n      episode_evidence: \"a vendor page\"\n",
    );
    let err = read.expect_err("an offers block silent on the browser must not read as none");
    assert!(
        err.contains("providers.yaml") && err.contains("'x'") && err.contains("without browser"),
        "{err}"
    );
    // a citation with no word: the evidence is there, the answer is not
    let (_dir, read) = share_with(
        "version: 1\nproviders:\n  x:\n    offers:\n      browser_evidence: \"a vendor page\"\n",
    );
    let err = read.expect_err("a missing browser word must not read as none");
    assert!(
        err.contains("providers.yaml") && err.contains("declares offers without browser"),
        "{err}"
    );
    assert!(
        !err.contains("browser_evidence"),
        "the word is what is missing: {err}"
    );

    let (_dir, read) = share_with(
        "version: 1\nproviders:\n  x:\n    offers:\n      browser: \"none\"\n      browser_evidence: \"searched the vendor docs; none\"\n",
    );
    let x = read
        .expect("a cited answer reads")
        .providers
        .into_iter()
        .find(|p| p.id == "x")
        .unwrap();
    assert_eq!(x.offers.browser, BrowserSource::None);
    assert_eq!(x.offers.browser_evidence, "searched the vendor docs; none");
    // An entry with no offers block declares nothing on any axis and still reads, as a
    // template-only provider does: that is the undeclared state the evidence gate names, and
    // cases 280 and 304 append such entries. It is narrower than I3829's criterion (1) as
    // worded ("a provider entry without browser_evidence"); the conflict is reported, not
    // settled here.
    let (_dir, read) = share_with("version: 1\nproviders:\n  y:\n    title: Y\n");
    let y = read
        .expect("an entry without offers reads")
        .providers
        .into_iter()
        .find(|p| p.id == "y")
        .unwrap();
    assert_eq!(y.offers, ProviderOffers::default());
}

/// `majordomus product providers` answers the browser of every provider from
/// share/providers.yaml: the JSON carries the declared word and the text a BROWSER column.
#[test]
fn the_product_providers_command_shows_the_browser_from_the_one_declaration() {
    let d = declared();
    let word = |id: &str| {
        d.providers
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .offers
            .browser
            .as_str()
    };
    assert_eq!(word("claude-code"), "bridge");
    assert_eq!(word("codex"), "none");
    let f = common::Fixture::new();

    let (code, out, err) =
        common::run_in(&f.root(), &["product", "providers", "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let answered = |id: &str| {
        v["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap_or_else(|| panic!("{id} is not answered: {out}"))["offers"]["browser"]
            .clone()
    };
    for id in ["claude-code", "codex"] {
        assert_eq!(answered(id), json!(word(id)), "{id}");
    }

    let (code, out, err) = common::run_in(&f.root(), &["product", "providers"], "");
    assert_eq!(code, 0, "{err}");
    let header = out.lines().next().unwrap();
    let column = header.find("BROWSER").unwrap_or_else(|| panic!("{out}"));
    let cell = |id: &str| {
        let row = out
            .lines()
            .find(|l| l.split_whitespace().next() == Some(id))
            .unwrap_or_else(|| panic!("{id}: {out}"));
        row[column..].split_whitespace().next().unwrap().to_string()
    };
    assert_eq!(cell("claude-code"), "bridge", "{out}");
    assert_eq!(cell("codex"), "none", "{out}");
}

#[test]
fn a_checkout_configured_for_claude_code_is_told_when_the_machine_has_no_bridge() {
    let d = declared();
    let checkout = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    assert!(
        browser_observations(checkout.path(), &d, Some(home.path())).is_empty(),
        "no .mcp.json, no provider configured, nothing said"
    );

    std::fs::write(checkout.path().join(".mcp.json"), "{}").unwrap();
    let seen = browser_observations(checkout.path(), &d, Some(home.path()));
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(
        (seen[0].provider.as_str(), seen[0].host.as_str()),
        ("claude-code", HOST)
    );
    assert_eq!(seen[0].manifest, None);

    let mut o = Observations::empty("fixture", 0);
    o.browser = seen;
    let check = derive(&o).check("integration.browser").unwrap().clone();
    assert_eq!(check.verdict, Verdict::Unavailable);
    assert!(check.summary.contains("Claude Code"), "{}", check.summary);
    assert!(
        check.next.as_deref().unwrap_or("").contains("/chrome"),
        "{check:?}"
    );
    assert!(derive(&o)
        .attention
        .iter()
        .any(|id| id == "integration.browser"));

    let dir = home
        .path()
        .join("Library/Application Support/Google/Chrome/NativeMessagingHosts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{HOST}.json")), "{}").unwrap();
    o.browser = browser_observations(checkout.path(), &d, Some(home.path()));
    let check = derive(&o).check("integration.browser").unwrap().clone();
    assert_eq!(check.verdict, Verdict::Active, "{check:?}");
    assert!(check
        .evidence
        .iter()
        .any(|e| e.source.ends_with(&format!("{HOST}.json"))));
    assert_eq!(check.next, None);
}

/// A bridge declared without the host it registers is one no machine can be seen to have:
/// the evidence gate refuses the declaration, and preflight, reading it anyway, says nothing
/// rather than inventing a host to look for.
#[test]
fn a_bridge_without_its_host_is_not_looked_for() {
    let (_share, read) = share_with(
        "version: 1\nproviders:\n  x:\n    client_config: .mcp.json\n    offers:\n      browser: \"bridge\"\n      browser_evidence: \"a vendor page\"\n",
    );
    let declared = read.expect("a bridge without a host still reads");
    let checkout = tempfile::tempdir().unwrap();
    std::fs::write(checkout.path().join(".mcp.json"), "{}").unwrap();
    let home = tempfile::tempdir().unwrap();
    assert!(
        browser_observations(checkout.path(), &declared, Some(home.path())).is_empty(),
        "no host declared, nothing to look for"
    );
}

#[test]
fn the_host_is_looked_for_in_every_chromium_browser_on_both_systems() {
    let home = tempfile::tempdir().unwrap();
    let dirs = native_host_dirs(home.path());
    assert_eq!(dirs.len(), 8);
    assert_eq!(native_host_manifest(home.path(), HOST), None);
    let brave = home
        .path()
        .join(".config/BraveSoftware/Brave-Browser/NativeMessagingHosts");
    std::fs::create_dir_all(&brave).unwrap();
    std::fs::write(brave.join(format!("{HOST}.json")), "{}").unwrap();
    assert_eq!(
        native_host_manifest(home.path(), HOST),
        Some(brave.join(format!("{HOST}.json")))
    );
}

#[test]
fn the_providers_document_shows_the_browser_column_from_the_declaration() {
    let md = providers_markdown(&json!({
        "count": 2,
        "scratch_roots": [],
        "providers": [
            {"id": "a", "title": "A", "bootstraps": [], "hooks": [], "scratch_roots": [],
             "offers": {"browser": "bridge", "browser_host": "com.example.host"}},
            {"id": "b", "title": "B", "bootstraps": [], "hooks": [], "scratch_roots": [],
             "offers": {"browser": "none"}},
            {"id": "c", "title": "C", "bootstraps": [], "hooks": [], "scratch_roots": []}
        ]
    }));
    assert!(
        md.contains("| hooks | browser | usage | scratch roots |"),
        "{md}"
    );
    assert!(md.contains("| `bridge` (`com.example.host`) |"), "{md}");
    assert!(
        md.contains("| `b` | B | — | — | — | `none` | — | — |"),
        "{md}"
    );
    assert!(md.contains("| `c` | C | — | — | — | — | — | — |"), "{md}");
}

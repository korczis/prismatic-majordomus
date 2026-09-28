//! What the public site serves, observed once per report.
//!
//! The publication model is `.ai/repo/ci/pages.yaml`, and nothing here is a second one: the
//! served identity document is the file its `deploy.identity` names, the published branch
//! is its `deploy.branch`, and the site is the `base_url` of `site/config.toml` (or, for a
//! repository without a site configuration, the installer's `base_url` of
//! `share/distribution.yaml`, which is the same origin).
//!
//! Two readings, and one of them is not this module's. The identity document is fetched
//! here, once, because `DEPLOYED` is an *ancestry* question — does the served commit contain
//! the feature's revision — and `scripts/pages verify` answers an *equality* question: does
//! the site serve exactly this commit. Verification is `scripts/pages`' own and is reused,
//! not re-derived: `scripts/pages verify` probes the site again from outside with the cache
//! defeated, and `scripts/pages built` reads GitHub's own build of the published branch.
//! Their exit codes are the vocabulary — 0 yes, 10 no, 12 not determinable — and 12 is
//! unknown here, never a pass.
//!
//! The network is injectable, so a test never reaches it: `MAJORDOMUS_DELIVERY_SITE_URL`
//! replaces the site's address, and a `file://` directory holding an identity document is a
//! site that both this module and `scripts/pages verify` read through the same `curl`.

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::metadata::yaml::parse_mapping;

/// The variable that replaces the public site's address, for a fixture or a mirror.
pub(crate) const SITE_URL_ENV: &str = "MAJORDOMUS_DELIVERY_SITE_URL";
/// The publication model.
pub(crate) const PAGES_MODEL: &str = ".ai/repo/ci/pages.yaml";
/// The script that owns publication and its verification.
pub(crate) const PAGES_SCRIPT: &str = "scripts/pages";
/// Seconds one probe of the site may take, the bound `scripts/pages verify` uses too.
const PROBE_SECONDS: u64 = 10;

/// What reading the public identity document gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum IdentityRead {
    /// The site answered with an identity naming the commit it was built from.
    Served {
        /// The commit, as the site states it.
        commit: String,
        /// Whether the tree it was built from was dirty, when the identity says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dirty: Option<bool>,
    },
    /// The site was asked and gave no identity: no network, no route, not a document.
    Unreachable {
        /// Why, in one clause.
        reason: String,
    },
    /// There was nothing to ask: no site address or no publication model.
    Unconfigured {
        /// Why, in one clause.
        reason: String,
    },
}

/// Whether the publication was verified from outside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum SiteVerification {
    /// Clean identity, a second probe agrees, GitHub's own build succeeded.
    Verified {
        /// What was observed.
        detail: String,
    },
    /// Measured, and it does not hold.
    Refused {
        /// What was observed.
        reason: String,
    },
    /// Could not be measured.
    Unmeasured {
        /// Why.
        reason: String,
    },
}

/// The publication as observed: where the site is, what it serves, and whether that was
/// verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Publication {
    /// The site's address, when one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The identity document's path under it, from the publication model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_path: Option<String>,
    /// What reading it gave.
    pub identity: IdentityRead,
    /// Whether the publication was verified.
    pub verification: SiteVerification,
}

/// The observer: where to look and with what.
#[derive(Debug, Clone)]
pub(crate) struct Probe {
    root: PathBuf,
    url: Result<String, String>,
    model: Result<Map<String, Value>, String>,
    curl: String,
    pages: PathBuf,
}

impl Probe {
    /// The probe of the repository at `root`, with the site address the environment may
    /// replace.
    pub(crate) fn for_repository(root: &Path) -> Self {
        let url = match std::env::var(SITE_URL_ENV) {
            Ok(v) if !v.trim().is_empty() => Ok(v.trim().to_string()),
            _ => base_url(root),
        };
        Probe {
            root: root.to_path_buf(),
            url,
            model: read_model(root),
            curl: "curl".into(),
            pages: root.join(PAGES_SCRIPT),
        }
    }

    /// Observe the publication: read the identity, then verify it.
    pub(crate) fn observe(&self) -> Publication {
        let identity_path = self
            .model
            .as_ref()
            .ok()
            .and_then(|m| string_at(m, &["deploy", "identity"]));
        let identity = match (&self.url, &self.model, &identity_path) {
            (Err(reason), _, _) | (_, Err(reason), _) => IdentityRead::Unconfigured {
                reason: reason.clone(),
            },
            (_, _, None) => IdentityRead::Unconfigured {
                reason: format!("{PAGES_MODEL} names no deploy.identity"),
            },
            (Ok(url), Ok(_), Some(path)) => self.read_identity(url, path),
        };
        let verification = self.verify(&identity);
        Publication {
            url: self.url.as_ref().ok().cloned(),
            identity_path,
            identity,
            verification,
        }
    }

    fn read_identity(&self, url: &str, path: &str) -> IdentityRead {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = format!("{}/{}", url.trim_end_matches('/'), path);
        let out = Command::new(&self.curl)
            .args(["-fsSL", "--max-time", &PROBE_SECONDS.to_string()])
            .args(["-H", "Cache-Control: no-cache"])
            .arg(format!("{target}?t={now}"))
            .output();
        let body = match out {
            Ok(o) if o.status.success() => o.stdout,
            Ok(o) => {
                return IdentityRead::Unreachable {
                    reason: format!(
                        "{target} did not answer (curl exit {})",
                        code(o.status.code())
                    ),
                }
            }
            Err(e) => {
                return IdentityRead::Unreachable {
                    reason: format!("{} could not run: {e}", self.curl),
                }
            }
        };
        let doc: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        match doc.get("commit").and_then(Value::as_str) {
            Some(c) if c.len() >= 7 && c.chars().all(|ch| ch.is_ascii_hexdigit()) => {
                IdentityRead::Served {
                    commit: c.to_string(),
                    dirty: doc.get("dirty").and_then(Value::as_bool),
                }
            }
            _ => IdentityRead::Unreachable {
                reason: format!("{target} answered without an identity naming a commit"),
            },
        }
    }

    fn verify(&self, identity: &IdentityRead) -> SiteVerification {
        let unmeasured = |reason: String| SiteVerification::Unmeasured { reason };
        let (commit, dirty) = match identity {
            IdentityRead::Served { commit, dirty } => (commit, *dirty),
            _ => return unmeasured("the public identity was not read".into()),
        };
        let short = commit.get(..12).unwrap_or(commit);
        match dirty {
            Some(true) => {
                return SiteVerification::Refused {
                    reason: format!("the public site was built from a dirty tree at {short}"),
                }
            }
            None => {
                return unmeasured(
                    "the public identity does not say whether its tree was clean".into(),
                )
            }
            Some(false) => {}
        }
        if !self.pages.is_file() {
            return unmeasured(format!(
                "this repository has no {PAGES_SCRIPT} to verify the publication with"
            ));
        }
        let url = self.url.as_deref().unwrap_or_default();
        match self.run_pages(&[
            "verify",
            "--url",
            url,
            "--commit",
            commit,
            "--timeout",
            "0",
            "--quiet",
        ]) {
            Some(0) => {}
            Some(10) => {
                return SiteVerification::Refused {
                    reason: format!("a second probe of the site did not serve {short}"),
                }
            }
            other => {
                return unmeasured(format!(
                    "{PAGES_SCRIPT} verify could not measure the site (exit {})",
                    code(other)
                ))
            }
        }
        let branch = self
            .model
            .as_ref()
            .ok()
            .and_then(|m| string_at(m, &["deploy", "branch"]))
            .unwrap_or_else(|| "gh-pages".into());
        let published = crate::git::read_only(&self.root)
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("refs/remotes/origin/{branch}^{{commit}}"))
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let Some(published) = published else {
            return unmeasured(format!(
                "this clone has no origin/{branch} to ask GitHub's build about"
            ));
        };
        let pshort = published.get(..12).unwrap_or(&published).to_string();
        match self.run_pages(&["built", "--commit", &published, "--porcelain"]) {
            Some(0) => SiteVerification::Verified {
                detail: format!(
                    "the site serves {short} from a clean tree, a second probe agrees, and GitHub's own build of {branch} {pshort} is built"
                ),
            },
            Some(10) => SiteVerification::Refused {
                reason: format!("GitHub's own build of {branch} {pshort} errored"),
            },
            other => unmeasured(format!(
                "GitHub's own build of {branch} {pshort} could not be read (exit {})",
                code(other)
            )),
        }
    }

    fn run_pages(&self, args: &[&str]) -> Option<i32> {
        Command::new(&self.pages)
            .args(args)
            .current_dir(&self.root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .and_then(|s| s.code())
    }
}

fn code(c: Option<i32>) -> String {
    c.map(|c| c.to_string()).unwrap_or_else(|| "none".into())
}

fn string_at(map: &Map<String, Value>, path: &[&str]) -> Option<String> {
    let (last, parents) = path.split_last()?;
    let mut node = map;
    for key in parents {
        node = node.get(*key)?.as_object()?;
    }
    node.get(*last)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn read_model(root: &Path) -> Result<Map<String, Value>, String> {
    let text = std::fs::read_to_string(root.join(PAGES_MODEL))
        .map_err(|_| format!("this repository has no publication model at {PAGES_MODEL}"))?;
    parse_mapping(&text).map_err(|e| format!("{PAGES_MODEL} does not parse: {e}"))
}

/// The site's address: `site/config.toml`'s `base_url`, else the installer's.
fn base_url(root: &Path) -> Result<String, String> {
    let from_site = std::fs::read_to_string(root.join(crate::web::discover::SITE_CONFIG))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key.trim() == "base_url").then(|| value.trim().trim_matches('"').to_string())
            })
        });
    let from_installer = || {
        std::fs::read_to_string(root.join("share/distribution.yaml"))
            .ok()
            .and_then(|text| parse_mapping(&text).ok())
            .and_then(|m| string_at(&m, &["installer", "base_url"]))
    };
    from_site
        .filter(|u| u.contains("://"))
        .or_else(from_installer)
        .ok_or_else(|| {
            "no public site address (site/config.toml base_url, share/distribution.yaml installer.base_url)"
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const COMMIT: &str = "3333333333333333333333333333333333333333";

    struct Site {
        dir: tempfile::TempDir,
    }

    impl Site {
        /// A repository with a publication model, a site directory served as `file://`, and
        /// a `scripts/pages` that answers `verify` and `built` with the given exits.
        fn new(identity: Option<&str>, verify: i32, built: i32) -> Self {
            let s = Site {
                dir: tempfile::tempdir().unwrap(),
            };
            let root = s.root();
            std::fs::create_dir_all(root.join(".ai/repo/ci")).unwrap();
            std::fs::write(
                root.join(PAGES_MODEL),
                "version: 1\ndeploy:\n  branch: gh-pages\n  identity: build.json\n",
            )
            .unwrap();
            std::fs::create_dir_all(root.join("public")).unwrap();
            if let Some(body) = identity {
                std::fs::write(root.join("public/build.json"), body).unwrap();
            }
            std::fs::create_dir_all(root.join("scripts")).unwrap();
            let script = root.join(PAGES_SCRIPT);
            std::fs::write(
                &script,
                format!("#!/bin/sh\ncase \"$1\" in verify) exit {verify};; built) exit {built};; esac\nexit 2\n"),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            let git = |args: &[&str]| {
                let ok = Command::new("git")
                    .arg("-C")
                    .arg(&root)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success();
                assert!(ok, "git {args:?}");
            };
            git(&["init", "-q", "."]);
            git(&[
                "-c",
                "user.email=t@e",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "p",
            ]);
            git(&["update-ref", "refs/remotes/origin/gh-pages", "HEAD"]);
            s
        }

        fn root(&self) -> PathBuf {
            self.dir.path().canonicalize().unwrap()
        }

        fn probe(&self) -> Probe {
            let mut p = Probe::for_repository(&self.root());
            p.url = Ok(format!("file://{}", self.root().join("public").display()));
            p
        }
    }

    fn clean() -> String {
        format!("{{\"commit\":\"{COMMIT}\",\"dirty\":false}}")
    }

    #[test]
    fn a_served_clean_identity_that_both_probes_confirm_is_verified() {
        let s = Site::new(Some(&clean()), 0, 0);
        let p = s.probe().observe();
        assert_eq!(
            p.identity,
            IdentityRead::Served {
                commit: COMMIT.into(),
                dirty: Some(false)
            }
        );
        assert_eq!(p.identity_path.as_deref(), Some("build.json"));
        assert!(
            matches!(p.verification, SiteVerification::Verified { .. }),
            "{p:?}"
        );
    }

    #[test]
    fn a_site_that_does_not_answer_is_unreachable_and_unmeasured() {
        let s = Site::new(None, 0, 0);
        let p = s.probe().observe();
        assert!(
            matches!(p.identity, IdentityRead::Unreachable { .. }),
            "{p:?}"
        );
        assert!(matches!(
            p.verification,
            SiteVerification::Unmeasured { .. }
        ));
        // an answer that is not an identity is no identity either
        let s = Site::new(Some("{\"commit\":\"not-a-sha\"}"), 0, 0);
        assert!(matches!(
            s.probe().observe().identity,
            IdentityRead::Unreachable { .. }
        ));
        // and a probe that cannot even run says so
        let mut probe = Site::new(Some(&clean()), 0, 0).probe();
        probe.curl = "/nonexistent/curl".into();
        assert!(matches!(
            probe.observe().identity,
            IdentityRead::Unreachable { reason } if reason.contains("could not run")
        ));
    }

    #[test]
    fn the_verdicts_of_scripts_pages_are_reused_and_twelve_is_never_a_pass() {
        let refused = |verify, built| {
            let s = Site::new(Some(&clean()), verify, built);
            s.probe().observe().verification
        };
        assert!(matches!(refused(10, 0), SiteVerification::Refused { .. }));
        assert!(matches!(
            refused(12, 0),
            SiteVerification::Unmeasured { .. }
        ));
        assert!(matches!(refused(0, 10), SiteVerification::Refused { .. }));
        assert!(matches!(
            refused(0, 12),
            SiteVerification::Unmeasured { .. }
        ));
    }

    #[test]
    fn a_dirty_or_silent_tree_is_not_verified() {
        let dirty = Site::new(
            Some(&format!("{{\"commit\":\"{COMMIT}\",\"dirty\":true}}")),
            0,
            0,
        );
        assert!(matches!(
            dirty.probe().observe().verification,
            SiteVerification::Refused { .. }
        ));
        let silent = Site::new(Some(&format!("{{\"commit\":\"{COMMIT}\"}}")), 0, 0);
        assert!(matches!(
            silent.probe().observe().verification,
            SiteVerification::Unmeasured { .. }
        ));
    }

    #[test]
    fn without_the_script_or_the_published_branch_verification_is_unmeasured() {
        let s = Site::new(Some(&clean()), 0, 0);
        std::fs::remove_file(s.root().join(PAGES_SCRIPT)).unwrap();
        assert!(matches!(
            s.probe().observe().verification,
            SiteVerification::Unmeasured { reason } if reason.contains("scripts/pages")
        ));
        let s = Site::new(Some(&clean()), 0, 0);
        let ok = Command::new("git")
            .arg("-C")
            .arg(s.root())
            .args(["update-ref", "-d", "refs/remotes/origin/gh-pages"])
            .status()
            .unwrap()
            .success();
        assert!(ok);
        assert!(matches!(
            s.probe().observe().verification,
            SiteVerification::Unmeasured { reason } if reason.contains("origin/gh-pages")
        ));
    }

    #[test]
    fn the_address_and_the_model_come_from_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // nothing configured: nothing to ask
        let p = Probe::for_repository(root);
        let observed = p.observe();
        assert!(matches!(
            observed.identity,
            IdentityRead::Unconfigured { .. }
        ));
        assert!(observed.url.is_none() || std::env::var(SITE_URL_ENV).is_ok());
        assert!(base_url(root).is_err());
        // the installer's address stands in for a repository with no site configuration
        std::fs::create_dir_all(root.join("share")).unwrap();
        std::fs::write(
            root.join("share/distribution.yaml"),
            "installer:\n  base_url: https://installer.example\n",
        )
        .unwrap();
        assert_eq!(base_url(root).as_deref(), Ok("https://installer.example"));
        // and the site's own configuration wins
        std::fs::create_dir_all(root.join("site")).unwrap();
        std::fs::write(
            root.join("site/config.toml"),
            "title = \"x\"\nbase_url = \"https://site.example\"\n",
        )
        .unwrap();
        assert_eq!(base_url(root).as_deref(), Ok("https://site.example"));
        // a model without an identity names nothing to read
        std::fs::create_dir_all(root.join(".ai/repo/ci")).unwrap();
        std::fs::write(
            root.join(PAGES_MODEL),
            "version: 1\ndeploy:\n  branch: gh-pages\n",
        )
        .unwrap();
        let mut p = Probe::for_repository(root);
        p.url = Ok("https://site.example".into());
        assert!(matches!(
            p.observe().identity,
            IdentityRead::Unconfigured { reason } if reason.contains("deploy.identity")
        ));
        assert_eq!(code(None), "none");
    }
}

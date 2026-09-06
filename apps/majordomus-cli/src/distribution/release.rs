//! The releases this repository published, and the public metadata derived from them.
//!
//! A record under `.ai/repo/releases/` is evidence: the release pipeline writes it from
//! what it actually uploaded, and every projection reads it rather than GitHub's API. The
//! installer resolves two files and nothing else — `releases/<tag>.json` for a pinned
//! version, `releases/latest.json` for the stable pointer — and both are rendered here.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Archive, Model, Project};
use crate::error::{Error, Result};
use crate::index::Index;
use crate::metadata::yaml;

/// The contract a record follows.
pub const SCHEMA: &str = "release/v1";

/// The kind a record is indexed as.
pub const KIND: &str = "release-record";

/// Where the records live, relative to the repository root.
pub const DIR: &str = ".ai/repo/releases";

/// Where the public metadata is published, relative to the repository root.
pub const PUBLIC_DIR: &str = "site/static/releases";

/// The name of the stable pointer, without its extension.
pub const LATEST: &str = "latest";

/// Which releases an unpinned installation may resolve to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Resolved by `latest`.
    Stable,
    /// Published, addressable by its exact tag, never resolved by default.
    Prerelease,
}

/// One published artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifact {
    /// The id of the target in the distribution model.
    pub target: String,
    /// The artifact's file name, as the naming function derives it.
    pub name: String,
    /// Where it is downloaded from.
    pub url: String,
    /// The digest the installer verifies before it extracts anything.
    pub sha256: String,
    /// The size in bytes, as published.
    pub size: u64,
}

/// One published release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// The contract this record follows.
    pub schema: String,
    /// The semantic version, without a leading `v`.
    pub version: String,
    /// The git tag: `v` and the version.
    pub tag: String,
    /// Which releases an unpinned installation may resolve to.
    pub channel: Channel,
    /// The commit the artifacts were built from.
    pub commit: String,
    /// When it was published, UTC.
    pub published_at: String,
    /// The release notes a person reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
    /// True when the release must no longer be resolved.
    #[serde(default)]
    pub yanked: bool,
    /// One entry per built target.
    pub artifacts: Vec<ReleaseArtifact>,
}

impl Release {
    /// Parse one record.
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        let release: Release = yaml::parse_into(text)?;
        if release.schema != SCHEMA {
            return Err(format!("schema is `{}`, not `{SCHEMA}`", release.schema));
        }
        if release.tag != format!("v{}", release.version) {
            return Err(format!(
                "tag `{}` is not `v` followed by version `{}`",
                release.tag, release.version
            ));
        }
        Ok(release)
    }

    /// Is this release a candidate for the stable pointer?
    pub fn is_stable_candidate(&self) -> bool {
        self.channel == Channel::Stable && !self.yanked
    }

    /// The artifact of a target id.
    pub fn artifact(&self, target: &str) -> Option<&ReleaseArtifact> {
        self.artifacts.iter().find(|a| a.target == target)
    }

    /// Every way this record disagrees with the model it claims to have been built from:
    /// a missing supported target, an artifact of a target that is not published, a name
    /// the naming function does not derive, a URL from another repository.
    pub fn findings(&self, model: &Model) -> Vec<String> {
        let mut out = Vec::new();
        let prefix = model.project.download_prefix();
        for t in model.published() {
            if self.artifact(&t.id).is_none() {
                out.push(format!(
                    "{}: no artifact for `{}`, which the model publishes; a partial release is not a release",
                    self.tag, t.id
                ));
            }
        }
        let mut seen: Vec<&str> = Vec::new();
        for a in &self.artifacts {
            if seen.contains(&a.target.as_str()) {
                out.push(format!("{}: two artifacts for `{}`", self.tag, a.target));
            }
            seen.push(&a.target);
            let Some(t) = model.target(&a.target) else {
                out.push(format!(
                    "{}: artifact for `{}`, which the model does not declare",
                    self.tag, a.target
                ));
                continue;
            };
            if !t.status.is_published() {
                out.push(format!(
                    "{}: artifact for `{}`, which the model does not publish",
                    self.tag, a.target
                ));
            }
            let expected = t.artifact_name(&model.project, &model.archive, &self.tag);
            if a.name != expected {
                out.push(format!(
                    "{}: artifact for `{}` is named `{}`; the naming function derives `{expected}`",
                    self.tag, a.target, a.name
                ));
            }
            if !a.url.starts_with(&prefix) {
                out.push(format!(
                    "{}: artifact for `{}` is served from `{}`, not from {prefix}",
                    self.tag, a.target, a.url
                ));
            }
            if !a.url.ends_with(&format!("/{}/{}", self.tag, a.name)) {
                out.push(format!(
                    "{}: the URL of `{}` does not end with its own tag and name",
                    self.tag, a.target
                ));
            }
            if a.sha256.len() != 64 || !a.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                out.push(format!(
                    "{}: the digest of `{}` is not 64 hexadecimal characters",
                    self.tag, a.target
                ));
            }
            if a.size == 0 {
                out.push(format!("{}: the size of `{}` is zero", self.tag, a.target));
            }
        }
        out
    }

    /// The public metadata a pinned installation reads. Rendered by hand rather than by
    /// `serde_json` so that every artifact is exactly one line: the installer parses this
    /// file with `sed`, in a shell that has no JSON parser, and
    /// `test/cases/85_installer.sh` holds both halves to that shape.
    pub fn public_json(&self, model: &Model) -> String {
        let mut s = String::from("{\n");
        s.push_str(&format!("  \"schema\": \"{}\",\n", SCHEMA));
        // Generated like everything else this tool writes, and it says so in the member the
        // typed-artifact check reads. Without it `majordomus generate` refuses to write the
        // file the moment a record exists — which is to say, from the first release onwards.
        s.push_str(&format!(
            "  \"generated\": \"{}\",\n",
            crate::generate::json_banner(&format!("{DIR}/{}.yaml", self.tag))
        ));
        s.push_str(&format!("  \"version\": \"{}\",\n", self.version));
        s.push_str(&format!("  \"tag\": \"{}\",\n", self.tag));
        s.push_str(&format!(
            "  \"channel\": \"{}\",\n",
            match self.channel {
                Channel::Stable => "stable",
                Channel::Prerelease => "prerelease",
            }
        ));
        s.push_str(&format!("  \"commit\": \"{}\",\n", self.commit));
        s.push_str(&format!("  \"published_at\": \"{}\",\n", self.published_at));
        if let Some(url) = &self.notes_url {
            s.push_str(&format!("  \"notes_url\": \"{url}\",\n"));
        }
        s.push_str("  \"artifacts\": {\n");
        // keyed by the Rust target triple: what the installer resolves the machine to
        let rows: Vec<String> = self
            .artifacts
            .iter()
            .filter_map(|a| model.target(&a.target).map(|t| (t, a)))
            .map(|(t, a)| {
                format!(
                    "    \"{}\": {{ \"name\": \"{}\", \"url\": \"{}\", \"sha256\": \"{}\", \"size\": {} }}",
                    t.rust_target, a.name, a.url, a.sha256, a.size
                )
            })
            .collect();
        s.push_str(&rows.join(",\n"));
        s.push_str("\n  }\n}\n");
        s
    }
}

/// Every record this repository holds, newest version first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Releases {
    /// The records, ordered by version, highest first.
    pub releases: Vec<Release>,
}

impl Releases {
    /// Read every record under `<root>/.ai/repo/releases/`. A repository that has
    /// published nothing has no directory, and that is not an error.
    pub fn load(root: &Path) -> Result<Self> {
        let dir = root.join(DIR);
        let mut releases = Vec::new();
        if dir.is_dir() {
            let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
                .map_err(|e| Error::io(&dir, e))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
                .collect();
            paths.sort();
            for path in paths {
                let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
                let release = Release::parse(&text).map_err(|reason| Error::InvalidRelease {
                    path: path.display().to_string(),
                    reason,
                })?;
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if stem != release.tag {
                    return Err(Error::InvalidRelease {
                        path: path.display().to_string(),
                        reason: format!("the file name is `{stem}` and the tag is `{}`; a record is named by its tag", release.tag),
                    });
                }
                releases.push(release);
            }
        }
        Ok(Self::ordered(releases))
    }

    /// The same, from the repository's index, which is what a capability sees.
    pub fn from_index(index: &Index) -> std::result::Result<Self, String> {
        let mut releases = Vec::new();
        for o in index.objects.iter().filter(|o| o.kind == KIND) {
            let release: Release = serde_json::from_value(o.metadata.clone())
                .map_err(|e| format!("{}: {e}", o.provenance.path))?;
            releases.push(release);
        }
        Ok(Self::ordered(releases))
    }

    pub(crate) fn ordered(mut releases: Vec<Release>) -> Self {
        releases.sort_by_key(|r| std::cmp::Reverse(version_key(&r.version)));
        Releases { releases }
    }

    /// The release an unpinned installation resolves to: the highest version among the
    /// stable, unyanked records. Derived, never authored.
    pub fn latest_stable(&self) -> Option<&Release> {
        self.releases.iter().find(|r| r.is_stable_candidate())
    }

    /// A release by tag.
    pub fn by_tag(&self, tag: &str) -> Option<&Release> {
        self.releases.iter().find(|r| r.tag == tag)
    }

    /// Is there any release at all?
    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }

    /// Every way the records disagree with the model or with each other.
    pub fn findings(&self, model: &Model) -> Vec<String> {
        let mut out = Vec::new();
        let mut tags: Vec<&str> = Vec::new();
        for r in &self.releases {
            if tags.contains(&r.tag.as_str()) {
                out.push(format!("two records claim the tag `{}`", r.tag));
            }
            tags.push(&r.tag);
            out.extend(r.findings(model));
        }
        out
    }
}

/// A comparable key for a semantic version: the three numbers, then whether it is a final
/// release (a pre-release sorts below its own final), then the pre-release text.
fn version_key(version: &str) -> (u64, u64, u64, u8, String) {
    let (core, pre) = match version.split_once('-') {
        Some((c, p)) => (c, p.to_string()),
        None => (version, String::new()),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        u8::from(pre.is_empty()),
        pre,
    )
}

/// The stable pointer, rendered from the record it points at. Identical in shape to a
/// pinned release's metadata, so that the installer has one parser and not two.
pub fn latest_json(release: &Release, model: &Model) -> String {
    release.public_json(model)
}

/// The conventional aggregate digest file, in `sha256sum -c` order.
pub fn checksums(release: &Release, _project: &Project, _archive: &Archive) -> String {
    let mut s = String::new();
    for a in &release.artifacts {
        s.push_str(&format!("{}  {}\n", a.sha256, a.name));
    }
    s
}

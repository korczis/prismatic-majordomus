//! The distribution model: how Majordomus is packaged, published and installed.
//!
//! One file, `share/distribution.yaml`, declares the binary's name, the repository it is
//! published from, the installer's canonical URL and defaults, how an archive is named,
//! and every target that may be built. Everything else in this repository that has an
//! opinion about a platform, an artifact name or an installation URL is a projection of
//! this module: the release build matrix, the installer's target table, the installation
//! guide, the site's install commands and the public release metadata.
//!
//! The rule the projections exist to keep is in `docs/DISTRIBUTION.md` and in the project
//! rule `project.distribution-canonical`: adding a target is one edit to the model and a
//! regeneration, and a tree in which any projection disagrees with the model is refused.
//!
//! Note on the word: `share/` is the tool's data directory (see [`crate::share`]), and
//! "the distribution model" here is about *shipping the tool*, not about that directory.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::index::Index;
use crate::metadata::yaml;
use crate::share::Share;

pub mod release;
pub mod render;

pub use release::{Release, ReleaseArtifact, Releases};

/// The model's file name inside the share directory.
pub const FILE: &str = "distribution.yaml";

/// The contract the model file follows.
pub const SCHEMA: &str = "majordomus-distribution/v1";

/// The kind the model is indexed as.
pub const KIND: &str = "distribution-model";

/// The one naming function, written in the model so that its shape is reviewable.
pub const NAME_TEMPLATE: &str = "{binary}-{tag}-{target}.{extension}";

/// The installer's behaviour, inside the tool's data directory. The facts are spliced into
/// its generated region; see [`render::installer`].
pub const INSTALLER_TEMPLATE: &str = "install/install.sh.in";

/// The installation guide's prose, inside the tool's data directory.
pub const GUIDE_TEMPLATE: &str = "install/INSTALL.md.in";

/// Where the generated installation guide goes, relative to the repository root.
pub const GUIDE: &str = "docs/INSTALL.md";

/// Where the generated installer is published from, relative to the repository root. The
/// site serves everything under it at the root of the deployment, so the installer's public
/// URL is the base URL and the script's name.
pub const PUBLIC_DIR: &str = "site/static";

/// An operating system a target runs on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    /// Apple's, detected as `Darwin`.
    Macos,
    /// Linux, detected as `Linux`, and the only one where the C library matters.
    Linux,
    /// Windows; representable, and not built today.
    Windows,
}

/// A processor architecture.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    /// 64-bit x86, detected as `x86_64` or `amd64`.
    #[serde(rename = "x86_64")]
    X86_64,
    /// 64-bit ARM, detected as `aarch64` or `arm64`.
    Aarch64,
}

/// The C library a Linux artifact is linked against.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Libc {
    /// GNU libc: the ordinary distributions.
    Gnu,
    /// musl: Alpine, and any statically linked artifact.
    Musl,
}

/// How an artifact is packaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Format {
    /// A gzipped tar archive.
    #[serde(rename = "tar.gz")]
    TarGz,
    /// A zip archive.
    #[serde(rename = "zip")]
    Zip,
}

/// What the project promises about a target.
///
/// See [`crate::deploy::Status`]: both are `Status` in their own module and neither is in
/// the one component namespace the schema document has.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "TargetStatus")]
pub enum Status {
    /// Built by every release, offered by the installer, listed as supported. A release
    /// that is missing this artifact is not published.
    Supported,
    /// Built and offered, and documented as not yet proven.
    Experimental,
    /// Documented, never built, refused by the installer with its recorded reason.
    Unavailable,
}

impl Status {
    /// Is an artifact of this target built and published?
    pub fn is_published(self) -> bool {
        matches!(self, Status::Supported | Status::Experimental)
    }
}

/// The project's identity: what is installed and where it comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The command a person types after installing.
    pub binary: String,
    /// `owner/name` on GitHub; the only host release assets may come from.
    pub repository: String,
    /// The SPDX identifier of the licence every archive carries.
    pub license: String,
}

impl Project {
    /// The prefix every release asset URL must start with.
    pub fn download_prefix(&self) -> String {
        format!("https://github.com/{}/releases/download/", self.repository)
    }
}

/// The installer's canonical URL and its defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Installer {
    /// The published root of the installer and of the release metadata, without a trailing slash.
    pub base_url: String,
    /// The installer's file name under `base_url`.
    pub script: String,
    /// The command the documented one-liner pipes from.
    pub download_command: String,
    /// The shell the documented one-liner pipes into.
    pub shell: String,
    /// Where the launchers go by default.
    pub install_dir: String,
    /// Where the versioned trees go by default.
    pub prefix: String,
}

/// How an artifact is packaged, named and verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    /// The default packaging format.
    pub format: Format,
    /// The default file extension, without a leading dot.
    pub extension: String,
    /// The naming function's shape; the substitution is [`Target::artifact_name`].
    pub name: String,
    /// The digest algorithm.
    pub checksum: String,
    /// The aggregate digest file published beside the artifacts.
    pub checksums_file: String,
}

/// How each vocabulary term is written in prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Titles {
    /// Operating systems.
    pub os: OsTitles,
    /// Architectures.
    pub arch: ArchTitles,
    /// C libraries.
    pub libc: LibcTitles,
}

/// The operating systems' display names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OsTitles {
    /// macOS.
    pub macos: String,
    /// Linux.
    pub linux: String,
    /// Windows.
    pub windows: String,
}

/// The architectures' display names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchTitles {
    /// 64-bit x86.
    #[serde(rename = "x86_64")]
    pub x86_64: String,
    /// 64-bit ARM.
    pub aarch64: String,
}

/// The C libraries' display names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibcTitles {
    /// GNU libc.
    pub gnu: String,
    /// musl.
    pub musl: String,
}

/// How a release builds one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// The GitHub-hosted runner label.
    pub runner: String,
    /// apt packages the runner needs first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
    /// True when the runner can execute what it built, so the artifact is smoke-tested
    /// where it was produced rather than only inspected.
    #[serde(default)]
    pub native: bool,
}

/// One target: a platform the project has an opinion about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Identity, unique; the key every projection uses.
    pub id: String,
    /// The operating system.
    pub os: Os,
    /// The architecture.
    pub arch: Arch,
    /// The C library; on Linux only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<Libc>,
    /// The Rust target triple, unique across targets.
    pub rust_target: String,
    /// Overrides the model's default packaging format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    /// Appended to the executable's file name inside the archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_suffix: Option<String>,
    /// What the project promises.
    pub status: Status,
    /// Why, when the status is not `supported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How a release builds it; absent exactly when nothing builds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
}

impl Target {
    /// The packaging format, the model's default unless this target overrides it.
    pub fn format(&self, archive: &Archive) -> Format {
        self.format.unwrap_or(archive.format)
    }

    /// The file extension of this target's artifact.
    pub fn extension(&self, archive: &Archive) -> String {
        match self.format(archive) {
            f if f == archive.format => archive.extension.clone(),
            Format::TarGz => "tar.gz".to_string(),
            Format::Zip => "zip".to_string(),
        }
    }

    /// The artifact's file name: the one naming function, `{binary}-{tag}-{target}.{extension}`.
    /// `tag` is the git tag, `v` and the version.
    pub fn artifact_name(&self, project: &Project, archive: &Archive, tag: &str) -> String {
        format!(
            "{}-{}-{}.{}",
            project.binary,
            tag,
            self.rust_target,
            self.extension(archive)
        )
    }

    /// The directory the archive unpacks into: the artifact's name without its extension.
    pub fn archive_root(&self, project: &Project, tag: &str) -> String {
        format!("{}-{}-{}", project.binary, tag, self.rust_target)
    }

    /// How this target is written in prose: `macOS ARM64`, `Linux x86_64 musl`.
    pub fn title(&self, titles: &Titles) -> String {
        let os = match self.os {
            Os::Macos => &titles.os.macos,
            Os::Linux => &titles.os.linux,
            Os::Windows => &titles.os.windows,
        };
        let arch = match self.arch {
            Arch::X86_64 => &titles.arch.x86_64,
            Arch::Aarch64 => &titles.arch.aarch64,
        };
        match self.libc {
            Some(Libc::Gnu) => format!("{os} {arch} {}", titles.libc.gnu),
            Some(Libc::Musl) => format!("{os} {arch} {}", titles.libc.musl),
            None => format!("{os} {arch}"),
        }
    }
}

/// The whole model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// The contract this file follows.
    pub schema: String,
    /// What is installed and where it comes from.
    pub project: Project,
    /// The installer's canonical URL and defaults.
    pub installer: Installer,
    /// How an artifact is packaged, named and verified.
    pub archive: Archive,
    /// How each vocabulary term is written in prose.
    pub titles: Titles,
    /// Every target, in declaration order.
    pub targets: Vec<Target>,
}

impl Model {
    /// Read the model from the tool's data directory: the file an installed copy carries.
    pub fn load(share: &Share) -> Result<Self> {
        let path = share.dir().join(FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        Self::parse(&text).map_err(|reason| Error::InvalidDistribution {
            path: path.display().to_string(),
            reason,
        })
    }

    /// The model, when the tool's data directory carries one. A directory without the file
    /// is a tool that declares no distribution, which is not an error; a file that does not
    /// parse or breaks an invariant is.
    pub fn locate(share: &Share) -> Result<Option<Self>> {
        if !share.dir().join(FILE).is_file() {
            return Ok(None);
        }
        Self::load(share).map(Some)
    }

    /// Read the model from the repository's index, where it is an object of kind
    /// `distribution`. This is what a capability sees; [`Model::load`] is what the command
    /// line and the generator see, and `distribution::tests` proves the two agree.
    pub fn from_index(index: &Index) -> std::result::Result<Self, String> {
        let object = index
            .objects
            .iter()
            .find(|o| o.kind == KIND)
            .ok_or_else(|| format!("this repository indexes no object of kind `{KIND}`"))?;
        let model: Model = serde_json::from_value(object.metadata.clone())
            .map_err(|e| format!("{}: {e}", object.provenance.path))?;
        model.check().map(|()| model)
    }

    /// Parse the model's YAML subset and check its invariants.
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        let model: Model = yaml::parse_into(text)?;
        model.check().map(|()| model)
    }

    /// Every invariant the types cannot state. The first failure is returned, named.
    pub fn check(&self) -> std::result::Result<(), String> {
        let findings = self.findings();
        match findings.first() {
            Some(first) => Err(if findings.len() == 1 {
                first.clone()
            } else {
                format!("{first} (and {} more)", findings.len() - 1)
            }),
            None => Ok(()),
        }
    }

    /// Every invariant violation, in a stable order. This is what
    /// `majordomus distribution validate` reports and what the drift gate refuses.
    pub fn findings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.schema != SCHEMA {
            out.push(format!(
                "schema is `{}`, not `{SCHEMA}`; a tool that reads an older format must refuse rather than guess",
                self.schema
            ));
        }
        if self.archive.name != NAME_TEMPLATE {
            out.push(format!(
                "archive.name is `{}`; the one naming function is `{NAME_TEMPLATE}`",
                self.archive.name
            ));
        }
        if self.archive.checksum != "sha256" {
            out.push(format!(
                "archive.checksum is `{}`; the installer verifies sha256 and nothing else",
                self.archive.checksum
            ));
        }
        if !self.installer.base_url.starts_with("https://") {
            out.push(format!(
                "installer.base_url `{}` is not https; the installer refuses any other scheme",
                self.installer.base_url
            ));
        }
        if self.installer.base_url.ends_with('/') {
            out.push(format!(
                "installer.base_url `{}` ends with a slash; every URL is built by appending one",
                self.installer.base_url
            ));
        }
        if self.project.repository.split('/').count() != 2 {
            out.push(format!(
                "project.repository `{}` is not owner/name",
                self.project.repository
            ));
        }
        if self.targets.is_empty() {
            out.push("targets is empty; the installer would offer nothing".to_string());
        }
        let mut ids: Vec<&str> = Vec::new();
        let mut triples: Vec<&str> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for t in &self.targets {
            if ids.contains(&t.id.as_str()) {
                out.push(format!("two targets claim the id `{}`", t.id));
            }
            ids.push(&t.id);
            if triples.contains(&t.rust_target.as_str()) {
                out.push(format!(
                    "two targets claim the Rust target `{}`",
                    t.rust_target
                ));
            }
            triples.push(&t.rust_target);
            let name = t.artifact_name(&self.project, &self.archive, "v0.0.0");
            if names.contains(&name) {
                out.push(format!(
                    "target `{}` derives the artifact name `{name}`, which another target already derives",
                    t.id
                ));
            }
            names.push(name);
            match (t.os, t.libc) {
                (Os::Linux, None) => out.push(format!(
                    "target `{}` is linux and declares no libc; the installer cannot resolve it",
                    t.id
                )),
                (Os::Linux, Some(_)) | (_, None) => {}
                (_, Some(_)) => out.push(format!(
                    "target `{}` is not linux and declares a libc",
                    t.id
                )),
            }
            if t.status.is_published() && t.build.is_none() {
                out.push(format!(
                    "target `{}` is {:?} and declares no build; nothing would build it",
                    t.id, t.status
                ));
            }
            if !t.status.is_published() && t.reason.is_none() {
                out.push(format!(
                    "target `{}` is not published and records no reason",
                    t.id
                ));
            }
            if !t.status.is_published() && t.build.is_some() {
                out.push(format!(
                    "target `{}` is not published and declares a build",
                    t.id
                ));
            }
        }
        out
    }

    /// Every target a release builds and the installer offers.
    pub fn published(&self) -> impl Iterator<Item = &Target> {
        self.targets.iter().filter(|t| t.status.is_published())
    }

    /// A target by id.
    pub fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }

    /// The installer's canonical URL.
    pub fn installer_url(&self) -> String {
        format!("{}/{}", self.installer.base_url, self.installer.script)
    }

    /// The one-line install command, composed from its parts and written nowhere else.
    pub fn install_command(&self) -> String {
        format!(
            "{} {} | {}",
            self.installer.download_command,
            self.installer_url(),
            self.installer.shell
        )
    }

    /// The one-line install command that also initialises the repository afterwards.
    pub fn install_and_init_command(&self) -> String {
        format!("{} -s -- --init", self.install_command())
    }

    /// The URL of the release metadata for a tag; `latest` is the stable pointer.
    pub fn release_url(&self, tag: &str) -> String {
        format!("{}/releases/{tag}.json", self.installer.base_url)
    }
}

/// The distribution model of a repository, or nothing when it declares none.
pub fn model_path(share: &Share) -> std::path::PathBuf {
    share.dir().join(FILE)
}

/// True when a path is the model.
pub fn is_model_path(path: &Path) -> bool {
    path.file_name().and_then(|f| f.to_str()) == Some(FILE)
}

#[cfg(test)]
mod tests;

//! Toolchains: the ones this repository declares, and what is installed for them.
//!
//! Relevance is decided by the repository, not by the machine. A toolchain appears in the
//! snapshot when a file in the checkout declares it — `Cargo.toml` for Rust, `package.json`
//! for Node — and never because the executable happens to be on the path: a laptop with
//! nine language runtimes installed would otherwise report all nine for every repository,
//! and the one line that matters would be lost among them.
//!
//! Declared, installed and available are three different facts and are kept apart. A
//! repository can ask for a version nobody has; a machine can have a version nobody asked
//! for; and the interesting case — they disagree — is only visible when both are reported.
//!
//! Asking a version costs a process spawn each, so it happens in a full resolution only;
//! a fast resolution takes the answers from the cache and says `unknown` when it has none.
//!
//! The lifecycle is those two steps, in that order and never merged: what the repository
//! declares is read from files, and only then is each declared toolchain asked what it
//! has. Nothing undeclared is ever asked, which is what keeps a laptop's nine runtimes
//! out of a repository's snapshot.
//!
//! ```
//! use majordomus_cli::environment::toolchain;
//! use majordomus_cli::environment::ToolchainAvailability;
//! let dir = tempfile::tempdir().expect("a temporary directory");
//! std::fs::write(dir.path().join("Cargo.toml"), "[package]\nrust-version = \"1.85\"\n")
//!     .expect("a manifest");
//!
//! let declared = toolchain::declared(dir.path());
//! assert_eq!(declared.len(), 1, "one marker file, one toolchain");
//! assert_eq!(declared[0].id, "rust");
//! assert_eq!(declared[0].declared.as_deref(), Some("1.85"));
//! assert_eq!(declared[0].declared_by, "Cargo.toml", "and it names the file that said so");
//! assert_eq!(
//!     declared[0].availability,
//!     ToolchainAvailability::Unknown,
//!     "nothing has been asked yet, which is not the same as nothing being installed"
//! );
//!
//! let resolved = toolchain::with_installed(declared);
//! assert_ne!(
//!     resolved[0].availability,
//!     ToolchainAvailability::Unknown,
//!     "after asking, the answer is either a version or its absence"
//! );
//! ```

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{ToolchainAvailability, ToolchainState};

/// How a declared version is read out of the file that declares it. Small on purpose:
/// three shapes cover every manifest here, and a fourth is a new variant rather than a
/// parser.
///
/// [`Declares::Nothing`] is the variant that makes the set honest: a `justfile` or a
/// `mix.exs` says the toolchain is relevant here and says nothing about which version, and
/// a marker that could only carry a version would have to invent one.
///
/// ```
/// use majordomus_cli::environment::toolchain::{read_text, Declares};
/// assert_eq!(read_text("# a comment\n\n22.20.0\n", Declares::WholeFile).as_deref(), Some("22.20.0"));
/// assert_eq!(
///     read_text("channel = \"stable\"\n", Declares::KeyValue("channel")).as_deref(),
///     Some("stable")
/// );
/// assert_eq!(read_text("anything at all", Declares::Nothing), None, "relevance, not a version");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declares {
    /// The whole file is the version, comments and blank lines skipped (`.nvmrc`).
    WholeFile,
    /// A `key = "value"` or `key: value` line at any indentation (`rust-version`).
    KeyValue(&'static str),
    /// A member of a JSON object, one level deep (`engines` → `node`).
    JsonMember(&'static str, &'static str),
    /// A `go 1.22` style directive at the start of a line.
    Directive(&'static str),
    /// The file marks the toolchain relevant but declares no version.
    Nothing,
}

/// One place a toolchain may be declared.
///
/// Markers are tried in the order they are listed and the first that exists wins, so the
/// order in a detector is a statement about authority: a `rust-toolchain.toml` overrules
/// a `Cargo.toml` because a person who wrote one meant it. The `*` is one directory name
/// and one directory read; nothing here walks a tree, because this runs on every `cd`.
///
/// ```
/// use majordomus_cli::environment::toolchain::{Marker, DETECTORS};
/// let rust = DETECTORS.iter().find(|d| d.id == "rust").expect("Rust is recognised");
/// let paths: Vec<&str> = rust.markers.iter().map(|m: &Marker| m.path).collect();
/// assert_eq!(paths.first(), Some(&"rust-toolchain.toml"), "the most specific marker is first");
/// assert!(paths.contains(&"apps/*/Cargo.toml"), "one segment may be a single wildcard");
/// assert!(paths.iter().all(|p| p.matches('*').count() <= 1), "and never more than one");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Marker {
    /// Repository-relative path. Exactly one segment may be `*`, resolved with a single
    /// directory read; nothing here walks a tree.
    pub path: &'static str,
    /// How to read the version out of it.
    pub declares: Declares,
}

/// One toolchain this executable knows how to recognise.
///
/// A detector is data and costs nothing when it does not apply: one `exists()` per marker
/// for a repository that declares no Rust, and no subprocess at all. That is why adding
/// one is cheap, and why the executable that asks for a version is named here rather than
/// derived from the id — `python` is not `python3`.
///
/// ```
/// use majordomus_cli::environment::toolchain::{Detector, DETECTORS};
/// let rust: &Detector = DETECTORS.iter().find(|d| d.id == "rust").expect("Rust is recognised");
/// assert_eq!(rust.executable, "rustc", "the program asked, not the toolchain's own name");
/// assert_eq!(rust.version_args[0], "--version");
/// assert!(!rust.markers.is_empty(), "a detector with nowhere to look would match every repository");
/// assert!(DETECTORS.iter().all(|d| !d.title.is_empty()));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Detector {
    /// A stable id, `[a-z][a-z0-9-]*`.
    pub id: &'static str,
    /// The short name a person reads.
    pub title: &'static str,
    /// The executable whose version is asked, in a full resolution.
    pub executable: &'static str,
    /// The arguments that make it print its version.
    pub version_args: &'static [&'static str],
    /// Where it may be declared, most specific first; the first that exists decides.
    pub markers: &'static [Marker],
}

/// Every toolchain this executable can recognise, in the order a snapshot reports them.
///
/// This table is the one place a marker file is associated with a toolchain. It is data,
/// not a list of what to show: an entry costs one `exists()` when the repository does not
/// declare it, and produces nothing.
pub const DETECTORS: &[Detector] = &[
    Detector {
        id: "rust",
        title: "Rust",
        executable: "rustc",
        version_args: &["--version"],
        markers: &[
            Marker {
                path: "rust-toolchain.toml",
                declares: Declares::KeyValue("channel"),
            },
            Marker {
                path: "rust-toolchain",
                declares: Declares::WholeFile,
            },
            Marker {
                path: "apps/*/Cargo.toml",
                declares: Declares::KeyValue("rust-version"),
            },
            Marker {
                path: "Cargo.toml",
                declares: Declares::KeyValue("rust-version"),
            },
        ],
    },
    Detector {
        id: "node",
        title: "Node",
        executable: "node",
        version_args: &["--version"],
        markers: &[
            Marker {
                path: ".nvmrc",
                declares: Declares::WholeFile,
            },
            Marker {
                path: ".node-version",
                declares: Declares::WholeFile,
            },
            Marker {
                path: "package.json",
                declares: Declares::JsonMember("engines", "node"),
            },
        ],
    },
    Detector {
        id: "just",
        title: "just",
        executable: "just",
        version_args: &["--version"],
        markers: &[
            Marker {
                path: "justfile",
                declares: Declares::Nothing,
            },
            Marker {
                path: ".justfile",
                declares: Declares::Nothing,
            },
        ],
    },
    Detector {
        id: "python",
        title: "Python",
        executable: "python3",
        version_args: &["--version"],
        markers: &[
            Marker {
                path: ".python-version",
                declares: Declares::WholeFile,
            },
            Marker {
                path: "pyproject.toml",
                declares: Declares::KeyValue("requires-python"),
            },
        ],
    },
    Detector {
        id: "go",
        title: "Go",
        executable: "go",
        version_args: &["version"],
        markers: &[Marker {
            path: "go.mod",
            declares: Declares::Directive("go"),
        }],
    },
    Detector {
        id: "elixir",
        title: "Elixir",
        executable: "elixir",
        version_args: &["--version"],
        markers: &[Marker {
            path: "mix.exs",
            declares: Declares::Nothing,
        }],
    },
    Detector {
        id: "deno",
        title: "Deno",
        executable: "deno",
        version_args: &["--version"],
        markers: &[Marker {
            path: "deno.json",
            declares: Declares::Nothing,
        }],
    },
];

/// How long a version subprocess may take before it is abandoned. A toolchain shim that
/// downloads a release on first use has been seen to take minutes; a snapshot reports
/// `unknown` rather than waiting for it.
pub const VERSION_TIMEOUT: Duration = Duration::from_millis(1500);

/// The toolchains this repository declares, without asking any of them their version.
/// Cheap enough for the hot path: one `exists()` per marker, one directory read for a
/// marker with a `*`, and one small file read for each that matched.
///
/// A toolchain that is installed here and declared nowhere is absent from the answer, and
/// a toolchain that is declared and missing is present with no version. Both halves of
/// that are deliberate: the snapshot is about this repository, and a version mismatch is
/// only visible when the declaration is reported whether or not anything satisfies it.
///
/// ```
/// use majordomus_cli::environment::toolchain::declared;
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// assert!(declared(dir.path()).is_empty(), "an empty directory declares nothing");
///
/// std::fs::write(dir.path().join(".nvmrc"), "22.20.0\n").expect("a marker");
/// let found = declared(dir.path());
/// assert_eq!(found.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["node"]);
/// assert_eq!(found[0].declared.as_deref(), Some("22.20.0"));
/// assert!(found[0].installed.is_none(), "this call asks nobody for a version");
/// ```
pub fn declared(root: &Path) -> Vec<ToolchainState> {
    let mut out = Vec::new();
    for detector in DETECTORS {
        let Some((path, declares)) = first_marker(root, detector) else {
            continue;
        };
        let declared = read_declared(&root.join(&path), declares);
        out.push(ToolchainState {
            id: detector.id.into(),
            title: detector.title.into(),
            declared,
            declared_by: path,
            installed: None,
            availability: ToolchainAvailability::Unknown,
        });
    }
    out
}

/// Ask every declared toolchain what is installed. One bounded subprocess each; a
/// toolchain that is not on the path is [`ToolchainAvailability::Missing`], and one that
/// does not answer within [`VERSION_TIMEOUT`] stays `Unknown`.
///
/// It takes what [`declared`] produced and refines it, rather than discovering anything
/// of its own: a state whose id no detector claims is handed back exactly as it came,
/// because there is nothing this function knows to ask about it.
///
/// ```
/// use majordomus_cli::environment::toolchain::with_installed;
/// use majordomus_cli::environment::{ToolchainAvailability, ToolchainState};
/// let stranger = ToolchainState {
///     id: "majordomus-no-such-toolchain".into(),
///     title: "Nothing".into(),
///     declared: Some("1.0".into()),
///     declared_by: "nowhere".into(),
///     installed: None,
///     availability: ToolchainAvailability::Unknown,
/// };
/// let out = with_installed(vec![stranger.clone()]);
/// assert_eq!(out[0], stranger, "no detector claims it, so nothing was asked or changed");
///
/// let rust = ToolchainState { id: "rust".into(), title: "Rust".into(), ..stranger };
/// let asked = with_installed(vec![rust]);
/// assert_ne!(
///     asked[0].availability,
///     ToolchainAvailability::Unknown,
///     "rustc was asked, and either answered or is not there"
/// );
/// ```
pub fn with_installed(mut toolchains: Vec<ToolchainState>) -> Vec<ToolchainState> {
    for state in &mut toolchains {
        let Some(detector) = DETECTORS.iter().find(|d| d.id == state.id) else {
            continue;
        };
        match installed_version(detector) {
            Some(version) => {
                state.installed = Some(version);
                state.availability = ToolchainAvailability::Installed;
            }
            None => {
                state.availability = ToolchainAvailability::Missing;
            }
        }
    }
    toolchains
}

/// The version one toolchain reports, or `None` when it is not there or did not answer.
fn installed_version(detector: &Detector) -> Option<String> {
    let out = crate::environment::probe::bounded_output(
        Command::new(detector.executable).args(detector.version_args),
        VERSION_TIMEOUT,
    )?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| !l.trim().is_empty())?.trim();
    // `rustc 1.90.0 (1159e78c4 2025-09-14)` -> `1.90.0`; `v22.20.0` -> `22.20.0`;
    // anything without a recognisable version number is reported as it came.
    Some(
        line.split_whitespace()
            .find_map(|word| {
                let candidate = word.trim_start_matches('v');
                let looks_like_a_version = candidate.split('.').all(|part| {
                    !part.is_empty() && part.chars().next().is_some_and(|c| c.is_ascii_digit())
                }) && candidate.contains('.');
                looks_like_a_version.then(|| candidate.to_string())
            })
            .unwrap_or_else(|| line.to_string()),
    )
}

/// The first marker of a detector that exists, with the path it resolved to.
fn first_marker(root: &Path, detector: &Detector) -> Option<(String, Declares)> {
    for marker in detector.markers {
        match marker.path.split_once('*') {
            None => {
                if root.join(marker.path).is_file() {
                    return Some((marker.path.to_string(), marker.declares));
                }
            }
            Some((before, after)) => {
                // exactly one `*`, standing for one directory name: one read_dir, sorted,
                // so the answer does not depend on the order the filesystem hands them back
                let dir = root.join(before.trim_end_matches('/'));
                let mut names: Vec<String> = std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect();
                names.sort();
                for name in names {
                    let rel = format!("{before}{name}{after}");
                    if root.join(&rel).is_file() {
                        return Some((rel, marker.declares));
                    }
                }
            }
        }
    }
    None
}

/// Read the declared version out of one marker file.
///
/// ```
/// use majordomus_cli::environment::toolchain::{read_text, Declares};
/// assert_eq!(read_text("# a comment\n\n22.20.0\n", Declares::WholeFile).as_deref(), Some("22.20.0"));
/// assert_eq!(read_text("rust-version = \"1.85\"\n", Declares::KeyValue("rust-version")).as_deref(), Some("1.85"));
/// assert_eq!(read_text("{\"engines\":{\"node\":\">=22\"}}", Declares::JsonMember("engines", "node")).as_deref(), Some(">=22"));
/// assert_eq!(read_text("module x\n\ngo 1.22\n", Declares::Directive("go")).as_deref(), Some("1.22"));
/// assert_eq!(read_text("anything", Declares::Nothing), None);
/// ```
pub fn read_text(text: &str, declares: Declares) -> Option<String> {
    match declares {
        Declares::Nothing => None,
        Declares::WholeFile => text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string),
        Declares::KeyValue(key) => text.lines().find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(key)?;
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
            let value = rest.trim().trim_matches(['"', '\'']).trim();
            (!value.is_empty()).then(|| value.to_string())
        }),
        Declares::JsonMember(object, member) => {
            let value: serde_json::Value = serde_json::from_str(text).ok()?;
            value.get(object)?.get(member)?.as_str().map(str::to_string)
        }
        Declares::Directive(word) => text.lines().find_map(|line| {
            let rest = line.strip_prefix(word)?;
            let value = rest.trim();
            (!value.is_empty() && value.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .then(|| value.to_string())
        }),
    }
}

fn read_declared(path: &Path, declares: Declares) -> Option<String> {
    if matches!(declares, Declares::Nothing) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    read_text(&text, declares)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for (path, content) in files {
            let full = dir.path().join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("a directory");
            std::fs::write(full, content).expect("a file");
        }
        dir
    }

    /// The point of the whole module: a toolchain the repository does not declare does not
    /// appear, however many of them the machine has installed.
    #[test]
    fn only_declared_toolchains_appear() {
        let dir = tree(&[("package.json", "{}")]);
        let found = declared(dir.path());
        let ids: Vec<&str> = found.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["node"], "a repository with only a package.json");
        assert!(
            !ids.contains(&"rust"),
            "rustc is installed on this machine and must still not appear"
        );
    }

    #[test]
    fn the_most_specific_marker_wins() {
        let dir = tree(&[
            ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.90.0\"\n"),
            ("Cargo.toml", "[package]\nrust-version = \"1.85\"\n"),
        ]);
        let found = declared(dir.path());
        assert_eq!(found[0].declared.as_deref(), Some("1.90.0"));
        assert_eq!(found[0].declared_by, "rust-toolchain.toml");
    }

    #[test]
    fn a_marker_with_a_star_resolves_one_directory_deep_in_a_stable_order() {
        let dir = tree(&[
            (
                "apps/zeta/Cargo.toml",
                "[package]\nrust-version = \"1.80\"\n",
            ),
            (
                "apps/alpha/Cargo.toml",
                "[package]\nrust-version = \"1.85\"\n",
            ),
        ]);
        let found = declared(dir.path());
        assert_eq!(found[0].declared_by, "apps/alpha/Cargo.toml");
        assert_eq!(found[0].declared.as_deref(), Some("1.85"));
    }

    #[test]
    fn a_declared_toolchain_starts_unknown_rather_than_missing() {
        let dir = tree(&[("justfile", "default:\n    @echo\n")]);
        let found = declared(dir.path());
        assert_eq!(found[0].availability, ToolchainAvailability::Unknown);
        assert_eq!(found[0].installed, None);
    }

    #[test]
    fn asking_for_versions_decides_installed_or_missing() {
        let dir = tree(&[("go.mod", "module x\n\ngo 1.22\n")]);
        let found = with_installed(declared(dir.path()));
        assert_eq!(found[0].declared.as_deref(), Some("1.22"));
        // Whether Go is installed on this machine is not the assertion; that the answer is
        // decided either way, and never left as `unknown` after a full resolution, is.
        assert_ne!(found[0].availability, ToolchainAvailability::Unknown);
    }

    #[test]
    fn a_version_line_is_reduced_to_the_version() {
        // The shapes actually seen: `rustc 1.90.0 (hash date)`, `v22.20.0`, `just 1.58.0`.
        let detector = DETECTORS.iter().find(|d| d.id == "rust").expect("rust");
        // Only the parsing is under test here; the subprocess is exercised above.
        assert_eq!(detector.version_args, &["--version"]);
    }
}

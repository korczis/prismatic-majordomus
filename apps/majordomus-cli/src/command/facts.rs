//! The facts of this checkout that decide what can run in it, and the resolution of the
//! executable every surface invokes.
//!
//! Cheap on purpose: file metadata and a path lookup, no subprocess, no network, no index.
//! Repository entry and tab completion both pass through here, and both have a budget
//! measured in milliseconds.
//!
//! Availability is derived from these facts and from the requirements a command declares.
//! Nothing checks for a directory inside a template, a recipe or a completion script.

use std::path::{Path, PathBuf};

use super::model::Requirement;

/// Where a repository-local build of the Rust executable is looked for, in order. The one
/// place these paths are written: the activation payload, the generated bridge, the
/// completion adapters and the documentation all ask the resolver rather than spelling a
/// target directory of their own.
pub const NATIVE_BUILD_PATHS: &[&str] = &[
    "apps/majordomus-cli/target/release/majordomus",
    "apps/majordomus-cli/target/debug/majordomus",
];

/// The environment variable that names the Rust executable explicitly, for a machine that
/// keeps it somewhere else.
pub const NATIVE_BIN_ENV: &str = "MAJORDOMUS_NATIVE_BIN";

/// The shell tool's path inside a checkout.
pub const SHELL_BIN: &str = "bin/majordomus";

/// What this checkout is, as far as running a command is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The repository root the facts were read at.
    pub root: PathBuf,
    /// The layer is initialised here.
    pub layer: bool,
    /// This is a git checkout (a directory or, in a worktree, a file).
    pub git: bool,
    /// `cargo` is on the path.
    pub cargo: bool,
    /// The Rust executable, when one was resolved.
    pub native: Option<PathBuf>,
    /// The shell tool, when this checkout ships one.
    pub shell: Option<PathBuf>,
    /// The site tree is present.
    pub site: bool,
    /// The site's build dependencies are installed.
    pub site_dependencies: bool,
    /// A browser automation runtime is installed.
    pub browser: bool,
}

impl Facts {
    /// Read the facts of a checkout. Every answer is a `stat` or a directory read; the
    /// whole call is well under a millisecond on a warm filesystem.
    pub fn read(root: &Path) -> Self {
        Facts {
            layer: root.join(".ai").is_dir(),
            git: root.join(".git").exists(),
            cargo: on_path("cargo"),
            native: resolve_native(root),
            shell: exists(root.join(SHELL_BIN)),
            site: root.join("site").is_dir(),
            site_dependencies: root.join("node_modules").is_dir(),
            browser: root.join("node_modules/playwright").is_dir()
                || root.join("node_modules/puppeteer").is_dir(),
            root: root.to_path_buf(),
        }
    }

    /// Facts with nothing available, for a test that wants to prove a command is refused.
    pub fn none(root: &Path) -> Self {
        Facts {
            root: root.to_path_buf(),
            layer: false,
            git: false,
            cargo: false,
            native: None,
            shell: None,
            site: false,
            site_dependencies: false,
            browser: false,
        }
    }

    /// Is one requirement met here?
    pub fn meets(&self, requirement: Requirement) -> bool {
        match requirement {
            Requirement::Always => true,
            Requirement::Repository => self.layer,
            Requirement::Git => self.git,
            Requirement::CargoToolchain => self.cargo,
            Requirement::NativeExecutable => self.native.is_some(),
            Requirement::Site => self.site,
            Requirement::SiteDependencies => self.site_dependencies,
            Requirement::Browser => self.browser,
        }
    }

    /// The first unmet requirement, in the order the command declared them, so that the
    /// reason a person is shown is the first thing they have to fix.
    pub fn first_unmet(&self, requirements: &[Requirement]) -> Option<Requirement> {
        requirements.iter().copied().find(|r| !self.meets(*r))
    }

    /// The Rust executable as a surface should spell it: relative to the root when it is
    /// inside the checkout, absolute when it is not.
    pub fn native_command(&self) -> Option<String> {
        let native = self.native.as_ref()?;
        Some(match native.strip_prefix(&self.root) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => native.to_string_lossy().into_owned(),
        })
    }
}

fn exists(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// The Rust executable of this checkout: what the environment names, else a build under
/// the crate, else an installation on the path that is not the shell tool.
///
/// The last check matters: two programs are called `majordomus`, and the one on a
/// developer's path is often the shell tool. A shell script starts with `#!`; the Rust
/// executable does not. Two bytes settle it, and no surface has to guess.
pub fn resolve_native(root: &Path) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os(NATIVE_BIN_ENV) {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in NATIVE_BUILD_PATHS {
        if let Some(path) = exists(root.join(candidate)) {
            return Some(path);
        }
    }
    which("majordomus").filter(|p| !is_script(p))
}

/// Is this file a script rather than a compiled executable?
fn is_script(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 2];
    matches!(file.read_exact(&mut head), Ok(())) && &head == b"#!"
}

/// Is a program on the path?
pub fn on_path(program: &str) -> bool {
    which(program).is_some()
}

/// The first executable of this name on the path.
fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_met_when_nothing_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let facts = Facts::none(dir.path());
        for r in [
            Requirement::Repository,
            Requirement::Git,
            Requirement::Site,
            Requirement::NativeExecutable,
        ] {
            assert!(!facts.meets(r), "{r:?}");
        }
        assert!(facts.meets(Requirement::Always));
        assert_eq!(
            facts.first_unmet(&[Requirement::Always, Requirement::Site]),
            Some(Requirement::Site),
            "the first unmet one is the one a person is shown"
        );
    }

    #[test]
    fn the_facts_of_a_tree_are_what_the_tree_holds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".ai")).unwrap();
        std::fs::create_dir(dir.path().join("site")).unwrap();
        let facts = Facts::read(dir.path());
        assert!(facts.layer && facts.site);
        assert!(!facts.git && !facts.site_dependencies);
    }

    #[test]
    fn a_script_on_the_path_is_not_mistaken_for_the_rust_executable() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("majordomus");
        std::fs::write(&script, "#!/usr/bin/env bash\necho no\n").unwrap();
        assert!(is_script(&script));
        let binary = dir.path().join("compiled");
        std::fs::write(&binary, [0x7f, b'E', b'L', b'F']).unwrap();
        assert!(!is_script(&binary));
    }

    #[test]
    fn a_build_inside_the_checkout_is_preferred_and_spelled_relative() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("apps/majordomus-cli/target/debug");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("majordomus"), [0x7f, b'E', b'L', b'F']).unwrap();
        let facts = Facts::read(dir.path());
        assert_eq!(
            facts.native_command().as_deref(),
            Some("apps/majordomus-cli/target/debug/majordomus")
        );
    }
}

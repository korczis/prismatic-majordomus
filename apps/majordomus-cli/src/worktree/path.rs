//! The one derivation: repository → container, and container + branch → worktree path.
//!
//! ```text
//! container_root(<parent>/<repo>)                 = <parent>/<repo>-wt
//! expected_path(<parent>/<repo>-wt, feature/x/y)  = <parent>/<repo>-wt/feature/x/y
//! ```
//!
//! A branch name is filesystem-derived input and is treated as such. [`BranchName`] carries
//! git's own rules for a reference name — no `..`, no component starting with `.`, none
//! ending in `.lock`, no control characters, none of `~ ^ : ? * [ \`, no `@{`, no leading
//! `-` — so that every component of a valid name is an ordinary directory name and the join
//! cannot leave the container. [`expected_path`] proves that anyway, lexically, on every
//! call: a derivation that trusts its own validation is one refactor away from a traversal.

use std::path::{Component, Path, PathBuf};

use super::error::{Result, WorktreeError};

/// What is appended to the primary checkout's directory name to name the container. The
/// one place the convention is written; every path, document and message derives from it.
pub const CONTAINER_SUFFIX: &str = "-wt";

/// A branch name git would accept for `refs/heads/`, whose components are therefore safe
/// directory names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BranchName(String);

impl BranchName {
    /// Validate a branch name by git's reference rules, offline.
    ///
    /// ```
    /// use majordomus_cli::worktree::BranchName;
    /// assert!(BranchName::parse("feature/improve-cli").is_ok());
    /// assert!(BranchName::parse("feature/providers/openai-streaming").is_ok());
    /// assert!(BranchName::parse("žluťoučký/kůň").is_ok());
    /// for bad in ["", "/", "a/", "/a", "a//b", "a/../b", "..", ".hidden", "a/.b", "a.lock",
    ///             "a b", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a@{b", "-x",
    ///             "a.", "a/b.lock/c", "a\u{7f}b", "a\nb"] {
    ///     assert!(BranchName::parse(bad).is_err(), "{bad:?} should be refused");
    /// }
    /// ```
    pub fn parse(name: &str) -> Result<Self> {
        match validate(name) {
            Ok(()) => Ok(BranchName(name.to_string())),
            Err(reason) => Err(WorktreeError::InvalidBranchName {
                given: name.to_string(),
                reason,
            }),
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The relative path under the container: the name's components, one directory each.
    ///
    /// ```
    /// use majordomus_cli::worktree::BranchName;
    /// use std::path::PathBuf;
    /// let b = BranchName::parse("fix/cockpit/reconnect").unwrap();
    /// assert_eq!(b.relative_path(), PathBuf::from("fix/cockpit/reconnect"));
    /// ```
    pub fn relative_path(&self) -> PathBuf {
        let mut p = PathBuf::new();
        for c in self.0.split('/') {
            p.push(c);
        }
        p
    }

    /// The components, in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl std::fmt::Display for BranchName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a branch name cannot be one, in one line; `Ok(())` when it can.
///
/// These are git's rules for a reference under `refs/heads/` (`git check-ref-format`),
/// written here so that a path can be derived without a subprocess. Git stays the
/// authority at the moment a branch is created, and the crate's tests hold the two
/// implementations to the same verdict over generated names.
pub fn validate(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("it is empty".into());
    }
    if name.starts_with('-') {
        return Err("it begins with `-`, which git reads as an option".into());
    }
    if name.starts_with('/') {
        return Err("it begins with `/`".into());
    }
    if name.ends_with('/') {
        return Err("it ends with `/`".into());
    }
    if name.ends_with('.') {
        return Err("it ends with `.`".into());
    }
    if name.contains("..") {
        return Err("it contains `..`".into());
    }
    if name.contains("@{") {
        return Err("it contains `@{`".into());
    }
    if name.contains("//") {
        return Err("it contains an empty component (`//`)".into());
    }
    for ch in name.chars() {
        if ch.is_ascii_control() {
            return Err("it contains a control character".into());
        }
        if matches!(ch, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\') {
            return Err(format!(
                "it contains `{ch}`, which git does not allow in a reference"
            ));
        }
    }
    for component in name.split('/') {
        if component.starts_with('.') {
            return Err(format!(
                "component `{component}` begins with `.`; git refuses it, and it would be a hidden directory"
            ));
        }
        if component.ends_with(".lock") {
            return Err(format!("component `{component}` ends with `.lock`"));
        }
    }
    Ok(())
}

/// The container of a repository whose primary checkout is `primary`: the checkout's
/// sibling, named after it with [`CONTAINER_SUFFIX`] appended. Never read from anywhere,
/// never configured.
///
/// ```
/// use majordomus_cli::worktree::container_root;
/// use std::path::{Path, PathBuf};
/// assert_eq!(container_root(Path::new("/a/foo")).unwrap(), PathBuf::from("/a/foo-wt"));
/// assert_eq!(container_root(Path::new("/src/acme/backend")).unwrap(), PathBuf::from("/src/acme/backend-wt"));
/// assert_eq!(container_root(Path::new("/a/some.repo")).unwrap(), PathBuf::from("/a/some.repo-wt"));
/// assert!(container_root(Path::new("/")).is_err());
/// ```
pub fn container_root(primary: &Path) -> Result<PathBuf> {
    let name = primary
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty());
    let parent = primary
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    match (parent, name) {
        (Some(parent), Some(name)) => Ok(parent.join(format!("{name}{CONTAINER_SUFFIX}"))),
        _ => Err(WorktreeError::NoParentDirectory {
            primary: primary.to_path_buf(),
        }),
    }
}

/// Where the worktree of `branch` belongs: the branch's components under the container.
///
/// Proved on every call, lexically and without touching the filesystem, to be strictly
/// below the container: every component of the result after the container's own is a
/// normal component, so the path cannot escape, cannot be the container itself, and cannot
/// carry `.` or `..`.
///
/// ```
/// use majordomus_cli::worktree::{expected_path, BranchName};
/// use std::path::{Path, PathBuf};
/// let root = Path::new("/a/foo-wt");
/// let b = BranchName::parse("feature/providers/openai-streaming").unwrap();
/// assert_eq!(expected_path(root, &b).unwrap(), PathBuf::from("/a/foo-wt/feature/providers/openai-streaming"));
/// ```
pub fn expected_path(container: &Path, branch: &BranchName) -> Result<PathBuf> {
    let path = container.join(branch.relative_path());
    let escape = || WorktreeError::PathEscape {
        branch: branch.as_str().to_string(),
        path: path.clone(),
        root: container.to_path_buf(),
    };
    if !path.starts_with(container) || path == container {
        return Err(escape());
    }
    let extra = path.strip_prefix(container).map_err(|_| escape())?;
    if extra.as_os_str().is_empty()
        || !extra
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(escape());
    }
    Ok(path)
}

/// The label a detached worktree is shown under. Display only: a detached HEAD has no
/// branch and therefore no canonical path, and nothing ever moves a worktree under this
/// name.
///
/// ```
/// use majordomus_cli::worktree::detached_label;
/// assert_eq!(detached_label("6df5eaf11efa4ccdcbca8f1990763f749256390e"), "detached/6df5eaf11efa");
/// assert_eq!(detached_label("abc"), "detached/abc");
/// ```
pub fn detached_label(head: &str) -> String {
    format!("detached/{}", &head[..12.min(head.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_is_preserved_never_flattened() {
        let root = Path::new("/a/foo-wt");
        for (branch, rel) in [
            ("feature/foo", "feature/foo"),
            ("feature/foo/bar", "feature/foo/bar"),
            ("fix/test", "fix/test"),
            ("refactor/a/b/c", "refactor/a/b/c"),
            ("main", "main"),
        ] {
            let b = BranchName::parse(branch).unwrap();
            assert_eq!(expected_path(root, &b).unwrap(), root.join(rel));
        }
    }

    #[test]
    fn the_container_is_the_sibling_whatever_the_parent() {
        assert_eq!(
            container_root(Path::new("/Users/me/dev/prismatic-majordomus")).unwrap(),
            PathBuf::from("/Users/me/dev/prismatic-majordomus-wt")
        );
        assert_eq!(
            container_root(Path::new("/a/b/c/d")).unwrap(),
            PathBuf::from("/a/b/c/d-wt")
        );
    }

    #[test]
    fn a_traversal_cannot_be_a_branch_name_and_cannot_escape() {
        for bad in ["../x", "a/../../etc", "..", "./x", "a/./b"] {
            assert!(
                BranchName::parse(bad).is_err(),
                "{bad} parsed as a branch name"
            );
        }
    }

    #[test]
    fn a_derived_path_is_always_strictly_below_the_container() {
        let root = Path::new("/a/foo-wt");
        let b = BranchName::parse("x").unwrap();
        let p = expected_path(root, &b).unwrap();
        assert!(p.starts_with(root) && p != root);
        assert_eq!(p.parent(), Some(root));
    }

    #[test]
    fn unicode_components_are_ordinary_directory_names() {
        let root = Path::new("/a/foo-wt");
        let b = BranchName::parse("feature/žluťoučký-kůň").unwrap();
        assert_eq!(
            expected_path(root, &b).unwrap(),
            PathBuf::from("/a/foo-wt/feature/žluťoučký-kůň")
        );
    }
}

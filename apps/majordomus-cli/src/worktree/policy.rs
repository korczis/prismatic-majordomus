//! The canonical worktree policy: the one place the `-wt` convention is written down.
//!
//! `.ai/repo/policy.yaml` holds it, `share/schemas/majordomus/policy/policy.v1.schema.json`
//! declares its shape, and `share/allow/policy.txt` is generated from that schema — so the
//! shell tool and this executable agree on the key set without either of them carrying a
//! copy of it. Nothing else in this repository writes `-wt`: the CLI derives the container
//! from here, doctor evaluates against here, the provider projections render from here, and
//! the tests change *this* to prove it.
//!
//! A repository that writes no `worktree:` section still gets the doctrine, because the
//! defaults below are the doctrine. Silence is not an opt-out.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::{Result, WorktreeError};
use super::identity::{RepositoryIdentity, ResolvedPath};

/// Where the container goes, relative to the primary checkout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RootStrategy {
    /// Beside the primary checkout, named after it: `parent(primary)/basename(primary)+suffix`.
    #[default]
    Sibling,
}

/// How a directory name is derived from a branch or a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NamingStrategy {
    /// One component: everything outside `A-Za-z0-9._-` becomes a separator, runs collapse.
    #[default]
    Slug,
}

/// What a violation of the layout means to a command that checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// A finding that fails the check and the command's exit code.
    #[default]
    Error,
    /// A finding that is reported and does not decide the exit code.
    Warn,
    /// Not evaluated.
    Off,
}

/// What a destructive command does when the work tree is not obviously disposable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    /// Refuse, and say what would have been lost. `--force` is the explicit override.
    #[default]
    Refuse,
    /// Proceed.
    Allow,
}

/// `worktree.root:` — how the container is located.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RootPolicy {
    #[serde(default)]
    /// The placement strategy; `sibling` is the doctrine.
    pub strategy: RootStrategy,
    #[serde(default = "default_suffix")]
    /// Appended to the primary checkout's directory name to name the container.
    pub suffix: String,
}

fn default_suffix() -> String {
    "-wt".to_string()
}

impl Default for RootPolicy {
    fn default() -> Self {
        RootPolicy {
            strategy: RootStrategy::Sibling,
            suffix: default_suffix(),
        }
    }
}

/// `worktree.naming:` — how a worktree directory is named.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamingPolicy {
    #[serde(default)]
    /// The derivation; `slug` is the only one implemented.
    pub strategy: NamingStrategy,
}

/// `worktree.enforcement:` — what the checks do with what they find.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnforcementPolicy {
    #[serde(default)]
    /// A registered linked worktree outside the canonical root.
    pub outside_root: Level,
}

/// `worktree.cleanup:` — how conservative the destructive commands are by default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanupPolicy {
    #[serde(default)]
    /// A work tree with uncommitted or untracked content.
    pub dirty: Disposition,
    #[serde(default)]
    /// A work tree git has locked.
    pub locked: Disposition,
}

/// The worktree section of the canonical policy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreePolicy {
    #[serde(default)]
    /// Where the container is.
    pub root: RootPolicy,
    #[serde(default)]
    /// How worktrees are named inside it.
    pub naming: NamingPolicy,
    #[serde(default)]
    /// What a violation means.
    pub enforcement: EnforcementPolicy,
    #[serde(default)]
    /// What the destructive commands refuse.
    pub cleanup: CleanupPolicy,
}

impl WorktreePolicy {
    /// The canonical container for a repository, derived from this policy and the
    /// repository's identity — never from the current directory, never from a marker file,
    /// never from an environment variable.
    ///
    /// This is the function the whole subsystem turns on. Run from a linked worktree it
    /// still answers the container, because [`RepositoryIdentity::primary_worktree`] is the
    /// primary checkout wherever the command was typed.
    pub fn canonical_root(&self, identity: &RepositoryIdentity) -> Result<ResolvedPath> {
        Ok(ResolvedPath::of(self.canonical_root_of(
            &identity.repository_parent()?,
            &identity.repository_name(),
        )?))
    }

    /// The same derivation over plain values, so it can be proved without a repository.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreePolicy;
    /// use std::path::PathBuf;
    /// let p = WorktreePolicy::default();
    /// assert_eq!(p.canonical_root_of(&PathBuf::from("/a"), "foo").unwrap(), PathBuf::from("/a/foo-wt"));
    /// assert_eq!(
    ///     p.canonical_root_of(&PathBuf::from("/a"), "something.bar").unwrap(),
    ///     PathBuf::from("/a/something.bar-wt")
    /// );
    /// assert_eq!(p.canonical_root_of(&PathBuf::from("/"), "foo").unwrap(), PathBuf::from("/foo-wt"));
    /// ```
    pub fn canonical_root_of(&self, parent: &std::path::Path, name: &str) -> Result<PathBuf> {
        self.check_suffix("(policy)")?;
        match self.root.strategy {
            RootStrategy::Sibling => Ok(parent.join(format!("{name}{}", self.root.suffix))),
        }
    }

    /// Refuse a suffix that cannot separate the container from the checkout. Named with the
    /// policy file so the message says where to edit.
    pub fn check_suffix(&self, path: &str) -> Result<()> {
        let s = &self.root.suffix;
        let reason = if s.is_empty() {
            Some("empty, so the container would be the checkout itself")
        } else if s.contains('/') || s.contains('\\') {
            Some("a path fragment, so the container would not be a sibling")
        } else if s.contains('\0') {
            Some("carrying a NUL byte")
        } else if s.contains('"') || s.contains('\'') {
            // The layer's YAML subset does not strip quotes from a quoted scalar that is
            // followed by a trailing comment — the shell reader and this one agree on that,
            // so it is the subset's shape and not a divergence. The symptom is a suffix that
            // still carries its quotes, and the consequence would be a container directory
            // named `repo"-wt"`. Refusing here turns a silently wrong path into a message
            // that names the line to edit.
            Some(
                "carrying a quote character, which usually means the value was written as a                  quoted scalar with a trailing comment on the same line; put the comment on                  its own line above the key",
            )
        } else {
            None
        };
        match reason {
            Some(reason) => Err(WorktreeError::InvalidRootSuffix {
                reason: reason.into(),
                path: path.to_string(),
            }),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::yaml;

    #[test]
    fn the_default_policy_is_the_doctrine() {
        let p = WorktreePolicy::default();
        assert_eq!(p.root.suffix, "-wt");
        assert_eq!(p.root.strategy, RootStrategy::Sibling);
        assert_eq!(p.enforcement.outside_root, Level::Error);
        assert_eq!(p.cleanup.dirty, Disposition::Refuse);
        assert_eq!(p.cleanup.locked, Disposition::Refuse);
    }

    #[test]
    fn the_suffix_is_the_only_thing_that_has_to_change_to_move_every_container() {
        let p: WorktreePolicy =
            yaml::parse_into("root:\n  strategy: sibling\n  suffix: \"-trees\"\n").unwrap();
        assert_eq!(
            p.canonical_root_of(std::path::Path::new("/a"), "foo")
                .unwrap(),
            PathBuf::from("/a/foo-trees")
        );
    }

    #[test]
    fn an_unusable_suffix_is_refused_where_it_is_written_and_not_where_it_is_used() {
        let p: WorktreePolicy = yaml::parse_into("root:\n  suffix: \"\"\n").unwrap();
        let e = p
            .canonical_root_of(std::path::Path::new("/a"), "foo")
            .unwrap_err();
        assert_eq!(e.code(), "InvalidRootSuffix");
        let p: WorktreePolicy = yaml::parse_into("root:\n  suffix: \"/wt\"\n").unwrap();
        assert_eq!(
            p.canonical_root_of(std::path::Path::new("/a"), "foo")
                .unwrap_err()
                .code(),
            "InvalidRootSuffix"
        );
    }

    #[test]
    fn a_suffix_that_kept_its_quotes_is_refused_rather_than_used_as_a_directory_name() {
        // exactly what `suffix: "-wt"   # comment` yields through the layer's reader
        let p: WorktreePolicy = yaml::parse_into("root:\n  suffix: '\"-wt\"'\n").unwrap();
        let e = p
            .canonical_root_of(std::path::Path::new("/a"), "foo")
            .unwrap_err();
        assert_eq!(e.code(), "InvalidRootSuffix");
        assert!(
            format!("{e}").contains("quoted scalar"),
            "the message names the cause: {e}"
        );
    }

    #[test]
    fn an_unknown_key_in_the_section_is_an_error_like_every_other_unknown_key() {
        let r: std::result::Result<WorktreePolicy, String> =
            yaml::parse_into("root:\n  suffix: \"-wt\"\n  colour: red\n");
        assert!(r.unwrap_err().contains("colour"));
    }

    #[test]
    fn an_absent_section_still_carries_the_doctrine() {
        let p: WorktreePolicy = yaml::parse_into("").unwrap_or_default();
        assert_eq!(p.root.suffix, "-wt");
    }
}

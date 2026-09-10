//! One directory name from one branch or label, derived the same way every time.
//!
//! A branch name is a path with slashes in it and very nearly no other rules; a directory
//! under the container is one component with the filesystem's rules. The mapping between
//! them is written once, here, because a second one would eventually disagree about where a
//! worktree is — and "where is it" is the whole point of the container.
//!
//! The mapping is deliberately lossy: `feature/foo` and `feature-foo` both name the
//! directory `feature-foo`. That is not a bug to be hidden with a hash, which would make
//! every directory unreadable to prevent a collision that almost never happens. It is a
//! collision to be *detected*: creating the second one finds the first registered at that
//! path and refuses by name, saying which branch is already there.

use super::error::{Result, WorktreeError};

/// A validated worktree directory name: exactly one filesystem component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorktreeName(String);

impl WorktreeName {
    /// Derive a name from any label — a branch, a description, something a person typed —
    /// and refuse what cannot become one readable component.
    ///
    /// Every character outside `A-Za-z0-9._-` becomes `-`; runs of `-` collapse; leading and
    /// trailing `-`, `.` and `_` are trimmed, so nothing produces a hidden directory, `.` or
    /// `..`. What is left of `..` or of a label of pure punctuation is nothing, and nothing
    /// is refused rather than invented.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreeName;
    /// assert_eq!(WorktreeName::derive("feature/foo").unwrap().as_str(), "feature-foo");
    /// assert_eq!(WorktreeName::derive("bugfix/bar/baz").unwrap().as_str(), "bugfix-bar-baz");
    /// assert_eq!(WorktreeName::derive("issue/123/foo").unwrap().as_str(), "issue-123-foo");
    /// assert_eq!(WorktreeName::derive("user@example/test").unwrap().as_str(), "user-example-test");
    /// assert_eq!(WorktreeName::derive("  spaced  out  ").unwrap().as_str(), "spaced-out");
    /// assert_eq!(WorktreeName::derive("issue-123-worktrees").unwrap().as_str(), "issue-123-worktrees");
    /// assert!(WorktreeName::derive("..").is_err());
    /// assert!(WorktreeName::derive("///").is_err());
    /// ```
    pub fn derive(label: &str) -> Result<Self> {
        let mut s = String::with_capacity(label.len());
        for ch in label.chars() {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
                s.push(ch);
            } else {
                // Anything else — a slash, a space, a colon, an emoji, a combining mark —
                // is one separator. Transliterating would need a table and would still be
                // wrong for most of Unicode; a separator is honest and reversible by hand.
                s.push('-');
            }
        }
        let mut collapsed = String::with_capacity(s.len());
        let mut last_dash = false;
        for ch in s.chars() {
            if ch == '-' {
                if !last_dash {
                    collapsed.push(ch);
                }
                last_dash = true;
            } else {
                collapsed.push(ch);
                last_dash = false;
            }
        }
        let trimmed = collapsed.trim_matches(|c| c == '-' || c == '.' || c == '_');
        Self::validate(trimmed, label)
    }

    /// Accept a name exactly as given, refusing anything that is not one plain component.
    /// Used where the caller means a literal directory and a silent rewrite would surprise —
    /// a selector, a migration target read back from the topology.
    pub fn exact(name: &str) -> Result<Self> {
        Self::validate(name, name)
    }

    fn validate(candidate: &str, given: &str) -> Result<Self> {
        let reason = if candidate.is_empty() {
            Some("it has no characters a directory name can keep".to_string())
        } else if candidate == "." || candidate == ".." {
            Some("it is a relative path, not a name".to_string())
        } else if candidate.contains('/') || candidate.contains('\\') {
            Some("it contains a path separator".to_string())
        } else if candidate.contains('\0') {
            Some("it contains a NUL byte".to_string())
        } else if candidate.starts_with('.') {
            Some("it would create a hidden directory".to_string())
        } else if candidate.len() > 255 {
            Some(format!(
                "it is {} bytes long, and a directory name is at most 255",
                candidate.len()
            ))
        } else {
            None
        };
        match reason {
            Some(reason) => Err(WorktreeError::InvalidWorktreeName {
                given: given.to_string(),
                reason,
            }),
            None => Ok(WorktreeName(candidate.to_string())),
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorktreeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The branch a worktree gets when the caller named the worktree and not the branch: the
/// label as given, which is already a valid branch name in the overwhelming majority of
/// cases and is checked against git before use.
///
/// Nothing invents an issue number, a prefix or a namespace here. A repository that wants
/// `issue/123-x` passes `--branch issue/123-x`; `--issue 123` builds that form from the
/// issue record and from nothing else.
pub fn default_branch_for(label: &str) -> String {
    label.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deriving_is_deterministic() {
        for label in ["feature/foo", "a b c", "ÜNICODE/žluť", "x"] {
            let a = WorktreeName::derive(label).map(|n| n.0.clone());
            let b = WorktreeName::derive(label).map(|n| n.0.clone());
            assert_eq!(a, b);
        }
    }

    #[test]
    fn unicode_becomes_separators_rather_than_a_transliteration_table() {
        assert_eq!(
            WorktreeName::derive("žluťoučký").unwrap().as_str(),
            "lu-ou-k"
        );
        assert_eq!(WorktreeName::derive("a→b").unwrap().as_str(), "a-b");
    }

    #[test]
    fn the_lossy_cases_are_equal_on_purpose_and_the_service_detects_them() {
        assert_eq!(
            WorktreeName::derive("feature/foo").unwrap(),
            WorktreeName::derive("feature-foo").unwrap()
        );
    }

    #[test]
    fn nothing_derives_to_a_hidden_directory_or_a_traversal() {
        assert!(WorktreeName::derive(".").is_err());
        assert!(WorktreeName::derive("..").is_err());
        assert!(WorktreeName::derive("../../etc").is_err_or_component());
        assert!(WorktreeName::derive(".hidden").unwrap().as_str() == "hidden");
        assert!(WorktreeName::derive("/").is_err());
    }

    #[test]
    fn exact_refuses_what_derive_would_have_rewritten() {
        assert!(WorktreeName::exact("feature/foo").is_err());
        assert_eq!(
            WorktreeName::exact("feature-foo").unwrap().as_str(),
            "feature-foo"
        );
    }

    #[test]
    fn an_overlong_name_is_refused_rather_than_truncated() {
        let long = "a".repeat(300);
        let e = WorktreeName::derive(&long).unwrap_err();
        assert_eq!(e.code(), "InvalidWorktreeName");
    }

    trait ErrOrComponent {
        fn is_err_or_component(&self) -> bool;
    }
    impl ErrOrComponent for Result<WorktreeName> {
        /// `../../etc` derives to `etc`: lossy, and still exactly one component, which is
        /// the invariant that matters. Either outcome is safe; escaping is not possible.
        fn is_err_or_component(&self) -> bool {
            match self {
                Err(_) => true,
                Ok(n) => !n.as_str().contains('/') && n.as_str() != ".." && n.as_str() != ".",
            }
        }
    }
}

//! The semantics a command declaration cannot carry, declared once beside it.
//!
//! clap knows a command's path, its help, its arguments, their defaults and the values an
//! enumerated argument accepts. It does not know — and no spelling can be trusted to
//! reveal — what a command *does*: whether it writes, whether it needs a terminal, what
//! must exist before it can run, and what it used to be called. Those facts are declared
//! in [`crate::cli::SEMANTICS`], in the same file as the clap declaration and beside the
//! examples that are there for the same reason, and `crate::cli::validate` refuses a
//! runnable command that has no entry.
//!
//! This is semantic definition, not registration: it is written once, in the file the
//! command is declared in, and every surface derives from it. Nothing anywhere else
//! restates an effect, a requirement or an alias.

use super::model::{Alias, Deprecation, EffectClass, Interactivity, Requirement, Visibility};

/// What one command of the native command line means. Keyed by the command's path
/// without the executable's own name, the same key [`crate::cli::CommandExamples`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSemantics {
    /// `bench baseline update`; the empty string is the root.
    pub command: &'static str,
    /// What it does to the world.
    pub effect: EffectClass,
    /// Whether it needs a terminal.
    pub interactivity: Interactivity,
    /// Who may see it.
    pub visibility: Visibility,
    /// What must exist before it can run here.
    pub requires: &'static [Requirement],
    /// Free tags. `entry` marks a command the repository's entry banner may recommend;
    /// `diagnostic` marks one that reports on the checkout's own health.
    pub tags: &'static [&'static str],
    /// Names it also answers to, each with the reason it exists. A projection that can
    /// carry an alias carries these and invents none.
    pub aliases: &'static [(&'static str, &'static str)],
    /// What to say when it is on the way out: the replacement's path and one line.
    pub deprecated: Option<(&'static str, &'static str)>,
}

impl CommandSemantics {
    /// The aliases as canonical values.
    pub fn alias_values(&self) -> Vec<Alias> {
        self.aliases
            .iter()
            .map(|(name, reason)| Alias {
                name: (*name).to_string(),
                reason: (*reason).to_string(),
            })
            .collect()
    }

    /// The deprecation as a canonical value, with the replacement resolved by the caller.
    pub fn deprecation_note(&self) -> Option<Deprecation> {
        self.deprecated.map(|(_, note)| Deprecation {
            replaced_by: None,
            note: note.to_string(),
        })
    }

    /// The path of the command that replaces this one, when it is deprecated.
    pub fn replacement_path(&self) -> Option<&'static str> {
        self.deprecated.map(|(path, _)| path)
    }

    /// The requirements, never empty: a declaration that names none needs nothing.
    pub fn requirements(&self) -> Vec<Requirement> {
        if self.requires.is_empty() {
            vec![Requirement::Always]
        } else {
            self.requires.to_vec()
        }
    }
}

/// The semantics of one command path, from the declaration.
pub fn lookup(command: &str) -> Option<&'static CommandSemantics> {
    crate::cli::SEMANTICS.iter().find(|s| s.command == command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declaration_with_no_requirement_needs_nothing() {
        let s = CommandSemantics {
            command: "x",
            effect: EffectClass::ReadOnly,
            interactivity: Interactivity::NonInteractive,
            visibility: Visibility::Public,
            requires: &[],
            tags: &[],
            aliases: &[],
            deprecated: None,
        };
        assert_eq!(s.requirements(), vec![Requirement::Always]);
        assert!(s.alias_values().is_empty());
        assert!(s.deprecation_note().is_none());
    }

    #[test]
    fn an_alias_carries_the_reason_it_exists() {
        let s = CommandSemantics {
            command: "x",
            effect: EffectClass::ReadOnly,
            interactivity: Interactivity::NonInteractive,
            visibility: Visibility::Public,
            requires: &[],
            aliases: &[("old", "the name this command had before it was nested")],
            tags: &[],
            deprecated: None,
        };
        let aliases = s.alias_values();
        assert_eq!(aliases[0].name, "old");
        assert!(aliases[0].reason.contains("before"));
    }
}

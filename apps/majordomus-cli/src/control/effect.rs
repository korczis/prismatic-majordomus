//! What running a command does, and what that alone decides.
//!
//! The classification is the one semantic fact about a command that cannot be inferred
//! from its spelling: `worktree remove` and `worktree list` are the same shape to clap and
//! nothing else. It is declared once, beside the command's own declaration, and every
//! surface reads the consequences from here rather than deciding them again — which
//! projection may offer the command, whether a caller must confirm it, and whether a
//! machine may invoke it at all.
//!
//! ```
//! use majordomus_cli::control::effect::{EffectClass, Interactivity};
//! // a read is offered everywhere; a removal is not offered to a machine surface
//! assert!(EffectClass::ReadOnly.machine_callable());
//! assert!(!EffectClass::Destructive.machine_callable());
//! assert!(EffectClass::Destructive.needs_confirmation());
//! // a server is not a call that returns, wherever it is classified
//! assert!(!Interactivity::LongRunning.machine_callable());
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a command changes. The order is the order of increasing consequence, and it is
/// the order the variants are declared in, so a comparison reads as one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    /// Reads and reports. Nothing outside the process is different afterwards.
    ReadOnly,
    /// Changes this machine outside the repository's tracked tree: a build output, a
    /// process, a cache, a runtime file under an ignored path.
    LocalMutation,
    /// Writes tracked files of the repository, or moves its git state.
    RepositoryMutation,
    /// Removes work that the repository cannot give back.
    Destructive,
}

impl EffectClass {
    /// May a machine surface — an MCP tool, an HTTP endpoint, a Cockpit action — invoke
    /// this at all? A read may; anything that changes the repository or destroys work is
    /// offered to a person on the command line and described, never executed, elsewhere.
    ///
    /// This is the whole of the exposure policy. A surface that wants an exception is
    /// asking for a second policy, and the answer is to classify the command correctly.
    pub fn machine_callable(self) -> bool {
        matches!(self, EffectClass::ReadOnly | EffectClass::LocalMutation)
    }

    /// Must a caller say yes before it runs?
    pub fn needs_confirmation(self) -> bool {
        matches!(
            self,
            EffectClass::RepositoryMutation | EffectClass::Destructive
        )
    }

    /// The word a surface shows.
    pub fn label(self) -> &'static str {
        match self {
            EffectClass::ReadOnly => "read-only",
            EffectClass::LocalMutation => "local",
            EffectClass::RepositoryMutation => "repository",
            EffectClass::Destructive => "destructive",
        }
    }

    /// One line: what a reader is being told by the classification.
    pub fn describe(self) -> &'static str {
        match self {
            EffectClass::ReadOnly => "reads and reports; changes nothing",
            EffectClass::LocalMutation => "changes this machine outside the tracked tree",
            EffectClass::RepositoryMutation => "writes tracked files or moves git state",
            EffectClass::Destructive => "removes work the repository cannot give back",
        }
    }
}

/// How a command occupies the caller. A command that never returns on its own cannot be a
/// request/response call whatever it changes, so this narrows exposure independently of
/// the effect class.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Interactivity {
    /// Starts, answers, exits.
    Batch,
    /// Runs until it is stopped: a server, a watcher.
    LongRunning,
    /// Needs a terminal: it asks, or it renders for one.
    Interactive,
}

impl Interactivity {
    /// May a machine surface invoke this? Only something that ends by itself.
    pub fn machine_callable(self) -> bool {
        matches!(self, Interactivity::Batch)
    }

    /// The word a surface shows.
    pub fn label(self) -> &'static str {
        match self {
            Interactivity::Batch => "batch",
            Interactivity::LongRunning => "long-running",
            Interactivity::Interactive => "interactive",
        }
    }
}

/// The semantics a command declares beside its own declaration: what it changes, and how
/// it occupies the caller. Everything else about a command is derived.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Semantics {
    /// What running it changes.
    pub effect: EffectClass,
    /// How it occupies the caller.
    pub interactivity: Interactivity,
}

impl Semantics {
    /// A read that returns: the common case, and the default nothing has to write out.
    pub const fn read_only() -> Self {
        Semantics {
            effect: EffectClass::ReadOnly,
            interactivity: Interactivity::Batch,
        }
    }

    /// A command with this effect that returns.
    pub const fn of(effect: EffectClass) -> Self {
        Semantics {
            effect,
            interactivity: Interactivity::Batch,
        }
    }

    /// The same semantics, running until it is stopped.
    pub const fn long_running(mut self) -> Self {
        self.interactivity = Interactivity::LongRunning;
        self
    }

    /// May a machine surface invoke this? Both halves must allow it.
    ///
    /// ```
    /// use majordomus_cli::control::effect::{EffectClass, Semantics};
    /// assert!(Semantics::read_only().machine_callable());
    /// assert!(!Semantics::read_only().long_running().machine_callable());
    /// assert!(!Semantics::of(EffectClass::Destructive).machine_callable());
    /// ```
    pub fn machine_callable(self) -> bool {
        self.effect.machine_callable() && self.interactivity.machine_callable()
    }

    /// Why a machine surface may not invoke it; `None` when it may.
    pub fn refusal(self) -> Option<String> {
        if !self.interactivity.machine_callable() {
            return Some(format!(
                "it is {} and never returns on its own",
                self.interactivity.label()
            ));
        }
        if !self.effect.machine_callable() {
            return Some(format!("it is {}", self.effect.describe()));
        }
        None
    }
}

impl Default for Semantics {
    fn default() -> Self {
        Semantics::read_only()
    }
}

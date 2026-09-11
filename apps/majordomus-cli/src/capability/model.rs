//! The descriptor: everything a projection may say about a capability, and nothing else.
//!
//! # What this module owns
//!
//! [`Capability`] is the value the whole executable is a projection of. It carries identity
//! ([`CapabilityId`], [`ModuleId`]), what the thing is ([`CapabilityKind`]), the canonical
//! schemas of its input and output, where it came from ([`Provenance`]), where it is
//! projected ([`Exposure`]), where it means anything ([`Availability`]), who it is for
//! ([`Visibility`]), where it stands ([`Stability`]), and the two operational policies a
//! call is subject to ([`CachePolicy`], [`BenchmarkPolicy`]).
//!
//! # What it deliberately does not own
//!
//! There is no handler here and no transport type. A descriptor is plain data: it
//! serialises, it round-trips, and it can be read by something that cannot execute
//! anything — which is what lets the generated reference, the site dataset and the OpenAPI
//! document be built from the same value the executor dispatches on. Execution lives in
//! [`super::handler`]; the collection and its invariants live in [`super::registry`].
//!
//! # Declared against classified
//!
//! Two fields are *classified* rather than declared: [`Availability::classify`] and
//! [`Visibility::classify`] compute their values from what the declaration already carries
//! — the kind for the first, the transports for the second. Asking each declaration to
//! restate them would be the same knowledge written twice, and the second copy is the one
//! that goes wrong. Both matches are
//! exhaustive with no fallback arm on purpose, so a new kind or a new transport is a compile
//! error here rather than a silent default in a page that then links to nothing.
//!
//! # Invariants
//!
//! Nothing in this module enforces anything: a descriptor can be built wrong and the
//! registry is what refuses it. What lives here is the vocabulary each check is written
//! against — the grammar of an id, the shape of a route, the states a policy may be in —
//! each with its own validator returning the reason it failed rather than a boolean.
//!
//! ```
//! use majordomus_cli::capability::{
//!     Availability, CapabilityId, CapabilityKind, CliExposure, Exposure, Visibility,
//! };
//!
//! let id = CapabilityId::parse("repository.info").unwrap();
//! assert_eq!(id.namespace(), "repository");
//!
//! // a capability offered only on the command line is developer-facing, and needs a
//! // process to answer at all — neither of which its declaration had to say
//! let exposure = Exposure {
//!     cli: Some(CliExposure { path: vec!["scope".into()] }),
//!     ..Default::default()
//! };
//! assert_eq!(Visibility::classify(&exposure), Visibility::Developer);
//! assert_eq!(
//!     Availability::classify(CapabilityKind::Query, &exposure),
//!     Availability::Runtime
//! );
//! ```

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::schema::CanonicalSchema;

/// A stable, globally meaningful identity: a namespace, a dot, and a local part.
/// `repository.info` and `objects.get` for executables; `<kind>.<identity>` for a
/// declarative object (`rule.majordomus.scope-integrity@1`, `document.docs/CLI.md`,
/// `policy..ai/repo/policy.yaml`).
///
/// Grammar: the namespace matches `[a-z][a-z0-9_-]*`; the local part is non-empty and
/// carries no whitespace or control character, any other Unicode included, because it is
/// opaque: a path, a versioned identity, or a name, as the kind's identity rule produced it.
///
/// ```
/// use majordomus_cli::capability::CapabilityId;
/// // an executable's identity and a declarative object's identity are one grammar: only
/// // the local part tells them apart, and nothing here reads the local part
/// let executable = CapabilityId::parse("objects.get").unwrap();
/// let object = CapabilityId::parse("document.docs/CLI.md").unwrap();
/// assert_eq!(executable.namespace(), "objects");
/// assert_eq!(object.namespace(), "document");
/// assert_eq!(object.as_str(), "document.docs/CLI.md", "the local part is carried, not parsed");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Read an identity, or answer with the reason the text is not one.
    ///
    /// The namespace is held to the grammar; the local part is only required to be
    /// non-empty and free of whitespace and control characters. That asymmetry is the
    /// point: the local part belongs to whichever kind produced it — a repository-relative
    /// path, a versioned rule identity, a name — and a validator that understood it here
    /// would have to be taught every kind the repository ever grows.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityId;
    /// let id = CapabilityId::parse("rule.majordomus.scope-integrity@1").unwrap();
    /// assert_eq!(id.namespace(), "rule");
    /// assert!(CapabilityId::parse("Repository.info").is_err(), "namespace is lowercase");
    /// assert!(CapabilityId::parse("repository").is_err(), "a dot is required");
    /// assert!(CapabilityId::parse("document.docs/Příručka.md").is_ok(), "the local part is opaque");
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Err("empty".into());
        }
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("carries whitespace or a control character".into());
        }
        let Some((namespace, local)) = text.split_once('.') else {
            return Err("needs a namespace, a dot, and a local part".into());
        };
        let mut chars = namespace.chars();
        let ok_namespace = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !ok_namespace {
            return Err(format!("namespace '{namespace}' is not [a-z][a-z0-9_-]*"));
        }
        if local.is_empty() {
            return Err("the local part after the dot is empty".into());
        }
        Ok(CapabilityId(text.to_string()))
    }

    /// An identity taken on trust, for a descriptor written in Rust.
    ///
    /// The `capability!` macro expands to a value, not to a `Result`, so a builtin's id is
    /// not checked where it is written. [`super::registry::Builder::build`] checks every id
    /// it is handed and refuses the whole registry with the offending id and its
    /// provenance named, which turns a mistyped literal into one legible startup failure
    /// instead of a panic from inside a macro expansion.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityId;
    /// // it accepts what `parse` refuses; the registry is what says no, and later
    /// let taken_on_trust = CapabilityId::unchecked("Repository.info");
    /// assert_eq!(taken_on_trust.as_str(), "Repository.info");
    /// assert!(CapabilityId::parse(taken_on_trust.as_str()).is_err(), "the grammar still refuses it");
    /// ```
    pub fn unchecked(text: &str) -> Self {
        CapabilityId(text.to_string())
    }

    /// The identity as text: what every projection prints, and what a resource URI, an
    /// HTTP route's operation id and a generated reference entry are all derived from.
    /// Borrowed from the identity, so printing one allocates nothing.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The namespace: everything before the first dot.
    ///
    /// This is the module a builtin belongs to and the kind of a declarative object, which
    /// is what lets the registry check that a capability was composed by the module it
    /// claims without being told the module a second time. The *first* dot decides, so a
    /// local part with dots of its own keeps every one of them.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityId;
    /// let rule = CapabilityId::parse("rule.majordomus.scope-integrity@1").unwrap();
    /// assert_eq!(rule.namespace(), "rule", "the first dot ends the namespace");
    /// ```
    pub fn namespace(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A module identity: the namespace of every capability the module composes, matching
/// `[a-z][a-z0-9_-]*`. Builtin modules declare theirs in `module!`; a declarative
/// object's module is its kind.
///
/// ```
/// use majordomus_cli::capability::{CapabilityId, ModuleId};
/// let module = ModuleId::parse("objects").unwrap();
/// let id = CapabilityId::parse("objects.get").unwrap();
/// assert_eq!(id.namespace(), module.as_str(), "a capability's namespace is its module");
/// assert!(ModuleId::parse("objects.get").is_err(), "a module identity carries no dot");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    /// Read a module identity, or answer with the reason the text is not one.
    ///
    /// The grammar is the namespace grammar of [`CapabilityId`] and nothing more, which is
    /// what makes "the namespace of every capability this module composes" a checkable
    /// statement rather than a convention. The empty string fails here, because a module
    /// with no name would stamp an empty namespace onto everything it carries.
    ///
    /// ```
    /// use majordomus_cli::capability::ModuleId;
    /// assert!(ModuleId::parse("repository").is_ok());
    /// assert!(ModuleId::parse("Repository").is_err());
    /// assert!(ModuleId::parse("").is_err());
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chars = text.chars();
        let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if ok {
            Ok(ModuleId(text.to_string()))
        } else {
            Err(format!("module id '{text}' is not [a-z][a-z0-9_-]*"))
        }
    }

    /// A module identity taken on trust, for a module declared in Rust.
    ///
    /// [`crate::module!`] stamps this onto every executable it composes before anything is
    /// validated, and the registry builder is where a malformed one is refused. The empty
    /// identity that `capability!` leaves behind until a module claims the capability is
    /// written this way too: an unclaimed capability is a value that exists and fails
    /// validation, rather than one that cannot be constructed at all.
    ///
    /// ```
    /// use majordomus_cli::capability::ModuleId;
    /// let stamped = ModuleId::unchecked("objects");
    /// assert_eq!(stamped, ModuleId::parse("objects").unwrap(), "the same value, unvalidated");
    /// assert!(ModuleId::parse(ModuleId::unchecked("Objects").as_str()).is_err());
    /// ```
    pub fn unchecked(text: &str) -> Self {
        ModuleId(text.to_string())
    }

    /// The module identity as text: the namespace every capability of the module carries,
    /// and the segment the generated reference files them all under.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an executable capability is not benchmarked. Typed, so that a waiver is a
/// reviewable statement and never a convenience; `not_executable` is the registry's own
/// reason for resources and is never written by hand.
///
/// ```
/// use majordomus_cli::capability::{BenchmarkPolicy, WaiverReason};
/// // the reason travels with the policy and reads back as itself, so a coverage report
/// // that says "waived" can always say what it was waived for
/// let waived = BenchmarkPolicy::Waived { reason: WaiverReason::Destructive };
/// let json = serde_json::to_string(&waived).unwrap();
/// assert_eq!(json, r#"{"policy":"waived","reason":"destructive"}"#);
/// assert_eq!(serde_json::from_str::<BenchmarkPolicy>(&json).unwrap(), waived);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WaiverReason {
    /// A resource: read, never executed; nothing to time but `objects.get`, which is.
    NotExecutable,
    /// The capability changes something outside this process and cannot be run in a loop.
    Destructive,
    /// The capability talks to something the benchmark host cannot provide.
    ExternalDependency,
    /// The capability starts, or answers about, work that exists only while it is running.
    /// A benchmark host cannot stage an execution to read, and running the operation in a
    /// loop would measure the work rather than the operation.
    TransientState,
}

/// Whether the capability is a benchmark target. `Required` is the default and the norm:
/// every executable capability is timed directly and through every transport it is
/// exposed on, with the cases its input type provides.
///
/// There is no way to say "not benchmarked" without saying why: the only alternative to
/// `Required` carries a [`WaiverReason`], so a gap in the timings is always a statement
/// somebody wrote and a reviewer can argue with.
///
/// ```
/// use majordomus_cli::capability::{BenchmarkPolicy, WaiverReason};
/// let required = BenchmarkPolicy::Required;
/// assert_eq!(serde_json::to_string(&required).unwrap(), r#"{"policy":"required"}"#);
/// let waived = BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency };
/// assert_ne!(waived, required, "a waiver is never counted as coverage");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum BenchmarkPolicy {
    /// Timed directly and through every exposure; coverage fails without a case.
    Required,
    /// Not timed, for the typed reason; coverage reports it as waived, never as covered.
    Waived {
        /// Why.
        reason: WaiverReason,
    },
}

/// Whether, and how, the executor keeps results of this capability. Cache lives in the
/// executor and nowhere else, so MCP, HTTP and the command line share one; the key is the
/// canonical id, the normalised input and the registry fingerprint, so a changed
/// repository never answers from an old entry.
///
/// ```
/// use majordomus_cli::capability::CachePolicy;
/// // the bound is part of the declaration, not a constant hidden in the executor
/// let policy = CachePolicy::Process { max_entries: 64, ttl_seconds: Some(30) };
/// policy.validate().unwrap();
/// assert!(policy.is_enabled());
/// assert!(!CachePolicy::Disabled.is_enabled(), "the default keeps nothing");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum CachePolicy {
    /// Every call runs the handler.
    Disabled,
    /// Results are kept in this process's memory, bounded, for equal inputs.
    Process {
        /// The most entries kept for this capability; the oldest is evicted first.
        max_entries: usize,
        /// Seconds an entry stays valid; `None` for the life of the process.
        #[serde(skip_serializing_if = "Option::is_none")]
        ttl_seconds: Option<u64>,
    },
}

impl CachePolicy {
    /// Is the policy well-formed? The reason when it is not.
    ///
    /// ```
    /// use majordomus_cli::capability::CachePolicy;
    /// assert!(CachePolicy::Disabled.validate().is_ok());
    /// assert!(CachePolicy::Process { max_entries: 0, ttl_seconds: None }.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        match self {
            CachePolicy::Disabled => Ok(()),
            CachePolicy::Process { max_entries: 0, .. } => {
                Err("a process cache with max_entries 0 keeps nothing".into())
            }
            CachePolicy::Process {
                ttl_seconds: Some(0),
                ..
            } => Err("a process cache with ttl_seconds 0 keeps nothing".into()),
            CachePolicy::Process { .. } => Ok(()),
        }
    }

    /// Does this policy keep anything at all?
    ///
    /// The executor asks before it builds a key. `Disabled` is not a cache of size zero:
    /// it is a call that always reaches the handler, and holding the two apart is what
    /// keeps a capability whose answer must be current out of a cache shared by every
    /// transport of the process.
    ///
    /// ```
    /// use majordomus_cli::capability::CachePolicy;
    /// assert!(!CachePolicy::Disabled.is_enabled());
    /// assert!(CachePolicy::Process { max_entries: 8, ttl_seconds: None }.is_enabled());
    /// ```
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CachePolicy::Disabled)
    }
}

/// What a capability is. Three kinds exist because three semantics exist: something that
/// is executed and changes nothing, something that is executed and changes this process's
/// own memory, and something that is read. Nothing of any kind writes to the repository.
///
/// How *long* a call takes is not a kind. A read that walks every file of the layer is
/// still a read, and the thing that makes it worth watching — that it reports as it goes
/// and stops when it is asked to — is one property of its handler, declared with
/// [`crate::capability::Executable::cancellable`] and carried on [`ExecutionPolicy`].
///
/// ```
/// use majordomus_cli::capability::CapabilityKind;
/// // the kind answers two independent questions, and every projection reads the answers
/// // rather than matching on the kind for itself
/// let kinds = [CapabilityKind::Query, CapabilityKind::Command, CapabilityKind::Resource];
/// assert_eq!(kinds.iter().filter(|k| k.is_executable()).count(), 2, "only a resource is read");
/// assert_eq!(kinds.iter().filter(|k| k.is_read_only()).count(), 2, "only a command writes");
/// assert_eq!(kinds.map(|k| k.as_str()), ["query", "command", "resource"]);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    /// Executable and read-only: a typed handler, an input schema, an output schema.
    Query,
    /// Executable with an effect on this process's in-memory state and nowhere else (a
    /// peer announcing itself): a typed handler, bound to `POST` over HTTP, and announced
    /// to MCP clients as not read-only.
    Command,
    /// Declarative content the repository holds: read as it is, never executed.
    Resource,
}

impl CapabilityKind {
    /// Is a capability of this kind called, with a handler, rather than read?
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityKind;
    /// assert!(CapabilityKind::Query.is_executable() && CapabilityKind::Command.is_executable());
    /// assert!(!CapabilityKind::Resource.is_executable());
    /// ```
    pub fn is_executable(self) -> bool {
        !matches!(self, CapabilityKind::Resource)
    }

    /// Does a call of this kind leave the process as it found it?
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityKind;
    /// assert!(CapabilityKind::Query.is_read_only());
    /// assert!(!CapabilityKind::Command.is_read_only());
    /// ```
    pub fn is_read_only(self) -> bool {
        !matches!(self, CapabilityKind::Command)
    }

    /// The kind as the single word every projection serialises and renders.
    ///
    /// The same string the enum's serde representation produces, deliberately: an OpenAPI
    /// enum, a site dataset and a table in the generated reference would otherwise be
    /// three vocabularies for one field, and a reader comparing two of them would conclude
    /// the surfaces disagree about the capability.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityKind;
    /// let kind = CapabilityKind::Command;
    /// assert_eq!(kind.as_str(), "command");
    /// assert_eq!(serde_json::to_string(&kind).unwrap(), "\"command\"", "one vocabulary");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityKind::Query => "query",
            CapabilityKind::Command => "command",
            CapabilityKind::Resource => "resource",
        }
    }
}

/// What running a capability changes outside the caller.
///
/// Classified, never declared: it follows from the kind, which is the field a declaration
/// already carries. A projection reads this to decide whether to ask before running
/// something — the Cockpit's confirmation is derived from it — instead of naming
/// capabilities it must treat carefully, which is a list that goes stale the day after it
/// is written.
///
/// ```
/// use majordomus_cli::capability::{CapabilityKind, Effect, ExecutionPolicy};
/// // nothing in this executable classifies to `RepositoryMutation`, and that is the
/// // doctrine rather than an omission: every kind there is reads, or touches this
/// // process and nothing else
/// for kind in [CapabilityKind::Query, CapabilityKind::Command, CapabilityKind::Resource] {
///     let effect = ExecutionPolicy::classify(kind).effect;
///     assert_ne!(effect, Effect::RepositoryMutation, "{kind:?} must not write to the repository");
/// }
/// assert_eq!(ExecutionPolicy::classify(CapabilityKind::Command).effect, Effect::ProcessState);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
// the component namespace of the OpenAPI document is flat, and `deploy` already has a
// `Concurrency`; each says which it is
#[schemars(rename = "ExecutionEffect")]
pub enum Effect {
    /// Nothing changes. Every query and every resource of this executable.
    Read,
    /// This process's own memory changes, and nothing outside it.
    ProcessState,
    /// The repository changes.
    ///
    /// Nothing classifies to this, and the doctrine of this tool is why: no capability of
    /// any kind writes to the repository. It is on the model so that the day one does, it
    /// says so here — where a projection already reads it and a client already asks before
    /// running it — rather than in whichever page happens to render its button.
    RepositoryMutation,
}

/// Whether two executions of one capability may overlap.
///
/// ```
/// use majordomus_cli::capability::{CapabilityKind, Concurrency, ExecutionPolicy};
/// // a read may overlap any number of other reads, because the index and the registry
/// // cannot change under them; anything that touches this process's memory takes turns
/// assert_eq!(
///     ExecutionPolicy::classify(CapabilityKind::Query).concurrency,
///     Concurrency::Unrestricted
/// );
/// assert_eq!(
///     ExecutionPolicy::classify(CapabilityKind::Command).concurrency,
///     Concurrency::Serial
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ExecutionConcurrency")]
pub enum Concurrency {
    /// Any number at once. Every read is one of these: the index and the registry are
    /// immutable for the life of the process, so concurrent readers cannot interfere.
    Unrestricted,
    /// One at a time. A second execution of the same capability waits for the first, which
    /// is what a capability that changes anything — this process's own memory included —
    /// needs in order to be reasoned about at all.
    Serial,
}

/// What running a capability as an execution means: what it changes, whether asking it to
/// stop achieves anything, and whether two of them may overlap.
///
/// The effect and the concurrency are classified from the kind by
/// [`ExecutionPolicy::classify`], for the same reason [`Availability`] is: the facts are
/// already on the declaration, and asking each `capability!` block to restate them would be
/// one more thing that can disagree with itself.
///
/// Cancellability is the one thing the kind cannot decide, because it is a fact about the
/// handler: whether it looks at its cancellation flag and stops. A handler that does says
/// so with [`crate::capability::Executable::cancellable`], and a client is then told
/// whether a Cancel button will achieve anything instead of being given one that lies.
///
/// ```
/// use majordomus_cli::capability::{CapabilityKind, ExecutionPolicy};
/// // the whole policy comes from the kind, except cancellability, which the handler
/// // claims for itself — and claiming it changes nothing else
/// let derived = ExecutionPolicy::classify(CapabilityKind::Query);
/// let claimed = derived.stoppable();
/// assert!(!derived.cancellable && claimed.cancellable);
/// assert_eq!(derived.effect, claimed.effect);
/// assert_eq!(derived.concurrency, claimed.concurrency);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionPolicy {
    /// What it changes.
    pub effect: Effect,
    /// Whether asking it to stop does anything. A task looks at its cancellation flag; a
    /// query and a command do not, and a client is told so rather than being given a
    /// button that lies.
    pub cancellable: bool,
    /// Whether two of them may overlap.
    pub concurrency: Concurrency,
}

impl ExecutionPolicy {
    /// The execution policy a kind implies.
    ///
    /// Two of the three fields are decided here and never restated in a declaration: what
    /// a call of this kind changes, and whether two of them may overlap. Cancellability is
    /// not decided here, because it is a fact about a handler's own code rather than about
    /// what the call means, and a policy that guessed it would hand a client a Cancel
    /// button that does nothing.
    ///
    /// ```
    /// use majordomus_cli::capability::{CapabilityKind, Concurrency, Effect, ExecutionPolicy};
    /// let query = ExecutionPolicy::classify(CapabilityKind::Query);
    /// assert_eq!(query.effect, Effect::Read);
    /// assert_eq!(query.concurrency, Concurrency::Unrestricted);
    /// assert!(!query.cancellable, "until its handler says it looks at the flag");
    /// assert!(query.stoppable().cancellable);
    /// let command = ExecutionPolicy::classify(CapabilityKind::Command);
    /// assert_eq!(command.effect, Effect::ProcessState);
    /// assert_eq!(command.concurrency, Concurrency::Serial);
    /// ```
    pub fn classify(kind: CapabilityKind) -> Self {
        match kind {
            // an immutable index and an immutable registry: readers cannot interfere
            CapabilityKind::Query | CapabilityKind::Resource => ExecutionPolicy {
                effect: Effect::Read,
                cancellable: false,
                concurrency: Concurrency::Unrestricted,
            },
            // it changes this process's memory, so two of them are made to take turns
            CapabilityKind::Command => ExecutionPolicy {
                effect: Effect::ProcessState,
                cancellable: false,
                concurrency: Concurrency::Serial,
            },
        }
    }

    /// The same policy, for a handler that looks at its cancellation flag and stops.
    ///
    /// ```
    /// use majordomus_cli::capability::{CapabilityKind, ExecutionPolicy};
    /// let p = ExecutionPolicy::classify(CapabilityKind::Query).stoppable();
    /// assert!(p.cancellable);
    /// assert_eq!(p.effect, ExecutionPolicy::classify(CapabilityKind::Query).effect);
    /// ```
    pub fn stoppable(self) -> Self {
        ExecutionPolicy {
            cancellable: true,
            ..self
        }
    }

    /// Should a client ask before running this? True for anything that changes something.
    ///
    /// ```
    /// use majordomus_cli::capability::{CapabilityKind, ExecutionPolicy};
    /// assert!(!ExecutionPolicy::classify(CapabilityKind::Query).needs_confirmation());
    /// assert!(ExecutionPolicy::classify(CapabilityKind::Command).needs_confirmation());
    /// ```
    pub fn needs_confirmation(self) -> bool {
        !matches!(self.effect, Effect::Read)
    }
}

/// Where a capability stands, in the repository's own vocabulary for claims. A capability
/// that is `Planned` or `Unsupported` may be listed but is never executable through any
/// projection; the registry refuses to build otherwise.
///
/// ```
/// use majordomus_cli::capability::Stability;
/// // five points on the scale, and one question a projection actually asks of them
/// let listed_only = [Stability::Planned, Stability::Unsupported];
/// assert!(listed_only.iter().all(|s| !s.executable()));
/// assert!(Stability::BehaviorallyVerified.executable());
/// assert_eq!(
///     serde_json::to_string(&Stability::BehaviorallyVerified).unwrap(),
///     "\"behaviorally_verified\""
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// Implemented, and no behavioural test names it yet.
    Implemented,
    /// Implemented and proved by a behavioural test.
    BehaviorallyVerified,
    /// Implemented, executable, and expected to change.
    Experimental,
    /// Specified and not implemented: listed, never executable.
    Planned,
    /// Considered and refused: listed with the reason, never executable.
    Unsupported,
}

impl Stability {
    /// May a capability standing here be offered as something a caller can run?
    ///
    /// The registry asks this of every declared exposure and refuses to build when the
    /// answer is no, so a specified-but-unwritten capability cannot reach a client as a
    /// tool, a route or a subcommand that then fails when it is called. It is still listed
    /// — that is the point of having the two states at all.
    ///
    /// ```
    /// use majordomus_cli::capability::Stability;
    /// assert!(Stability::Implemented.executable());
    /// assert!(!Stability::Planned.executable(), "a specification is listed, never called");
    /// assert!(!Stability::Unsupported.executable(), "and so is a refusal, with its reason");
    /// ```
    pub fn executable(self) -> bool {
        matches!(
            self,
            Stability::Implemented | Stability::BehaviorallyVerified | Stability::Experimental
        )
    }
}

/// Where a capability came from. Never an absolute path.
///
/// Both variants exist to answer one question — which file a reader should open — and the
/// builtin's answer is computed from its Rust module path by [`Provenance::source_path`]
/// rather than written down beside it, because a path written down beside a declaration is
/// a path that survives the file being moved.
///
/// ```
/// use majordomus_cli::capability::Provenance;
/// let p = Provenance::Builtin { module: "majordomus_cli::capability::builtin::health".into() };
/// assert_eq!(p.source_path(), "apps/majordomus-cli/src/capability/builtin/health.rs");
/// assert_eq!(p.to_string(), "builtin majordomus_cli::capability::builtin::health");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase")]
#[schemars(rename = "CapabilityProvenance")]
pub enum Provenance {
    /// Written in Rust, in the named module of this executable.
    Builtin {
        /// The Rust module the descriptor was composed in.
        module: String,
    },
    /// Read from the repository's layer.
    Declarative {
        /// Repository-relative path.
        path: String,
        /// The directory the path sits in, repository-relative; `.` for the root.
        directory: String,
        /// The `sources.yaml` class that discovered the file.
        source_class: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// The manifest section the path falls under, when it falls under one.
        section: Option<String>,
        /// IANA media type of the object's content.
        media_type: String,
        /// For one member of a collection file, its key path in the file (`claims.3`).
        #[serde(skip_serializing_if = "Option::is_none")]
        member: Option<String>,
    },
}

/// Where this crate lives in the repository, so that a builtin descriptor's Rust module
/// path names a file a reader can open. A fact of the tree's layout; the site generator
/// refuses a path that does not exist.
pub const CRATE_DIR: &str = "apps/majordomus-cli";

impl Provenance {
    /// The repository-relative path of the source: a declarative object's file, or the
    /// Rust file a builtin descriptor was composed in, by the crate's layout
    /// (`majordomus_cli::capability::builtin::objects` is
    /// `apps/majordomus-cli/src/capability/builtin/objects.rs`).
    ///
    /// ```
    /// use majordomus_cli::capability::Provenance;
    /// let p = Provenance::Builtin { module: "majordomus_cli::capability::builtin::objects".into() };
    /// assert_eq!(p.source_path(), "apps/majordomus-cli/src/capability/builtin/objects.rs");
    /// ```
    pub fn source_path(&self) -> String {
        match self {
            Provenance::Builtin { module } => {
                let inner = module
                    .strip_prefix("majordomus_cli::")
                    .unwrap_or(module.as_str());
                format!("{CRATE_DIR}/src/{}.rs", inner.replace("::", "/"))
            }
            Provenance::Declarative { path, .. } => path.clone(),
        }
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provenance::Builtin { module } => write!(f, "builtin {module}"),
            Provenance::Declarative {
                path,
                member: Some(m),
                ..
            } => write!(f, "{path}#{m}"),
            Provenance::Declarative {
                path, member: None, ..
            } => write!(f, "{path}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// An MCP resource: its URI and the short name a client lists.
///
/// The URI is the identity a client reads by and the registry refuses two capabilities
/// claiming one of them; the name is only what a list shows, and nothing resolves it.
///
/// ```
/// use majordomus_cli::capability::McpResource;
/// let res = McpResource {
///     uri: "majordomus://rule/project.worktree-topology@1".into(),
///     name: "project.worktree-topology@1".into(),
/// };
/// assert!(res.uri.starts_with("majordomus://"), "the registry refuses any other scheme");
/// assert!(res.uri.ends_with(&res.name), "the identity is the tail of its own URI");
/// ```
pub struct McpResource {
    /// `majordomus://<kind>/<identity>`, or `majordomus://repository`.
    pub uri: String,
    /// The short name a client lists; the identity for a declarative object.
    pub name: String,
}

/// How, if at all, a capability appears to an MCP client.
///
/// The tool and the resource are independent: a capability may be neither, either, or —
/// as `repository.info` is — both, callable by name and readable at a URI. Either one
/// makes it reachable by anything that can attach to the process, which is why
/// [`Visibility::classify`] reads the presence of this and not its contents.
///
/// ```
/// use majordomus_cli::capability::{Exposure, McpExposure, Visibility};
/// let as_tool = McpExposure { tool: Some("majordomus_health".into()), ..Default::default() };
/// assert!(as_tool.resource.is_none(), "absence is explicit, never inferred");
/// let exposure = Exposure { mcp: Some(as_tool), ..Default::default() };
/// assert_eq!(Visibility::classify(&exposure), Visibility::Public);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpExposure {
    /// As a tool with this name (`[a-z0-9_]+`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// As a readable resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<McpResource>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "UPPERCASE")]
/// The HTTP methods a capability may be bound to.
///
/// Two, because this projection serves two: a read whose input is the query string, and a
/// call whose input is the body. Anything else is not a method the router has a binding
/// for, so [`HttpMethod::parse`] answers `None` rather than growing a variant nothing can
/// dispatch.
///
/// ```
/// use majordomus_cli::capability::HttpMethod;
/// assert_eq!(HttpMethod::parse(HttpMethod::Post.as_str()), Some(HttpMethod::Post));
/// assert_eq!(HttpMethod::parse("get"), None, "the wire name is upper case");
/// assert_eq!(HttpMethod::parse("DELETE"), None, "and this projection serves no other");
/// ```
pub enum HttpMethod {
    /// Read-only; the input is bound from the query string.
    Get,
    /// The input is bound from the JSON body. No builtin uses it yet.
    Post,
}

impl HttpMethod {
    /// The method as it appears on the wire.
    ///
    /// The inverse of [`HttpMethod::parse`], and the one string both the router's binding
    /// and the OpenAPI operation are built from, so a route cannot be described with one
    /// method and served under another.
    ///
    /// ```
    /// use majordomus_cli::capability::HttpMethod;
    /// assert_eq!(HttpMethod::Get.as_str(), "GET");
    /// assert_eq!(HttpMethod::parse(HttpMethod::Get.as_str()), Some(HttpMethod::Get));
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        }
    }
    /// The method for a wire name; `None` for one this projection does not serve.
    ///
    /// ```
    /// use majordomus_cli::capability::HttpMethod;
    /// assert_eq!(HttpMethod::parse("GET"), Some(HttpMethod::Get));
    /// assert_eq!(HttpMethod::parse("DELETE"), None);
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "GET" => Some(HttpMethod::Get),
            "POST" => Some(HttpMethod::Post),
            _ => None,
        }
    }
}

/// How a capability appears over HTTP. `GET` binds every top-level input property as a
/// query parameter; `POST` binds the input as the JSON request body. Paths are absolute
/// and live under [`HttpExposure::PREFIX`].
///
/// ```
/// use majordomus_cli::capability::{HttpExposure, HttpMethod};
/// // the prefix is on the type rather than repeated in every declaration, and a path
/// // that does not start with it is refused before the router ever sees it
/// let route = HttpExposure {
///     method: HttpMethod::Get,
///     path: format!("{}health", HttpExposure::PREFIX),
/// };
/// route.validate().unwrap();
/// assert_eq!(route.path, "/api/v1/health");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpExposure {
    /// The method.
    pub method: HttpMethod,
    /// The absolute path, under [`HttpExposure::PREFIX`].
    pub path: String,
}

impl HttpExposure {
    /// Every capability route starts here; the version is part of the contract.
    pub const PREFIX: &'static str = "/api/v1/";

    /// Is the path one this projection can serve? The reason when it is not.
    ///
    /// ```
    /// use majordomus_cli::capability::{HttpExposure, HttpMethod};
    /// let ok = HttpExposure { method: HttpMethod::Get, path: "/api/v1/objects".into() };
    /// assert!(ok.validate().is_ok());
    /// let bad = HttpExposure { method: HttpMethod::Get, path: "/objects".into() };
    /// assert!(bad.validate().unwrap_err().contains("/api/v1/"));
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.path;
        if !p.starts_with(Self::PREFIX) {
            return Err(format!("path '{p}' is not under {}", Self::PREFIX));
        }
        if p.ends_with('/') || p.contains("//") || p.contains('?') || p.contains('#') {
            return Err(format!(
                "path '{p}' has a trailing slash, an empty segment, or a query or fragment"
            ));
        }
        if !p
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        {
            return Err(format!(
                "path '{p}' carries a character outside [A-Za-z0-9/-_.]"
            ));
        }
        Ok(())
    }
}

/// How a capability appears on the command line: the words after `majordomus`.
///
/// The words, not a line of text. The command tree is built by walking them, so a
/// subcommand never has to be recovered by splitting a string, and two capabilities
/// claiming one path are a registry error rather than whichever one clap saw last.
///
/// ```
/// use majordomus_cli::capability::{CliExposure, Exposure, Visibility};
/// let exposure = CliExposure { path: vec!["capabilities".into(), "list".into()] };
/// assert_eq!(exposure.path.join(" "), "capabilities list");
/// let only_cli = Exposure { cli: Some(exposure), ..Default::default() };
/// assert_eq!(Visibility::classify(&only_cli), Visibility::Developer);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliExposure {
    /// The words after `majordomus`, e.g. `["capabilities", "list"]`.
    pub path: Vec<String>,
}

/// The projections a capability declares. Absence is explicit: `None` means not exposed
/// there, and nothing infers an exposure a descriptor did not declare.
///
/// ```
/// use majordomus_cli::capability::{Exposure, HttpExposure, HttpMethod};
/// // the default is exposed nowhere, and it serialises to an empty object: a projection
/// // reads absence instead of guessing from the id or the module
/// let nowhere = Exposure::default();
/// assert!(nowhere.is_empty());
/// assert_eq!(serde_json::to_string(&nowhere).unwrap(), "{}");
/// let over_http = Exposure {
///     http: Some(HttpExposure { method: HttpMethod::Get, path: "/api/v1/health".into() }),
///     ..Default::default()
/// };
/// assert!(!over_http.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Exposure {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The MCP projection, when declared.
    pub mcp: Option<McpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The HTTP projection, when declared.
    pub http: Option<HttpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The command-line projection, when declared.
    pub cli: Option<CliExposure>,
}

impl Exposure {
    /// Is this capability projected nowhere at all?
    ///
    /// True of a descriptor that exists, is listed as existing, and is offered by no
    /// surface — which [`Visibility::classify`] then reports as `Internal`. Worth asking
    /// separately from that, because the registry's summary counts what nothing offers,
    /// and a capability that quietly fell off every transport is otherwise
    /// indistinguishable from one that was never meant to be on any.
    ///
    /// ```
    /// use majordomus_cli::capability::{CliExposure, Exposure};
    /// assert!(Exposure::default().is_empty());
    /// let cli = Exposure {
    ///     cli: Some(CliExposure { path: vec!["scope".into()] }),
    ///     ..Default::default()
    /// };
    /// assert!(!cli.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.mcp.is_none() && self.http.is_none() && self.cli.is_none()
    }
}

/// Where a capability means anything: the environment a caller must be in for it to
/// answer at all.
///
/// The published site and the running server are genuinely different places. Without this
/// on the model, every template grows its own idea of what works where — and the usual
/// shape that takes is a condition on the page's own address, which is a rule hidden
/// where nobody will find it and nothing can test it. This is the only thing a projection
/// may ask.
///
/// It is classified rather than declared: the facts that decide it — what kind of thing
/// this is and which transports it is projected through — are already on the descriptor,
/// and asking each declaration to restate them would be the same knowledge written twice.
/// [`Availability::classify`] is the one place the rule lives.
///
/// ```
/// use majordomus_cli::capability::{Availability, CapabilityKind, Exposure};
/// // the layer's own content is readable from a published page; anything with a handler
/// // needs a process, whichever transport reaches it
/// let nowhere = Exposure::default();
/// assert_eq!(Availability::classify(CapabilityKind::Resource, &nowhere), Availability::Always);
/// assert_eq!(Availability::classify(CapabilityKind::Command, &nowhere), Availability::Runtime);
/// // the two environments nothing classifies to yet still have their word on the wire,
/// // so a value captured at build time can be labelled as a capture when one arrives
/// assert_eq!(serde_json::to_string(&Availability::BuildTime).unwrap(), "\"build_time\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
// Three things in this executable answer to `Availability` — a capability's, a
// surface's, and whether a surface is answering right now. The schema component namespace
// is flat, so each says which it is.
#[schemars(rename = "CapabilityAvailability")]
pub enum Availability {
    /// True in every environment, a published page with no server included: the layer's
    /// own content, which a build renders and a process serves from the same index.
    Always,
    /// A process must be running to answer: everything with a handler, whether it is
    /// reached over HTTP, over MCP or from the command line.
    Runtime,
    /// A value captured when the site was generated, rendered afterwards as the capture
    /// it is. Nothing classifies to this yet; the static projection of the graph is what
    /// will declare it, and it is on the model so that a captured value can be labelled
    /// as captured instead of being shown as current.
    BuildTime,
    /// A process must be running and the caller must be one it has authenticated. Nothing
    /// in this repository authenticates a caller yet; a surface that does will say so
    /// here rather than in the template that renders its link.
    Authenticated,
}

impl Availability {
    /// Classify from what the descriptor already declares.
    ///
    /// Today the kind decides alone, and the exposure is accepted without being read: the
    /// layer's content is readable wherever the index is, and a handler needs a process
    /// whichever transport reaches it. The parameter is taken all the same, because the
    /// transports are the only other thing that could move the answer — a capability whose
    /// caller had to be authenticated would be saying so in an exposure, not in a kind —
    /// and a caller of this function never has to know which half of the descriptor
    /// decided.
    ///
    /// The match is exhaustive on purpose and has no fallback arm: a new kind, or a
    /// transport that changes what an environment can offer, is a compile error here
    /// rather than a silent `Always` in a page that then links to nothing.
    ///
    /// ```
    /// use majordomus_cli::capability::{Availability, CapabilityKind, Exposure};
    /// let nowhere = Exposure::default();
    /// assert_eq!(Availability::classify(CapabilityKind::Resource, &nowhere), Availability::Always);
    /// assert_eq!(Availability::classify(CapabilityKind::Query, &nowhere), Availability::Runtime);
    /// ```
    pub fn classify(kind: CapabilityKind, _exposure: &Exposure) -> Availability {
        match kind {
            // the content is the layer's, and the index that holds it is read the same way
            // by a build and by a process
            CapabilityKind::Resource => Availability::Always,
            // a handler answers, and a handler needs a process to run in — the transport
            // decides who may call it, not whether anything can
            CapabilityKind::Query | CapabilityKind::Command => Availability::Runtime,
        }
    }
}

/// Who a capability is for, and whether anything offers it.
///
/// Internal is a statement, not an omission: a capability nothing projects is invisible
/// either way, and the difference between deliberate and forgotten is exactly what this
/// records.
///
/// ```
/// use majordomus_cli::capability::{CliExposure, Exposure, HttpExposure, HttpMethod, Visibility};
/// // reachable over a network, reachable by whoever runs the executable, or offered by
/// // nothing — and the third is a statement rather than a gap
/// let over_http = Exposure {
///     http: Some(HttpExposure { method: HttpMethod::Get, path: "/api/v1/health".into() }),
///     ..Default::default()
/// };
/// assert_eq!(Visibility::classify(&over_http), Visibility::Public);
/// let cli = Exposure {
///     cli: Some(CliExposure { path: vec!["doctor".into()] }),
///     ..Default::default()
/// };
/// assert_eq!(Visibility::classify(&cli), Visibility::Developer);
/// assert_eq!(Visibility::classify(&Exposure::default()), Visibility::Internal);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CapabilityVisibility")]
pub enum Visibility {
    /// Offered to anyone who can reach the process: an HTTP route or an MCP entry.
    Public,
    /// Offered to whoever runs the executable, and to nobody over a network.
    Developer,
    /// Projected nowhere. It exists, it is listed as existing, and no surface offers it.
    Internal,
}

impl Visibility {
    /// Classify from the transports the descriptor declares.
    ///
    /// ```
    /// use majordomus_cli::capability::{CliExposure, Exposure, Visibility};
    /// assert_eq!(Visibility::classify(&Exposure::default()), Visibility::Internal);
    /// let cli = Exposure { cli: Some(CliExposure { path: vec!["scope".into()] }), ..Default::default() };
    /// assert_eq!(Visibility::classify(&cli), Visibility::Developer);
    /// ```
    pub fn classify(exposure: &Exposure) -> Visibility {
        if exposure.mcp.is_some() || exposure.http.is_some() {
            Visibility::Public
        } else if exposure.cli.is_some() {
            Visibility::Developer
        } else {
            Visibility::Internal
        }
    }
}

/// The canonical descriptor. Everything a projection may say about a capability is here.
///
/// It is plain data with no handler and no transport in it, which is the property the rest
/// of the executable rests on: the same value the executor dispatches on round-trips
/// through JSON, so the generated reference, the site dataset and the OpenAPI document are
/// readings of it rather than second descriptions of the same capability.
///
/// ```
/// use majordomus_cli::capability;
/// use majordomus_cli::capability::{
///     Availability, BenchmarkCases, Capability, CapabilityError, CapabilityKind, CaseContext,
///     Context, Exposure, NamedCase, Stability, Visibility,
/// };
/// #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
/// struct In {}
/// impl BenchmarkCases for In {
///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
///         vec![NamedCase::new("default", In {})]
///     }
/// }
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Out { ok: bool }
/// fn ping(_: &Context, _: In) -> Result<Out, CapabilityError> { Ok(Out { ok: true }) }
///
/// let descriptor: Capability = capability! {
///     id: "demo.ping", title: "Ping", description: "Answers.", input: In, output: Out,
///     stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
///     handler: ping,
/// }.capability;
///
/// // three fields the declaration above never wrote: they follow from what it did write
/// assert_eq!(descriptor.kind, CapabilityKind::Query);
/// assert_eq!(descriptor.availability, Availability::Runtime);
/// assert_eq!(descriptor.visibility, Visibility::Internal);
///
/// // and the descriptor survives the boundary every projection reads it across
/// let json = serde_json::to_string(&descriptor).unwrap();
/// assert_eq!(serde_json::from_str::<Capability>(&json).unwrap(), descriptor);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    /// The canonical identity.
    pub id: CapabilityId,
    /// The module that composes it: the id's namespace for a builtin, the kind for a
    /// declarative object.
    pub module: ModuleId,
    /// Query, command or resource.
    pub kind: CapabilityKind,
    /// The short name every projection shows.
    pub title: String,
    /// The one-paragraph description every projection shows.
    pub description: String,
    /// The canonical schema of the input; an empty object for a resource.
    pub input: CanonicalSchema,
    /// The canonical schema of the output; the object view for a resource.
    pub output: CanonicalSchema,
    /// Where it came from.
    pub provenance: Provenance,
    /// Where it is projected; absence is explicit.
    pub exposure: Exposure,
    /// Where it means anything: classified from the kind, so that a projection reads a
    /// field instead of deciding for itself.
    pub availability: Availability,
    /// Who it is for: classified from the same transports.
    pub visibility: Visibility,
    /// Where it stands.
    pub stability: Stability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Free tags, from the declarative object's `tags` or the descriptor.
    pub tags: Vec<String>,
    /// Whether it is a benchmark target; the cases come from the input type.
    pub benchmark: BenchmarkPolicy,
    /// Whether the executor keeps its results.
    pub cache: CachePolicy,
    /// What running it as an execution means: classified from the kind, so that a client
    /// reads a fact rather than deciding for itself.
    pub execution: ExecutionPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_grammar() {
        assert!(CapabilityId::parse("repository.info").is_ok());
        assert!(CapabilityId::parse("rule.majordomus.scope-integrity@1").is_ok());
        assert!(CapabilityId::parse("document.docs/CLI.md").is_ok());
        for bad in [
            "",
            "repository",
            "Repository.info",
            "a.",
            "a b.c",
            "1a.b",
            "a.b\u{7}",
        ] {
            assert!(CapabilityId::parse(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(
            CapabilityId::parse("objects.get").unwrap().namespace(),
            "objects"
        );
    }

    #[test]
    fn http_exposure_validation() {
        let ok = HttpExposure {
            method: HttpMethod::Get,
            path: "/api/v1/objects".into(),
        };
        assert!(ok.validate().is_ok());
        for bad in [
            "/objects",
            "/api/v1/objects/",
            "/api/v1//x",
            "/api/v1/x?y",
            "/api/v1/x y",
        ] {
            assert!(
                HttpExposure {
                    method: HttpMethod::Get,
                    path: bad.into()
                }
                .validate()
                .is_err(),
                "{bad}"
            );
        }
    }
    #[test]
    fn the_model_draws_the_four_environments_apart() {
        // the distinctions exist so a surface can say which environment it means; two of
        // them have no member yet, and the classifier says so rather than pretending
        let all = [
            Availability::Always,
            Availability::Runtime,
            Availability::BuildTime,
            Availability::Authenticated,
        ];
        let names: Vec<String> = all
            .iter()
            .map(|a| serde_json::to_string(a).unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "\"always\"",
                "\"runtime\"",
                "\"build_time\"",
                "\"authenticated\""
            ]
        );
        for a in all {
            assert_eq!(
                serde_json::from_str::<Availability>(&serde_json::to_string(&a).unwrap()).unwrap(),
                a
            );
        }
    }

    #[test]
    fn what_is_projected_nowhere_is_internal_and_says_so() {
        let nowhere = Exposure::default();
        assert!(nowhere.is_empty());
        assert_eq!(Visibility::classify(&nowhere), Visibility::Internal);
        let over_http = Exposure {
            http: Some(HttpExposure {
                method: HttpMethod::Get,
                path: "/api/v1/thing".into(),
            }),
            ..Default::default()
        };
        assert_eq!(Visibility::classify(&over_http), Visibility::Public);
        assert_eq!(
            Availability::classify(CapabilityKind::Query, &over_http),
            Availability::Runtime
        );
    }
}

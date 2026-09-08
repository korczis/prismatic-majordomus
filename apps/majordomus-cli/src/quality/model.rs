//! The vocabulary of a quality report: what was measured, what is wrong, and what a
//! reader — a person, a pipeline or an agent — is to do about it.
//!
//! # Why the finding is typed
//!
//! A gate that prints `quality failed` is hostile infrastructure. Every finding here
//! carries a stable [`ViolationCode`], the [rule](ViolationCode::rule) that requires it,
//! the file and line it happened at, the symbol it is about, and a remediation in the
//! imperative. That is what lets one measurement render as a terminal report, a JSON
//! document, an HTTP response, a Cockpit panel and a CI annotation without any of them
//! deciding anything of their own — the same relationship every other projection in this
//! executable has with the registry.
//!
//! # The shape
//!
//! ```text
//! QualityReport
//!   ├── public_api  : items measured, documented, exampled
//!   ├── modules     : modules measured, documented, exampled, behaviourally tested
//!   ├── operations  : canonical capabilities against CLI, HTTP, OpenAPI, MCP
//!   └── violations  : every finding, most severe first
//! ```
//!
//! ```
//! use majordomus_cli::quality::{QualityReport, Severity};
//!
//! // An empty report is a clean report, and a clean report passes.
//! let report = QualityReport::default();
//! assert!(report.passes());
//! assert_eq!(report.exit_code(), 0);
//! assert_eq!(report.count(Severity::Error), 0);
//! ```

use serde::{Deserialize, Serialize};

/// The schema of a quality report. Bumped when a consumer would have to change.
pub const SCHEMA: &str = "majordomus/quality/v1";

/// How much a finding matters.
///
/// There are two levels and not five, because the only question a gate can answer is
/// whether the change may land. A warning is a finding the repository has decided not to
/// block on; nothing else is a warning.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "QualitySeverity")]
pub enum Severity {
    /// The gate fails.
    Error,
    /// Reported, and the gate passes.
    Warning,
}

impl Severity {
    /// The word a rendering shows.
    ///
    /// ```
    /// use majordomus_cli::quality::Severity;
    /// assert_eq!(Severity::Error.as_str(), "error");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// What is wrong, as a stable machine-readable code.
///
/// The codes are the contract between the validator and everything downstream: CI
/// annotations, Cockpit filters, documentation anchors and an agent deciding what to fix.
/// A code is never renamed once it has shipped; a rule that stops existing takes its code
/// with it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ViolationCode {
    /// An exported item carries no documentation at all.
    RustPublicMissingDocs,
    /// An exported item's documentation says nothing its signature had not already said.
    RustPublicThinDocs,
    /// An exported item that carries behaviour has no executable example.
    RustPublicMissingExample,
    /// Every example an item has is `ignore`d or is prose in a fenced box.
    RustExampleNotExecutable,
    /// An example asserts nothing, or asserts something that is true of any program.
    RustExamplePlaceholder,
    /// An example never names the item it is documenting.
    RustExampleDoesNotNameSubject,
    /// An exported module carries no module-level documentation.
    RustModuleMissingDocs,
    /// An exported module has no module-level executable example.
    RustModuleMissingExample,
    /// No test names an exported module, and it declares none of its own.
    RustModuleMissingBehaviouralTest,
    /// A command of the command line is neither a canonical capability nor classified as
    /// belonging to the command line alone.
    OperationCliUnclassified,
    /// A command is classified as belonging to the command line alone, and the command
    /// line no longer has it.
    OperationClassificationStale,
    /// A command is classified as local *and* bound to a capability: two answers to one
    /// question.
    OperationClassificationConflict,
    /// A capability declares an HTTP route and the OpenAPI document does not describe it.
    OperationMissingOpenapi,
    /// A capability declares a projection that the projection itself does not carry.
    OperationProjectionMissing,
    /// A projection carries an entry the registry does not hold.
    OperationProjectionOrphan,
}

impl ViolationCode {
    /// The code as it is written everywhere: `RUST_PUBLIC_MISSING_EXAMPLE`.
    ///
    /// ```
    /// use majordomus_cli::quality::ViolationCode;
    /// assert_eq!(ViolationCode::RustPublicMissingExample.as_str(), "RUST_PUBLIC_MISSING_EXAMPLE");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ViolationCode::RustPublicMissingDocs => "RUST_PUBLIC_MISSING_DOCS",
            ViolationCode::RustPublicThinDocs => "RUST_PUBLIC_THIN_DOCS",
            ViolationCode::RustPublicMissingExample => "RUST_PUBLIC_MISSING_EXAMPLE",
            ViolationCode::RustExampleNotExecutable => "RUST_EXAMPLE_NOT_EXECUTABLE",
            ViolationCode::RustExamplePlaceholder => "RUST_EXAMPLE_PLACEHOLDER",
            ViolationCode::RustExampleDoesNotNameSubject => "RUST_EXAMPLE_DOES_NOT_NAME_SUBJECT",
            ViolationCode::RustModuleMissingDocs => "RUST_MODULE_MISSING_DOCS",
            ViolationCode::RustModuleMissingExample => "RUST_MODULE_MISSING_EXAMPLE",
            ViolationCode::RustModuleMissingBehaviouralTest => {
                "RUST_MODULE_MISSING_BEHAVIOURAL_TEST"
            }
            ViolationCode::OperationCliUnclassified => "OPERATION_CLI_UNCLASSIFIED",
            ViolationCode::OperationClassificationStale => "OPERATION_CLASSIFICATION_STALE",
            ViolationCode::OperationClassificationConflict => "OPERATION_CLASSIFICATION_CONFLICT",
            ViolationCode::OperationMissingOpenapi => "OPERATION_MISSING_OPENAPI",
            ViolationCode::OperationProjectionMissing => "OPERATION_PROJECTION_MISSING",
            ViolationCode::OperationProjectionOrphan => "OPERATION_PROJECTION_ORPHAN",
        }
    }

    /// The rule of the repository's layer that requires this, by canonical id.
    ///
    /// A finding that could not name a rule would be an opinion. Every code names one, and
    /// the suite proves the rule exists.
    ///
    /// ```
    /// use majordomus_cli::quality::ViolationCode;
    /// assert_eq!(
    ///     ViolationCode::RustModuleMissingExample.rule(),
    ///     "project.rust-public-api-quality@1"
    /// );
    /// assert_eq!(
    ///     ViolationCode::OperationCliUnclassified.rule(),
    ///     "project.operation-transport-parity@1"
    /// );
    /// ```
    pub fn rule(self) -> &'static str {
        match self {
            ViolationCode::OperationCliUnclassified
            | ViolationCode::OperationClassificationStale
            | ViolationCode::OperationClassificationConflict
            | ViolationCode::OperationMissingOpenapi
            | ViolationCode::OperationProjectionMissing
            | ViolationCode::OperationProjectionOrphan => "project.operation-transport-parity@1",
            _ => "project.rust-public-api-quality@1",
        }
    }

    /// Why it matters, in one sentence: what goes wrong in the world if this stands.
    ///
    /// ```
    /// use majordomus_cli::quality::ViolationCode;
    /// assert!(ViolationCode::RustExamplePlaceholder.why().contains("nothing"));
    /// ```
    pub fn why(self) -> &'static str {
        match self {
            ViolationCode::RustPublicMissingDocs => {
                "an exported item with no documentation is API somebody has to read the implementation to use"
            }
            ViolationCode::RustPublicThinDocs => {
                "documentation that repeats the signature costs a reader a lookup and tells them nothing"
            }
            ViolationCode::RustPublicMissingExample => {
                "prose about behaviour drifts; an executable example cannot, because the toolchain runs it"
            }
            ViolationCode::RustExampleNotExecutable => {
                "an ignored or non-Rust block is compiled by nobody, so it documents whatever the API used to be"
            }
            ViolationCode::RustExamplePlaceholder => {
                "an example that asserts nothing proves nothing and exists only to satisfy a counter"
            }
            ViolationCode::RustExampleDoesNotNameSubject => {
                "an example that never uses the item it documents is not evidence about that item"
            }
            ViolationCode::RustModuleMissingDocs => {
                "a module is a conceptual boundary, and one that does not explain itself is a directory"
            }
            ViolationCode::RustModuleMissingExample => {
                "a reader arriving at a module needs one worked lifecycle, not a list of items"
            }
            ViolationCode::RustModuleMissingBehaviouralTest => {
                "a module nothing exercises is a module whose behaviour no change can break visibly"
            }
            ViolationCode::OperationCliUnclassified => {
                "a command that is neither a capability nor declared command-line-only is missing from the API by accident rather than by decision"
            }
            ViolationCode::OperationClassificationStale => {
                "a classification naming a command that no longer exists is a statement about nothing, and hides the next one that goes stale"
            }
            ViolationCode::OperationClassificationConflict => {
                "a command cannot be both projected to the API and declared local; one of the two is wrong"
            }
            ViolationCode::OperationMissingOpenapi => {
                "a route absent from the document is a route no client, no Swagger UI and no generated reference knows about"
            }
            ViolationCode::OperationProjectionMissing => {
                "a declared projection that does not exist is a promise the descriptor makes and the process does not keep"
            }
            ViolationCode::OperationProjectionOrphan => {
                "an entry no descriptor declares is a definition living in a projection, which is the duplication the registry exists to prevent"
            }
        }
    }

    /// What to do about it, in the imperative. Read by a person and by an agent.
    ///
    /// ```
    /// use majordomus_cli::quality::ViolationCode;
    /// assert!(ViolationCode::OperationCliUnclassified.remediation().contains("CliExposure"));
    /// ```
    pub fn remediation(self) -> &'static str {
        match self {
            ViolationCode::RustPublicMissingDocs => {
                "write a doc comment saying what the item is for, what it does with its inputs, and what it may fail with"
            }
            ViolationCode::RustPublicThinDocs => {
                "say what the signature cannot: the invariant, the side effect, the failure, or why the item exists"
            }
            ViolationCode::RustPublicMissingExample => {
                "add a ``` block that calls the item and asserts the result; or reduce the item's visibility to pub(crate) if no consumer needs it"
            }
            ViolationCode::RustExampleNotExecutable => {
                "drop `ignore` so the toolchain compiles the block, or use `no_run` when only running is undesirable"
            }
            ViolationCode::RustExamplePlaceholder => {
                "assert something the item computes, not something true of every program"
            }
            ViolationCode::RustExampleDoesNotNameSubject => {
                "call the documented item in the example, by the name a caller would write"
            }
            ViolationCode::RustModuleMissingDocs => {
                "write a //! header: what the module owns, its invariants, its error model, and how it relates to its neighbours"
            }
            ViolationCode::RustModuleMissingExample => {
                "add a ``` block to the //! header showing the module's normal lifecycle end to end"
            }
            ViolationCode::RustModuleMissingBehaviouralTest => {
                "add a #[cfg(test)] test beside the declaration, or exercise the module from a test under tests/"
            }
            ViolationCode::OperationCliUnclassified => {
                "give the capability a CliExposure so the command is the projection of one, or declare the command in cli::LOCAL with the reason it belongs to the command line alone"
            }
            ViolationCode::OperationClassificationStale => {
                "remove the entry from cli::LOCAL: the command it names is gone"
            }
            ViolationCode::OperationClassificationConflict => {
                "remove the cli::LOCAL entry, or the CliExposure; the command has an answer already"
            }
            ViolationCode::OperationMissingOpenapi => {
                "rebuild the OpenAPI document from the registry rather than editing it; a route it lacks is a route the generator did not see"
            }
            ViolationCode::OperationProjectionMissing => {
                "check the descriptor's exposure against the projection builder; nothing but the descriptor may decide this"
            }
            ViolationCode::OperationProjectionOrphan => {
                "delete the entry from the projection and declare it on the capability, where every projection reads it"
            }
        }
    }

    /// How bad it is. Every code this subsystem ships is an error: a warning-forever mode
    /// is how a rule becomes decoration.
    pub fn severity(self) -> Severity {
        Severity::Error
    }

    /// Every code, for the projections that list them and the test that proves each one is
    /// distinct.
    ///
    /// ```
    /// use majordomus_cli::quality::ViolationCode;
    /// let all = ViolationCode::all();
    /// let mut codes: Vec<&str> = all.iter().map(|c| c.as_str()).collect();
    /// codes.sort();
    /// codes.dedup();
    /// assert_eq!(codes.len(), all.len(), "every code is distinct");
    /// ```
    pub fn all() -> Vec<ViolationCode> {
        use ViolationCode::*;
        vec![
            RustPublicMissingDocs,
            RustPublicThinDocs,
            RustPublicMissingExample,
            RustExampleNotExecutable,
            RustExamplePlaceholder,
            RustExampleDoesNotNameSubject,
            RustModuleMissingDocs,
            RustModuleMissingExample,
            RustModuleMissingBehaviouralTest,
            OperationCliUnclassified,
            OperationClassificationStale,
            OperationClassificationConflict,
            OperationMissingOpenapi,
            OperationProjectionMissing,
            OperationProjectionOrphan,
        ]
    }
}

/// One finding: what, where, why, and what to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Violation {
    /// The stable code.
    pub code: ViolationCode,
    /// How bad it is.
    pub severity: Severity,
    /// The canonical id of the rule that requires this.
    pub rule: String,
    /// Where it is, repository-relative; empty when the finding is not about a file.
    pub path: String,
    /// The line, 1-based; `None` when the finding is not about a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The item's full path, or the command, or the capability id: what the finding is about.
    pub symbol: String,
    /// One sentence, specific to this occurrence.
    pub message: String,
    /// Why it matters, from the code.
    pub why: String,
    /// What to do, from the code.
    pub remediation: String,
}

impl Violation {
    /// A finding about an item at a place.
    ///
    /// The `why` and the `remediation` come from the code rather than from the caller, so
    /// that two findings of one kind cannot explain themselves differently.
    ///
    /// ```
    /// use majordomus_cli::quality::{Violation, ViolationCode};
    /// let v = Violation::new(
    ///     ViolationCode::RustModuleMissingExample,
    ///     "majordomus_cli::graph",
    ///     "apps/majordomus-cli/src/graph.rs",
    ///     Some(1),
    ///     "the module header carries no executable example",
    /// );
    /// assert_eq!(v.rule, "project.rust-public-api-quality@1");
    /// assert!(v.remediation.contains("//!"));
    /// ```
    pub fn new(
        code: ViolationCode,
        symbol: impl Into<String>,
        path: impl Into<String>,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Violation {
            code,
            severity: code.severity(),
            rule: code.rule().to_string(),
            path: path.into(),
            line,
            symbol: symbol.into(),
            message: message.into(),
            why: code.why().to_string(),
            remediation: code.remediation().to_string(),
        }
    }

    /// The finding as one line, the way the terminal and a CI annotation show it.
    ///
    /// ```
    /// use majordomus_cli::quality::{Violation, ViolationCode};
    /// let v = Violation::new(
    ///     ViolationCode::RustPublicMissingExample,
    ///     "majordomus_cli::graph::build",
    ///     "apps/majordomus-cli/src/graph.rs",
    ///     Some(42),
    ///     "carries behaviour and no executable example",
    /// );
    /// assert_eq!(
    ///     v.line_summary(),
    ///     "apps/majordomus-cli/src/graph.rs:42: RUST_PUBLIC_MISSING_EXAMPLE majordomus_cli::graph::build — carries behaviour and no executable example"
    /// );
    /// ```
    pub fn line_summary(&self) -> String {
        let at = match self.line {
            Some(l) => format!("{}:{l}", self.path),
            None if self.path.is_empty() => String::new(),
            None => self.path.clone(),
        };
        let head = if at.is_empty() {
            String::new()
        } else {
            format!("{at}: ")
        };
        format!(
            "{head}{} {} — {}",
            self.code.as_str(),
            self.symbol,
            self.message
        )
    }
}

/// What the exported item surface looks like.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PublicApiQuality {
    /// Items the crate exports, of every kind.
    pub items: usize,
    /// Of those, how many carry documentation.
    pub documented: usize,
    /// How many owe an executable example under the policy.
    pub owe_example: usize,
    /// Of those, how many have one that counts.
    pub exampled: usize,
    /// How many are exempt from the example policy for a structural reason, and why, by
    /// reason, so that the exemptions are visible rather than implied by a subtraction.
    pub exempt: Vec<Exemption>,
}

/// A group of items the example policy does not apply to, with the reason it does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Exemption {
    /// The reason, as the policy names it.
    pub reason: String,
    /// How many items it covers.
    pub items: usize,
}

/// What the exported module surface looks like.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModuleQuality {
    /// Modules the crate exports, the crate root included.
    pub modules: usize,
    /// Of those, how many carry a `//!` header.
    pub documented: usize,
    /// How many carry an executable example in that header.
    pub exampled: usize,
    /// How many something exercises: an in-file test, or a test that names them.
    pub behaviourally_tested: usize,
}

/// What the canonical operations look like against the transports that project them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationParity {
    /// Executable capabilities in the registry.
    pub canonical: usize,
    /// Of those, how many declare a command-line projection.
    pub cli: usize,
    /// How many declare an HTTP route.
    pub http: usize,
    /// How many the OpenAPI document describes.
    pub openapi: usize,
    /// How many declare an MCP tool.
    pub mcp: usize,
    /// Commands of the command line, leaves and runnable parents.
    pub cli_commands: usize,
    /// Of those, how many are the projection of a capability.
    pub cli_from_capability: usize,
    /// How many are declared to belong to the command line alone, with a reason.
    pub cli_local: usize,
}

/// One measurement of one crate, and everything found wrong in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QualityReport {
    /// [`SCHEMA`].
    #[serde(default)]
    pub schema: String,
    /// What was measured: the crate directory, repository-relative.
    pub target: String,
    /// The exported item surface.
    pub public_api: PublicApiQuality,
    /// The exported module surface.
    pub modules: ModuleQuality,
    /// The canonical operations against their transports.
    pub operations: OperationParity,
    /// Every finding, errors first, then by file and line.
    pub violations: Vec<Violation>,
}

impl QualityReport {
    /// Nothing wrong at any severity that blocks?
    ///
    /// ```
    /// use majordomus_cli::quality::{QualityReport, Violation, ViolationCode};
    /// let mut r = QualityReport::default();
    /// assert!(r.passes());
    /// r.violations.push(Violation::new(
    ///     ViolationCode::RustModuleMissingDocs, "m", "src/m.rs", Some(1), "no header"));
    /// assert!(!r.passes());
    /// ```
    pub fn passes(&self) -> bool {
        self.count(Severity::Error) == 0
    }

    /// How many findings of a severity.
    pub fn count(&self, severity: Severity) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == severity)
            .count()
    }

    /// The process exit code: `0` clean, `10` contract unmet, which is the code every
    /// other unmet contract in this executable uses.
    ///
    /// ```
    /// use majordomus_cli::quality::{QualityReport, Violation, ViolationCode};
    /// let mut r = QualityReport::default();
    /// assert_eq!(r.exit_code(), 0);
    /// r.violations.push(Violation::new(
    ///     ViolationCode::OperationCliUnclassified, "majordomus x", "", None, "unclassified"));
    /// assert_eq!(r.exit_code(), 10);
    /// ```
    pub fn exit_code(&self) -> u8 {
        if self.passes() {
            0
        } else {
            10
        }
    }

    /// Findings of one code, in report order.
    ///
    /// ```
    /// use majordomus_cli::quality::{QualityReport, Violation, ViolationCode};
    /// let mut r = QualityReport::default();
    /// r.violations.push(Violation::new(
    ///     ViolationCode::RustModuleMissingExample, "m", "src/m.rs", Some(1), "none"));
    /// assert_eq!(r.of(ViolationCode::RustModuleMissingExample).len(), 1);
    /// assert!(r.of(ViolationCode::RustModuleMissingDocs).is_empty());
    /// ```
    pub fn of(&self, code: ViolationCode) -> Vec<&Violation> {
        self.violations.iter().filter(|v| v.code == code).collect()
    }

    /// Sort the findings the way every rendering shows them: errors first, then by file,
    /// then by line, then by code, so that two runs over one tree print the same report.
    pub fn sort(&mut self) {
        self.violations.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.line.cmp(&b.line))
                .then_with(|| a.code.cmp(&b.code))
                .then_with(|| a.symbol.cmp(&b.symbol))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_names_a_rule_a_reason_and_a_remedy() {
        for code in ViolationCode::all() {
            assert!(code.rule().starts_with("project."), "{code:?}");
            assert!(code.why().len() > 30, "{code:?} explains nothing");
            assert!(
                code.remediation().len() > 20,
                "{code:?} tells nobody what to do"
            );
            // the code as text is the enum name in screaming snake, and round-trips
            let json = serde_json::to_string(&code).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
            assert_eq!(serde_json::from_str::<ViolationCode>(&json).unwrap(), code);
        }
    }

    #[test]
    fn a_report_with_a_finding_fails_and_says_so_in_both_renderings() {
        let mut r = QualityReport::default();
        r.violations.push(Violation::new(
            ViolationCode::RustPublicMissingExample,
            "majordomus_cli::a::b",
            "apps/majordomus-cli/src/a.rs",
            Some(7),
            "carries behaviour and no executable example",
        ));
        assert!(!r.passes());
        assert_eq!(r.exit_code(), 10);
        // the JSON and the human rendering agree about the outcome: never one without the other
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["violations"][0]["code"], "RUST_PUBLIC_MISSING_EXAMPLE");
        assert_eq!(
            json["violations"][0]["rule"],
            "project.rust-public-api-quality@1"
        );
        assert!(r.violations[0].line_summary().contains("src/a.rs:7"));
    }

    #[test]
    fn the_order_of_findings_does_not_depend_on_the_order_they_were_found() {
        let v = |path: &str, line: usize| {
            Violation::new(
                ViolationCode::RustModuleMissingDocs,
                "m",
                path,
                Some(line),
                "none",
            )
        };
        let mut a = QualityReport {
            violations: vec![v("b.rs", 2), v("a.rs", 9), v("a.rs", 1)],
            ..QualityReport::default()
        };
        let mut b = QualityReport {
            violations: vec![v("a.rs", 1), v("b.rs", 2), v("a.rs", 9)],
            ..QualityReport::default()
        };
        a.sort();
        b.sort();
        assert_eq!(a.violations, b.violations);
        assert_eq!(a.violations[0].path, "a.rs");
        assert_eq!(a.violations[0].line, Some(1));
    }
}

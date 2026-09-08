//! What this executable's own public surface is held to, measured by the executable.
//!
//! # What it is for
//!
//! Every other subsystem here is a projection of the capability registry, and the registry
//! is what makes an operation impossible to declare twice. Nothing played that role for
//! the crate's Rust surface: documentation, examples and behavioural coverage were
//! properties a reviewer noticed or did not. This subsystem makes them a measurement with
//! a stable code, a rule, a location and a remedy, so that the same finding reaches a
//! terminal, a pipeline, an HTTP client and an agent from one reading.
//!
//! # The parts
//!
//! ```text
//! source   the crate as a syntax tree: what is exported, where, with what documentation
//! policy   what an item owes, and what counts as having paid it
//! parity   the registry against MCP, HTTP, OpenAPI and the command line
//! report   one measurement: counts and findings, sorted, deterministic
//! model    the vocabulary: codes, severities, the report's own shape
//! ```
//!
//! Discovery happens once and every renderer reads the value: the same relationship the
//! rest of the executable has with the registry, for the same reason.
//!
//! # Invariants
//!
//! * **Deterministic.** File reads are the only I/O; no network, no clock, no environment.
//!   Two runs over one tree produce equal reports, which is what lets a report be compared
//!   and committed.
//! * **Whole or nothing.** A source file that cannot be parsed is an error, never a skip:
//!   a partial inventory that looked clean is the one result this must not produce.
//! * **No exemption list.** What the policy does not ask of an item is derived from the
//!   item's kind and shape, and is reported as an exemption with its reason, so that what
//!   is not demanded is as visible as what is.
//!
//! # Errors
//!
//! Everything here returns [`crate::Error::InvalidSource`] and nothing else: a directory
//! that is not a crate, a file that cannot be read, a file that does not parse, or a
//! `mod` declaration that resolves to no file.
//!
//! ```
//! use majordomus_cli::quality;
//!
//! // the whole lifecycle: build a crate, measure it, read the verdict
//! let dir = tempfile::tempdir().unwrap();
//! let src = dir.path().join("src");
//! std::fs::create_dir_all(&src).unwrap();
//! std::fs::write(src.join("lib.rs"), "//! Thin.\npub fn go() {}\n").unwrap();
//!
//! let report = quality::report::rust_only(dir.path(), "fixture").unwrap();
//! assert!(!report.passes());
//! assert_eq!(report.exit_code(), 10);
//!
//! // and every finding names the rule that requires it and what to do about it
//! let first = &report.violations[0];
//! assert!(first.rule.starts_with("project."));
//! assert!(!first.remediation.is_empty());
//! ```

pub mod model;
pub mod parity;
pub mod policy;
pub mod report;
pub mod source;

pub use model::{
    Exemption, ModuleQuality, OperationParity, PublicApiQuality, QualityReport, Severity,
    Violation, ViolationCode, SCHEMA,
};
pub use policy::{ExampleRequirement, Verdict};
pub use source::{Example, Inventory, Item, ItemKind};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subsystem_measures_its_own_crate_and_answers_about_it() {
        // the dogfooding assertion: this runs against the crate it lives in, which is the
        // only way the numbers in the report are ever about something real
        let report = report::rust_only(std::path::Path::new("."), "apps/majordomus-cli").unwrap();
        assert!(report.public_api.items > 100, "the crate exports a surface");
        assert_eq!(
            report.public_api.items, report.public_api.documented,
            "missing_docs is denied at compile time, so every exported item is documented"
        );
        assert_eq!(report.modules.modules, report.modules.documented);
        assert!(report
            .violations
            .iter()
            .all(|v| v.severity == Severity::Error));
    }

    #[test]
    fn a_directory_that_is_not_a_crate_is_an_error_and_never_an_empty_pass() {
        let dir = tempfile::tempdir().unwrap();
        let err = report::rust_only(dir.path(), "nothing").unwrap_err();
        assert!(matches!(err, crate::Error::InvalidSource { .. }));
        assert_eq!(err.exit_code(), 10);
    }
}

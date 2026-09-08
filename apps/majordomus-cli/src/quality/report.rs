//! Assembling one measurement: the inventory and the registry in, a [`QualityReport`] out.
//!
//! # One discovery, many renderings
//!
//! The crate is parsed once, the registry is read once, and everything downstream — the
//! terminal rendering, the JSON, the HTTP response, the Cockpit panel, the CI annotations
//! — is a reading of the single value this module returns. That is the same relationship
//! every other projection in this executable has with the registry, and it is why a
//! `--json` report and a human report can never disagree about whether the gate passed.
//!
//! # What it measures
//!
//! ```text
//! src/**.rs  --syn-->  Inventory  --policy-->  item and module findings
//! registry + clap tree + OpenAPI  --parity-->  operation findings
//! ```
//!
//! ```
//! use majordomus_cli::quality::report;
//!
//! // a crate of one module, documented, exampled and tested: the shape of a clean report
//! let dir = tempfile::tempdir().unwrap();
//! let src = dir.path().join("src");
//! std::fs::create_dir_all(&src).unwrap();
//! std::fs::write(src.join("lib.rs"), concat!(
//!     "//! A crate that does one thing, and this header says at some length what that one\n",
//!     "//! thing is, which is what the module policy asks of every exported module here.\n",
//!     "//!\n",
//!     "//! ```\n",
//!     "//! assert_eq!(majordomus_cli::two(), 2);\n",
//!     "//! ```\n",
//!     "/// Answers with two, every time, which is more than the signature promises.\n",
//!     "///\n",
//!     "/// ```\n",
//!     "/// assert_eq!(majordomus_cli::two(), 2);\n",
//!     "/// ```\n",
//!     "pub fn two() -> usize { 2 }\n",
//!     "#[cfg(test)]\n",
//!     "mod tests { #[test] fn it_is_two() { assert_eq!(super::two(), 2); } }\n",
//! )).unwrap();
//!
//! let report = report::rust_only(dir.path(), "a-crate").unwrap();
//! assert!(report.passes(), "{:?}", report.violations);
//! assert_eq!(report.modules.modules, 1);
//! assert_eq!(report.public_api.owe_example, 1);
//! ```

use std::path::Path;

use crate::capability::CapabilityRegistry;
use crate::cli::CommandDoc;
use crate::error::Error;

use super::model::{
    Exemption, ModuleQuality, PublicApiQuality, QualityReport, Violation, ViolationCode, SCHEMA,
};
use super::policy::{ExampleRequirement, Verdict, MIN_MODULE_PROSE_WORDS, MIN_PROSE_WORDS};
use super::source::{Inventory, ItemKind};

/// Measure the Rust surface of a crate: documentation, examples and module coverage.
///
/// The operations half needs a registry and a command tree; a caller that has neither —
/// a test over a fixture crate, for instance — uses this and gets a report whose
/// `operations` section is all zeroes rather than a wrong one.
///
/// ```
/// use majordomus_cli::quality::report;
/// let dir = tempfile::tempdir().unwrap();
/// let src = dir.path().join("src");
/// std::fs::create_dir_all(&src).unwrap();
/// std::fs::write(src.join("lib.rs"), "//! Thin.\npub fn go() {}\n").unwrap();
///
/// let r = report::rust_only(dir.path(), "thin").unwrap();
/// assert!(!r.passes(), "a thin header and an unexampled function are both findings");
/// assert_eq!(r.operations.canonical, 0, "nothing was measured about operations");
/// ```
pub fn rust_only(crate_dir: &Path, target: &str) -> Result<QualityReport, Error> {
    let inventory = Inventory::of_crate(crate_dir)?;
    Ok(assemble(&inventory, target, None))
}

/// Measure everything: the Rust surface and the transport parity of the operations.
///
/// `crate_dir` is the directory holding `Cargo.toml`; `target` is what the report calls
/// what it measured, repository-relative. The OpenAPI document is passed in rather than
/// built here, because the caller already has one and building a second would be the
/// duplication this subsystem exists to find.
pub fn inspect(
    crate_dir: &Path,
    target: &str,
    registry: &CapabilityRegistry,
    tree: &CommandDoc,
    openapi: &serde_json::Value,
) -> Result<QualityReport, Error> {
    let inventory = Inventory::of_crate(crate_dir)?;
    Ok(assemble(
        &inventory,
        target,
        Some((registry, tree, openapi)),
    ))
}

fn assemble(
    inventory: &Inventory,
    target: &str,
    operations: Option<(&CapabilityRegistry, &CommandDoc, &serde_json::Value)>,
) -> QualityReport {
    let mut report = QualityReport {
        schema: SCHEMA.to_string(),
        target: target.to_string(),
        ..QualityReport::default()
    };
    let (api, module_quality, mut violations) = rust_surface(inventory, target);
    report.public_api = api;
    report.modules = module_quality;

    if let Some((registry, tree, openapi)) = operations {
        let (counts, findings) = super::parity::inspect(registry, tree, openapi);
        report.operations = counts;
        violations.extend(findings);
    }

    report.violations = violations;
    report.sort();
    report
}

/// The item and module halves of the measurement.
fn rust_surface(
    inventory: &Inventory,
    target: &str,
) -> (PublicApiQuality, ModuleQuality, Vec<Violation>) {
    let mut api = PublicApiQuality::default();
    let mut modules = ModuleQuality::default();
    let mut violations = Vec::new();
    let mut exempt: std::collections::BTreeMap<&'static str, usize> = Default::default();

    for item in inventory.exported() {
        let at = |file: &str| format!("{target}/{file}");
        api.items += 1;
        let documented = !item.doc.trim().is_empty();
        if documented {
            api.documented += 1;
        }

        if item.kind == ItemKind::Module {
            modules.modules += 1;
            measure_module(item, inventory, target, &mut modules, &mut violations);
            continue;
        }

        // ---- documentation: the compiler already refuses the absence, so what is left is
        // the ceremonial form of it. The length bar applies where a signature can hide
        // something — a type, a function, a method, a macro — and not to a field, a
        // variant or a constant, which are named and typed and whose gloss is often
        // complete in four words. Demanding more of those buys padding, which is the same
        // trade the example policy refuses to make.
        if !documented {
            violations.push(Violation::new(
                ViolationCode::RustPublicMissingDocs,
                &item.path,
                at(&item.file),
                Some(item.line),
                format!(
                    "this exported {} carries no documentation",
                    item.kind.noun()
                ),
            ));
        } else if item.kind.carries_behaviour() && item.prose_words() < MIN_PROSE_WORDS {
            violations.push(Violation::new(
                ViolationCode::RustPublicThinDocs,
                &item.path,
                at(&item.file),
                Some(item.line),
                format!(
                    "{} words of prose: too few to say anything the signature has not",
                    item.prose_words()
                ),
            ));
        }

        // ---- examples
        let requirement = ExampleRequirement::of(item);
        let Some(reason) = requirement.reason() else {
            api.owe_example += 1;
            match Verdict::best(&item.executable_examples_or_all(), &item.name) {
                Some(Verdict::Counts) => api.exampled += 1,
                Some(verdict) => {
                    let code = verdict.code().expect("a non-counting verdict has a code");
                    violations.push(Violation::new(
                        code,
                        &item.path,
                        at(&item.file),
                        Some(item.line),
                        example_message(verdict, &item.name),
                    ));
                }
                None => violations.push(Violation::new(
                    ViolationCode::RustPublicMissingExample,
                    &item.path,
                    at(&item.file),
                    Some(item.line),
                    format!(
                        "this exported {} carries behaviour and no example of it",
                        item.kind.noun()
                    ),
                )),
            }
            continue;
        };
        *exempt.entry(reason).or_default() += 1;
    }

    api.exempt = exempt
        .into_iter()
        .map(|(reason, items)| Exemption {
            reason: reason.to_string(),
            items,
        })
        .collect();
    (api, modules, violations)
}

fn measure_module(
    item: &super::source::Item,
    inventory: &Inventory,
    target: &str,
    modules: &mut ModuleQuality,
    violations: &mut Vec<Violation>,
) {
    let at = format!("{target}/{}", item.file);
    let documented = !item.doc.trim().is_empty();
    if documented {
        modules.documented += 1;
    }
    if !documented {
        violations.push(Violation::new(
            ViolationCode::RustModuleMissingDocs,
            &item.path,
            &at,
            Some(item.line),
            "this exported module carries no //! header".to_string(),
        ));
    } else if item.prose_words() < MIN_MODULE_PROSE_WORDS {
        violations.push(Violation::new(
            ViolationCode::RustModuleMissingDocs,
            &item.path,
            &at,
            Some(item.line),
            format!(
                "{} words of header: a module is a boundary and this does not explain one",
                item.prose_words()
            ),
        ));
    }

    // a module's example is judged against the module's own name, and against the last
    // segment of any item it holds: a lifecycle example names what it exercises
    let subject = &item.name;
    match Verdict::best(&item.executable_examples_or_all(), subject) {
        Some(Verdict::Counts) => modules.exampled += 1,
        Some(Verdict::DoesNotNameSubject) => {
            // a module header may show a lifecycle through the items it holds rather than
            // through its own name, which is the normal and better shape
            if item
                .executable_examples()
                .iter()
                .any(|e| names_something_of(&e.code(), inventory, &item.path))
            {
                modules.exampled += 1;
            } else {
                violations.push(Violation::new(
                    ViolationCode::RustModuleMissingExample,
                    &item.path,
                    &at,
                    Some(item.line),
                    "the header's example exercises nothing this module declares".to_string(),
                ));
            }
        }
        Some(verdict) => violations.push(Violation::new(
            verdict.code().expect("a non-counting verdict has a code"),
            &item.path,
            &at,
            Some(item.line),
            example_message(verdict, subject),
        )),
        None => violations.push(Violation::new(
            ViolationCode::RustModuleMissingExample,
            &item.path,
            &at,
            Some(item.line),
            "the //! header carries no executable example of the module's lifecycle".to_string(),
        )),
    }

    if inventory.exercised_by_a_test(&item.path) {
        modules.behaviourally_tested += 1;
    } else {
        violations.push(Violation::new(
            ViolationCode::RustModuleMissingBehaviouralTest,
            &item.path,
            &at,
            Some(item.line),
            "no #[cfg(test)] test stands beside it and no test under tests/ names it".to_string(),
        ));
    }
}

/// Does the example exercise something this module declares?
fn names_something_of(code: &str, inventory: &Inventory, module: &str) -> bool {
    let prefix = format!("{module}::");
    inventory
        .items
        .iter()
        .filter(|i| i.exported && i.owner == module && i.path.starts_with(&prefix))
        .any(|i| super::policy::names(code, &i.name))
}

fn example_message(verdict: Verdict, subject: &str) -> String {
    match verdict {
        Verdict::Counts => String::new(),
        Verdict::Placeholder => {
            "every example asserts only what is true of any program".to_string()
        }
        Verdict::DoesNotNameSubject => {
            format!("no example names `{subject}`, so none is evidence about it")
        }
        Verdict::NotExecutable => {
            "every fenced block is ignored or is prose, so the toolchain compiles none of them"
                .to_string()
        }
    }
}

impl super::source::Item {
    /// The blocks to judge: the executable ones when there are any, and otherwise every
    /// block, so that an item whose only block is `ignore`d is told that rather than being
    /// told it has no example.
    fn executable_examples_or_all(&self) -> Vec<&super::source::Example> {
        let executable = self.executable_examples();
        if executable.is_empty() {
            self.examples.iter().collect()
        } else {
            executable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_of(lib: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("lib.rs"), lib).unwrap();
        dir
    }

    const HEADER: &str = "//! A fixture crate whose header is long enough to be a header: it says what the\n//! module owns, which is the one function below it and nothing else at all.\n//!\n//! ```\n//! assert_eq!(majordomus_cli::two(), 2);\n//! ```\n";

    #[test]
    fn a_documented_exampled_and_tested_crate_has_no_findings() {
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\n///\n/// ```\n/// assert_eq!(majordomus_cli::two(), 2);\n/// ```\npub fn two() -> usize {{ 2 }}\n#[cfg(test)]\nmod tests {{ #[test] fn it_is_two() {{ assert_eq!(super::two(), 2); }} }}\n"
        ));
        let r = rust_only(dir.path(), "fixture").unwrap();
        assert!(r.passes(), "{:#?}", r.violations);
        assert_eq!(r.public_api.owe_example, 1);
        assert_eq!(r.public_api.exampled, 1);
        assert_eq!(r.modules.behaviourally_tested, 1);
    }

    #[test]
    fn each_way_of_failing_raises_its_own_code_and_nothing_else() {
        // no example at all
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\npub fn two() -> usize {{ 2 }}\n#[cfg(test)]\nmod tests {{ #[test] fn t() {{ assert_eq!(super::two(), 2); }} }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(r.of(ViolationCode::RustPublicMissingExample).len(), 1);
        assert_eq!(r.violations.len(), 1, "{:#?}", r.violations);

        // an ignored one
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\n///\n/// ```ignore\n/// two();\n/// ```\npub fn two() -> usize {{ 2 }}\n#[cfg(test)]\nmod tests {{ #[test] fn t() {{ assert_eq!(super::two(), 2); }} }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(r.of(ViolationCode::RustExampleNotExecutable).len(), 1);

        // a placeholder
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\n///\n/// ```\n/// let _ = two;\n/// assert!(true);\n/// ```\npub fn two() -> usize {{ 2 }}\n#[cfg(test)]\nmod tests {{ #[test] fn t() {{ assert_eq!(super::two(), 2); }} }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(r.of(ViolationCode::RustExamplePlaceholder).len(), 1);

        // an example about something else entirely
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\n///\n/// ```\n/// assert_eq!(1 + 1, 2, \"{{}}\", 2);\n/// let v: Vec<u8> = Vec::new();\n/// assert!(v.is_empty());\n/// ```\npub fn two() -> usize {{ 2 }}\n#[cfg(test)]\nmod tests {{ #[test] fn t() {{ assert_eq!(super::two(), 2); }} }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(
            r.of(ViolationCode::RustExampleDoesNotNameSubject).len(),
            1,
            "{:#?}",
            r.violations
        );
    }

    #[test]
    fn a_module_with_no_test_beside_it_and_none_naming_it_is_reported() {
        let dir = crate_of(&format!(
            "{HEADER}/// Answers with two, which the signature does not promise.\n///\n/// ```\n/// assert_eq!(majordomus_cli::two(), 2);\n/// ```\npub fn two() -> usize {{ 2 }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(
            r.of(ViolationCode::RustModuleMissingBehaviouralTest).len(),
            1
        );
        assert_eq!(r.modules.behaviourally_tested, 0);
    }

    #[test]
    fn what_the_policy_does_not_ask_for_is_counted_and_named() {
        let dir = crate_of(&format!(
            "{HEADER}/// A limit that exists for a reason worth writing down here.\npub const LIMIT: usize = 3;\n#[cfg(test)]\nmod tests {{ #[test] fn t() {{ assert_eq!(super::LIMIT, 3); }} }}\n"
        ));
        let r = rust_only(dir.path(), "f").unwrap();
        assert_eq!(r.public_api.owe_example, 0);
        assert_eq!(r.public_api.exempt.len(), 1);
        assert_eq!(r.public_api.exempt[0].items, 1);
        assert!(r.public_api.exempt[0].reason.contains("value"));
        assert!(r.passes(), "{:#?}", r.violations);
    }

    #[test]
    fn a_report_of_the_same_tree_twice_is_the_same_report() {
        let dir = crate_of(&format!("{HEADER}pub fn two() -> usize {{ 2 }}\n"));
        let a = rust_only(dir.path(), "f").unwrap();
        let b = rust_only(dir.path(), "f").unwrap();
        assert_eq!(a, b);
    }
}

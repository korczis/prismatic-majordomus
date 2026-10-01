//! `reasoning.check`: the executable half of the rules `project.advisors-are-optional`
//! and `project.review-is-recorded-not-claimed`. Each check reads files and says what it
//! found; none of them asks an advisor anything.
//!
//! * `catalogue` — the advisor catalogue resolves against the provider table and the
//!   model catalogue (one inventory, referenced, never restated);
//! * `adapters` — every adapter the catalogue names has its transport module, so the Node
//!   layer is derived from the catalogue rather than keeping a list of its own;
//! * `coupling` — the provider-independent code (this module, its capability, its CLI
//!   rendering, its Cockpit page, the transport driver) names no advisor, adapter,
//!   executable, provider, vendor or model of the catalogue as a string literal;
//! * `ci` — no CI workflow or gate names a model vendor's credential variable, so no
//!   required gate can depend on a live model;
//! * `claims` — no document asserts a claim the catalogue forbids ("consensus
//!   guarantees ...") or that an advisor is required;
//! * `records` — every stored record is readable, every consultation names an advisor
//!   its plan selected and one this repository knows, every conclusion's computed review
//!   matches the consultations it cites.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::catalogue::{AdvisorCatalogue, References};
use super::record::{ConsultationStatus, ReasoningBody};
use super::store::Loaded;

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningFinding {
    /// The check that found it.
    pub check: String,
    /// Where: a file, a record, an advisor.
    pub subject: String,
    /// What is wrong.
    pub message: String,
}

/// The source files that must stay provider-independent, relative to the root.
pub const INDEPENDENT_SOURCES: &[&str] = &[
    "apps/majordomus-cli/src/reasoning",
    "apps/majordomus-cli/src/capability/builtin/reasoning.rs",
    "apps/majordomus-cli/src/commands/reasoning.rs",
    "apps/majordomus-cli/src/cockpit/reasoning.rs",
    "scripts/lib/advisors/driver.mjs",
    "scripts/lib/advisors/contract.mjs",
];

/// Where the transport adapters live, relative to the root.
pub const ADAPTER_DIR: &str = "scripts/lib/advisors";

fn files_under(root: &Path, rel: &str, ext: &[&str]) -> Vec<std::path::PathBuf> {
    let path = root.join(rel);
    // ordered by construction: a directory listing has no order of its own
    let mut out = std::collections::BTreeSet::new();
    if path.is_file() {
        out.insert(path);
    } else if let Ok(entries) = std::fs::read_dir(&path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Ok(r) = p.strip_prefix(root) {
                    out.extend(files_under(root, &r.display().to_string(), ext));
                }
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| ext.contains(&x))
            {
                out.insert(p);
            }
        }
    }
    out.into_iter().collect()
}

/// The names the independent code must not spell.
pub fn catalogue_names(catalogue: &AdvisorCatalogue) -> Vec<String> {
    let mut names = Vec::new();
    for a in &catalogue.advisors {
        names.push(a.id.clone());
        names.push(a.adapter.clone());
        names.extend(a.executable.clone());
        names.extend(a.provider.clone());
        names.extend(a.vendor.clone());
        names.extend(a.model.clone());
    }
    if let Some(p) = &catalogue.peers {
        names.push(p.adapter.clone());
    }
    crate::order::canonical(&mut names);
    names.dedup();
    names
}

/// String literals of a source naming any of `names`: `"name"` or `'name'`, exactly.
pub fn literals_naming(text: &str, names: &[String]) -> Vec<String> {
    let mut hits = Vec::new();
    for n in names {
        for quote in ['"', '\''] {
            let needle = format!("{quote}{n}{quote}");
            if text.contains(&needle) && !hits.contains(n) {
                hits.push(n.clone());
            }
        }
    }
    hits
}

/// Run every check.
pub fn run(
    root: &Path,
    catalogue: &AdvisorCatalogue,
    refs: &References,
    loaded: &Loaded,
) -> Vec<ReasoningFinding> {
    let mut out = Vec::new();
    let mut push = |check: &str, subject: String, message: String| {
        out.push(ReasoningFinding {
            check: check.into(),
            subject,
            message,
        })
    };

    for d in catalogue.diagnostics(refs) {
        push("catalogue", "share/advisors.yaml".into(), d);
    }

    if root.join(ADAPTER_DIR).is_dir() {
        let mut adapters: Vec<&str> = catalogue
            .advisors
            .iter()
            .map(|a| a.adapter.as_str())
            .collect();
        if let Some(p) = &catalogue.peers {
            adapters.push(&p.adapter);
        }
        for adapter in adapters {
            let module = format!("{ADAPTER_DIR}/{adapter}.mjs");
            if !root.join(&module).is_file() {
                push(
                    "adapters",
                    module.clone(),
                    format!("the catalogue names adapter '{adapter}' and no transport module {module} implements it"),
                );
            }
        }
    }

    let names = catalogue_names(catalogue);
    for rel in INDEPENDENT_SOURCES {
        for file in files_under(root, rel, &["rs", "mjs"]) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for n in literals_naming(&text, &names) {
                push(
                    "coupling",
                    file.strip_prefix(root).unwrap_or(&file).display().to_string(),
                    format!("provider-independent code names '{n}'; it must reach advisors through the catalogue"),
                );
            }
        }
    }

    let credentials: Vec<&str> = refs
        .models
        .vendors
        .iter()
        .filter_map(|v| v.credential_env.as_deref())
        .collect();
    let mut ci_files = files_under(root, ".github/workflows", &["yml", "yaml"]);
    ci_files.extend(files_under(root, ".ai/repo/ci/gates.yaml", &["yaml"]));
    for file in ci_files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for var in &credentials {
            if text.contains(var) {
                push(
                    "ci",
                    file.strip_prefix(root)
                        .unwrap_or(&file)
                        .display()
                        .to_string(),
                    format!(
                        "CI names the model credential {var}; no gate may depend on a live model"
                    ),
                );
            }
        }
    }

    let mut phrases: Vec<String> = catalogue
        .forbidden_claims
        .iter()
        .map(|p| p.to_lowercase())
        .collect();
    for a in &catalogue.advisors {
        let t = a.title.to_lowercase();
        phrases.push(format!("requires {t}"));
        phrases.push(format!("{t} is required"));
    }
    let mut docs = files_under(root, "docs", &["md"]);
    docs.extend(files_under(root, "site/content", &["md"]));
    docs.extend(files_under(root, ".ai/repo", &["md"]));
    docs.push(root.join("README.md"));
    for file in docs {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let lower = text.to_lowercase();
        for p in &phrases {
            if lower.contains(p.as_str()) {
                push(
                    "claims",
                    file.strip_prefix(root)
                        .unwrap_or(&file)
                        .display()
                        .to_string(),
                    format!("asserts '{p}', a claim the reasoning design refuses"),
                );
            }
        }
    }

    for u in &loaded.unreadable {
        push(
            "records",
            u.clone(),
            "a reasoning record that does not parse".into(),
        );
    }
    for r in &loaded.records {
        match &r.body {
            ReasoningBody::Consultation(c) => {
                let known = catalogue.get(&c.advisor).is_some() || c.advisor.starts_with("peer:");
                if !known {
                    push(
                        "records",
                        r.id.clone(),
                        format!(
                            "names advisor '{}', which the catalogue does not declare",
                            c.advisor
                        ),
                    );
                }
                match loaded.get(&c.plan).map(|p| &p.body) {
                    Some(ReasoningBody::Plan(p)) => {
                        if !p.plan.selected.iter().any(|s| s.advisor == c.advisor) {
                            push(
                                "records",
                                r.id.clone(),
                                format!(
                                    "claims advisor '{}', which plan {} did not select",
                                    c.advisor, c.plan
                                ),
                            );
                        }
                    }
                    _ => push(
                        "records",
                        r.id.clone(),
                        format!("cites plan {}, which is not recorded", c.plan),
                    ),
                }
            }
            ReasoningBody::Conclusion(k) => {
                let mut computed: Vec<String> = Vec::new();
                for id in &k.input.consultations {
                    match loaded.get(id).map(|x| &x.body) {
                        Some(ReasoningBody::Consultation(c))
                            if c.status == ConsultationStatus::Completed =>
                        {
                            if !computed.contains(&c.advisor) {
                                computed.push(c.advisor.clone());
                            }
                        }
                        _ => push(
                            "records",
                            r.id.clone(),
                            format!("cites {id}, which is not a completed consultation"),
                        ),
                    }
                }
                crate::order::canonical(&mut computed);
                if computed != k.reviewed_by || k.independent_review_count != k.reviewed_by.len() {
                    push(
                        "records",
                        r.id.clone(),
                        format!(
                            "claims review by [{}] ({}), and the consultations it cites show [{}]",
                            k.reviewed_by.join(", "),
                            k.independent_review_count,
                            computed.join(", ")
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    out
}

/// The checks, by name, in the order [`run`] performs them.
pub const CHECKS: &[&str] = &[
    "catalogue",
    "adapters",
    "coupling",
    "ci",
    "claims",
    "records",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_is_a_name_and_a_word_inside_prose_is_not() {
        let names = vec!["alpha".to_string()];
        assert_eq!(literals_naming("let x = \"alpha\";", &names), ["alpha"]);
        assert_eq!(
            literals_naming("import('./alpha.mjs')", &names),
            Vec::<String>::new()
        );
        assert_eq!(
            literals_naming("// the alpha advisor", &names),
            Vec::<String>::new()
        );
    }

    /// The anti-coupling proof for this crate: the shipped catalogue's names appear in no
    /// provider-independent source file as a literal.
    #[test]
    fn the_reasoning_core_names_no_advisor_of_the_shipped_catalogue() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let share = root.join("share");
        let catalogue = AdvisorCatalogue::load(&share).unwrap();
        assert!(
            !catalogue.advisors.is_empty(),
            "the shipped catalogue declares advisors"
        );
        let names = catalogue_names(&catalogue);
        let mut scanned = 0;
        for rel in INDEPENDENT_SOURCES {
            for file in files_under(&root, rel, &["rs", "mjs"]) {
                scanned += 1;
                let text = std::fs::read_to_string(&file).unwrap();
                assert_eq!(
                    literals_naming(&text, &names),
                    Vec::<String>::new(),
                    "{} names an advisor",
                    file.display()
                );
            }
        }
        assert!(
            scanned >= 7,
            "the scan reached the module ({scanned} files)"
        );
    }
}

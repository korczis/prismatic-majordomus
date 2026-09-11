//! The canonicality audit: every capability has one canonical source, everything else
//! is derived from it, and a second hand-kept representation is a defect. The audit
//! reads the knowledge model — capability nodes with their declaration files and
//! exposures, artifact nodes with what they were derived from — and the tree, and names:
//!
//! - **orphan projections**: a generated artifact the manifest declares with no source it
//!   was derived from;
//! - **undeclared generated files**: a tracked file in a generated tree that no plan
//!   declares;
//! - **missing artifacts**: a declared artifact git does not track;
//! - **suspected mirrors**: a hand-written file that lists many capability identifiers,
//!   which is what a registry copied by hand looks like;
//! - **expired exceptions**: a typed exception past its date.
//!
//! The metric is the manual maintenance surface (MMS) of a capability: how many files a
//! person must touch to change it. Its declaration counts as one; every hand-written
//! file that names it counts as one more; generated files count as none. The
//! architecture is complete when MMS is one.
//!
//! An exception is typed and time-limited: `id`, `reason`, `owner`, `expires`,
//! `validation`. It is read from the knowledge section beside the baseline and shown
//! wherever the audit is.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::metadata::yaml;

use super::baseline::Baseline;
use super::model::{GapCategory, KnowledgeModel};
use super::extract::ExtractionContext;
use super::{Adjacency, Inputs};

/// The exceptions file, under the knowledge section.
pub const EXCEPTIONS_FILE: &str = "exceptions.yaml";

/// The schema the exceptions file declares.
pub const EXCEPTIONS_SCHEMA: &str = "majordomus/knowledge-exceptions/v1";

/// A file that names this many distinct capability ids by hand is a suspected mirror.
pub const MIRROR_THRESHOLD: usize = 6;

/// One typed, time-limited exception to a canonicality rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityException")]
pub struct Exception {
    /// The violation id it excepts.
    pub id: String,
    /// Why the violation stands for now.
    pub reason: String,
    /// Who answers for it.
    pub owner: String,
    /// The day it expires, `YYYY-MM-DD`; after it, the violation counts again.
    pub expires: String,
    /// How to tell the exception is no longer needed.
    pub validation: String,
}

/// The exceptions file, typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "CanonicalityExceptions")]
pub struct Exceptions {
    /// `majordomus/knowledge-exceptions/v1`.
    #[serde(default)]
    pub schema: String,
    /// The exceptions.
    #[serde(default)]
    pub exceptions: Vec<Exception>,
}

impl Exceptions {
    /// Read the exceptions at `path`; an absent file is no exception.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(crate::error::Error::Io {
                    path: path.to_path_buf(),
                    source: e,
                })
            }
        };
        let value = serde_json::Value::Object(
            yaml::parse_mapping(&text).map_err(|r| super::baseline::contract(path, r))?,
        );
        let (value, _) = super::migrate::migrate(super::migrate::Family::Exceptions, value)
            .map_err(|r| super::baseline::contract(path, r))?;
        let out: Exceptions =
            serde_json::from_value(value).map_err(|e| super::baseline::contract(path, e))?;
        for e in &out.exceptions {
            if !is_date(&e.expires) {
                return Err(super::baseline::contract(
                    path,
                    format!("exception {} expires `{}`, which is not a YYYY-MM-DD date", e.id, e.expires),
                ));
            }
        }
        Ok(out)
    }
}

/// Is a string a `YYYY-MM-DD` date?
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit())
}

/// Today, `YYYY-MM-DD`, from the clock; a test passes its own.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil_from_days((secs / 86_400) as i64)
}

/// Howard Hinnant's days-to-civil, for a date with no dependency.
fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// One surface a capability is projected on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalitySurface")]
pub struct Surface {
    /// `mcp`, `http`, `cli`, `openapi`, `docs`, `cockpit`, `benchmark`, `site`.
    pub name: String,
    /// What it is on that surface.
    pub detail: String,
    /// Derived from the canonical source by the executable: always true here, listed so
    /// that a reader sees the derivation rather than takes it on faith.
    pub derived: bool,
}

/// Where a capability's id appears by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityMention")]
pub struct Mention {
    /// The file.
    pub path: String,
    /// The line.
    pub line: usize,
    /// The line's text, trimmed.
    pub text: String,
}

/// The canonicality of one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityCapabilityCanonicality")]
pub struct CapabilityCanonicality {
    /// The capability id.
    pub id: String,
    /// The file it is declared in: the one canonical source.
    pub canonical_source: String,
    /// The surfaces it is projected on.
    pub surfaces: Vec<Surface>,
    /// The hand-written files that name it, apart from its declaration.
    pub mentions: Vec<Mention>,
    /// The manual maintenance surface: one for the declaration plus one per hand-written
    /// file naming it.
    pub mms: usize,
    /// `pass` when MMS is one, `review` otherwise.
    pub verdict: String,
}

/// One violation the audit found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityViolation")]
pub struct Violation {
    /// The id, stable across runs: `<class>:<subject>`.
    pub id: String,
    /// `orphan_projection`, `undeclared_generated`, `missing_artifact`, `suspected_mirror`
    /// or `expired_exception`.
    pub class: String,
    /// What it is about.
    pub subject: String,
    /// Why it is a violation.
    pub reason: String,
    /// What to do.
    pub remedy: String,
    /// The baseline tolerates it.
    pub tolerated: bool,
    /// An active exception covers it, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excepted_by: Option<String>,
}

/// An exception with its status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityExceptionStatus")]
pub struct ExceptionStatus {
    /// The exception.
    #[serde(flatten)]
    pub exception: Exception,
    /// `active`, `expired` or `unused` (no violation carries its id).
    pub status: String,
}

/// The audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CanonicalityAudit")]
pub struct Audit {
    /// `majordomus/canonicality-audit/v1`.
    pub schema: String,
    /// The day the audit ran, for the exceptions.
    pub date: String,
    /// Every capability.
    pub capabilities: Vec<CapabilityCanonicality>,
    /// Every violation.
    pub violations: Vec<Violation>,
    /// The exceptions, with status.
    pub exceptions: Vec<ExceptionStatus>,
    /// The mean manual maintenance surface over the capabilities, times 100 (so that the
    /// number is an integer and the ideal is 100).
    pub mms_centi: usize,
    /// The number of capabilities whose MMS is one.
    pub canonical: usize,
    /// `pass` when no violation counts, `fail` otherwise.
    pub verdict: String,
    /// One line.
    pub summary: String,
}

impl Audit {
    /// Did the audit pass?
    pub fn passed(&self) -> bool {
        self.verdict == "pass"
    }
}

/// The schema the audit carries.
pub const AUDIT_SCHEMA: &str = "majordomus/canonicality-audit/v1";

/// The derived trees the repository declares in `.gitattributes` with `merge=derived`:
/// the one place the repository says which files are written by a generator and merged
/// by regeneration. Read as patterns; `**` crosses directories, `*` does not.
pub fn derived_patterns(ctx: &ExtractionContext<'_>) -> Vec<String> {
    let Some(text) = ctx.read(".gitattributes") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') || l.is_empty() {
                return None;
            }
            let mut parts = l.split_whitespace();
            let pattern = parts.next()?;
            parts.any(|a| a == "merge=derived").then(|| pattern.to_string())
        })
        .collect()
}

/// Does a `.gitattributes` pattern match a path? `**` crosses directories, `*` does not,
/// and a pattern without a wildcard names one path.
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    fn go(p: &[char], s: &[char]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => {
                // `**` then optionally a slash: any run, directories included
                let rest = if p.get(2) == Some(&'/') { &p[3..] } else { &p[2..] };
                (0..=s.len()).any(|i| go(rest, &s[i..]))
            }
            Some('*') => (0..=s.len())
                .take_while(|&i| i == 0 || s[i - 1] != '/')
                .any(|i| go(&p[1..], &s[i..])),
            Some(c) => s.first() == Some(c) && go(&p[1..], &s[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = path.chars().collect();
    go(&p, &s)
}

/// Is a path one a person writes: not generated, not a test, not the baseline?
fn is_hand_written(path: &str, generated: &BTreeSet<&str>, generated_dirs: &[String]) -> bool {
    if generated.contains(path) {
        return false;
    }
    if generated_dirs.iter().any(|d| pattern_matches(d, path)) {
        return false;
    }
    if path.starts_with("tests/") || path.starts_with("test/") || path.contains("/tests/") || path.contains("/fixtures/") {
        return false;
    }
    if path.ends_with(".lock") || path.ends_with(".png") || path.ends_with(".svg") {
        return false;
    }
    !path.ends_with(super::baseline::FILE)
}

/// Run the audit over a scanned model.
pub fn audit(model: &KnowledgeModel, inputs: &Inputs<'_>, baseline: &Baseline, exceptions: &Exceptions, date: &str) -> Audit {
    let adjacency = Adjacency::of(&model.relations);
    // the generated set: every artifact of the manifest, and every directory they live in
    let artifacts: Vec<&super::model::Node> = model.nodes.iter().filter(|n| n.kind == "artifact").collect();
    let generated: BTreeSet<&str> = artifacts.iter().filter_map(|n| n.source.as_deref()).collect();
    let ctx = inputs.context();
    // the derived trees: what `.gitattributes` declares as merged by regeneration, which
    // is the repository's own statement of what a generator writes; plus every tree the
    // manifest fills completely
    let mut generated_dirs: Vec<String> = derived_patterns(&ctx);
    let manifest_dirs: Vec<String> = generated
        .iter()
        .filter_map(|p| p.rsplit_once('/').map(|(d, _)| format!("{d}/**")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|d| {
            let prefix = d.trim_end_matches("**");
            inputs
                .tracked
                .iter()
                .filter(|p| p.starts_with(prefix))
                .all(|p| generated.contains(p.as_str()) || p.ends_with("/README.md") || p.ends_with(".gitkeep"))
        })
        .collect();
    for d in manifest_dirs {
        if !generated_dirs.contains(&d) {
            generated_dirs.push(d);
        }
    }
    let mut violations = Vec::new();
    // orphan projections, missing artifacts
    for a in &artifacts {
        let derived = adjacency.outgoing.get(a.id.as_str()).into_iter().flatten().any(|r| r.kind == "derived_from");
        let path = a.source.as_deref().unwrap_or("");
        if !derived {
            violations.push(Violation {
                id: format!("orphan_projection:{path}"),
                class: "orphan_projection".into(),
                subject: a.id.clone(),
                reason: format!("{path} is generated and names no tracked source it is derived from"),
                remedy: "give the generation target a `derived_from` naming a tracked file or directory, and commit both".into(),
                tolerated: false,
                excepted_by: None,
            });
        }
        if !path.is_empty() && !inputs.root.join(path).is_file() {
            violations.push(Violation {
                id: format!("missing_artifact:{path}"),
                class: "missing_artifact".into(),
                subject: a.id.clone(),
                reason: format!("the manifest declares {path} and the tree does not have it"),
                remedy: "run `majordomus generate` and commit what it writes, or drop the target".into(),
                tolerated: false,
                excepted_by: None,
            });
        }
    }
    // undeclared generated files: a file in a tree the executable's manifest fills that
    // the manifest does not declare. A tree another producer fills (`scripts/derive`, the
    // site generator) is derived too, and its own check holds it; it is not audited here
    let manifest_root = format!("{}/", crate::generate::OUT_DIR);
    for p in inputs.tracked.iter() {
        if p.starts_with(&manifest_root)
            && !generated.is_empty()
            && !generated.contains(p.as_str())
            && !p.ends_with("/README.md")
            && !p.ends_with(".gitkeep")
        {
            violations.push(Violation {
                id: format!("undeclared_generated:{p}"),
                class: "undeclared_generated".into(),
                subject: format!("file:{p}"),
                reason: format!("{p} sits in a generated tree and no generation target declares it"),
                remedy: "declare the target that writes it, or move the file out of the generated tree".into(),
                tolerated: false,
                excepted_by: None,
            });
        }
    }
    // capabilities: sources, surfaces, mentions
    let ids: Vec<(String, String)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == "capability")
        .map(|n| (n.id.trim_start_matches("capability:").to_string(), n.id.clone()))
        .collect();
    let mut mentions: BTreeMap<&str, Vec<Mention>> = BTreeMap::new();
    // the ids a file names on listing lines — a table row, a list item, a heading — which
    // is how a registry copied by hand is laid out; a sentence naming an id is prose
    let mut per_file: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let declarations: BTreeSet<String> = model
        .nodes
        .iter()
        .filter(|n| n.kind == "capability")
        .filter_map(|n| n.source.clone())
        .collect();
    // the ids grouped by their module word, so that a line is held against the ids of
    // the modules it names and not against every id of the registry
    let mut by_module: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, _) in &ids {
        let module = id.split('.').next().unwrap_or(id.as_str());
        by_module.entry(module).or_default().push(id.as_str());
    }
    for p in inputs.tracked.iter() {
        if !is_hand_written(p, &generated, &generated_dirs) || declarations.contains(p) {
            continue;
        }
        if !is_text_path(p) {
            continue;
        }
        let Some(text) = ctx.read(p) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if !line.contains('.') {
                continue;
            }
            for (module, module_ids) in &by_module {
                if !line.contains(module) {
                    continue;
                }
                for id in module_ids {
                    if contains_word(line, id) {
                        mentions.entry(id).or_default().push(Mention {
                            path: p.clone(),
                            line: i + 1,
                            text: line.trim().chars().take(120).collect(),
                        });
                        if is_prose_path(p) && is_listing_line(line) {
                            per_file.entry(p.as_str()).or_default().insert(id);
                        }
                    }
                }
            }
        }
    }
    let mut capabilities = Vec::new();
    for (id, node_id) in &ids {
        let n = model.node(node_id).expect("listed from the model");
        let mut surfaces = Vec::new();
        for c in &n.claims {
            let name = match c.predicate.as_str() {
                "mcp_tool" => "mcp",
                "mcp_resource" => "mcp",
                "http_route" => "http",
                "cli_path" => "cli",
                _ => continue,
            };
            surfaces.push(Surface {
                name: name.into(),
                detail: c.value.as_str().unwrap_or("").to_string(),
                derived: true,
            });
        }
        if surfaces.iter().any(|s| s.name == "http") {
            surfaces.push(Surface { name: "openapi".into(), detail: "operation from the descriptor".into(), derived: true });
        }
        surfaces.push(Surface { name: "docs".into(), detail: "docs/generated/registry.json and the site's registry pages".into(), derived: true });
        surfaces.push(Surface { name: "cockpit".into(), detail: n.route.clone().unwrap_or_default(), derived: true });
        if inputs.registry.cases(id).is_some() {
            surfaces.push(Surface { name: "benchmark".into(), detail: "cases from the input type".into(), derived: true });
        }
        let m: Vec<Mention> = mentions.remove(id.as_str()).unwrap_or_default();
        let files: BTreeSet<&str> = m.iter().map(|x| x.path.as_str()).collect();
        let mms = 1 + files.len();
        capabilities.push(CapabilityCanonicality {
            id: id.clone(),
            canonical_source: n.source.clone().unwrap_or_default(),
            surfaces,
            mentions: m,
            mms,
            verdict: if mms == 1 { "pass".into() } else { "review".into() },
        });
    }
    // suspected mirrors
    for (path, named) in &per_file {
        if named.len() >= MIRROR_THRESHOLD {
            violations.push(Violation {
                id: format!("suspected_mirror:{path}"),
                class: "suspected_mirror".into(),
                subject: format!("file:{path}"),
                reason: format!("{path} lists {} capability identifiers by hand in tables or lists, which is what a registry copied by hand looks like", named.len()),
                remedy: "derive the listing from the registry (a generation target, or a page over docs/generated/registry.json), or except it with a reason and a date".into(),
                tolerated: false,
                excepted_by: None,
            });
        }
    }
    // exceptions and tolerance
    let mut statuses = Vec::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for e in &exceptions.exceptions {
        let expired = e.expires.as_str() < date;
        if expired {
            violations.push(Violation {
                id: format!("expired_exception:{}", e.id),
                class: "expired_exception".into(),
                subject: e.id.clone(),
                reason: format!("the exception for {} expired on {} (owner {})", e.id, e.expires, e.owner),
                remedy: format!("fix it, or renew the exception with a reason: {}", e.validation),
                tolerated: false,
                excepted_by: None,
            });
        }
        statuses.push((e, expired));
    }
    for v in &mut violations {
        v.tolerated = baseline.tolerates_violation(&format!("canonicality:{}", v.id));
        if let Some((e, _)) = statuses.iter().find(|(e, expired)| !expired && e.id == v.id) {
            v.excepted_by = Some(e.id.clone());
            used.insert(e.id.as_str());
        }
    }
    let exceptions_out = statuses
        .iter()
        .map(|(e, expired)| ExceptionStatus {
            exception: (*e).clone(),
            status: if *expired {
                "expired".into()
            } else if used.contains(e.id.as_str()) {
                "active".into()
            } else {
                "unused".into()
            },
        })
        .collect();
    violations.sort_by(|a, b| a.id.cmp(&b.id));
    let counting = violations.iter().filter(|v| !v.tolerated && v.excepted_by.is_none()).count();
    let canonical = capabilities.iter().filter(|c| c.mms == 1).count();
    let mms_centi = if capabilities.is_empty() {
        100
    } else {
        capabilities.iter().map(|c| c.mms * 100).sum::<usize>() / capabilities.len()
    };
    let summary = format!(
        "{} capabilities, {} canonical (MMS 1), mean MMS {}.{:02}; {} violation(s), {} counting",
        capabilities.len(),
        canonical,
        mms_centi / 100,
        mms_centi % 100,
        violations.len(),
        counting
    );
    Audit {
        schema: AUDIT_SCHEMA.into(),
        date: date.into(),
        capabilities,
        violations,
        exceptions: exceptions_out,
        mms_centi,
        canonical,
        verdict: if counting == 0 { "pass".into() } else { "fail".into() },
        summary,
    }
}

/// Add the audit's violations to the model as canonicality gaps, so that the check and
/// the baseline see them with everything else. Reads the exceptions file beside the
/// baseline; a file that does not parse is a diagnostic, not a crash.
pub fn annotate(model: &mut KnowledgeModel, inputs: &Inputs<'_>, baseline: &Baseline) {
    let exceptions = match Exceptions::load(&inputs.exceptions_path()) {
        Ok(e) => e,
        Err(e) => {
            model.diagnostics.push(crate::model::Diagnostic::warning(
                "knowledge_exceptions_unreadable",
                Some(format!("{}/{EXCEPTIONS_FILE}", inputs.section)),
                e.to_string(),
            ));
            Exceptions::default()
        }
    };
    let a = audit(model, inputs, baseline, &exceptions, &today());
    for v in a.violations.iter().filter(|v| v.excepted_by.is_none()) {
        let id = format!("canonicality:{}", v.id);
        if model.gaps.iter().any(|g| g.id == id) {
            continue;
        }
        model.gaps.push(super::model::Gap {
            id,
            category: GapCategory::Canonicality,
            subject: v.subject.clone(),
            reason: v.reason.clone(),
            remedy: v.remedy.clone(),
        });
    }
    model.gaps.sort_by(|a, b| a.id.cmp(&b.id));
}

/// Is a path a document a person reads, as opposed to code or typed data?
fn is_prose_path(p: &str) -> bool {
    p.ends_with(".md") || p.ends_with(".markdown") || p.ends_with(".txt")
}

/// Is a line laid out as an entry of a listing: a table row, a list item, a heading?
fn is_listing_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('|')
        || t.starts_with("- ")
        || t.starts_with("* ")
        || t.starts_with('#')
        || t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains(". ")
}

fn is_text_path(p: &str) -> bool {
    let ext = p.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "md" | "rs" | "sh" | "yaml" | "yml" | "toml" | "json" | "txt" | "html" | "tera" | "just" | "awk" | "ts" | "js" | "css"
    ) || !p.contains('.')
}

/// Does a line contain `id` as a whole word — not as part of a longer dotted name?
fn contains_word(line: &str, id: &str) -> bool {
    let mut from = 0;
    while let Some(at) = line[from..].find(id) {
        let start = from + at;
        let end = start + id.len();
        let before = line[..start].chars().last();
        let after = line[end..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        if boundary(before) && boundary(after) {
            return true;
        }
        from = end;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mention_is_a_whole_dotted_word() {
        assert!(contains_word("run `objects.get` first", "objects.get"));
        assert!(!contains_word("objects.getter", "objects.get"));
        assert!(!contains_word("my.objects.get", "objects.get"));
        assert!(contains_word("objects.get", "objects.get"));
    }

    #[test]
    fn gitattributes_patterns_match_the_way_git_matches_them() {
        assert!(pattern_matches("docs/generated/**", "docs/generated/modules/x.md"));
        assert!(!pattern_matches("docs/generated/**", "docs/other.md"));
        assert!(pattern_matches("share/schemas/**/*.schema.json", "share/schemas/majordomus/x/y.schema.json"));
        assert!(!pattern_matches("share/schemas/**/*.schema.json", "share/schemas/majordomus/x/y.proto"));
        assert!(pattern_matches("AGENTS.md", "AGENTS.md"));
        assert!(!pattern_matches("AGENTS.md", "docs/AGENTS.md"));
        assert!(pattern_matches("docs/*.md", "docs/CLI.md"));
        assert!(!pattern_matches("docs/*.md", "docs/sub/CLI.md"));
    }

    #[test]
    fn a_listing_line_is_a_table_row_a_list_item_or_a_heading() {
        assert!(is_listing_line("| `objects.get` | one object |"));
        assert!(is_listing_line("- `objects.get`"));
        assert!(is_listing_line("## objects.get"));
        assert!(is_listing_line("1. objects.get"));
        assert!(!is_listing_line("The page calls objects.get once."));
    }

    #[test]
    fn dates_are_validated_and_today_is_a_date() {
        assert!(is_date("2026-09-07"));
        assert!(!is_date("2026-9-7"));
        assert!(is_date(&today()));
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(20_703), "2026-09-07");
    }
}

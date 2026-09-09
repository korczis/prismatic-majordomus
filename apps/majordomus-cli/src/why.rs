//! The Why catalogue: the operational moments this tool is a response to, the audiences
//! that recognise them and the areas they fall under, read from the index and answered as
//! one immutable typed model.
//!
//! Nothing here discovers or parses a file. The moments, audiences and areas are ordinary
//! declarative objects of the layer — kinds `moment`, `audience` and `area` in
//! `share/kinds.yaml`, discovered by the source classes in the repository's
//! `sources.yaml`, and already validated against their JSON Schemas by the time the index
//! holds them. This module projects that validated metadata into typed records, resolves
//! every reference the records make, derives every relation nobody authored, and answers
//! queries over the result.
//!
//! Built once, at the moment a [`crate::capability::Context`] is composed, and shared by
//! every projection: the command line, the HTTP routes, the MCP tools, the derived graph
//! and the site's dataset are readers of this one value. A request never rebuilds it.
//!
//! Two directions are kept apart on purpose. **Authored** is what a file says: its hook,
//! its summary, the audiences and areas it belongs to, its signals, its examples, and the
//! things of this repository it names. **Derived** is everything else — the route, the
//! members of an audience, the moments that answer a claim, the responsibilities a moment
//! touches, the reverse of `related`, every count and every facet — and none of it may be
//! written into a source file. The schemas refuse the derived keys, and this module is
//! where they come from instead.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::capability::CapabilityRegistry;
use crate::index::Index;
use crate::model::{Object, Severity};

/// The kind of an operational moment.
pub const MOMENT: &str = "moment";
/// The kind of an audience.
pub const AUDIENCE: &str = "audience";
/// The kind of an operational area.
pub const AREA: &str = "area";
/// The section the catalogue is published under.
pub const ROUTE: &str = "/why/";

/// The identities the section's own routes use. A moment called one of these would claim a
/// route the section already owns — `/why/audiences/` is the taxonomy, not a moment — and
/// the collision would show up as a page quietly replaced rather than as an error.
pub const RESERVED: &[&str] = &["audiences", "areas", "graph", "index"];

/// A moment that is complete and public. Draft records are listed as drafts and counted
/// nowhere; deprecated ones keep their route and are listed nowhere.
pub const STABLE: &str = "stable";

// ---------------------------------------------------------------- authored records

/// One observable symptom of a moment: a question a reader can answer about their own
/// week. The questionnaire on the site and the input of `why diagnose` are these and
/// nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Signal {
    /// Unique within the moment; the identity a diagnosis selects by.
    pub id: String,
    /// The symptom, phrased so a reader can say whether it happened to them.
    pub text: String,
}

/// One concrete situation, in one audience, with what happens today and what happens
/// instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Example {
    /// Unique within the moment.
    pub id: String,
    /// The audience this situation belongs to; must name an audience of the catalogue.
    pub audience: String,
    /// One line naming the situation.
    pub title: String,
    /// What happens without the tool.
    pub before: String,
    /// What happens with it.
    pub after: String,
}

/// One operational moment, as its file declares it plus the two facts the file cannot
/// hold: where it came from and where it is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Moment {
    /// The identity, the slug and the file name.
    pub id: String,
    /// The moment as a heading.
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Two or three words for a narrow column, when the record carries one.
    pub short_title: Option<String>,
    /// The first-person line an index shows.
    pub hook: String,
    /// One line: what is actually wrong.
    pub summary: String,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    /// `low`, `medium` or `high`.
    pub severity: String,
    /// `rare`, `occasional`, `common` or `constant`.
    pub frequency: String,
    #[serde(default)]
    /// Presentation order, lowest first.
    pub weight: u32,
    #[serde(default)]
    /// Whether the homepage shows it.
    pub featured: bool,
    /// The audiences that recognise it.
    pub audiences: Vec<String>,
    /// The operational areas it falls under.
    pub areas: Vec<String>,
    #[serde(default)]
    /// The stages of work at which it shows up.
    pub lifecycle: Vec<String>,
    #[serde(default)]
    /// Free tags.
    pub tags: Vec<String>,
    /// The observable symptoms.
    pub signals: Vec<Signal>,
    /// The concrete situations.
    pub examples: Vec<Example>,
    #[serde(default)]
    /// Commands of the tool that answer it.
    pub commands: Vec<String>,
    #[serde(default)]
    /// Capability ids of the executable that answer it.
    pub capabilities: Vec<String>,
    #[serde(default)]
    /// Claims that say what is guaranteed here.
    pub claims: Vec<String>,
    #[serde(default)]
    /// Rules of the effective set that govern it.
    pub doctrines: Vec<String>,
    #[serde(default)]
    /// Use cases that show the way out.
    pub use_cases: Vec<String>,
    #[serde(default)]
    /// Moments explicitly related to this one. The reverse is derived.
    pub related: Vec<String>,
    #[serde(default)]
    /// Other words a reader might search for.
    pub aliases: Vec<String>,

    #[serde(default)]
    /// Derived: `/why/<id>/`. Never authored; the schema refuses a `route` key.
    pub route: String,
    #[serde(default)]
    /// Derived: the repository-relative file the record came from.
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The Markdown body, without its front matter.
    pub body: String,
    #[serde(skip)]
    /// Derived once, when the catalogue is built: everything a reader might remember about
    /// this moment, lower-cased. A query matches against this rather than rebuilding it,
    /// which is what keeps a search over the catalogue independent of how the records are
    /// shaped (`project.derived-once`).
    search: String,
}

/// One audience. Membership is never listed here: it is each moment's `audiences`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Audience {
    /// The identity, the slug and the file name.
    pub id: String,
    /// Who they are.
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Two or three words for a filter chip.
    pub short_title: Option<String>,
    /// One line: the working situation.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One line: how work flows for them.
    pub workflow: Option<String>,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    #[serde(default)]
    /// Presentation order.
    pub weight: u32,
    #[serde(default)]
    /// Free tags.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Derived: `/why/audiences/<id>/`.
    pub route: String,
    #[serde(default)]
    /// Derived: the file it came from.
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The Markdown body.
    pub body: String,
}

/// One operational area. Membership is never listed here either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Area {
    /// The identity, the slug and the file name.
    pub id: String,
    /// The area.
    pub title: String,
    /// One line: what falls under it.
    pub summary: String,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    #[serde(default)]
    /// Presentation order.
    pub weight: u32,
    #[serde(default)]
    /// Free tags.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Derived: `/why/areas/<id>/`.
    pub route: String,
    #[serde(default)]
    /// Derived: the file it came from.
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The Markdown body.
    pub body: String,
}

// The three catalogues are ordered the same way, once. `weight` is the ranking each record
// declares; the canonical order takes it as the rank and ends on the identity, so two
// records that declare the same weight cannot swap places between runs or between machines.

impl crate::order::Ordered for Moment {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id).ranked(i64::from(self.weight))
    }
}

impl crate::order::Ordered for Audience {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id).ranked(i64::from(self.weight))
    }
}

impl crate::order::Ordered for Area {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id).ranked(i64::from(self.weight))
    }
}

// ---------------------------------------------------------------- findings

// How much a finding matters is the layer's own [`Severity`]: `error` is a broken
// catalogue — a reference that resolves to nothing, or two records claiming one identity —
// and `warning` is a record that is valid and does not meet the floor its status promises.
// A second severity vocabulary would be a second noun for one idea.

/// One thing wrong with the catalogue, named where it is, with the nearest candidate when
/// there is one. A finding always carries enough to fix it without searching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// `error` or `warning`.
    pub severity: Severity,
    /// A short category: `unknown_reference`, `duplicate_identity`, `missing_content`, ...
    pub code: String,
    /// The repository-relative file the finding is in.
    pub path: String,
    /// The record's identity, when one record owns the finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The front-matter key the finding is about, when one key owns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// What is wrong, in one sentence.
    pub message: String,
    /// The nearest existing name, when the value looks like a typo of one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_you_mean: Option<String>,
}

// ---------------------------------------------------------------- the catalogue

/// The validated catalogue and every index derived from it. Immutable: built once from an
/// index and a registry, read many times.
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    moments: Vec<Moment>,
    audiences: Vec<Audience>,
    areas: Vec<Area>,
    findings: Vec<Finding>,
    fingerprint: String,
    by_id: BTreeMap<String, usize>,
    by_audience: BTreeMap<String, Vec<usize>>,
    by_area: BTreeMap<String, Vec<usize>>,
    by_tag: BTreeMap<String, Vec<usize>>,
    by_lifecycle: BTreeMap<String, Vec<usize>>,
    by_command: BTreeMap<String, Vec<usize>>,
    by_capability: BTreeMap<String, Vec<usize>>,
    by_claim: BTreeMap<String, Vec<usize>>,
    by_doctrine: BTreeMap<String, Vec<usize>>,
    by_use_case: BTreeMap<String, Vec<usize>>,
    by_signal: BTreeMap<String, usize>,
    backlinks: BTreeMap<String, Vec<String>>,
    responsibilities: BTreeMap<String, Vec<String>>,
}

/// Parse one object's validated metadata into a typed record, and attach the two derived
/// facts the file cannot hold.
fn record<T: for<'de> Deserialize<'de>>(o: &Object) -> Option<T> {
    serde_json::from_value::<T>(o.metadata.clone()).ok()
}

/// Levenshtein distance, for the nearest-candidate hint on an unresolved reference.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The nearest candidate to `value`, when one is close enough to be a plausible typo.
fn nearest<'a>(value: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let limit = (value.chars().count() / 3).max(2);
    candidates
        .map(|c| (distance(value, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c.to_string())
}

impl Catalogue {
    /// Build the catalogue from an index and the registry its capability references
    /// resolve against. Deterministic: the same objects produce the same value, and no
    /// derivation reads a clock or the filesystem.
    pub fn build(index: &Index, registry: &CapabilityRegistry) -> Self {
        let mut c = Catalogue::default();
        let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();

        for o in index.objects.iter() {
            let path = o.provenance.path.clone();
            // The index refuses two objects with one URI, so a duplicate identity here is
            // a duplicate *file name stem* against the id, which the index cannot see.
            if matches!(o.kind.as_str(), MOMENT | AUDIENCE | AREA) {
                if let Some(first) = seen.insert((o.kind.clone(), o.identity.clone()), path.clone())
                {
                    c.findings.push(Finding {
                        severity: Severity::Error,
                        code: "duplicate_identity".into(),
                        path: path.clone(),
                        id: Some(o.identity.clone()),
                        field: Some("id".into()),
                        message: format!(
                            "a second {} claims the identity '{}' (first: {first})",
                            o.kind, o.identity
                        ),
                        did_you_mean: None,
                    });
                }
                c.check_file_name(o);
            }
            match o.kind.as_str() {
                MOMENT => {
                    if let Some(mut m) = record::<Moment>(o) {
                        m.route = format!("{ROUTE}{}/", m.id);
                        m.source = path;
                        m.body = o.body.clone();
                        m.search = m.search_text();
                        c.moments.push(m);
                    }
                }
                AUDIENCE => {
                    if let Some(mut a) = record::<Audience>(o) {
                        a.route = format!("{ROUTE}audiences/{}/", a.id);
                        a.source = path;
                        a.body = o.body.clone();
                        c.audiences.push(a);
                    }
                }
                AREA => {
                    if let Some(mut a) = record::<Area>(o) {
                        a.route = format!("{ROUTE}areas/{}/", a.id);
                        a.source = path;
                        a.body = o.body.clone();
                        c.areas.push(a);
                    }
                }
                _ => {}
            }
        }

        // Weight is the catalogue's own ranking; the canonical order takes it as the rank
        // and breaks its ties by identity, the way every other collection is ordered.
        crate::order::canonical(&mut c.moments);
        crate::order::canonical(&mut c.audiences);
        crate::order::canonical(&mut c.areas);

        c.adopt_diagnostics(index);
        c.index_moments();
        c.resolve(index, registry);
        c.derive_responsibilities(index);
        c.fingerprint = c.compute_fingerprint();
        c
    }

    /// Every diagnostic the index raised about a file of this section. An object the index
    /// refused — two files claiming one identity, malformed front matter, a schema
    /// violation — never reaches the catalogue at all, so without this the catalogue would
    /// report itself valid while the thing it is a catalogue of had been excluded.
    fn adopt_diagnostics(&mut self, index: &Index) {
        let Some(section) = index.repository.sections.get("why") else {
            return;
        };
        for d in index
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
        {
            let Some(path) = d.path.as_deref() else {
                continue;
            };
            if !path.starts_with(section.as_str()) {
                continue;
            }
            self.findings.push(Finding {
                severity: Severity::Error,
                code: d.code.clone(),
                path: path.to_string(),
                id: None,
                field: None,
                message: d.message.clone(),
                did_you_mean: None,
            });
        }
    }

    fn check_file_name(&mut self, o: &Object) {
        let stem = o
            .provenance
            .path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".md"))
            .unwrap_or_default();
        if stem != o.identity {
            self.findings.push(Finding {
                severity: Severity::Error,
                code: "filename_mismatch".into(),
                path: o.provenance.path.clone(),
                id: Some(o.identity.clone()),
                field: Some("id".into()),
                message: format!(
                    "the file is named '{stem}.md' and the record's id is '{}'; the id is the file name and the route",
                    o.identity
                ),
                did_you_mean: Some(format!("{}.md", o.identity)),
            });
        }
    }

    fn index_moments(&mut self) {
        for (i, m) in self.moments.iter().enumerate() {
            self.by_id.insert(m.id.clone(), i);
            for (map, values) in [
                (&mut self.by_audience, &m.audiences),
                (&mut self.by_area, &m.areas),
                (&mut self.by_tag, &m.tags),
                (&mut self.by_lifecycle, &m.lifecycle),
                (&mut self.by_command, &m.commands),
                (&mut self.by_capability, &m.capabilities),
                (&mut self.by_claim, &m.claims),
                (&mut self.by_doctrine, &m.doctrines),
                (&mut self.by_use_case, &m.use_cases),
            ] {
                for v in values {
                    map.entry(v.clone()).or_default().push(i);
                }
            }
            for s in &m.signals {
                self.by_signal.insert(s.id.clone(), i);
            }
            for r in &m.related {
                self.backlinks
                    .entry(r.clone())
                    .or_default()
                    .push(m.id.clone());
            }
        }
        for v in self.backlinks.values_mut() {
            v.sort();
            v.dedup();
        }
    }

    /// Every reference every record makes, resolved against the thing it names. A name
    /// that resolves to nothing is an error with the nearest candidate offered.
    fn resolve(&mut self, index: &Index, registry: &CapabilityRegistry) {
        let audiences: BTreeSet<String> = self.audiences.iter().map(|a| a.id.clone()).collect();
        let areas: BTreeSet<String> = self.areas.iter().map(|a| a.id.clone()).collect();
        let moments: BTreeSet<String> = self.moments.iter().map(|m| m.id.clone()).collect();
        let of_kind = |k: &str| -> BTreeSet<String> {
            index
                .objects
                .iter()
                .filter(|o| o.kind == k)
                .map(|o| o.identity.clone())
                .collect()
        };
        let commands = of_kind("command");
        let claims = of_kind("claim");
        let use_cases = of_kind("use-case");
        // a rule's identity is `<id>@<version>`; a moment names the id, which is what a
        // use case and an application name too
        let rules: BTreeSet<String> = index
            .objects
            .iter()
            .filter(|o| o.kind == "rule")
            .filter_map(|o| o.metadata.get("id").and_then(Value::as_str))
            .map(str::to_string)
            .collect();
        let capabilities: BTreeSet<String> = registry.iter().map(|c| c.id.to_string()).collect();

        let mut findings = Vec::new();
        for m in &self.moments {
            let mut check =
                |field: &str, values: &[String], known: &BTreeSet<String>, what: &str| {
                    for v in values {
                        if !known.contains(v) {
                            findings.push(Finding {
                                severity: Severity::Error,
                                code: "unknown_reference".into(),
                                path: m.source.clone(),
                                id: Some(m.id.clone()),
                                field: Some(field.into()),
                                message: format!("unknown {what} reference: \"{v}\""),
                                did_you_mean: nearest(v, known.iter().map(String::as_str)),
                            });
                        }
                    }
                };
            check("audiences", &m.audiences, &audiences, "audience");
            check("areas", &m.areas, &areas, "area");
            check("related", &m.related, &moments, "moment");
            check("commands", &m.commands, &commands, "command");
            check("capabilities", &m.capabilities, &capabilities, "capability");
            check("claims", &m.claims, &claims, "claim");
            check("doctrines", &m.doctrines, &rules, "rule");
            check("use_cases", &m.use_cases, &use_cases, "use case");

            for e in &m.examples {
                if !audiences.contains(&e.audience) {
                    findings.push(Finding {
                        severity: Severity::Error,
                        code: "unknown_reference".into(),
                        path: m.source.clone(),
                        id: Some(m.id.clone()),
                        field: Some(format!("examples.{}.audience", e.id)),
                        message: format!("unknown audience reference: \"{}\"", e.audience),
                        did_you_mean: nearest(&e.audience, audiences.iter().map(String::as_str)),
                    });
                }
            }
            if RESERVED.contains(&m.id.as_str()) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: "reserved_identity".into(),
                    path: m.source.clone(),
                    id: Some(m.id.clone()),
                    field: Some("id".into()),
                    message: format!(
                        "'{}' is a route this section owns; a moment may not claim it (reserved: {})",
                        m.id,
                        RESERVED.join(", ")
                    ),
                    did_you_mean: None,
                });
            }
            if m.related.contains(&m.id) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: "self_reference".into(),
                    path: m.source.clone(),
                    id: Some(m.id.clone()),
                    field: Some("related".into()),
                    message: "a moment may not be related to itself".into(),
                    did_you_mean: None,
                });
            }
            findings.extend(self.quality(m));
        }
        findings.extend(self.audience_coverage());
        findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then(a.path.cmp(&b.path))
                .then(a.code.cmp(&b.code))
                .then(a.message.cmp(&b.message))
        });
        self.findings.extend(findings);
    }

    /// The quality floors a public record promises. A draft is exempt and says so by
    /// being a draft.
    fn quality(&self, m: &Moment) -> Vec<Finding> {
        if m.status != STABLE {
            return Vec::new();
        }
        let mut out = Vec::new();
        fn fail(out: &mut Vec<Finding>, m: &Moment, field: &str, message: String) {
            out.push(Finding {
                severity: Severity::Warning,
                code: "missing_content".into(),
                path: m.source.clone(),
                id: Some(m.id.clone()),
                field: Some(field.into()),
                message,
                did_you_mean: None,
            });
        }
        if m.signals.is_empty() {
            fail(
                &mut out,
                m,
                "signals",
                "a stable moment carries at least one signal".into(),
            );
        }
        if m.examples.len() < 3 {
            fail(
                &mut out,
                m,
                "examples",
                format!(
                    "a stable moment carries at least three concrete examples; this one has {}",
                    m.examples.len()
                ),
            );
        }
        let audiences: BTreeSet<&str> = m.examples.iter().map(|e| e.audience.as_str()).collect();
        if m.examples.len() >= 3 && audiences.len() < 2 {
            fail(
                &mut out,
                m,
                "examples",
                "the examples of a stable moment cover more than one audience".into(),
            );
        }
        let ids: BTreeSet<&str> = m.examples.iter().map(|e| e.id.as_str()).collect();
        if ids.len() != m.examples.len() {
            out.push(Finding {
                severity: Severity::Error,
                code: "duplicate_identity".into(),
                path: m.source.clone(),
                id: Some(m.id.clone()),
                field: Some("examples".into()),
                message: "two examples of one moment share an id".into(),
                did_you_mean: None,
            });
        }
        let signal_ids: BTreeSet<&str> = m.signals.iter().map(|s| s.id.as_str()).collect();
        if signal_ids.len() != m.signals.len() {
            out.push(Finding {
                severity: Severity::Error,
                code: "duplicate_identity".into(),
                path: m.source.clone(),
                id: Some(m.id.clone()),
                field: Some("signals".into()),
                message: "two signals of one moment share an id".into(),
                did_you_mean: None,
            });
        }
        if m.capabilities.is_empty() && m.commands.is_empty() && m.claims.is_empty() {
            fail(
                &mut out,
                m,
                "capabilities",
                "a stable moment names the mechanism that answers it: a capability, a command or a claim".into(),
            );
        }
        for heading in [
            "## The moment",
            "## Why it happens",
            "## What it does not do",
        ] {
            if !m.body.lines().any(|l| l.trim_end() == heading) {
                fail(
                    &mut out,
                    m,
                    "body",
                    format!("a stable moment's body carries `{heading}`"),
                );
            }
        }
        out
    }

    /// A public audience nobody recognises is a filter that answers nothing.
    fn audience_coverage(&self) -> Vec<Finding> {
        const FLOOR: usize = 3;
        self.audiences
            .iter()
            .filter(|a| a.status == STABLE)
            .filter_map(|a| {
                let n = self
                    .by_audience
                    .get(&a.id)
                    .map(|v| v.iter().filter(|i| self.moments[**i].status == STABLE).count())
                    .unwrap_or(0);
                (n < FLOOR).then(|| Finding {
                    severity: Severity::Warning,
                    code: "thin_audience".into(),
                    path: a.source.clone(),
                    id: Some(a.id.clone()),
                    field: None,
                    message: format!(
                        "a stable audience is named by at least {FLOOR} stable moments; this one is named by {n}"
                    ),
                    did_you_mean: None,
                })
            })
            .collect()
    }

    /// A moment's responsibilities are those of the claims it stands on. Every claim
    /// declares the responsibility it belongs to, so this relation is derived and a
    /// moment never authors it.
    fn derive_responsibilities(&mut self, index: &Index) {
        let of_claim: BTreeMap<&str, &str> = index
            .objects
            .iter()
            .filter(|o| o.kind == "claim")
            .filter_map(|o| {
                let r = o.metadata.get("responsibility").and_then(Value::as_str)?;
                (r != "none").then_some((o.identity.as_str(), r))
            })
            .collect();
        for m in &self.moments {
            let mut r: Vec<String> = m
                .claims
                .iter()
                .filter_map(|c| of_claim.get(c.as_str()).map(|s| s.to_string()))
                .collect();
            r.sort();
            r.dedup();
            if !r.is_empty() {
                self.responsibilities.insert(m.id.clone(), r);
            }
        }
    }

    /// A hash of the catalogue's own sources, in identity order. It depends on nothing
    /// but these files, so two runs over one tree agree even when the rest of the index
    /// has moved between them.
    fn compute_fingerprint(&self) -> String {
        let mut h = Sha256::new();
        for (kind, id, source, body) in self
            .moments
            .iter()
            .map(|m| (MOMENT, &m.id, &m.source, &m.body))
            .chain(
                self.audiences
                    .iter()
                    .map(|a| (AUDIENCE, &a.id, &a.source, &a.body)),
            )
            .chain(self.areas.iter().map(|a| (AREA, &a.id, &a.source, &a.body)))
        {
            h.update(kind.as_bytes());
            h.update([0]);
            h.update(id.as_bytes());
            h.update([0]);
            h.update(source.as_bytes());
            h.update([0]);
            h.update(body.as_bytes());
            h.update(*b"\n");
        }
        format!("{:x}", h.finalize())
    }
}

// ---------------------------------------------------------------- reading it

/// What a listing is narrowed by. Every field is optional and they compose as a
/// conjunction; the values a caller may pass are the facets the catalogue reports, never a
/// list written anywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Query {
    #[serde(default)]
    /// Only moments this audience recognises.
    pub audience: Option<String>,
    #[serde(default)]
    /// Only moments in this operational area.
    pub area: Option<String>,
    #[serde(default)]
    /// Only moments carrying this tag.
    pub tag: Option<String>,
    #[serde(default)]
    /// Only moments of this severity.
    pub severity: Option<String>,
    #[serde(default)]
    /// Only moments of this frequency.
    pub frequency: Option<String>,
    #[serde(default)]
    /// Only moments at this stage of work.
    pub lifecycle: Option<String>,
    #[serde(default)]
    /// Only moments naming this capability of the executable.
    pub capability: Option<String>,
    #[serde(default)]
    /// Only moments naming this command.
    pub command: Option<String>,
    #[serde(default)]
    /// Only moments the homepage features.
    pub featured: Option<bool>,
    #[serde(default)]
    /// Only moments of this status. Absent means the public ones (`stable`); pass `any`
    /// for everything the catalogue holds.
    pub status: Option<String>,
    #[serde(default)]
    /// Case-insensitive text, matched against the identity, the titles, the hook, the
    /// summary, the tags, the aliases, the signals, the examples and the body.
    pub q: Option<String>,
}

impl Catalogue {
    /// Every moment the catalogue holds, in presentation order, whatever its status.
    pub fn all(&self) -> &[Moment] {
        &self.moments
    }
    /// Every audience, in presentation order.
    pub fn audiences(&self) -> &[Audience] {
        &self.audiences
    }
    /// Every area, in presentation order.
    pub fn areas(&self) -> &[Area] {
        &self.areas
    }
    /// One moment by id.
    pub fn moment(&self, id: &str) -> Option<&Moment> {
        self.by_id.get(id).map(|i| &self.moments[*i])
    }
    /// One audience by id.
    pub fn audience(&self, id: &str) -> Option<&Audience> {
        self.audiences.iter().find(|a| a.id == id)
    }
    /// One area by id.
    pub fn area(&self, id: &str) -> Option<&Area> {
        self.areas.iter().find(|a| a.id == id)
    }
    /// Every finding: unresolved references, duplicate identities, and records that do
    /// not meet the floor their status promises.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
    /// How many findings are errors.
    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }
    /// The hash of the catalogue's own sources.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// The responsibilities a moment touches, derived from the claims it names.
    pub fn responsibilities(&self, id: &str) -> &[String] {
        self.responsibilities
            .get(id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    /// The moments that name this one in their `related`. Never authored.
    pub fn backlinks(&self, id: &str) -> &[String] {
        self.backlinks.get(id).map(Vec::as_slice).unwrap_or(&[])
    }
    /// The moments that name this thing, whatever kind of thing it is. This is the one
    /// reverse index every "the moments this answers" list is read from.
    pub fn naming(&self, field: &str, value: &str) -> Vec<&Moment> {
        let map = match field {
            "audience" => &self.by_audience,
            "area" => &self.by_area,
            "tag" => &self.by_tag,
            "lifecycle" => &self.by_lifecycle,
            "command" => &self.by_command,
            "capability" => &self.by_capability,
            "claim" => &self.by_claim,
            "doctrine" => &self.by_doctrine,
            "use_case" => &self.by_use_case,
            _ => return Vec::new(),
        };
        map.get(value)
            .map(|v| v.iter().map(|i| &self.moments[*i]).collect())
            .unwrap_or_default()
    }
    /// The moment a signal belongs to.
    pub fn signal_owner(&self, signal: &str) -> Option<&Moment> {
        self.by_signal.get(signal).map(|i| &self.moments[*i])
    }

    /// The moments a query selects, in presentation order.
    pub fn select(&self, q: &Query) -> Vec<&Moment> {
        let status = q.status.as_deref().unwrap_or(STABLE);
        let text = q.q.as_ref().map(|s| s.to_lowercase());
        self.moments
            .iter()
            .filter(|m| status == "any" || m.status == status)
            .filter(|m| q.audience.as_ref().is_none_or(|v| m.audiences.contains(v)))
            .filter(|m| q.area.as_ref().is_none_or(|v| m.areas.contains(v)))
            .filter(|m| q.tag.as_ref().is_none_or(|v| m.tags.contains(v)))
            .filter(|m| q.severity.as_deref().is_none_or(|v| m.severity == v))
            .filter(|m| q.frequency.as_deref().is_none_or(|v| m.frequency == v))
            .filter(|m| q.lifecycle.as_ref().is_none_or(|v| m.lifecycle.contains(v)))
            .filter(|m| {
                q.capability
                    .as_ref()
                    .is_none_or(|v| m.capabilities.contains(v))
            })
            .filter(|m| q.command.as_ref().is_none_or(|v| m.commands.contains(v)))
            .filter(|m| q.featured.is_none_or(|v| m.featured == v))
            .filter(|m| text.as_deref().is_none_or(|t| m.matches(t)))
            .collect()
    }
}

impl Moment {
    /// The text a search reads: everything a reader might remember about this moment.
    pub fn search_text(&self) -> String {
        let mut s = String::new();
        for part in [
            self.id.as_str(),
            self.title.as_str(),
            self.short_title.as_deref().unwrap_or(""),
            self.hook.as_str(),
            self.summary.as_str(),
            self.body.as_str(),
        ] {
            s.push_str(part);
            s.push('\n');
        }
        for list in [&self.tags, &self.aliases, &self.audiences, &self.areas] {
            for v in list {
                s.push_str(v);
                s.push('\n');
            }
        }
        for sig in &self.signals {
            s.push_str(&sig.text);
            s.push('\n');
        }
        for e in &self.examples {
            for part in [&e.title, &e.before, &e.after] {
                s.push_str(part);
                s.push('\n');
            }
        }
        s.to_lowercase()
    }

    /// Does the lower-cased needle occur anywhere in the search text? The text was built
    /// once, when the catalogue was; this is a substring test and nothing else.
    pub fn matches(&self, needle: &str) -> bool {
        self.search.contains(needle)
    }

    /// The name a narrow column shows.
    pub fn label(&self) -> &str {
        self.short_title.as_deref().unwrap_or(&self.title)
    }
}

// ---------------------------------------------------------------- facets

/// One selectable value of one facet, with how many public moments carry it. Every filter
/// a reader is offered is one of these; nothing anywhere lists the values by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FacetValue {
    /// The value as a moment declares it.
    pub value: String,
    /// The name to show; the value itself when the taxonomy holds no title for it.
    pub label: String,
    /// How many public moments carry it.
    pub count: usize,
}

/// The filters the catalogue offers, derived from the records rather than declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Facets {
    /// By audience.
    pub audiences: Vec<FacetValue>,
    /// By operational area.
    pub areas: Vec<FacetValue>,
    /// By tag.
    pub tags: Vec<FacetValue>,
    /// By severity.
    pub severities: Vec<FacetValue>,
    /// By frequency.
    pub frequencies: Vec<FacetValue>,
    /// By stage of work.
    pub lifecycle: Vec<FacetValue>,
    /// By capability of the executable.
    pub capabilities: Vec<FacetValue>,
    /// By command.
    pub commands: Vec<FacetValue>,
}

impl Catalogue {
    /// The facets, counted over the public moments. A value with no public moment is not
    /// offered: a filter that answers nothing is not a filter.
    pub fn facets(&self) -> Facets {
        let public: Vec<&Moment> = self.moments.iter().filter(|m| m.status == STABLE).collect();
        let titles: BTreeMap<&str, &str> = self
            .audiences
            .iter()
            .map(|a| (a.id.as_str(), a.short_title.as_deref().unwrap_or(&a.title)))
            .chain(self.areas.iter().map(|a| (a.id.as_str(), a.title.as_str())))
            .collect();
        let tally = |f: &dyn Fn(&Moment) -> Vec<String>| -> Vec<FacetValue> {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for m in &public {
                for v in f(m) {
                    *counts.entry(v).or_default() += 1;
                }
            }
            let mut out: Vec<FacetValue> = counts
                .into_iter()
                .map(|(value, count)| FacetValue {
                    label: titles
                        .get(value.as_str())
                        .map_or(value.clone(), |t| t.to_string()),
                    value,
                    count,
                })
                .collect();
            out.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
            out
        };
        // audiences and areas keep the taxonomy's own order, so the filter row reads the
        // way the catalogue is written; the open-ended facets are ordered by weight of use
        let ordered =
            |ids: Vec<(&str, &str)>, f: &dyn Fn(&Moment) -> Vec<String>| -> Vec<FacetValue> {
                let counts = tally(f);
                ids.into_iter()
                    .filter_map(|(id, label)| {
                        counts.iter().find(|c| c.value == id).map(|c| FacetValue {
                            value: id.to_string(),
                            label: label.to_string(),
                            count: c.count,
                        })
                    })
                    .collect()
            };
        Facets {
            audiences: ordered(
                self.audiences
                    .iter()
                    .filter(|a| a.status != "deprecated")
                    .map(|a| (a.id.as_str(), a.short_title.as_deref().unwrap_or(&a.title)))
                    .collect(),
                &|m| m.audiences.clone(),
            ),
            areas: ordered(
                self.areas
                    .iter()
                    .filter(|a| a.status != "deprecated")
                    .map(|a| (a.id.as_str(), a.title.as_str()))
                    .collect(),
                &|m| m.areas.clone(),
            ),
            tags: tally(&|m| m.tags.clone()),
            severities: tally(&|m| vec![m.severity.clone()]),
            frequencies: tally(&|m| vec![m.frequency.clone()]),
            lifecycle: tally(&|m| m.lifecycle.clone()),
            capabilities: tally(&|m| m.capabilities.clone()),
            commands: tally(&|m| m.commands.clone()),
        }
    }
}

// ---------------------------------------------------------------- diagnosis

/// One recommendation, with the moments that produced it. A recommendation with no
/// `matched_because` is a recommendation nobody can check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recommendation {
    /// The thing recommended: a capability id, a command, a claim, a rule, a use case.
    pub id: String,
    /// How many of the selected moments name it.
    pub count: usize,
    /// The selected moments that named it, in presentation order.
    pub matched_because: Vec<String>,
}

/// What a reader's selection implies, computed by counting rather than by inference.
///
/// The arithmetic is the whole model and is stated so a reader can check it: a selection
/// resolves to a set of moments; an area or an audience scores the number of selected
/// moments that name it; a recommendation scores the number that name it and carries
/// their ids. There is no weighting and no percentage, because there is no model behind
/// one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnosis {
    /// The moments the selection resolved to, in presentation order.
    pub moments: Vec<String>,
    /// Selected names that resolved to nothing.
    pub unresolved: Vec<String>,
    /// Operational areas by how many selected moments fall under them, heaviest first.
    pub areas: Vec<Recommendation>,
    /// Audiences by how many selected moments they recognise, heaviest first.
    pub audiences: Vec<Recommendation>,
    /// Capabilities of the executable that answer the selected moments.
    pub capabilities: Vec<Recommendation>,
    /// Commands that answer them.
    pub commands: Vec<Recommendation>,
    /// Claims that say what is guaranteed.
    pub claims: Vec<Recommendation>,
    /// Rules of the effective set that govern them.
    pub doctrines: Vec<Recommendation>,
    /// Use cases that show the way out.
    pub use_cases: Vec<Recommendation>,
    /// Moments the selection did not include that share an area with one that it did.
    pub also_worth_reading: Vec<String>,
}

impl Catalogue {
    /// Diagnose a selection. Each name may be a signal id or a moment id; a signal
    /// resolves to the moment that owns it, so a reader ticking symptoms and a script
    /// naming moments reach the same answer.
    pub fn diagnose(&self, selected: &[String]) -> Diagnosis {
        let mut ids: Vec<&str> = Vec::new();
        let mut unresolved: Vec<String> = Vec::new();
        for name in selected {
            if let Some(m) = self.signal_owner(name).or_else(|| self.moment(name)) {
                if !ids.contains(&m.id.as_str()) {
                    ids.push(&m.id);
                }
            } else {
                unresolved.push(name.clone());
            }
        }
        let chosen: Vec<&Moment> = self
            .moments
            .iter()
            .filter(|m| ids.contains(&m.id.as_str()))
            .collect();

        let rank = |f: &dyn Fn(&Moment) -> Vec<String>| -> Vec<Recommendation> {
            let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for m in &chosen {
                for v in f(m) {
                    by.entry(v).or_default().push(m.id.clone());
                }
            }
            let mut out: Vec<Recommendation> = by
                .into_iter()
                .map(|(id, matched_because)| Recommendation {
                    id,
                    count: matched_because.len(),
                    matched_because,
                })
                .collect();
            out.sort_by(|a, b| b.count.cmp(&a.count).then(a.id.cmp(&b.id)));
            out
        };

        let areas = rank(&|m| m.areas.clone());
        let mut also: Vec<String> = Vec::new();
        for m in self.moments.iter().filter(|m| m.status == STABLE) {
            if ids.contains(&m.id.as_str()) {
                continue;
            }
            let shares_area = m.areas.iter().any(|a| areas.iter().any(|r| &r.id == a));
            let is_related = chosen.iter().any(|c| c.related.contains(&m.id));
            if shares_area || is_related {
                also.push(m.id.clone());
            }
        }

        Diagnosis {
            moments: chosen.iter().map(|m| m.id.clone()).collect(),
            unresolved,
            audiences: rank(&|m| m.audiences.clone()),
            areas,
            capabilities: rank(&|m| m.capabilities.clone()),
            commands: rank(&|m| m.commands.clone()),
            claims: rank(&|m| m.claims.clone()),
            doctrines: rank(&|m| m.doctrines.clone()),
            use_cases: rank(&|m| m.use_cases.clone()),
            also_worth_reading: also,
        }
    }
}

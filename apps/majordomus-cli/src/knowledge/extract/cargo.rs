//! The `cargo` extractor: every tracked `Cargo.toml`, read as the manifest it is. Packages
//! become components, their binaries executables, their library a library, their
//! dependencies dependency nodes with an edge each; every dependency is one piece of
//! entry-level evidence, so a change to one dependency invalidates one claim.
//!
//! No `cargo` process is run and no TOML crate is pulled in: the subset a manifest is
//! written in — tables, arrays of tables, `key = "string"`, `key = { ... }` inline tables,
//! `key = [ ... ]` arrays — is read here, deterministically, on every platform the
//! repository is checked out on. A construct outside it is skipped, never guessed.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::knowledge::model::{
    ExtractorInfo, Fingerprint, Granularity, KindInfo, Ownership, PredicateInfo, Provenance,
    Relation, RelationInfo, Verification, Visibility,
};

use super::{claim, claim_with, tracked_where, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "cargo";

/// The cargo extractor.
pub struct Cargo;

/// The extractor.
pub fn extractor() -> Cargo {
    Cargo
}

// ---------------------------------------------------------------- a small TOML reader

/// One value of the TOML subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Toml {
    /// A string, an integer, a boolean, as text.
    Scalar(String),
    /// `[a, b]`.
    Array(Vec<Toml>),
    /// `{ k = v, ... }`.
    Table(BTreeMap<String, Toml>),
}

impl Toml {
    /// The text of a scalar.
    pub fn text(&self) -> Option<&str> {
        match self {
            Toml::Scalar(s) => Some(s),
            _ => None,
        }
    }
    /// A key of a table.
    pub fn get(&self, key: &str) -> Option<&Toml> {
        match self {
            Toml::Table(t) => t.get(key),
            _ => None,
        }
    }
}

/// A parsed manifest: tables by dotted name, and arrays of tables (`[[bin]]`) by name.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Manifest {
    /// `[package]`, `[dependencies]`, `[workspace]`, ... keyed by the header text.
    pub tables: BTreeMap<String, BTreeMap<String, Toml>>,
    /// `[[bin]]`, `[[bench]]`, ... in declaration order.
    pub arrays: BTreeMap<String, Vec<BTreeMap<String, Toml>>>,
}

impl Manifest {
    /// One key of one table.
    pub fn get(&self, table: &str, key: &str) -> Option<&Toml> {
        self.tables.get(table).and_then(|t| t.get(key))
    }
}

fn parse_scalar(raw: &str) -> Option<Toml> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Some(Toml::Scalar(raw[1..raw.len() - 1].replace("\\\"", "\"")));
    }
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return Some(Toml::Scalar(raw[1..raw.len() - 1].to_string()));
    }
    Some(Toml::Scalar(raw.to_string()))
}

/// Split `a, b, "c, d"` on commas outside quotes, brackets and braces.
fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '[' | '{' => {
                    depth += 1;
                    cur.push(c);
                }
                ']' | '}' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            },
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_value(raw: &str) -> Option<Toml> {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return Some(Toml::Array(
            split_top(inner).iter().filter_map(|v| parse_value(v)).collect(),
        ));
    }
    if let Some(inner) = raw.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let mut table = BTreeMap::new();
        for pair in split_top(inner) {
            if let Some((k, v)) = pair.split_once('=') {
                if let Some(v) = parse_value(v) {
                    table.insert(k.trim().trim_matches('"').to_string(), v);
                }
            }
        }
        return Some(Toml::Table(table));
    }
    parse_scalar(raw)
}

/// Strip a trailing `# comment` outside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => return &line[..i],
                _ => {}
            },
        }
    }
    line
}

/// Read a manifest in the subset. Multi-line arrays are joined until the brackets
/// balance; anything else outside the subset is skipped.
///
/// ```
/// use majordomus_cli::knowledge::extract::cargo::parse;
/// let m = parse("[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\nclap = \"4\"\n\n[[bin]]\nname = \"x-cli\"\n");
/// assert_eq!(m.get("package", "name").and_then(|v| v.text()), Some("x"));
/// assert_eq!(m.get("dependencies", "clap").and_then(|v| v.text()), Some("4"));
/// assert_eq!(m.get("dependencies", "serde").and_then(|v| v.get("version")).and_then(|v| v.text()), Some("1"));
/// assert_eq!(m.arrays["bin"][0]["name"].text(), Some("x-cli"));
/// ```
pub fn parse(text: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut table: Option<String> = None;
    let mut array: Option<String> = None;
    let mut pending = String::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() && pending.is_empty() {
            continue;
        }
        if !pending.is_empty() {
            pending.push(' ');
            pending.push_str(line);
            if balanced(&pending) {
                let done = std::mem::take(&mut pending);
                insert(&mut m, &table, &array, &done);
            }
            continue;
        }
        if let Some(inner) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            array = Some(inner.trim().to_string());
            table = None;
            m.arrays
                .entry(inner.trim().to_string())
                .or_default()
                .push(BTreeMap::new());
            continue;
        }
        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            table = Some(inner.trim().trim_matches('"').to_string());
            array = None;
            m.tables.entry(inner.trim().to_string()).or_default();
            continue;
        }
        if line.contains('=') {
            if balanced(line) {
                insert(&mut m, &table, &array, line);
            } else {
                pending = line.to_string();
            }
        }
    }
    m
}

fn balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '[' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                _ => {}
            },
        }
    }
    depth == 0 && quote.is_none()
}

fn insert(m: &mut Manifest, table: &Option<String>, array: &Option<String>, line: &str) {
    let Some((k, v)) = line.split_once('=') else {
        return;
    };
    let key = k.trim().trim_matches('"').to_string();
    let Some(value) = parse_value(v) else {
        return;
    };
    if let Some(a) = array {
        if let Some(last) = m.arrays.get_mut(a).and_then(|v| v.last_mut()) {
            last.insert(key, value);
        }
        return;
    }
    let t = table.clone().unwrap_or_default();
    m.tables.entry(t).or_default().insert(key, value);
}

// ---------------------------------------------------------------- the extraction

/// The dependency tables a manifest may carry, with the relation each one states.
const DEPENDENCY_TABLES: &[(&str, &str)] = &[
    ("dependencies", "depends_on"),
    ("dev-dependencies", "depends_on_for_development"),
    ("build-dependencies", "depends_on_for_build"),
];

fn dependency_entries(text: &str) -> Vec<(String, String)> {
    let m = parse(text);
    let mut out = Vec::new();
    for (table, _) in DEPENDENCY_TABLES {
        if let Some(t) = m.tables.get(*table) {
            for (name, value) in t {
                out.push((format!("{table}.{name}"), render(value)));
            }
        }
    }
    out
}

fn render(v: &Toml) -> String {
    match v {
        Toml::Scalar(s) => s.clone(),
        Toml::Array(items) => format!(
            "[{}]",
            items.iter().map(render).collect::<Vec<_>>().join(",")
        ),
        Toml::Table(t) => format!(
            "{{{}}}",
            t.iter()
                .map(|(k, v)| format!("{k}={}", render(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn is_manifest(path: &str) -> bool {
    path == "Cargo.toml" || path.ends_with("/Cargo.toml")
}

impl Extractor for Cargo {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "Cargo manifests".into(),
            description: "Every tracked Cargo.toml: the package it declares as a component, its binaries and library, its dependencies as nodes with one entry-level piece of evidence each, and the workspace members. Read from the manifest text alone; no cargo process runs.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![
                KindInfo { kind: "component".into(), meaning: "a package a manifest declares".into() },
                KindInfo { kind: "executable".into(), meaning: "a binary a package builds".into() },
                KindInfo { kind: "library".into(), meaning: "a library a package builds".into() },
                KindInfo { kind: "dependency".into(), meaning: "a crate a package depends on".into() },
                KindInfo { kind: "workspace".into(), meaning: "a Cargo workspace and its members".into() },
            ],
            relations: vec![
                RelationInfo { kind: "depends_on".into(), meaning: "the component depends on the crate".into(), propagates: true },
                RelationInfo { kind: "depends_on_for_development".into(), meaning: "the component's tests depend on the crate".into(), propagates: false },
                RelationInfo { kind: "depends_on_for_build".into(), meaning: "the component's build script depends on the crate".into(), propagates: false },
                RelationInfo { kind: "builds".into(), meaning: "the package builds the executable or library".into(), propagates: true },
                RelationInfo { kind: "member_of".into(), meaning: "the package is a member of the workspace".into(), propagates: false },
            ],
            predicates: vec![
                PredicateInfo { name: "version".into(), meaning: "the declared version".into(), functional: true },
                PredicateInfo { name: "edition".into(), meaning: "the Rust edition".into(), functional: true },
                PredicateInfo { name: "description".into(), meaning: "the one-line description".into(), functional: true },
                PredicateInfo { name: "license".into(), meaning: "the licence identifier".into(), functional: true },
                PredicateInfo { name: "manifest".into(), meaning: "the manifest path".into(), functional: true },
                PredicateInfo { name: "binary".into(), meaning: "the name of a binary the package builds".into(), functional: false },
                PredicateInfo { name: "dependency".into(), meaning: "a dependency and its requirement".into(), functional: false },
                PredicateInfo { name: "rust_version".into(), meaning: "the minimum supported Rust version".into(), functional: true },
                PredicateInfo { name: "package".into(), meaning: "the package that builds the executable or library".into(), functional: true },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        let manifests: Vec<String> = tracked_where(ctx, is_manifest).cloned().collect();
        for path in manifests {
            let Some(text) = ctx.read(&path) else {
                out.diagnostics.push(crate::model::Diagnostic::warning(
                    "knowledge_unreadable",
                    Some(path.clone()),
                    "the manifest could not be read as text, or the scope refuses it",
                ));
                continue;
            };
            let manifest = parse(&text);
            let file_ev = out.file_evidence(ID, &path, text.as_bytes(), Visibility::Public);
            let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".");

            if let Some(ws) = manifest.tables.get("workspace") {
                let id = format!("workspace:{dir}");
                let mut node = NodeSpec {
                    id: id.clone(),
                    title: format!("workspace at {dir}"),
                    summary: Some("a Cargo workspace".into()),
                    provenance: Provenance::Observed,
                    ownership: Ownership::External,
                    visibility: Visibility::Public,
                    evidence: vec![file_ev.clone()],
                    source: Some(path.clone()),
                    extractor: ID,
                }
                .build();
                node.claims.push(claim(
                    &id,
                    "manifest",
                    Value::String(path.clone()),
                    Provenance::Observed,
                    vec![file_ev.clone()],
                    Verification::Existence,
                ));
                out.node(node);
                if let Some(Toml::Array(members)) = ws.get("members") {
                    for member in members.iter().filter_map(Toml::text) {
                        let member_dir = if dir == "." {
                            member.to_string()
                        } else {
                            format!("{dir}/{member}")
                        };
                        let member_manifest = format!("{member_dir}/Cargo.toml");
                        if let Some(name) = ctx
                            .read(&member_manifest)
                            .map(|t| parse(&t))
                            .and_then(|m| m.get("package", "name").and_then(|v| v.text().map(str::to_string)))
                        {
                            out.relation(Relation {
                                source: format!("component:{name}"),
                                target: id.clone(),
                                kind: "member_of".into(),
                                provenance: Provenance::Observed,
                                evidence: vec![file_ev.clone()],
                            });
                        }
                    }
                }
            }

            let Some(name) = manifest
                .get("package", "name")
                .and_then(|v| v.text().map(str::to_string))
            else {
                continue;
            };
            let id = format!("component:{name}");
            let package_ev = out.entry_evidence(
                ID,
                &path,
                "package",
                render(&Toml::Table(
                    manifest.tables.get("package").cloned().unwrap_or_default(),
                ))
                .as_bytes(),
                Visibility::Public,
            );
            let description = manifest
                .get("package", "description")
                .and_then(|v| v.text().map(str::to_string));
            let mut node = NodeSpec {
                id: id.clone(),
                title: name.clone(),
                summary: description.clone(),
                provenance: Provenance::Observed,
                ownership: Ownership::External,
                visibility: Visibility::Public,
                evidence: vec![file_ev.clone(), package_ev.clone()],
                source: Some(path.clone()),
                extractor: ID,
            }
            .build();
            node.claims.push(claim(
                &id,
                "manifest",
                Value::String(path.clone()),
                Provenance::Observed,
                vec![file_ev.clone()],
                Verification::Existence,
            ));
            for key in ["version", "edition", "license", "rust-version", "description"] {
                if let Some(v) = manifest.get("package", key).and_then(Toml::text) {
                    node.claims.push(claim(
                        &id,
                        &key.replace('-', "_"),
                        Value::String(v.to_string()),
                        Provenance::Observed,
                        vec![package_ev.clone()],
                        Verification::Content,
                    ));
                }
            }
            // the binaries: every [[bin]], or the default one a src/main.rs implies
            let mut binaries: Vec<String> = manifest
                .arrays
                .get("bin")
                .into_iter()
                .flatten()
                .filter_map(|b| b.get("name").and_then(Toml::text).map(str::to_string))
                .collect();
            let main = if dir == "." {
                "src/main.rs".to_string()
            } else {
                format!("{dir}/src/main.rs")
            };
            if binaries.is_empty() && ctx.is_tracked(&main) {
                binaries.push(name.clone());
            }
            for bin in &binaries {
                let bid = format!("executable:{bin}");
                let mut bnode = NodeSpec {
                    id: bid.clone(),
                    title: bin.clone(),
                    summary: Some(format!("a binary of package {name}")),
                    provenance: Provenance::Observed,
                    ownership: Ownership::External,
                    visibility: Visibility::Public,
                    evidence: vec![file_ev.clone()],
                    source: Some(path.clone()),
                    extractor: ID,
                }
                .build();
                bnode.claims.push(claim(
                    &bid,
                    "package",
                    Value::String(name.clone()),
                    Provenance::Observed,
                    vec![file_ev.clone()],
                    Verification::Existence,
                ));
                out.node(bnode);
                node.claims.push(claim_with(
                    &id,
                    "binary",
                    bin,
                    Value::String(bin.clone()),
                    Provenance::Observed,
                    vec![file_ev.clone()],
                    Verification::Existence,
                ));
                out.relation(Relation {
                    source: id.clone(),
                    target: bid,
                    kind: "builds".into(),
                    provenance: Provenance::Observed,
                    evidence: vec![file_ev.clone()],
                });
            }
            // the library: [lib] name, or the package name when src/lib.rs exists
            let lib_rs = if dir == "." {
                "src/lib.rs".to_string()
            } else {
                format!("{dir}/src/lib.rs")
            };
            let lib = manifest
                .get("lib", "name")
                .and_then(|v| v.text().map(str::to_string))
                .or_else(|| ctx.is_tracked(&lib_rs).then(|| name.replace('-', "_")));
            if let Some(lib) = lib {
                let lid = format!("library:{lib}");
                let mut lnode = NodeSpec {
                    id: lid.clone(),
                    title: lib.clone(),
                    summary: Some(format!("the library of package {name}")),
                    provenance: Provenance::Observed,
                    ownership: Ownership::External,
                    visibility: Visibility::Public,
                    evidence: vec![file_ev.clone()],
                    source: Some(path.clone()),
                    extractor: ID,
                }
                .build();
                lnode.claims.push(claim(
                    &lid,
                    "package",
                    Value::String(name.clone()),
                    Provenance::Observed,
                    vec![file_ev.clone()],
                    Verification::Existence,
                ));
                out.node(lnode);
                out.relation(Relation {
                    source: id.clone(),
                    target: lid,
                    kind: "builds".into(),
                    provenance: Provenance::Observed,
                    evidence: vec![file_ev.clone()],
                });
            }
            // the dependencies, one entry of evidence each
            for (table, relation) in DEPENDENCY_TABLES {
                let Some(deps) = manifest.tables.get(*table) else {
                    continue;
                };
                for (dep, value) in deps {
                    let member = format!("{table}.{dep}");
                    let ev = out.entry_evidence(
                        ID,
                        &path,
                        &member,
                        render(value).as_bytes(),
                        Visibility::Public,
                    );
                    let requirement = match value {
                        Toml::Scalar(s) => s.clone(),
                        other => other
                            .get("version")
                            .and_then(Toml::text)
                            .map(str::to_string)
                            .unwrap_or_else(|| render(other)),
                    };
                    let did = format!("dependency:{dep}");
                    out.node(
                        NodeSpec {
                            id: did.clone(),
                            title: dep.clone(),
                            summary: Some("a crate a manifest depends on".into()),
                            provenance: Provenance::Observed,
                            ownership: Ownership::External,
                            visibility: Visibility::Public,
                            evidence: vec![ev.clone()],
                            source: None,
                            extractor: ID,
                        }
                        .build(),
                    );
                    node.claims.push(claim_with(
                        &id,
                        "dependency",
                        &member,
                        Value::String(format!("{dep} {requirement}")),
                        Provenance::Observed,
                        vec![ev.clone()],
                        Verification::Content,
                    ));
                    out.relation(Relation {
                        source: id.clone(),
                        target: did,
                        kind: (*relation).into(),
                        provenance: Provenance::Observed,
                        evidence: vec![ev],
                    });
                }
            }
            out.node(node);
        }
        out
    }

    fn entries(&self, path: &str, content: &str) -> Vec<(String, Fingerprint)> {
        if !is_manifest(path) {
            return Vec::new();
        }
        let m = parse(content);
        let mut out = vec![(
            "package".to_string(),
            Fingerprint::sha256(
                render(&Toml::Table(m.tables.get("package").cloned().unwrap_or_default()))
                    .as_bytes(),
                Granularity::Entry,
            ),
        )];
        for (member, rendered) in dependency_entries(content) {
            out.push((
                member,
                Fingerprint::sha256(rendered.as_bytes(), Granularity::Entry),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[package]
name = "demo"
version = "0.3.0"
edition = "2021"
description = "A demo." # trailing comment

[lib]
name = "demo_lib"

[[bin]]
name = "demo-cli"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
clap = "4.5"

[dev-dependencies]
tempfile = "3"

[workspace]
members = [
  "apps/a",
  "apps/b",
]
"#;

    #[test]
    fn the_subset_reader_reads_a_real_shape() {
        let m = parse(MANIFEST);
        assert_eq!(m.get("package", "version").and_then(Toml::text), Some("0.3.0"));
        assert_eq!(m.get("package", "description").and_then(Toml::text), Some("A demo."));
        assert_eq!(m.get("lib", "name").and_then(Toml::text), Some("demo_lib"));
        assert_eq!(m.arrays["bin"][0]["name"].text(), Some("demo-cli"));
        assert_eq!(
            m.get("dependencies", "serde").and_then(|v| v.get("version")).and_then(Toml::text),
            Some("1")
        );
        assert_eq!(m.get("dev-dependencies", "tempfile").and_then(Toml::text), Some("3"));
        let members = m.get("workspace", "members").unwrap();
        assert_eq!(
            members,
            &Toml::Array(vec![Toml::Scalar("apps/a".into()), Toml::Scalar("apps/b".into())])
        );
    }

    #[test]
    fn entries_change_one_at_a_time() {
        let a = Cargo.entries("Cargo.toml", MANIFEST);
        let b = Cargo.entries("Cargo.toml", &MANIFEST.replace("clap = \"4.5\"", "clap = \"4.6\""));
        let changed: Vec<&String> = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| x.1 != y.1)
            .map(|(x, _)| &x.0)
            .collect();
        assert_eq!(changed, vec!["dependencies.clap"]);
        assert!(Cargo.entries("README.md", "x").is_empty());
    }
}

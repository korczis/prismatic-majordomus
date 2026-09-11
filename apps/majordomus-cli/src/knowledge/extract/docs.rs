//! The `docs` extractor: the documentation a repository already has, discovered where it
//! lives and registered as external knowledge. README files, guides under `docs/`,
//! architecture decisions, runbooks, contributing guides and changelogs become document
//! nodes owned by whoever wrote them; nothing is copied into the layer.
//!
//! What is read off a document deterministically: that it exists, its first heading, its
//! class by the convention its path follows, and every inline link to a repository path
//! outside a code fence, which becomes a `references` relation with the line it was seen
//! on as its evidence. What a document *says* in prose is not extracted here: a statement
//! in prose is a candidate for the semantic layer, never an observed fact.

use serde_json::Value;

use crate::knowledge::model::{
    ExtractorInfo, KindInfo, Ownership, PredicateInfo, Provenance, Relation, RelationInfo,
    Verification, Visibility,
};

use super::{claim, claim_with, tracked_where, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "docs";

/// The docs extractor.
pub struct Docs;

/// The extractor.
pub fn extractor() -> Docs {
    Docs
}

/// How a document is classified by the convention its path follows. The first match
/// decides; the order is from the most specific convention to the least.
pub fn classify(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if !lower.ends_with(".md") && !lower.ends_with(".markdown") {
        return None;
    }
    let dir = lower.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    if name.starts_with("readme") {
        return Some("readme");
    }
    if name.starts_with("contributing") {
        return Some("contributing");
    }
    if name.starts_with("changelog") || name.starts_with("history") {
        return Some("changelog");
    }
    if name.starts_with("security") {
        return Some("security");
    }
    if dir.split('/').any(|d| matches!(d, "adr" | "adrs" | "decisions")) {
        return Some("decision");
    }
    if dir.split('/').any(|d| matches!(d, "rfc" | "rfcs")) {
        return Some("rfc");
    }
    if dir.split('/').any(|d| matches!(d, "runbook" | "runbooks" | "playbooks")) {
        return Some("runbook");
    }
    if name.contains("architecture") || dir.split('/').any(|d| d == "architecture") {
        return Some("architecture");
    }
    if dir == "docs" || dir.starts_with("docs/") || dir == "doc" || dir.starts_with("doc/") {
        return Some("guide");
    }
    None
}

/// The first level-one heading of a Markdown text, outside a code fence.
pub fn title_of(text: &str) -> Option<String> {
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if let Some(t) = line.strip_prefix("# ") {
            let t = t.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// The front matter of a Markdown text, when it carries one, as the layer's reader parses
/// it; `None` for none or for one that does not parse.
fn front_matter(text: &str) -> Option<serde_json::Map<String, Value>> {
    let split = crate::metadata::frontmatter::split(text).ok()?;
    crate::metadata::frontmatter::parse(split.front?).ok()
}

/// Every inline link target that names a repository path, with the line it was seen on,
/// outside code fences: `[text](docs/CLI.md#anchor)` yields `docs/CLI.md`. Targets with a
/// scheme, protocol-relative ones, absolute ones and bare anchors are not repository
/// paths and are skipped. A relative target is resolved against the document's directory.
///
/// ```
/// use majordomus_cli::knowledge::extract::docs::links;
/// let text = "see [x](../docs/CLI.md#a) and [y](https://example.com) and\n```\n[z](in/code.md)\n```\n[w](./b.md)";
/// assert_eq!(links("guides/one.md", text), vec![("docs/CLI.md".to_string(), 1), ("guides/b.md".to_string(), 5)]);
/// ```
pub fn links(path: &str, text: &str) -> Vec<(String, usize)> {
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut out = Vec::new();
    let mut fence = false;
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let Some(end) = after.find(')') else { break };
            let raw = &after[..end];
            rest = &after[end + 1..];
            let target = raw.split_whitespace().next().unwrap_or("");
            let target = target.split('#').next().unwrap_or("");
            if target.is_empty()
                || target.starts_with('/')
                || target.starts_with("//")
                || target.contains("://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            if let Some(resolved) = resolve_relative(dir, target) {
                out.push((resolved, i + 1));
            }
        }
    }
    out
}

/// Resolve a relative link against a directory, lexically; `None` when it climbs out of
/// the repository.
fn resolve_relative(dir: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

impl Extractor for Docs {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "Existing documentation".into(),
            description: "The documentation the repository already has — README files, guides under docs/, architecture decisions, RFCs, runbooks, contributing and security guides, changelogs — discovered where it lives by the convention its path follows and registered as external knowledge. Its title and its links to repository paths are read; its prose is not interpreted.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![
                KindInfo { kind: "document".into(), meaning: "a Markdown document the repository carries, owned by whoever wrote it".into() },
                KindInfo { kind: "file".into(), meaning: "a tracked file a document names and nothing else models".into() },
                KindInfo { kind: "directory".into(), meaning: "a directory a document links to".into() },
            ],
            relations: vec![
                RelationInfo { kind: "references".into(), meaning: "the document links to the target".into(), propagates: true },
                RelationInfo { kind: "describes".into(), meaning: "the document's declared subject is the target".into(), propagates: true },
            ],
            predicates: vec![
                PredicateInfo { name: "title".into(), meaning: "the first level-one heading".into(), functional: true },
                PredicateInfo { name: "class".into(), meaning: "readme, guide, decision, rfc, runbook, architecture, contributing, security or changelog, by the path's convention".into(), functional: true },
                PredicateInfo { name: "path".into(), meaning: "where the document is".into(), functional: true },
                PredicateInfo { name: "reference".into(), meaning: "a repository path the document links to".into(), functional: false },
                PredicateInfo { name: "status".into(), meaning: "the status the front matter declares".into(), functional: true },
                PredicateInfo { name: "describes".into(), meaning: "the subject the front matter declares the document to be about".into(), functional: false },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        // the layer's own objects are the layer extractor's; a document under `.ai/` is
        // read there with its contract, and reading it here too would be two nodes
        let paths: Vec<String> = tracked_where(ctx, |p| {
            !p.starts_with(".ai/") && classify(p).is_some()
        })
        .cloned()
        .collect();
        let generated = ctx.generated_paths();
        // an object of the index is the same node under the layer's id, so a link to it
        // lands on the layer's node rather than on a second one
        let by_path: std::collections::BTreeMap<&str, String> = ctx
            .index
            .objects
            .iter()
            .filter(|o| o.provenance.member.is_none())
            .map(|o| (o.provenance.path.as_str(), super::layer::node_id(o)))
            .collect();
        for path in paths {
            let Some(text) = ctx.read(&path) else {
                continue;
            };
            let ev = out.file_evidence(ID, &path, text.as_bytes(), Visibility::Public);
            // a document the layer indexed is the layer's node, under the layer's id: this
            // extractor adds what it read to that node rather than standing a second one
            // beside it, and its provenance is declared so that the merge keeps the layer's
            let layer_id = by_path.get(path.as_str()).cloned();
            let id = layer_id.clone().unwrap_or_else(|| format!("document:{path}"));
            let class = classify(&path).unwrap_or("guide");
            let title = title_of(&text).unwrap_or_else(|| path.clone());
            let front = front_matter(&text);
            let mut node = NodeSpec {
                id: id.clone(),
                title: title.clone(),
                summary: front
                    .as_ref()
                    .and_then(|f| f.get("description"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                provenance: if layer_id.is_some() { Provenance::Declared } else { Provenance::Observed },
                ownership: Ownership::External,
                visibility: Visibility::Public,
                evidence: vec![ev.clone()],
                source: Some(path.clone()),
                extractor: ID,
            }
            .build();
            node.claims.push(claim(
                &id,
                "path",
                Value::String(path.clone()),
                Provenance::Observed,
                vec![ev.clone()],
                Verification::Existence,
            ));
            node.claims.push(claim(
                &id,
                "class",
                Value::String(class.into()),
                Provenance::Observed,
                vec![ev.clone()],
                Verification::Existence,
            ));
            node.claims.push(claim(
                &id,
                "title",
                Value::String(title),
                Provenance::Observed,
                vec![ev.clone()],
                Verification::Content,
            ));
            if let Some(f) = &front {
                if let Some(status) = f.get("status").and_then(Value::as_str) {
                    node.claims.push(claim(
                        &id,
                        "status",
                        Value::String(status.into()),
                        Provenance::Declared,
                        vec![ev.clone()],
                        Verification::Content,
                    ));
                }
                // `describes: [component:x, ...]` in a document's front matter is the one
                // declared relation a plain document may carry: what it is about
                if let Some(subjects) = f.get("describes").and_then(Value::as_array) {
                    for subject in subjects.iter().filter_map(Value::as_str) {
                        node.claims.push(claim_with(
                            &id,
                            "describes",
                            subject,
                            Value::String(subject.into()),
                            Provenance::Declared,
                            vec![ev.clone()],
                            Verification::Existence,
                        ));
                        out.relation(Relation {
                            source: id.clone(),
                            target: subject.to_string(),
                            kind: "describes".into(),
                            provenance: Provenance::Declared,
                            evidence: vec![ev.clone()],
                        });
                    }
                }
            }
            for (target, line) in links(&path, &text) {
                if target == path {
                    continue;
                }
                let Some(target_id) = super::resolve_path(&mut out, ctx, &by_path, &generated, &target, ID) else {
                    // a link to a path git does not track: the document says something
                    // the tree does not hold, which is a gap and not an edge
                    node.claims.push({
                        let mut c = claim_with(
                            &id,
                            "reference",
                            &format!("{line}"),
                            Value::String(target.clone()),
                            Provenance::Observed,
                            vec![ev.clone()],
                            Verification::Existence,
                        );
                        c.state = crate::knowledge::model::ClaimState::Unverified;
                        c.confidence = c.confidence.unresolved();
                        c.reason = Some(format!("line {line} links to {target}, which git does not track"));
                        c
                    });
                    out.gap(
                        crate::knowledge::model::GapCategory::UnresolvedReference,
                        &format!("{id}#{line}"),
                        &format!("{path} line {line} links to {target}, which is not a tracked path"),
                        "fix the link, or track the file it names",
                    );
                    continue;
                };
                node.claims.push(claim_with(
                    &id,
                    "reference",
                    &format!("{line}"),
                    Value::String(target.clone()),
                    Provenance::Observed,
                    vec![ev.clone()],
                    Verification::Existence,
                ));
                out.relation(Relation {
                    source: id.clone(),
                    target: target_id,
                    kind: "references".into(),
                    provenance: Provenance::Observed,
                    evidence: vec![ev.clone()],
                });
            }
            out.node(node);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_are_classified_by_the_convention_their_path_follows() {
        assert_eq!(classify("README.md"), Some("readme"));
        assert_eq!(classify("apps/x/README.md"), Some("readme"));
        assert_eq!(classify("docs/CLI.md"), Some("guide"));
        assert_eq!(classify("docs/adr/0001-x.md"), Some("decision"));
        assert_eq!(classify("docs/rfcs/0007-y.md"), Some("rfc"));
        assert_eq!(classify("runbooks/oncall.md"), Some("runbook"));
        assert_eq!(classify("docs/ARCHITECTURE.md"), Some("architecture"));
        assert_eq!(classify("CONTRIBUTING.md"), Some("contributing"));
        assert_eq!(classify("CHANGELOG.md"), Some("changelog"));
        assert_eq!(classify("src/main.rs"), None);
        assert_eq!(classify("notes/todo.md"), None);
    }

    #[test]
    fn the_title_is_the_first_heading_outside_a_fence() {
        assert_eq!(title_of("```\n# not this\n```\n# This\n").as_deref(), Some("This"));
        assert_eq!(title_of("no heading"), None);
    }

    #[test]
    fn links_resolve_relative_to_the_document_and_skip_what_is_not_a_path() {
        let l = links("docs/a/b.md", "[x](../c.md) [y](#anchor) [z](/abs) [w](https://e) [v](../../../out.md)");
        assert_eq!(l, vec![("docs/c.md".to_string(), 1)]);
    }
}

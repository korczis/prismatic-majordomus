//! The site's entity projection as a target of the canonical generation (I1007, slice 1).
//!
//! `site/data/generated/entities.json` is what the site renders an entity page from: the
//! kinds the repository publishes as pages, a catalogue of every kind the publication
//! declaration names, and every published object with its references and the references
//! that resolve to it. Until this target existed, `scripts/generate-site-data` computed it
//! in the shell, with an awk reader of `site/data/publication.toml` and a jq program over
//! `docs/generated/graph.json`. This module is that computation, ported rule for rule, so
//! the projection is a function of the same graph the server answers from and the shell
//! can become an adapter that only reads it (slice 2).
//!
//! The port is held to the shell's output byte for byte by
//! `the_projection_is_the_shells_byte_for_byte`, over the files this repository commits:
//! the same graph and the same declaration give the same bytes, or the test names the first
//! line that differs.

use serde_json::{Map, Value};

use crate::error::{Error, Result};

/// The kinds of a publication declaration, as `scripts/generate-site-data` reads it: one
/// object per `[[kinds]]` table, every `key = value` line a member whose value is the TOML
/// value read as JSON, in the file's order. `#` comment lines and anything that is neither
/// a table header nor a key line are skipped, as the shell's awk skips them; a key line
/// before the first table belongs to no kind and is skipped too, where the shell would have
/// written JSON nobody could read.
pub(crate) fn publication_kinds(declaration: &str) -> Result<Vec<Map<String, Value>>> {
    let mut kinds: Vec<Map<String, Value>> = Vec::new();
    for line in declaration.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with("[[kinds]]") {
            kinds.push(Map::new());
            continue;
        }
        let key_len = line
            .bytes()
            .take_while(|b| b.is_ascii_lowercase() || *b == b'_')
            .count();
        if key_len == 0 {
            continue;
        }
        let rest = line[key_len..].trim_start_matches([' ', '\t']);
        let Some(value) = rest.strip_prefix('=') else {
            continue;
        };
        let Some(kind) = kinds.last_mut() else {
            continue;
        };
        let value = value.trim_start_matches([' ', '\t']);
        let parsed: Value = serde_json::from_str(value).map_err(|e| Error::Protocol {
            reason: format!(
                "{}: the value of '{}' is not one the site generator can read ({e}): {value}",
                crate::entity::PUBLICATION,
                &line[..key_len]
            ),
        })?;
        kind.insert(line[..key_len].to_string(), parsed);
    }
    Ok(kinds)
}

/// The entity projection of a composed graph document (`docs/generated/graph.json`, its
/// `graph` member read) under a publication declaration: the value the shell wrote as
/// `site/data/generated/entities.json`.
pub(crate) fn projection(graph_document: &Value, declaration: &str) -> Result<Value> {
    let pubs = publication_kinds(declaration)?;
    if pubs.is_empty() {
        return Err(Error::Protocol {
            reason: format!("{} declares no kind", crate::entity::PUBLICATION),
        });
    }
    let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_string);
    let kind_of = |k: &Map<String, Value>| text(k.get("kind")).unwrap_or_default();
    let graph = &graph_document["graph"];
    let empty = Vec::new();
    let nodes = graph["nodes"].as_array().unwrap_or(&empty);
    let internal = |n: &&Value| !truthy(&n["external"]);
    let by_id: std::collections::HashMap<&str, &Value> = nodes
        .iter()
        .filter(internal)
        .filter_map(|n| n["id"].as_str().map(|id| (id, n)))
        .collect();
    let by_kind: std::collections::HashMap<String, &Map<String, Value>> =
        pubs.iter().map(|k| (kind_of(k), k)).collect();
    let edges: Vec<&Value> = graph["edges"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter(|e| e["kind"].as_str() != Some("is_a"))
        .collect();
    let last_segment = |route: &Value| -> Value {
        match route.as_str() {
            Some(r) => Value::String(r.rsplit('/').next().unwrap_or_default().to_string()),
            None => Value::Null,
        }
    };

    // the public address of a node: its own page under an entity kind, the section's page
    // under a section kind, and none for a kind that is not published
    let public = |id: &str| -> Value {
        let Some(n) = by_id.get(id) else {
            return Value::Null;
        };
        let Some(k) = n["kind"].as_str().and_then(|kind| by_kind.get(kind)) else {
            return Value::Null;
        };
        match k.get("projection").and_then(Value::as_str) {
            Some("entity") => {
                let route = text(k.get("route")).unwrap_or_default();
                let slug = last_segment(&n["route"]);
                Value::String(format!("{route}{}/", slug.as_str().unwrap_or_default()))
            }
            Some("section") => k.get("route").cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }
    };
    let side = |edge: &Value, id: &str| -> Value {
        let node = by_id.get(id);
        let label = node
            .map(|n| &n["label"])
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| Value::String(without_kind_prefix(id)));
        let kind = node
            .map(|n| &n["kind"])
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| Value::String(id.split(':').next().unwrap_or_default().into()));
        let mut m = Map::new();
        m.insert("edge".into(), edge["kind"].clone());
        m.insert("label".into(), label);
        m.insert("kind".into(), kind);
        m.insert("route".into(), public(id));
        m.insert("external".into(), Value::Bool(node.is_none()));
        Value::Object(m)
    };
    let entity = |n: &Value| -> Value {
        let id = n["id"].as_str().unwrap_or_default();
        let mut m = Map::new();
        m.insert("kind".into(), n["kind"].clone());
        m.insert("id".into(), n["id"].clone());
        m.insert("identity".into(), n["label"].clone());
        m.insert("slug".into(), last_segment(&n["route"]));
        m.insert("cockpit".into(), n["route"].clone());
        m.insert(
            "title".into(),
            if truthy(&n["summary"]) {
                n["summary"].clone()
            } else {
                n["label"].clone()
            },
        );
        m.insert("status".into(), n["status"].clone());
        let tags = &n["facts"]["tags"];
        m.insert(
            "tags".into(),
            if truthy(tags) {
                tags.clone()
            } else {
                Value::Array(Vec::new())
            },
        );
        m.insert("source".into(), n["source"].clone());
        m.insert(
            "references".into(),
            Value::Array(
                edges
                    .iter()
                    .filter(|e| e["source"].as_str() == Some(id))
                    .map(|e| side(e, e["target"].as_str().unwrap_or_default()))
                    .collect(),
            ),
        );
        m.insert(
            "referenced_by".into(),
            Value::Array(
                edges
                    .iter()
                    .filter(|e| e["target"].as_str() == Some(id))
                    .map(|e| side(e, e["source"].as_str().unwrap_or_default()))
                    .collect(),
            ),
        );
        Value::Object(m)
    };
    let own = |kind: &str| -> Vec<&Value> {
        nodes
            .iter()
            .filter(internal)
            .filter(|n| n["kind"].as_str() == Some(kind))
            .collect()
    };

    let entity_kinds: Vec<&Map<String, Value>> = pubs
        .iter()
        .filter(|k| k.get("projection").and_then(Value::as_str) == Some("entity"))
        .collect();
    let kinds: Vec<Value> = entity_kinds
        .iter()
        .map(|k| {
            let members = own(&kind_of(k));
            let mut m = (*k).clone();
            m.insert("count".into(), Value::from(members.len()));
            m.insert(
                "dir".into(),
                match k.get("route").and_then(Value::as_str) {
                    Some(r) => {
                        let r = r.strip_prefix('/').unwrap_or(r);
                        Value::String(r.strip_suffix('/').unwrap_or(r).to_string())
                    }
                    None => k.get("route").cloned().unwrap_or(Value::Null),
                },
            );
            // `unique`: every status once, in the canonical order, which for the lower-case
            // words a status is agrees with jq's (the parity test holds that)
            let mut statuses: Vec<String> = members
                .iter()
                .filter_map(|n| n["status"].as_str().map(str::to_string))
                .collect();
            crate::order::canonical_strings(&mut statuses);
            statuses.dedup();
            m.insert(
                "statuses".into(),
                Value::Array(statuses.into_iter().map(Value::String).collect()),
            );
            Value::Object(m)
        })
        .collect();
    let mut catalogue: Vec<CatalogueRow> = pubs
        .iter()
        .map(|k| {
            let or_null = |key: &str| {
                k.get(key)
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let mut m = Map::new();
            m.insert("kind".into(), k.get("kind").cloned().unwrap_or(Value::Null));
            m.insert(
                "projection".into(),
                k.get("projection").cloned().unwrap_or(Value::Null),
            );
            m.insert("route".into(), or_null("route"));
            m.insert("title".into(), or_null("title"));
            m.insert("reason".into(), or_null("reason"));
            m.insert("count".into(), Value::from(own(&kind_of(k)).len()));
            CatalogueRow {
                kind: kind_of(k),
                row: Value::Object(m),
            }
        })
        .collect();
    // `sort_by(.kind)`, in the canonical order: kinds are lower-case words, where it is jq's
    crate::order::canonical(&mut catalogue);
    let catalogue: Vec<Value> = catalogue.into_iter().map(|c| c.row).collect();
    let published: Vec<String> = entity_kinds.iter().map(|k| kind_of(k)).collect();
    let entities: Vec<Value> = nodes
        .iter()
        .filter(internal)
        .filter(|n| {
            n["kind"]
                .as_str()
                .is_some_and(|k| published.iter().any(|p| p == k))
        })
        .map(entity)
        .collect();

    let mut doc = Map::new();
    doc.insert("schema".into(), Value::from(1));
    doc.insert(
        "source".into(),
        Value::String("docs/generated/graph.json".into()),
    );
    doc.insert("kinds".into(), Value::Array(kinds));
    doc.insert("catalogue".into(), Value::Array(catalogue));
    doc.insert("entities".into(), Value::Array(entities));
    Ok(Value::Object(doc))
}

/// jq's truth: everything but `null` and `false`, which is what `//` falls through on.
fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Null | Value::Bool(false))
}

/// `sub("^[a-z-]+:"; "")`: an id without the kind it is prefixed with.
fn without_kind_prefix(id: &str) -> String {
    let prefix = id
        .bytes()
        .take_while(|b| b.is_ascii_lowercase() || *b == b'-')
        .count();
    match id[prefix..].strip_prefix(':') {
        Some(rest) if prefix > 0 => rest.to_string(),
        _ => id.to_string(),
    }
}

/// One row of the catalogue, ordered by its kind.
struct CatalogueRow {
    kind: String,
    row: Value,
}

impl crate::order::Ordered for CatalogueRow {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.kind, &self.kind)
    }
}

/// The projection as `jq .` prints it: two-space indentation and one trailing newline.
pub(crate) fn render(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DECLARATION: &str = "# a comment\n\
        [[kinds]]\n\
        kind = \"adr\"\n\
        projection = \"entity\"\n\
        route = \"/adrs/\"\n\
        weight = 45\n\
        \n\
        [[kinds]]\n\
        kind = \"rule\"\n\
        projection = \"section\"\n\
        route = \"/rules/\"\n\
        \n\
        [[kinds]]\n\
        kind = \"knowledge\"\n\
        projection = \"none\"\n\
        reason = \"private\"\n";

    fn graph() -> Value {
        json!({ "graph": {
            "nodes": [
                {"id": "adr:0002", "kind": "adr", "label": "adr-0002",
                 "route": "/cockpit/objects/adr/adr-0002", "status": "proposed",
                 "summary": "Second", "source": ".ai/repo/adrs/0002.md"},
                {"id": "adr:0001", "kind": "adr", "label": "adr-0001",
                 "route": "/cockpit/objects/adr/adr-0001", "status": "accepted",
                 "facts": {"tags": ["x"]}, "source": ".ai/repo/adrs/0001.md"},
                {"id": "rule:a", "kind": "rule", "label": "a",
                 "route": "/cockpit/objects/rule/a"},
                {"id": "outside:z", "kind": "outside", "label": "z", "external": true},
            ],
            "edges": [
                {"source": "adr:0002", "target": "adr:0001", "kind": "supersedes"},
                {"source": "adr:0002", "target": "rule:a", "kind": "cites"},
                {"source": "adr:0001", "target": "outside:z", "kind": "names"},
                {"source": "adr:0001", "target": "adr", "kind": "is_a"},
            ],
        }})
    }

    #[test]
    fn the_declaration_is_read_as_the_site_generator_reads_it() {
        let kinds = publication_kinds(DECLARATION).unwrap();
        assert_eq!(kinds.len(), 3);
        assert_eq!(kinds[0]["weight"], json!(45), "a number stays a number");
        let keys: Vec<&String> = kinds[0].keys().collect();
        assert_eq!(
            keys,
            ["kind", "projection", "route", "weight"],
            "the file's order"
        );
        assert!(publication_kinds("kind = \"orphan\"\n").unwrap().is_empty());
        assert!(publication_kinds("[[kinds]]\nkind = unquoted\n").is_err());
    }

    #[test]
    fn the_projection_publishes_entity_kinds_and_resolves_every_side() {
        let doc = projection(&graph(), DECLARATION).unwrap();
        assert_eq!(
            doc["kinds"].as_array().unwrap().len(),
            1,
            "only the entity kind"
        );
        let adr = &doc["kinds"][0];
        assert_eq!(adr["count"], 2);
        assert_eq!(adr["dir"], "adrs");
        assert_eq!(
            adr["statuses"],
            json!(["accepted", "proposed"]),
            "sorted, once each"
        );
        let catalogue: Vec<&str> = doc["catalogue"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["kind"].as_str().unwrap())
            .collect();
        assert_eq!(catalogue, ["adr", "knowledge", "rule"], "sorted by kind");
        assert_eq!(doc["catalogue"][1]["route"], Value::Null);
        assert_eq!(doc["catalogue"][1]["reason"], "private");

        let second = &doc["entities"][0];
        assert_eq!(second["id"], "adr:0002", "the graph's order");
        assert_eq!(second["title"], "Second");
        assert_eq!(second["tags"], json!([]));
        assert_eq!(second["references"][0]["route"], "/adrs/adr-0001/");
        assert_eq!(
            second["references"][1]["route"], "/rules/",
            "a section's page"
        );
        let first = &doc["entities"][1];
        assert_eq!(first["title"], "adr-0001", "no summary: the label");
        assert_eq!(
            first["references"].as_array().unwrap().len(),
            1,
            "is_a is no edge"
        );
        assert_eq!(first["references"][0]["external"], true);
        assert_eq!(first["references"][0]["label"], "z");
        assert_eq!(first["references"][0]["route"], Value::Null);
        assert_eq!(first["referenced_by"][0]["id"], Value::Null);
        assert_eq!(first["referenced_by"][0]["label"], "adr-0002");
    }

    /// The port against the shell it replaces, over this repository's own committed files:
    /// the graph and the declaration the last derive read, and the entities.json it wrote.
    #[test]
    fn the_projection_is_the_shells_byte_for_byte() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).ok();
        let (Some(graph), Some(declaration), Some(shell)) = (
            read("docs/generated/graph.json"),
            read(crate::entity::PUBLICATION),
            read("site/data/generated/entities.json"),
        ) else {
            return; // a tree without the site has nothing to compare
        };
        let graph: Value = serde_json::from_str(&graph).unwrap();
        let ours = render(&projection(&graph, &declaration).unwrap());
        if ours != shell {
            let at = ours
                .lines()
                .zip(shell.lines())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| ours.lines().count().min(shell.lines().count()));
            panic!(
                "line {} differs:\n  ours:  {}\n  shell: {}",
                at + 1,
                ours.lines().nth(at).unwrap_or("(end)"),
                shell.lines().nth(at).unwrap_or("(end)")
            );
        }
    }
}

//! Generated projections that are committed for review: the OpenAPI document, the
//! capability reference, and the shell tool's key allow-lists derived from the JSON
//! Schemas. All come through this one pipeline; the committed files are caches of it,
//! never sources, and `--check` says when they are stale.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::capability::{CapabilityKind, CapabilityRegistry, Provenance};
use crate::error::{Error, Result};
use crate::http::openapi;
use crate::metadata::KindSchema;
use crate::share::Share;

/// Where generated artifacts live, relative to the repository root.
pub const OUT_DIR: &str = "docs/generated";

pub const HEADER: &str = "GENERATED FILE — DO NOT EDIT DIRECTLY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    OpenApi,
    Docs,
    /// `<share>/allow/<name>.txt` for every schema that carries `x-majordomus-allow`.
    Allow,
}

impl Target {
    pub const ALL: &'static [Target] = &[Target::OpenApi, Target::Docs, Target::Allow];
}

/// The schema extension that names the allow-list a schema derives.
pub const ALLOW_EXTENSION: &str = "x-majordomus-allow";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Repository-relative path.
    pub path: String,
    pub content: String,
}
/// The registry's artifacts of the selected targets: the OpenAPI document and the
/// reference. `Target::Allow` is not the registry's and is answered by
/// [`allow_artifacts`].
pub fn artifacts(
    registry: &CapabilityRegistry,
    version: &str,
    targets: &[Target],
) -> Result<Vec<Artifact>> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            Target::OpenApi => out.push(Artifact {
                path: format!("{OUT_DIR}/openapi.json"),
                content: openapi::render(
                    &openapi::document(registry, version)
                        .map_err(|reason| Error::Http { reason })?,
                ),
            }),
            Target::Docs => out.push(Artifact {
                path: format!("{OUT_DIR}/capabilities.md"),
                content: reference(registry, version),
            }),
            Target::Allow => {}
        }
    }
    Ok(out)
}

/// The shell tool's allow-lists, one per schema that carries `x-majordomus-allow`, under
/// the share directory: a repository-relative path when the share is inside the
/// repository, absolute otherwise.
pub fn allow_artifacts(schema: &KindSchema, share: &Share, root: &Path) -> Vec<Artifact> {
    let dir = share.allow_dir();
    let dir = dir
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or(dir);
    schema
        .schemas()
        .filter_map(|(_, sch)| {
            let name = sch.json.get(ALLOW_EXTENSION).and_then(Value::as_str)?;
            Some(Artifact {
                path: format!("{}/{name}.txt", dir.display()),
                content: allow_lines(&sch.json).join("\n") + "\n",
            })
        })
        .collect()
}

/// The allow-list a JSON Schema derives: one anchored pattern per key path the schema
/// allows, in the schema's order, as the shell tool's `mj_yaml_unknown_keys` reads them
/// (`grep -E -f`). A list of scalars is `name(\.[0-9]+)?`, a list of objects recurses
/// through `name\.[0-9]+\.`, a nested object through `name\.`. Local `$ref`s into
/// `$defs` are followed. No blank lines and no comments: every line is a pattern.
pub fn allow_lines(schema: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk_allow(schema, schema, "", &mut out);
    out
}

fn resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    match node
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/"))
    {
        Some(pointer) => pointer.split('/').fold(root, |v, seg| &v[seg]),
        None => node,
    }
}

fn escape(key: &str) -> String {
    let mut s = String::with_capacity(key.len());
    for c in key.chars() {
        if ".+*?()[]{}^$|\\".contains(c) {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

fn walk_allow(root: &Value, node: &Value, prefix: &str, out: &mut Vec<String>) {
    let node = resolve(root, node);
    let Some(props) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (key, prop) in props {
        let prop = resolve(root, prop);
        let path = format!("{prefix}{}", escape(key));
        let is_object = prop.get("properties").is_some();
        let items = prop.get("items").map(|i| resolve(root, i));
        if is_object {
            walk_allow(root, prop, &format!("{path}\\."), out);
        } else if let Some(items) = items {
            if items.get("properties").is_some() {
                walk_allow(root, items, &format!("{path}\\.[0-9]+\\."), out);
            } else {
                out.push(format!("^{path}(\\.[0-9]+)?$"));
            }
        } else {
            out.push(format!("^{path}$"));
        }
    }
}

/// Write every artifact under `root`, creating the directory. Returns the paths written.
pub fn write(root: &Path, artifacts: &[Artifact]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for a in artifacts {
        let path = root.join(&a.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        std::fs::write(&path, &a.content).map_err(|e| Error::io(&path, e))?;
        written.push(path);
    }
    Ok(written)
}

/// Compare every artifact with what is on disk under `root`; write nothing.
pub fn check(root: &Path, artifacts: &[Artifact]) -> Result<()> {
    let mut stale = Vec::new();
    for a in artifacts {
        let path = root.join(&a.path);
        match std::fs::read_to_string(&path) {
            Ok(on_disk) if on_disk == a.content => {}
            Ok(_) => stale.push(format!("{} (differs)", a.path)),
            Err(_) => stale.push(format!("{} (missing)", a.path)),
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(Error::Stale { files: stale })
    }
}

/// The capability reference: the builtin capabilities in full, the declarative kinds by
/// rule. Declarative objects are not enumerated here because that inventory belongs to
/// the repository's own state, changes with it, and is answered live by
/// `majordomus capabilities list`.
fn reference(registry: &CapabilityRegistry, version: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("<!-- {HEADER}\n     Source: the canonical Majordomus capability registry; regenerate with `majordomus generate`\n     Generator: majordomus-cli {version} -->\n"));
    s.push_str("# Capability reference\n\n");
    s.push_str("Every capability this executable ships, as the registry holds it. MCP tools and resources, HTTP routes, the OpenAPI document (`openapi.json` beside this file, and `/openapi.json` when serving), Swagger UI and the command line's `capabilities` commands are projections of the same entries; nothing below is declared anywhere else.\n\n");
    s.push_str("## Executable capabilities\n\n");
    s.push_str("| id | stability | MCP tool | MCP resource | HTTP | CLI | provenance |\n|---|---|---|---|---|---|---|\n");
    let builtin: Vec<_> = registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .collect();
    for c in &builtin {
        let mcp_tool = c
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.tool.clone())
            .map(|t| format!("`{t}`"))
            .unwrap_or("—".into());
        let mcp_res = c
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .map(|r| format!("`{}`", r.uri))
            .unwrap_or("—".into());
        let http = c
            .exposure
            .http
            .as_ref()
            .map(|h| format!("`{} {}`", h.method.as_str(), h.path))
            .unwrap_or("—".into());
        let cli = c
            .exposure
            .cli
            .as_ref()
            .map(|x| format!("`majordomus {}`", x.path.join(" ")))
            .unwrap_or("—".into());
        let stability = serde_json::to_value(c.stability)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            c.id, stability, mcp_tool, mcp_res, http, cli, c.provenance
        ));
    }
    s.push('\n');
    for c in &builtin {
        s.push_str(&format!(
            "### `{}` — {}\n\n{}\n\n",
            c.id, c.title, c.description
        ));
        let (props, required) = c.input.properties();
        if props.is_empty() {
            s.push_str("Input: none.\n\n");
        } else {
            s.push_str("| input | type | required | description |\n|---|---|---|---|\n");
            for (name, schema) in props {
                let ty = match schema.get("type") {
                    Some(serde_json::Value::String(t)) => t.clone(),
                    Some(serde_json::Value::Array(a)) => a
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" or "),
                    _ => schema
                        .get("$ref")
                        .and_then(|r| r.as_str())
                        .map(|r| r.rsplit('/').next().unwrap_or(r).to_string())
                        .unwrap_or("object".into()),
                };
                let desc = schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .replace('|', "\\|");
                s.push_str(&format!(
                    "| `{name}` | {ty} | {} | {desc} |\n",
                    if required.contains(&name) {
                        "yes"
                    } else {
                        "no"
                    }
                ));
            }
            s.push('\n');
        }
        s.push_str(&format!(
            "Output: `{}`.\n\n",
            c.output.name.as_deref().unwrap_or("object")
        ));
    }
    s.push_str("## Declarative resources\n\n");
    s.push_str("Every object of the repository's AI layer is a capability of kind `resource` with the id `<kind>.<identity>` (`rule.majordomus.scope-integrity@1`, `prompt.continue`, `document.docs/CLI.md`), exposed as the MCP resource `majordomus://<kind>/<identity>` and read over HTTP through `objects.get`. They are not listed here: they are the repository's, not the executable's, and `majordomus capabilities list --kind resource` answers for the repository at hand.\n\n");
    s.push_str("## Infrastructure routes\n\n");
    s.push_str("The HTTP projection's own routes, not capabilities: ");
    s.push_str(
        &openapi::INFRASTRUCTURE_ROUTES
            .iter()
            .map(|r| format!("`{r}`"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    s.push_str(
        ". `/docs` is a Swagger UI shell that loads `/openapi.json`; it embeds no specification.\n",
    );
    let _ = CapabilityKind::Query; // the kind vocabulary is documented in docs/CAPABILITIES.md
    s
}

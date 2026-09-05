//! Generated projections that are committed for review: the OpenAPI document and the
//! capability reference. Both come from the registry through this one pipeline; the
//! committed files are caches of it, never sources, and `--check` says when they are
//! stale.

use std::path::{Path, PathBuf};

use crate::capability::{CapabilityKind, CapabilityRegistry, Provenance};
use crate::error::{Error, Result};
use crate::http::openapi;

/// Where generated artifacts live, relative to the repository root.
pub const OUT_DIR: &str = "docs/generated";

pub const HEADER: &str = "GENERATED FILE — DO NOT EDIT DIRECTLY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    OpenApi,
    Docs,
}

impl Target {
    pub const ALL: &'static [Target] = &[Target::OpenApi, Target::Docs];

    pub fn file_name(self) -> &'static str {
        match self {
            Target::OpenApi => "openapi.json",
            Target::Docs => "capabilities.md",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Repository-relative path.
    pub path: String,
    pub content: String,
}

/// Every artifact of the selected targets, derived from the registry.
pub fn artifacts(
    registry: &CapabilityRegistry,
    version: &str,
    targets: &[Target],
) -> Result<Vec<Artifact>> {
    let mut out = Vec::new();
    for t in targets {
        let content = match t {
            Target::OpenApi => openapi::render(
                &openapi::document(registry, version).map_err(|reason| Error::Http { reason })?,
            ),
            Target::Docs => reference(registry, version),
        };
        out.push(Artifact {
            path: format!("{OUT_DIR}/{}", t.file_name()),
            content,
        });
    }
    Ok(out)
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

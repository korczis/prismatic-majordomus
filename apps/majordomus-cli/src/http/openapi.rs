//! The OpenAPI projection: an OpenAPI 3.1 document built from the registry and nothing
//! else. `operationId` is the canonical id; every operation carries the canonical id,
//! kind, stability and the other projections as `x-majordomus-*` extensions so that a
//! reader can reconcile it against MCP and the CLI. Deterministic: every map is sorted,
//! nothing carries a timestamp.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::capability::{CanonicalSchema, CapabilityRegistry, HttpMethod};

use super::router::ErrorBody;

/// The OpenAPI version emitted.
pub const OPENAPI_VERSION: &str = "3.1.0";

/// The schema dialect declared: OpenAPI 3.1's base dialect, which is JSON Schema 2020-12
/// plus the OAS vocabulary, and the only value Swagger UI accepts without a warning.
pub const OAS_DIALECT: &str = "https://spec.openapis.org/oas/3.1/dialect/base";

/// The routes that are the projection's own, not capabilities.
pub const INFRASTRUCTURE_ROUTES: &[&str] = &["/", "/openapi.json", "/docs"];

/// The OpenAPI document of the registry. Fails only on a schema component name defined twice with different content.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry};
/// use majordomus_cli::http::openapi::document;
/// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
/// let doc = document(&registry, "0.0.0").unwrap();
/// assert_eq!(doc["openapi"], "3.1.0");
/// assert_eq!(doc["paths"]["/api/v1/object"]["get"]["operationId"], "objects.get");
/// assert_eq!(doc["paths"]["/api/v1/object"]["get"]["x-majordomus-mcp"]["tool"], "majordomus_get");
/// ```
pub fn document(registry: &CapabilityRegistry, version: &str) -> Result<Value, String> {
    let mut components: BTreeMap<String, Value> = BTreeMap::new();
    let error_ref = CanonicalSchema::of::<ErrorBody>().openapi_ref(&mut components)?;
    let mut paths: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    for c in registry.iter() {
        let Some(http) = &c.exposure.http else {
            continue;
        };
        let output_ref = c.output.openapi_ref(&mut components)?;
        let mut op = serde_json::Map::new();
        op.insert("operationId".into(), json!(c.id));
        op.insert("summary".into(), json!(c.title));
        op.insert("description".into(), json!(c.description));
        op.insert("tags".into(), json!([c.id.namespace()]));
        match http.method {
            HttpMethod::Get => {
                let input = c.input.for_openapi(&mut components)?;
                let (props, required) = CanonicalSchema {
                    name: None,
                    schema: input,
                }
                .properties();
                let params: Vec<Value> = props
                    .into_iter()
                    .map(|(name, mut schema)| {
                        let description = schema.get("description").cloned();
                        if let Value::Object(m) = &mut schema {
                            m.remove("description");
                        }
                        let mut p = json!({ "name": name, "in": "query", "required": required.contains(&name), "schema": schema });
                        if let Some(d) = description {
                            p["description"] = d;
                        }
                        p
                    })
                    .collect();
                op.insert("parameters".into(), Value::Array(params));
            }
            HttpMethod::Post => {
                let input_ref = c.input.openapi_ref(&mut components)?;
                op.insert("requestBody".into(), json!({ "required": true, "content": { "application/json": { "schema": input_ref } } }));
            }
        }
        op.insert(
            "responses".into(),
            json!({
                "200": { "description": format!("{}: the result", c.title), "content": { "application/json": { "schema": output_ref } } },
                "default": { "description": "An error: invalid input (400), not found (404), refused (422) or internal (500)", "content": { "application/json": { "schema": error_ref } } }
            }),
        );
        op.insert("x-majordomus-id".into(), json!(c.id));
        op.insert("x-majordomus-kind".into(), json!(c.kind));
        op.insert("x-majordomus-stability".into(), json!(c.stability));
        op.insert("x-majordomus-provenance".into(), json!(c.provenance));
        if let Some(mcp) = &c.exposure.mcp {
            op.insert("x-majordomus-mcp".into(), json!(mcp));
        }
        if let Some(cli) = &c.exposure.cli {
            op.insert("x-majordomus-cli".into(), json!(cli.path.join(" ")));
        }
        paths
            .entry(http.path.clone())
            .or_default()
            .insert(http.method.as_str().to_lowercase(), Value::Object(op));
    }
    Ok(json!({
        "openapi": OPENAPI_VERSION,
        "jsonSchemaDialect": OAS_DIALECT,
        "info": {
            "title": "Majordomus",
            "version": version,
            "description": "The read-only HTTP projection of the Majordomus capability registry. Every operation is a capability; operationId is its canonical id; the same capability is reachable through MCP and the command line where its exposure says so.",
            "license": { "name": "MIT" }
        },
        "servers": [{ "url": "/" }],
        "paths": paths,
        "components": { "schemas": components },
        "x-majordomus": {
            "generator": format!("majordomus-cli {version}"),
            "infrastructure": INFRASTRUCTURE_ROUTES,
            "binding": "GET binds every top-level input property as a query parameter; POST binds the input as the JSON body"
        }
    }))
}

/// Serialise deterministically, pretty, with a trailing newline.
pub fn render(doc: &Value) -> String {
    let mut s = serde_json::to_string_pretty(doc).unwrap_or_else(|_| doc.to_string());
    s.push('\n');
    s
}

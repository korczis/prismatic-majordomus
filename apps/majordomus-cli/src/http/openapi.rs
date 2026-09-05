//! The OpenAPI projection: an OpenAPI 3.1 document built from the registry and nothing
//! else. `operationId` is the canonical id; every operation carries the canonical id,
//! kind, stability, policies and the other projections as `x-majordomus-*` extensions so
//! that a reader can reconcile it against MCP and the CLI. The tags are the modules; the
//! examples are the capabilities' own benchmark cases, so an operation without an example
//! is an operation without a case, and the executable will not compile in that state; the
//! error responses are the statuses the router maps [`crate::capability::CapabilityError`]
//! to, by kind. Deterministic: every map is sorted, nothing carries a timestamp.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::about;
use crate::capability::{
    CanonicalSchema, CapabilityKind, CapabilityRegistry, CaseContext, HttpMethod,
};

use super::router::ErrorBody;

/// The OpenAPI version emitted.
pub const OPENAPI_VERSION: &str = "3.1.0";

/// The schema dialect declared: OpenAPI 3.1's base dialect, which is JSON Schema 2020-12
/// plus the OAS vocabulary, and the only value Swagger UI accepts without a warning.
pub const OAS_DIALECT: &str = "https://spec.openapis.org/oas/3.1/dialect/base";

/// The routes that are the projection's own, not capabilities.
pub const INFRASTRUCTURE_ROUTES: &[&str] = &["/", "/openapi.json", "/docs", "/mcp"];

/// The error statuses the router answers, by code, with the reason each one is given.
/// `refused` is a command's alone: a query has nothing to refuse.
const ERROR_STATUSES: &[(&str, &str, &str)] = &[
    ("400", "invalid_input", "The input does not fit the schema: a parameter of the wrong type, an unknown one, or a value the capability rejects."),
    ("404", "not_found", "The input names something the repository does not hold."),
    ("422", "refused", "The command was understood and turned down for the reason the message gives."),
    ("500", "internal", "The capability failed for a reason of its own; the message names it."),
];

/// The OpenAPI document of the registry. `cases` is the repository the examples are
/// drawn from: every operation's examples are its capability's benchmark cases against
/// that index; `None` renders the document without examples, for a registry that has no
/// repository behind it. Fails only on a schema component name defined twice with
/// different content.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry};
/// use majordomus_cli::http::openapi::document;
/// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
/// let doc = document(&registry, "0.0.0", None).unwrap();
/// assert_eq!(doc["openapi"], "3.1.0");
/// assert_eq!(doc["paths"]["/api/v1/object"]["get"]["operationId"], "objects.get");
/// assert_eq!(doc["paths"]["/api/v1/object"]["get"]["x-majordomus-mcp"]["tool"], "majordomus_get");
/// assert_eq!(doc["paths"]["/api/v1/object"]["get"]["tags"], serde_json::json!(["objects"]));
/// assert!(doc["tags"].as_array().unwrap().iter().any(|t| t["name"] == "objects"));
/// assert!(doc["paths"]["/api/v1/object"]["get"]["responses"]["404"].is_object());
/// assert!(doc["paths"]["/api/v1/object"]["get"]["responses"].get("422").is_none(), "a query has nothing to refuse");
/// assert!(doc["paths"]["/api/v1/peers/announce"]["post"]["responses"]["422"].is_object());
/// ```
pub fn document(
    registry: &CapabilityRegistry,
    version: &str,
    cases: Option<&CaseContext<'_>>,
) -> Result<Value, String> {
    let _phase = crate::perf::phase(crate::perf::Phase::OpenApiBuild);
    crate::perf::Counters::bump(&crate::perf::COUNTERS.openapi_builds);
    let mut components: BTreeMap<String, Value> = BTreeMap::new();
    let error_ref = CanonicalSchema::of::<ErrorBody>().openapi_ref(&mut components)?;
    let mut paths: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    let mut tagged: BTreeMap<String, usize> = BTreeMap::new();
    for c in registry.iter() {
        let Some(http) = &c.exposure.http else {
            continue;
        };
        let output_ref = c.output.openapi_ref(&mut components)?;
        let examples: Vec<(String, Value)> = match cases {
            Some(ctx) => registry
                .cases(c.id.as_str())
                .map(|provider| {
                    provider(ctx)
                        .into_iter()
                        .map(|case| (case.name.to_string(), case.input))
                        .collect()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let mut op = Map::new();
        op.insert("operationId".into(), json!(c.id));
        op.insert("summary".into(), json!(c.title));
        op.insert("description".into(), json!(c.description));
        op.insert("tags".into(), json!([c.id.namespace()]));
        *tagged.entry(c.id.namespace().to_string()).or_default() += 1;
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
                    .map(|(name, schema)| {
                        query_parameter(&name, schema, required.contains(&name), &examples)
                    })
                    .collect();
                op.insert("parameters".into(), Value::Array(params));
            }
            HttpMethod::Post => {
                let input_ref = c.input.openapi_ref(&mut components)?;
                let mut content = json!({ "schema": input_ref });
                if !examples.is_empty() {
                    content["examples"] =
                        named_examples(examples.iter().map(|(n, v)| (n.as_str(), v.clone())));
                }
                op.insert(
                    "requestBody".into(),
                    json!({ "required": true, "description": format!("The input of `{}`.", c.id), "content": { "application/json": content } }),
                );
            }
        }
        op.insert(
            "responses".into(),
            responses(c.kind, &c.title, &output_ref, &error_ref),
        );
        op.insert("x-majordomus-id".into(), json!(c.id));
        op.insert("x-majordomus-kind".into(), json!(c.kind));
        op.insert("x-majordomus-stability".into(), json!(c.stability));
        op.insert("x-majordomus-provenance".into(), json!(c.provenance));
        op.insert("x-majordomus-benchmark".into(), json!(c.benchmark));
        op.insert("x-majordomus-cache".into(), json!(c.cache));
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
    // the tags are the modules that put something on the wire, in the modules' own words
    let tags: Vec<Value> = registry
        .modules()
        .filter(|m| tagged.contains_key(m.id.as_str()))
        .map(|m| json!({ "name": m.id, "description": m.description }))
        .collect();
    Ok(json!({
        "openapi": OPENAPI_VERSION,
        "jsonSchemaDialect": OAS_DIALECT,
        "info": {
            "title": about::NAME,
            "version": version,
            "summary": about::SUMMARY,
            "description": about::description(),
            "license": { "name": about::LICENSE, "identifier": about::LICENSE },
            "contact": { "name": about::NAME, "url": about::REPOSITORY }
        },
        "externalDocs": { "description": "The API reference, rendered from this document.", "url": about::REFERENCE_URL },
        "servers": [{ "url": "/", "description": "The shared server `majordomus mcp` binds, or `majordomus serve`; loopback, the port in its log." }],
        "tags": tags,
        "paths": paths,
        "components": { "schemas": components },
        "x-majordomus": {
            "generator": format!("majordomus-cli {version}"),
            "infrastructure": INFRASTRUCTURE_ROUTES,
            "binding": "GET binds every top-level input property as a query parameter; POST binds the input as the JSON body",
            "errors": ERROR_STATUSES.iter().map(|(status, code, reason)| json!({ "status": status, "code": code, "reason": reason })).collect::<Vec<_>>()
        }
    }))
}

/// One query parameter from an input property. A query string carries text, never
/// `null`, so an optional property's nullability and `null` default are the schema's way
/// of saying "absent", which `required: false` already says; the parameter's examples are
/// the cases that set it.
fn query_parameter(
    name: &str,
    mut schema: Value,
    required: bool,
    cases: &[(String, Value)],
) -> Value {
    let description = schema.get("description").cloned();
    if let Value::Object(m) = &mut schema {
        m.remove("description");
        if let Some(Value::Array(types)) = m.get("type").cloned() {
            let kept: Vec<Value> = types.into_iter().filter(|t| t != "null").collect();
            m.insert(
                "type".into(),
                match kept.len() {
                    1 => kept[0].clone(),
                    _ => Value::Array(kept),
                },
            );
        }
        if m.get("default") == Some(&Value::Null) {
            m.remove("default");
        }
    }
    let mut p = json!({ "name": name, "in": "query", "required": required, "schema": schema });
    if let Some(d) = description {
        p["description"] = d;
    }
    let set = cases.iter().filter_map(|(case, input)| {
        input
            .get(name)
            .filter(|v| !v.is_null())
            .map(|v| (case.as_str(), v.clone()))
    });
    let examples = named_examples(set);
    if examples.as_object().is_some_and(|m| !m.is_empty()) {
        p["examples"] = examples;
    }
    p
}

/// OpenAPI's `examples` map: case name to `{ "value": ... }`, sorted.
fn named_examples<'a>(cases: impl Iterator<Item = (&'a str, Value)>) -> Value {
    let map: Map<String, Value> = cases
        .map(|(name, value)| (name.to_string(), json!({ "value": value })))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    Value::Object(map)
}

/// The responses of one operation: the result, one entry per error status the router can
/// answer for this kind, and `default` for anything the transport adds (405, an
/// unparseable request).
fn responses(kind: CapabilityKind, title: &str, output_ref: &Value, error_ref: &Value) -> Value {
    let mut map = Map::new();
    map.insert(
        "200".into(),
        json!({ "description": format!("{title}: the result."), "content": { "application/json": { "schema": output_ref } } }),
    );
    for (status, code, reason) in ERROR_STATUSES {
        if *code == "refused" && kind != CapabilityKind::Command {
            continue;
        }
        map.insert(
            (*status).into(),
            json!({ "description": format!("`{code}`: {reason}"), "content": { "application/json": { "schema": error_ref } } }),
        );
    }
    map.insert(
        "default".into(),
        json!({ "description": "Any other failure the transport reports: 405 `method_not_allowed` for another method on this path, 400 `invalid_input` for a body that is not a JSON object.", "content": { "application/json": { "schema": error_ref } } }),
    );
    Value::Object(map)
}

/// Serialise deterministically, pretty, with a trailing newline.
pub fn render(doc: &Value) -> String {
    let mut s = serde_json::to_string_pretty(doc).unwrap_or_else(|_| doc.to_string());
    s.push('\n');
    s
}

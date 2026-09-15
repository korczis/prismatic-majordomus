//! One schema per type, and every projection reads it.
//!
//! A [`CanonicalSchema`] is a JSON Schema (draft 2020-12, as schemars emits it) derived
//! from the Rust type that the handler actually deserialises and serialises. That is the
//! whole architectural point: MCP's `inputSchema`, the OpenAPI operation's parameters and
//! its `components/schemas` entry, and the Cockpit runner's form are four *renderings* of
//! this value, produced by [`CanonicalSchema::for_mcp`] and
//! [`CanonicalSchema::for_openapi`]. None of them describes a payload of its own, so none
//! of them can describe it differently from the code that reads it.
//!
//! # The two dialects
//!
//! MCP takes the schema inline. OpenAPI wants named components and `$ref`s, so
//! [`CanonicalSchema::for_openapi`] hoists the type into a `components` map under the
//! name schemars gave it and answers with the reference — which is why the component
//! namespace is flat and why two types with one title are a build-time conflict rather
//! than a document that silently describes one of them twice.
//!
//! # One shape under two names
//!
//! A `#[serde(transparent)]` newtype — a module's input named for its operation over a
//! domain type named for the domain — has a schema whose root is a `$ref` into its own
//! `$defs`. [`CanonicalSchema::resolved`] follows that reference, and every projection
//! that binds properties reads it through [`CanonicalSchema::properties`], so the wrapper
//! names the component and the inner type supplies the shape.
//!
//! # Query strings
//!
//! An HTTP `GET` binds each top-level property from the query string, where everything is
//! a string. [`coerce`] is the one place that turns `"12"` into a number and `"true"` into
//! a boolean, guided by the property's own schema, so a route cannot invent a coercion the
//! schema does not describe.
//!
//! ```
//! use majordomus_cli::capability::CanonicalSchema;
//! use schemars::JsonSchema;
//!
//! /// A search over the index.
//! #[derive(JsonSchema)]
//! struct SearchInput {
//!     /// What to look for.
//!     query: String,
//!     /// How many hits at most.
//!     limit: u32,
//! }
//!
//! let schema = CanonicalSchema::of::<SearchInput>();
//! assert_eq!(schema.name.as_deref(), Some("SearchInput"));
//!
//! // the properties every projection binds, derived once
//! let (properties, required) = schema.properties();
//! let names: Vec<&str> = properties.iter().map(|(n, _)| n.as_str()).collect();
//! assert!(names.contains(&"query") && names.contains(&"limit"));
//! assert!(required.contains(&"query".to_string()));
//!
//! // MCP takes it inline; OpenAPI takes a reference and the component beside it
//! assert!(schema.for_mcp()["properties"]["query"].is_object());
//! let mut components = std::collections::BTreeMap::new();
//! let reference = schema.openapi_ref(&mut components).unwrap();
//! assert_eq!(reference["$ref"], "#/components/schemas/SearchInput");
//! assert!(components.contains_key("SearchInput"));
//! ```

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON Schema with, when the type has one, a stable component name (the type's title).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CanonicalSchema {
    /// The component name projections use (`RepositoryInfo`); `None` for an anonymous
    /// schema such as the empty input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The schema itself, without `$schema`; nested types live under `$defs`.
    pub schema: Value,
}

impl CanonicalSchema {
    /// The schema of a Rust type.
    ///
    /// ```
    /// use majordomus_cli::capability::CanonicalSchema;
    /// use schemars::JsonSchema;
    ///
    /// /// The thing.
    /// #[derive(JsonSchema)]
    /// struct Thing {
    ///     /// A name.
    ///     name: String,
    ///     count: Option<u32>,
    /// }
    /// let s = CanonicalSchema::of::<Thing>();
    /// assert_eq!(s.name.as_deref(), Some("Thing"));
    /// let (props, required) = s.properties();
    /// assert_eq!(props.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["name", "count"]);
    /// assert_eq!(required, ["name"]);
    /// ```
    pub fn of<T: JsonSchema>() -> Self {
        crate::perf::Counters::bump(&crate::perf::COUNTERS.schema_generations);
        let mut schema = schemars::schema_for!(T).to_value();
        let name = schema
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Value::Object(m) = &mut schema {
            m.remove("$schema");
        }
        // schemars copies the doc comment into `description` verbatim, and every consumer
        // of that field — OpenAPI, MCP, Swagger UI, the published site — reads it as
        // CommonMark. Rustdoc is not CommonMark, so the dialect is translated here, at the
        // one place a doc comment becomes a projection, rather than by each projection.
        super::rustdoc::translate_descriptions(&mut schema);
        CanonicalSchema { name, schema }
    }

    /// No input: an object with no properties and none allowed.
    pub fn empty() -> Self {
        CanonicalSchema {
            name: None,
            schema: serde_json::json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    /// This schema with a root `$ref` followed into its own `$defs`: the object the type
    /// describes, rather than the reference that names it.
    ///
    /// A `#[serde(transparent)]` newtype is one shape under two names, and that is how
    /// schemars writes it: the wrapper's title and description at the root, the shape
    /// under `$defs`, and nothing but a `$ref` between them. Every projection that binds
    /// *properties* — a `GET` operation's query parameters, the router's coercion of the
    /// query string, the Cockpit's form, the generated module reference — reads the root,
    /// and a root that is a reference has no properties. Unfollowed, the document says the
    /// operation takes nothing while the route still binds every field of the inner
    /// struct, and the router coerces each one against an empty schema: every integer and
    /// boolean parameter then arrives as a string and the call is refused as invalid.
    ///
    /// The `$defs` travel with the resolved schema, so a nested reference still resolves
    /// and [`for_openapi`](Self::for_openapi) still hoists every definition.
    ///
    /// ```
    /// use majordomus_cli::capability::CanonicalSchema;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// /// What to look for.
    /// #[derive(Serialize, Deserialize, JsonSchema)]
    /// struct SearchInput {
    ///     /// The words.
    ///     query: String,
    ///     /// How many hits at most.
    ///     limit: Option<u32>,
    /// }
    ///
    /// /// The input of `fixture.search`.
    /// #[derive(Serialize, Deserialize, JsonSchema)]
    /// #[serde(transparent)]
    /// struct Wrapper(SearchInput);
    ///
    /// let schema = CanonicalSchema::of::<Wrapper>();
    /// // the wrapper names the component, and its root is a reference
    /// assert_eq!(schema.name.as_deref(), Some("Wrapper"));
    /// assert_eq!(schema.schema["$ref"], "#/$defs/SearchInput");
    /// // and the properties every projection binds are the inner type's
    /// let (properties, required) = schema.properties();
    /// let names: Vec<&str> = properties.iter().map(|(n, _)| n.as_str()).collect();
    /// assert_eq!(names, ["query", "limit"]);
    /// assert_eq!(required, ["query"]);
    /// // a schema whose root is already the object is its own resolution
    /// let plain = CanonicalSchema::of::<SearchInput>();
    /// assert_eq!(plain.resolved().schema, plain.schema);
    /// ```
    pub fn resolved(&self) -> Cow<'_, CanonicalSchema> {
        let target = self
            .schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.strip_prefix("#/$defs/"));
        let definition = target.and_then(|t| {
            self.schema
                .get("$defs")
                .and_then(Value::as_object)
                .and_then(|defs| defs.get(t))
                .cloned()
        });
        let Some(mut schema) = definition else {
            return Cow::Borrowed(self);
        };
        if let (Value::Object(m), Some(defs)) = (&mut schema, self.schema.get("$defs")) {
            m.insert("$defs".into(), defs.clone());
        }
        Cow::Owned(CanonicalSchema {
            name: self.name.clone(),
            schema,
        })
    }

    /// Top-level properties with their schemas, and which are required, for a schema that
    /// describes an object. Order is the schema's. A root `$ref` is followed first, so a
    /// transparent newtype yields the properties of the type it wraps.
    pub fn properties(&self) -> (Vec<(String, Value)>, Vec<String>) {
        let resolved = self.resolved();
        let props = resolved
            .schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let required = resolved
            .schema
            .get("required")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        (props, required)
    }

    /// The schema as an MCP client expects it: self-contained, `$defs` kept inline.
    pub fn for_mcp(&self) -> Value {
        self.schema.clone()
    }

    /// Split into the top-level schema and its named definitions with every `$ref`
    /// rewritten to `#/components/schemas/<name>`, for OpenAPI. A definition name that
    /// is already registered with different content is a conflict, reported by name.
    pub fn for_openapi(&self, components: &mut BTreeMap<String, Value>) -> Result<Value, String> {
        let mut top = self.schema.clone();
        let defs = match &mut top {
            Value::Object(m) => m.remove("$defs").and_then(|d| match d {
                Value::Object(d) => Some(d),
                _ => None,
            }),
            _ => None,
        };
        rewrite_refs(&mut top);
        if let Some(defs) = defs {
            for (name, mut def) in defs {
                rewrite_refs(&mut def);
                register(components, name, def)?;
            }
        }
        Ok(top)
    }

    /// Register this schema under its name and return a `$ref` to it; an anonymous schema
    /// is returned inline.
    pub fn openapi_ref(&self, components: &mut BTreeMap<String, Value>) -> Result<Value, String> {
        let top = self.for_openapi(components)?;
        match &self.name {
            Some(name) => {
                let mut stripped = top;
                if let Value::Object(m) = &mut stripped {
                    m.remove("title");
                }
                register(components, name.clone(), stripped)?;
                Ok(serde_json::json!({ "$ref": format!("#/components/schemas/{name}") }))
            }
            None => Ok(top),
        }
    }
}

fn register(
    components: &mut BTreeMap<String, Value>,
    name: String,
    def: Value,
) -> Result<(), String> {
    match components.get(&name) {
        Some(existing) if *existing != def => Err(format!(
            "schema component '{name}' is defined twice with different content"
        )),
        Some(_) => Ok(()),
        None => {
            components.insert(name, def);
            Ok(())
        }
    }
}

fn rewrite_refs(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(r)) = m.get_mut("$ref") {
                if let Some(name) = r.strip_prefix("#/$defs/") {
                    *r = format!("#/components/schemas/{name}");
                }
            }
            for (_, child) in m.iter_mut() {
                rewrite_refs(child);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(rewrite_refs),
        _ => {}
    }
}

/// Every type a property schema names, following the one level of `anyOf`/`oneOf` that
/// schemars writes for an `Option<T>` of a structured type. A property whose type cannot be
/// read names none, and the caller leaves the value a string.
fn declared_types(property: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut collect = |v: Option<&Value>| match v {
        Some(Value::String(s)) => out.push(s.clone()),
        Some(Value::Array(a)) => out.extend(a.iter().filter_map(Value::as_str).map(String::from)),
        _ => {}
    };
    collect(property.get("type"));
    for key in ["anyOf", "oneOf"] {
        for member in property
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect(member.get("type"));
        }
    }
    out
}

/// The JSON type word for a value, for a message a caller can act on.
fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Convert a query-string value to the JSON type a property schema names: integers and
/// booleans are parsed, a structured parameter is read as the JSON text the binding sent,
/// and everything else stays a string.
///
/// The array and object cases are the exact inverse of
/// [`Request::bind`](crate::http::Request::bind), which puts a non-scalar on the query
/// string as JSON. Without them a `GET` route could be *sent* a list and could not *read*
/// one, which is a projection that only works in one direction — and the generic test that
/// replays every capability's benchmark cases over the wire is where that showed.
///
/// ```
/// use majordomus_cli::capability::schema::coerce;
/// use serde_json::json;
/// assert_eq!(coerce(&json!({ "type": "integer" }), "7").unwrap(), json!(7));
/// assert_eq!(coerce(&json!({ "type": "string" }), "7").unwrap(), json!("7"));
/// assert!(coerce(&json!({ "type": "boolean" }), "yes").is_err());
///
/// // a list parameter arrives as the JSON the binding wrote, nullable or not
/// let list = json!({ "type": ["array", "null"], "items": { "type": "string" } });
/// assert_eq!(coerce(&list, r#"["a","b"]"#).unwrap(), json!(["a", "b"]));
/// // and a value that is not that list is refused rather than passed on as a string
/// assert!(coerce(&list, "a,b").is_err());
/// ```
pub fn coerce(property: &Value, raw: &str) -> Result<Value, String> {
    let types = declared_types(property);
    let types: Vec<&str> = types.iter().map(String::as_str).collect();
    if types.contains(&"array") || types.contains(&"object") {
        let parsed: Value = serde_json::from_str(raw)
            .map_err(|e| format!("'{raw}' is not the JSON this parameter takes: {e}"))?;
        let fits = (parsed.is_array() && types.contains(&"array"))
            || (parsed.is_object() && types.contains(&"object"))
            || (parsed.is_null() && types.contains(&"null"));
        return if fits {
            Ok(parsed)
        } else {
            Err(format!(
                "'{raw}' parses as {}, and this parameter takes {}",
                kind_of(&parsed),
                types.join(" or ")
            ))
        };
    }
    if types.contains(&"integer") {
        return raw
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| format!("'{raw}' is not an integer"));
    }
    if types.contains(&"number") {
        return raw
            .parse::<f64>()
            .map(Value::from)
            .map_err(|_| format!("'{raw}' is not a number"));
    }
    if types.contains(&"boolean") {
        return match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format!("'{raw}' is not a boolean")),
        };
    }
    Ok(Value::String(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(JsonSchema)]
    #[allow(dead_code)]
    struct Inner {
        n: u32,
    }
    /// Doc comment becomes the description.
    #[derive(JsonSchema)]
    #[allow(dead_code)]
    struct Outer {
        /// A name.
        name: String,
        inner: Inner,
        maybe: Option<u64>,
    }

    #[test]
    fn schema_of_a_type_has_name_defs_and_properties() {
        let s = CanonicalSchema::of::<Outer>();
        assert_eq!(s.name.as_deref(), Some("Outer"));
        assert!(s.schema.get("$schema").is_none());
        let (props, required) = s.properties();
        assert_eq!(
            props.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["name", "inner", "maybe"]
        );
        assert_eq!(required, ["name", "inner"]);
        assert_eq!(
            s.schema["description"],
            "Doc comment becomes the description."
        );
    }

    #[test]
    fn openapi_projection_hoists_defs_and_rewrites_refs() {
        let s = CanonicalSchema::of::<Outer>();
        let mut components = BTreeMap::new();
        let r = s.openapi_ref(&mut components).unwrap();
        assert_eq!(
            r,
            serde_json::json!({ "$ref": "#/components/schemas/Outer" })
        );
        assert!(components.contains_key("Inner"));
        assert_eq!(
            components["Outer"]["properties"]["inner"]["$ref"],
            "#/components/schemas/Inner"
        );
        assert!(components["Outer"].get("$defs").is_none());
        // the same type twice is fine; a different type with the same name is not
        assert!(s.openapi_ref(&mut components).is_ok());
        components.insert("Inner".into(), serde_json::json!({ "type": "string" }));
        assert!(s.openapi_ref(&mut components).is_err());
    }

    /// A module's input over a domain type: one shape, the wrapper's name.
    #[derive(JsonSchema)]
    #[serde(transparent)]
    #[allow(dead_code)]
    struct Wrapped(Outer);

    #[test]
    fn a_transparent_newtype_projects_the_shape_it_wraps() {
        let s = CanonicalSchema::of::<Wrapped>();
        // the root is a reference, and the name is still the wrapper's
        assert_eq!(s.name.as_deref(), Some("Wrapped"));
        assert_eq!(s.schema["$ref"], "#/$defs/Outer");
        // the properties a projection binds are the wrapped type's, not none
        let (props, required) = s.properties();
        assert_eq!(
            props.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["name", "inner", "maybe"]
        );
        assert_eq!(required, ["name", "inner"]);
        // and a query parameter carries the wrapped type's own schema, so the router
        // coerces `maybe` to a number rather than leaving it a string
        assert_eq!(
            props.iter().find(|(k, _)| k == "maybe").unwrap().1["type"],
            serde_json::json!(["integer", "null"])
        );
        // the OpenAPI projection of the resolved schema is the object, with every nested
        // definition still hoisted
        let mut components = BTreeMap::new();
        let top = s.resolved().for_openapi(&mut components).unwrap();
        assert!(top["properties"]["name"].is_object());
        assert_eq!(
            top["properties"]["inner"]["$ref"],
            "#/components/schemas/Inner"
        );
        assert!(components.contains_key("Inner") && components.contains_key("Outer"));
        // the unresolved schema is still what names the component, for a request body
        let mut components = BTreeMap::new();
        assert_eq!(
            s.openapi_ref(&mut components).unwrap(),
            serde_json::json!({ "$ref": "#/components/schemas/Wrapped" })
        );
    }

    #[test]
    fn coercion_follows_the_property_type() {
        assert_eq!(
            coerce(&serde_json::json!({ "type": "integer" }), "7").unwrap(),
            7
        );
        assert!(coerce(&serde_json::json!({ "type": "integer" }), "x").is_err());
        assert_eq!(
            coerce(&serde_json::json!({ "type": ["integer", "null"] }), "7").unwrap(),
            7
        );
        assert_eq!(
            coerce(&serde_json::json!({ "type": "boolean" }), "true").unwrap(),
            true
        );
        assert_eq!(
            coerce(&serde_json::json!({ "type": "string" }), "7").unwrap(),
            "7"
        );
    }
}

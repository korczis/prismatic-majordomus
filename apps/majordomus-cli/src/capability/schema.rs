//! The canonical schema of an input or an output: one JSON Schema (draft 2020-12, as
//! schemars emits it) derived from the Rust type, carried as data. MCP and OpenAPI take
//! their schemas from here and nowhere else.

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
        let mut schema = schemars::schema_for!(T).to_value();
        let name = schema
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Value::Object(m) = &mut schema {
            m.remove("$schema");
        }
        CanonicalSchema { name, schema }
    }

    /// No input: an object with no properties and none allowed.
    pub fn empty() -> Self {
        CanonicalSchema {
            name: None,
            schema: serde_json::json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    /// Top-level properties with their schemas, and which are required, for a schema that
    /// describes an object. Order is the schema's.
    pub fn properties(&self) -> (Vec<(String, Value)>, Vec<String>) {
        let props = self
            .schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let required = self
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

/// Convert a query-string value to the JSON type a property schema names: integers and
/// booleans are parsed, everything else stays a string.
///
/// ```
/// use majordomus_cli::capability::schema::coerce;
/// use serde_json::json;
/// assert_eq!(coerce(&json!({ "type": "integer" }), "7").unwrap(), json!(7));
/// assert_eq!(coerce(&json!({ "type": "string" }), "7").unwrap(), json!("7"));
/// assert!(coerce(&json!({ "type": "boolean" }), "yes").is_err());
/// ```
pub fn coerce(property: &Value, raw: &str) -> Result<Value, String> {
    let ty = property.get("type");
    let types: Vec<&str> = match ty {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
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

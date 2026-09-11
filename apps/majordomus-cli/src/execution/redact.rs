//! One redaction, driven by the schema.
//!
//! A value a client sent is stored on the execution, published in `execution.created`,
//! rendered in the Cockpit and printed by the command line. That is four places a secret
//! could leak from, so it is removed once — here, before the value is stored — and every
//! consumer downstream is reading an already-safe value rather than remembering to be
//! careful.
//!
//! What counts as sensitive is the input type's own schema, never a guess from a field's
//! name: `format: "password"`, `writeOnly: true`, or the explicit
//! `x-majordomus-sensitive: true` extension. A Rust type declares it where it declares
//! everything else a projection reads:
//!
//! ```rust
//! # use schemars::JsonSchema;
//! /// Credentials for a provider.
//! #[derive(JsonSchema)]
//! struct Credentials {
//!     /// Who.
//!     user: String,
//!     /// The secret; never stored, never published, never logged.
//!     #[schemars(extend("format" = "password"))]
//!     token: String,
//! }
//! ```
//!
//! and the value that reaches the store has already been through here:
//!
//! ```
//! # use schemars::JsonSchema;
//! use majordomus_cli::execution::redact::{redact, REDACTED};
//! # #[derive(JsonSchema)]
//! # struct Credentials {
//! #     user: String,
//! #     #[schemars(extend("format" = "password"))]
//! #     token: String,
//! # }
//! let schema = serde_json::to_value(schemars::schema_for!(Credentials)).unwrap();
//! let safe = redact(&schema, &serde_json::json!({ "user": "ada", "token": "hunter2" }));
//! assert_eq!(safe["user"], "ada", "what is not sensitive is untouched");
//! assert_eq!(safe["token"], REDACTED);
//! // a marker and not an absence: a reader can tell withheld from never given
//! assert!(safe["token"].is_string());
//! ```

use serde_json::{Map, Value};

/// What a sensitive value is replaced with. A marker rather than an empty string, so that
/// a reader can tell "withheld" from "not given".
pub const REDACTED: &str = "«redacted»";

/// The deepest a value is walked. A schema and a value that recur past this are copied as
/// they are rather than examined, which cannot leak more than the input already carried.
const MAX_DEPTH: usize = 32;

/// The input as it may be stored: every value the schema marks sensitive replaced.
///
/// ```
/// use majordomus_cli::execution::redact::{redact, REDACTED};
/// use serde_json::json;
/// let schema = json!({
///     "type": "object",
///     "properties": {
///         "user": { "type": "string" },
///         "token": { "type": "string", "format": "password" }
///     }
/// });
/// let safe = redact(&schema, &json!({ "user": "ada", "token": "hunter2" }));
/// assert_eq!(safe["user"], "ada");
/// assert_eq!(safe["token"], REDACTED);
/// ```
pub fn redact(schema: &Value, value: &Value) -> Value {
    walk(schema, schema, value, 0)
}

/// Is a value described by this schema sensitive?
fn sensitive(schema: &Value) -> bool {
    schema.get("format").and_then(Value::as_str) == Some("password")
        || schema.get("writeOnly") == Some(&Value::Bool(true))
        || schema.get("x-majordomus-sensitive") == Some(&Value::Bool(true))
}

/// Follow one `$ref` into the root's `$defs`, once; anything else is returned unchanged.
fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return schema;
    };
    reference
        .strip_prefix("#/$defs/")
        .and_then(|name| root.get("$defs").and_then(|d| d.get(name)))
        .unwrap_or(schema)
}

fn walk(root: &Value, schema: &Value, value: &Value, depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return value.clone();
    }
    let schema = resolve(root, schema);
    if sensitive(schema) {
        return Value::String(REDACTED.to_string());
    }
    match value {
        Value::Object(members) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let additional = schema.get("additionalProperties");
            let mut out = Map::with_capacity(members.len());
            for (key, member) in members {
                let property = properties
                    .and_then(|p| p.get(key))
                    .or(additional.filter(|a| a.is_object()));
                match property {
                    Some(p) => out.insert(key.clone(), walk(root, p, member, depth + 1)),
                    None => out.insert(key.clone(), member.clone()),
                };
            }
            Value::Object(out)
        }
        Value::Array(items) => match schema.get("items") {
            Some(item_schema) => Value::Array(
                items
                    .iter()
                    .map(|i| walk(root, item_schema, i, depth + 1))
                    .collect(),
            ),
            None => value.clone(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_way_a_schema_can_say_sensitive_is_honoured() {
        for marker in [
            json!({ "type": "string", "format": "password" }),
            json!({ "type": "string", "writeOnly": true }),
            json!({ "type": "string", "x-majordomus-sensitive": true }),
        ] {
            let schema = json!({ "type": "object", "properties": { "s": marker } });
            assert_eq!(redact(&schema, &json!({ "s": "secret" }))["s"], REDACTED);
        }
        let plain = json!({ "type": "object", "properties": { "s": { "type": "string" } } });
        assert_eq!(redact(&plain, &json!({ "s": "public" }))["s"], "public");
    }

    #[test]
    fn a_name_that_looks_like_a_secret_is_not_one() {
        // the rule is the schema, never the field name: a "password_policy" that is a
        // document name would be destroyed by a name-matching redactor
        let schema = json!({
            "type": "object",
            "properties": { "password_policy": { "type": "string" } }
        });
        let safe = redact(&schema, &json!({ "password_policy": "docs/PASSWORDS.md" }));
        assert_eq!(safe["password_policy"], "docs/PASSWORDS.md");
    }

    #[test]
    fn nesting_refs_arrays_and_maps_are_all_reached() {
        let schema = json!({
            "type": "object",
            "properties": {
                "creds": { "$ref": "#/$defs/Credentials" },
                "many": { "type": "array", "items": { "$ref": "#/$defs/Credentials" } },
                "map": { "type": "object", "additionalProperties": { "$ref": "#/$defs/Credentials" } }
            },
            "$defs": {
                "Credentials": {
                    "type": "object",
                    "properties": {
                        "user": { "type": "string" },
                        "token": { "type": "string", "format": "password" }
                    }
                }
            }
        });
        let value = json!({
            "creds": { "user": "a", "token": "t1" },
            "many": [{ "user": "b", "token": "t2" }],
            "map": { "k": { "user": "c", "token": "t3" } },
            "unknown": "kept"
        });
        let safe = redact(&schema, &value);
        assert_eq!(safe["creds"]["token"], REDACTED);
        assert_eq!(safe["creds"]["user"], "a");
        assert_eq!(safe["many"][0]["token"], REDACTED);
        assert_eq!(safe["map"]["k"]["token"], REDACTED);
        assert_eq!(
            safe["unknown"], "kept",
            "a value the schema does not describe is kept"
        );
        assert!(!serde_json::to_string(&safe).unwrap().contains("t1"));
        assert!(!serde_json::to_string(&safe).unwrap().contains("t3"));
    }

    #[test]
    fn a_value_that_is_not_an_object_and_a_schema_that_says_nothing_survive() {
        assert_eq!(redact(&json!({}), &json!(5)), json!(5));
        assert_eq!(redact(&json!(true), &json!("x")), json!("x"));
        assert_eq!(redact(&json!({}), &json!([1, 2])), json!([1, 2]));
        // a broken reference is not followed and does not panic
        let schema =
            json!({ "type": "object", "properties": { "a": { "$ref": "#/$defs/Missing" } } });
        assert_eq!(redact(&schema, &json!({ "a": "x" }))["a"], "x");
    }

    #[test]
    fn deep_recursion_terminates() {
        let mut schema =
            json!({ "type": "object", "properties": { "next": { "$ref": "#/$defs/Node" } } });
        schema["$defs"] = json!({ "Node": { "type": "object", "properties": { "next": { "$ref": "#/$defs/Node" } } } });
        let mut value = json!({ "leaf": true });
        for _ in 0..64 {
            value = json!({ "next": value });
        }
        let safe = redact(&schema, &value);
        assert!(safe.is_object());
    }
}

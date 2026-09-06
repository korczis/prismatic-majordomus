//! The projections of a `.proto` document schema.
//!
//! The `.proto` is canonical and nothing else is written by hand. Two artifacts come out
//! of it here:
//!
//!   the JSON Schema beside it, which the Rust indexer validates front matter against and
//!   from which `share/allow/<name>.txt` is derived, so the shell tool and the executable
//!   read one contract rather than two;
//!
//!   the section list `share/sections/<name>.txt`, which is the body half of the contract
//!   in the plainest form a shell can read.
//!
//! Both are tracked, because `bin/majordomus doctor` is pure shell and must validate a
//! fresh checkout without a Rust build having ever run. `scripts/derive-check` regenerates
//! them and fails on a difference, which is what keeps "canonical" true.

use serde_json::{json, Map, Value};

use super::{Enum, Field, Message, ProtoFile, MSG_BODY, MSG_DOCUMENT, MSG_HEADER};

/// The extension carrying the schema identity on a derived JSON Schema.
pub const SCHEMA_EXTENSION: &str = "x-majordomus-schema";

/// The extension naming the `.proto` a JSON Schema was derived from. A schema that carries
/// it is not to be edited; the file it names is.
pub const DERIVED_EXTENSION: &str = "x-majordomus-derived-from";

/// The extension carrying the body contract, for a reader of the JSON Schema alone.
pub const SECTIONS_EXTENSION: &str = "x-majordomus-sections";

/// The file name a schema identity derives for its JSON Schema projection, relative to the
/// schema root: `majordomus.adr/v1` is `majordomus/adr/adr.v1.schema.json`.
pub fn schema_path(schema_id: &str) -> crate::error::Result<String> {
    let proto = ProtoFile::expected_path(schema_id)?;
    Ok(proto.replace(super::PROTO_SUFFIX, crate::share::SCHEMA_SUFFIX))
}

/// The whole document as one JSON Schema: `header` against the `Header` message, and for a
/// Markdown kind `body` against the sections the `Body` message names.
pub fn to_json_schema(file: &ProtoFile) -> crate::error::Result<Value> {
    let header = file.header()?;
    let body = file.body();

    let mut defs = Map::new();
    defs.insert(MSG_HEADER.into(), message_schema(file, header));
    // every other message the header reaches, and Section when there is a body
    for message in &file.messages {
        if message.name == MSG_HEADER || message.name == MSG_DOCUMENT || message.body {
            continue;
        }
        if message.name == "Section" && body.is_none() {
            continue;
        }
        defs.insert(message.name.clone(), message_schema(file, message));
    }

    let mut properties = Map::new();
    properties.insert(
        MSG_HEADER.to_lowercase(),
        json!({ "$ref": "#/$defs/Header" }),
    );
    let mut required = vec![json!("header")];

    if let Some(body) = body {
        defs.insert(MSG_BODY.into(), body_schema(body));
        properties.insert(MSG_BODY.to_lowercase(), json!({ "$ref": "#/$defs/Body" }));
        required.push(json!("body"));
    }

    let name = file
        .schema_id
        .split_once('/')
        .and_then(|(q, _)| q.rsplit_once('.').map(|(_, n)| n.to_string()))
        .unwrap_or_else(|| file.schema_id.clone());

    let mut root = Map::new();
    root.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert(
        "$id".into(),
        json!(format!(
            "https://majordomus.dev/schemas/{}",
            schema_path(&file.schema_id)?
        )),
    );
    root.insert(SCHEMA_EXTENSION.into(), json!(file.schema_id));
    root.insert(
        DERIVED_EXTENSION.into(),
        json!(ProtoFile::expected_path(&file.schema_id)?),
    );
    if let Some(allow) = &file.allow_list {
        root.insert(crate::generate::ALLOW_EXTENSION.into(), json!(allow));
    }
    if let Some(message) = file.message(MSG_DOCUMENT) {
        if !message.doc.is_empty() {
            root.insert("description".into(), json!(message.doc));
        }
    }
    root.insert("title".into(), json!(name));
    root.insert("type".into(), json!("object"));
    root.insert("additionalProperties".into(), json!(false));
    root.insert("required".into(), Value::Array(required));
    root.insert("properties".into(), Value::Object(properties));
    root.insert("$defs".into(), Value::Object(defs));
    Ok(Value::Object(root))
}

/// One message as a JSON Schema object.
fn message_schema(file: &ProtoFile, message: &Message) -> Value {
    let mut out = Map::new();
    if !message.doc.is_empty() {
        out.insert("description".into(), json!(message.doc));
    }
    out.insert("type".into(), json!("object"));
    out.insert("additionalProperties".into(), json!(!message.closed));
    let required: Vec<Value> = message
        .fields
        .iter()
        .filter(|f| f.options.required)
        .map(|f| json!(f.key()))
        .collect();
    if !required.is_empty() {
        out.insert("required".into(), Value::Array(required));
    }
    let mut properties = Map::new();
    for field in &message.fields {
        properties.insert(field.key().to_string(), field_schema(file, field));
    }
    out.insert("properties".into(), Value::Object(properties));
    Value::Object(out)
}

/// One field as a JSON Schema value; a repeated field is the array around it.
fn field_schema(file: &ProtoFile, field: &Field) -> Value {
    let mut inner = scalar_schema(file, field);
    if let Value::Object(map) = &mut inner {
        if !field.doc.is_empty() && !field.repeated {
            map.insert("description".into(), json!(field.doc));
            // description first reads better; serde_json preserves insertion order, so
            // rebuild with it in front
            let mut ordered = Map::new();
            ordered.insert("description".into(), json!(field.doc));
            for (k, v) in map.iter() {
                if k != "description" {
                    ordered.insert(k.clone(), v.clone());
                }
            }
            *map = ordered;
        }
    }
    if !field.repeated {
        return inner;
    }
    let mut out = Map::new();
    if !field.doc.is_empty() {
        out.insert("description".into(), json!(field.doc));
    }
    out.insert("type".into(), json!("array"));
    if let Some(n) = field.options.min_items {
        out.insert("minItems".into(), json!(n));
    }
    out.insert("items".into(), inner);
    Value::Object(out)
}

/// The type half of a field: a scalar with its constraints, an enum's closed set, or a
/// reference to another message.
fn scalar_schema(file: &ProtoFile, field: &Field) -> Value {
    if let Some(constant) = &field.options.const_value {
        return json!({ "const": constant });
    }
    if let Some(enumeration) = file.enumeration(&field.type_name) {
        return json!({ "enum": enum_values(enumeration) });
    }
    if file.message(&field.type_name).is_some() {
        return json!({ "$ref": format!("#/$defs/{}", field.type_name) });
    }
    let mut out = Map::new();
    match field.type_name.as_str() {
        "string" => {
            out.insert("type".into(), json!("string"));
            if let Some(n) = field.options.min_length {
                out.insert("minLength".into(), json!(n));
            }
            if let Some(p) = &field.options.pattern {
                out.insert("pattern".into(), json!(p));
            }
        }
        "bool" => {
            out.insert("type".into(), json!("boolean"));
        }
        "uint32" | "uint64" | "int32" | "int64" => {
            out.insert("type".into(), json!("integer"));
            let minimum =
                field.options.minimum.or_else(|| {
                    field.type_name.starts_with('u').then_some(0).filter(|_| {
                        field.options.maximum.is_some() || field.options.minimum.is_some()
                    })
                });
            if let Some(n) = minimum {
                out.insert("minimum".into(), json!(n));
            }
            if let Some(n) = field.options.maximum {
                out.insert("maximum".into(), json!(n));
            }
        }
        other => {
            // A type this reader does not model would silently validate nothing; say so in
            // the artifact rather than emit an empty schema.
            out.insert(
                "$comment".into(),
                json!(format!("unmodelled proto type '{other}'")),
            );
        }
    }
    Value::Object(out)
}

/// The written form of every value of an enum, the proto3 zero value aside.
fn enum_values(enumeration: &Enum) -> Vec<Value> {
    enumeration
        .values
        .iter()
        .filter_map(|v| v.written.as_ref())
        .map(|v| json!(v))
        .collect()
}

/// The `Body` message as a JSON Schema object, with the section contract as an extension:
/// JSON Schema has no word for "a level-two heading called Context that carries content",
/// so the mechanical shape is expressed in JSON Schema and the contract beside it.
fn body_schema(body: &Message) -> Value {
    let mut out = Map::new();
    if !body.doc.is_empty() {
        out.insert("description".into(), json!(body.doc));
    }
    out.insert("type".into(), json!("object"));
    out.insert("additionalProperties".into(), json!(false));
    out.insert("required".into(), json!(["sections"]));
    out.insert(
        "properties".into(),
        json!({
            "sections": {
                "description": "Every heading of the body, in document order, as the reader flattens it.",
                "type": "array",
                "items": { "$ref": "#/$defs/Section" }
            }
        }),
    );
    out.insert(
        SECTIONS_EXTENSION.into(),
        Value::Array(body.fields.iter().map(section_entry).collect()),
    );
    Value::Object(out)
}

fn section_entry(field: &Field) -> Value {
    let mut out = Map::new();
    out.insert(
        "heading".into(),
        json!(field.options.heading.clone().unwrap_or_default()),
    );
    out.insert("level".into(), json!(field.options.level.unwrap_or(1)));
    out.insert("required".into(), json!(field.options.required));
    out.insert("non_empty".into(), json!(field.options.non_empty));
    if !field.doc.is_empty() {
        out.insert("description".into(), json!(field.doc));
    }
    Value::Object(out)
}

/// The section list the shell tool reads: one section per line, tab-separated, in document
/// order — level, heading, `required`/`optional`, `non-empty`/`any`. A kind with no body
/// contract projects an empty file, which is an answer rather than a missing artifact.
///
/// ```
/// use majordomus_cli::proto;
/// let text = r#"
/// syntax = "proto3";
/// package majordomus.adr.v1;
/// option (majordomus.schema_id) = "majordomus.adr/v1";
/// option (majordomus.document_format) = "markdown";
/// message Header { string id = 1; }
/// message Body {
///   option (majordomus.body) = true;
///   Section context = 1 [(majordomus.heading) = "Context", (majordomus.level) = 2,
///                        (majordomus.required) = true, (majordomus.non_empty) = true];
///   Section notes = 2 [(majordomus.heading) = "Notes", (majordomus.level) = 2];
/// }
/// message Section { string heading = 1; }
/// "#;
/// let file = proto::parse("adr.v1.proto", text).unwrap();
/// assert_eq!(proto::project::section_lines(&file), [
///     "2\tContext\trequired\tnon-empty",
///     "2\tNotes\toptional\tany",
/// ]);
/// ```
pub fn section_lines(file: &ProtoFile) -> Vec<String> {
    let Some(body) = file.body() else {
        return Vec::new();
    };
    body.fields
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}",
                f.options.level.unwrap_or(1),
                f.options.heading.clone().unwrap_or_default(),
                if f.options.required {
                    "required"
                } else {
                    "optional"
                },
                if f.options.non_empty {
                    "non-empty"
                } else {
                    "any"
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
syntax = "proto3";
package majordomus.adr.v1;
import "majordomus/options.proto";
option (majordomus.schema_id) = "majordomus.adr/v1";
option (majordomus.allow_list) = "adr";
option (majordomus.document_format) = "markdown";

// The whole record.
message Document {
  Header header = 1;
  Body body = 2;
}

message Header {
  option (majordomus.closed) = true;
  // The format version.
  string schema = 1 [(majordomus.required) = true, (majordomus.const) = "adr/v1"];
  string id = 2 [(majordomus.required) = true, (majordomus.pattern) = "^adr-[0-9]{4}$"];
  string title = 3 [(majordomus.required) = true, (majordomus.min_length) = 1];
  Status status = 4 [(majordomus.required) = true];
  repeated string tags = 5 [(majordomus.pattern) = "^[a-z]+$"];
  Provenance provenance = 6;
  uint32 weight = 7 [(majordomus.minimum) = 0, (majordomus.maximum) = 9];
}

enum Status {
  STATUS_UNSPECIFIED = 0;
  STATUS_PROPOSED = 1 [(majordomus.value) = "proposed"];
  STATUS_ACCEPTED = 2 [(majordomus.value) = "accepted"];
}

message Provenance {
  option (majordomus.closed) = true;
  Origin origin = 1 [(majordomus.required) = true];
}

enum Origin {
  ORIGIN_UNSPECIFIED = 0;
  ORIGIN_AUTHORED = 1 [(majordomus.value) = "authored"];
}

message Body {
  option (majordomus.body) = true;
  Section context = 1 [(majordomus.heading) = "Context", (majordomus.level) = 2,
                       (majordomus.required) = true, (majordomus.non_empty) = true];
}

message Section {
  string heading = 1;
  uint32 level = 2;
  bool empty = 3;
}
"#;

    fn schema() -> Value {
        let file = crate::proto::parse("adr.v1.proto", SRC).expect("parses");
        to_json_schema(&file).expect("projects")
    }

    #[test]
    fn the_root_names_the_proto_it_came_from() {
        let s = schema();
        assert_eq!(s[SCHEMA_EXTENSION], json!("majordomus.adr/v1"));
        assert_eq!(s[DERIVED_EXTENSION], json!("majordomus/adr/adr.v1.proto"));
        assert_eq!(s[crate::generate::ALLOW_EXTENSION], json!("adr"));
        assert_eq!(s["required"], json!(["header", "body"]));
        assert_eq!(s["description"], json!("The whole record."));
    }

    #[test]
    fn a_const_an_enum_and_a_reference_each_project_their_own_shape() {
        let h = &schema()["$defs"]["Header"];
        assert_eq!(h["properties"]["schema"]["const"], json!("adr/v1"));
        assert_eq!(
            h["properties"]["status"]["enum"],
            json!(["proposed", "accepted"])
        );
        assert_eq!(
            h["properties"]["provenance"]["$ref"],
            json!("#/$defs/Provenance")
        );
        assert_eq!(h["additionalProperties"], json!(false));
        assert_eq!(h["required"], json!(["schema", "id", "title", "status"]));
    }

    #[test]
    fn a_repeated_field_becomes_an_array_around_its_item() {
        let tags = &schema()["$defs"]["Header"]["properties"]["tags"];
        assert_eq!(tags["type"], json!("array"));
        assert_eq!(tags["items"]["pattern"], json!("^[a-z]+$"));
    }

    #[test]
    fn constraints_and_prose_survive_the_projection() {
        let h = &schema()["$defs"]["Header"];
        assert_eq!(h["properties"]["id"]["pattern"], json!("^adr-[0-9]{4}$"));
        assert_eq!(h["properties"]["title"]["minLength"], json!(1));
        assert_eq!(h["properties"]["weight"]["maximum"], json!(9));
        assert_eq!(
            h["properties"]["schema"]["description"],
            json!("The format version.")
        );
    }

    #[test]
    fn the_body_carries_the_section_contract() {
        let body = &schema()["$defs"]["Body"];
        let sections = body[SECTIONS_EXTENSION].as_array().expect("sections");
        assert_eq!(sections[0]["heading"], json!("Context"));
        assert_eq!(sections[0]["level"], json!(2));
        assert_eq!(sections[0]["non_empty"], json!(true));
    }

    #[test]
    fn the_projected_schema_is_a_valid_json_schema_and_accepts_a_real_record() {
        let s = schema();
        let validator = jsonschema::validator_for(&s).expect("compiles");
        assert!(validator.is_valid(&json!({
            "header": { "schema": "adr/v1", "id": "adr-0001", "title": "t", "status": "proposed" },
            "body": { "sections": [] }
        })));
        assert!(!validator.is_valid(&json!({
            "header": { "schema": "adr/v1", "id": "nope", "title": "t", "status": "proposed" },
            "body": { "sections": [] }
        })));
    }

    #[test]
    fn the_allow_list_derives_from_the_header_alone() {
        let s = schema();
        assert_eq!(
            crate::generate::allow_lines(&s),
            [
                "^schema$",
                "^id$",
                "^title$",
                "^status$",
                "^tags(\\.[0-9]+)?$",
                "^provenance\\.origin$",
                "^weight$",
            ]
        );
    }
}

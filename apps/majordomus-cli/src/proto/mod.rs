//! The reader for the layer's Markdown document schemas.
//!
//! A Markdown kind of this layer is described by one `.proto` file: `Header` is the YAML
//! front matter, `Body` names the sections the Markdown body must carry, and `Document` is
//! the pair. The dialect is proto3 plus the option vocabulary in
//! `share/schemas/majordomus/options.proto` — what proto3 has no native word for
//! (presence, a constant, a pattern) is a custom option there.
//!
//! Nothing parses a `.proto` at run time. This module is used only by
//! [`crate::generate`], which projects each file into the artifacts the validators
//! actually read: the shell tool's allow-list and section list, and a JSON Schema for the
//! Rust indexer. That keeps the hot path free of a second parser (`hot-path-reads-once`)
//! and keeps one definition of each contract (ADR 0004): the `.proto` is canonical and
//! every other form is derived from it.
//!
//! The parser is deliberately small: it reads the dialect these schemas are written in and
//! refuses everything else by name, rather than silently accepting proto it does not model.

mod lex;
pub mod project;

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{Error, Result};
use lex::{Token, Tokenizer};

// The file-level options `share/schemas/majordomus/options.proto` declares.
/// The identity a schema declares, which fixes its own path.
pub const OPT_SCHEMA_ID: &str = "majordomus.schema_id";
/// The allow-list this schema projects for the shell tool.
pub const OPT_ALLOW_LIST: &str = "majordomus.allow_list";
/// How a file of this kind is read; only `markdown` is described in proto.
pub const OPT_DOCUMENT_FORMAT: &str = "majordomus.document_format";

// The message-level options.
/// No key beyond the fields declared.
pub const OPT_CLOSED: &str = "majordomus.closed";
/// The message names body sections rather than a data shape.
pub const OPT_BODY: &str = "majordomus.body";

// The field-level options.
/// The key as written in the document, when the proto field name cannot spell it.
pub const OPT_KEY: &str = "majordomus.key";
/// The key must be present.
pub const OPT_REQUIRED: &str = "majordomus.required";
/// The value must equal this string exactly.
pub const OPT_CONST: &str = "majordomus.const";
/// The value must match this anchored expression.
pub const OPT_PATTERN: &str = "majordomus.pattern";
/// Least length of a string.
pub const OPT_MIN_LENGTH: &str = "majordomus.min_length";
/// Least number of items of a repeated field.
pub const OPT_MIN_ITEMS: &str = "majordomus.min_items";
/// Inclusive lower bound of an integer.
pub const OPT_MINIMUM: &str = "majordomus.minimum";
/// Inclusive upper bound of an integer.
pub const OPT_MAXIMUM: &str = "majordomus.maximum";
/// The heading text of a body section.
pub const OPT_HEADING: &str = "majordomus.heading";
/// The heading level of a body section.
pub const OPT_LEVEL: &str = "majordomus.level";
/// The section must carry content, not only exist.
pub const OPT_NON_EMPTY: &str = "majordomus.non_empty";

/// The enum-value option carrying the string the value is written as.
pub const OPT_VALUE: &str = "majordomus.value";

// The message names this layer gives meaning to.
/// The message naming the whole document: the header and the body.
pub const MSG_DOCUMENT: &str = "Document";
/// The message describing the YAML front matter.
pub const MSG_HEADER: &str = "Header";
/// The message naming the sections of the Markdown body.
pub const MSG_BODY: &str = "Body";

/// The suffix of a document schema written in proto.
pub const PROTO_SUFFIX: &str = ".proto";

/// One parsed `.proto` document schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoFile {
    /// Where it was read from, for diagnostics.
    pub source: String,
    /// The `package` declaration.
    pub package: String,
    /// `option (majordomus.schema_id)`, the identity that fixes this file's path.
    pub schema_id: String,
    /// `option (majordomus.allow_list)`; absent means the kind projects no allow-list.
    pub allow_list: Option<String>,
    /// `option (majordomus.document_format)`.
    pub document_format: String,
    /// Every message, in declaration order.
    pub messages: Vec<Message>,
    /// Every enum, in declaration order.
    pub enums: Vec<Enum>,
}

/// One `message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The message name as declared.
    pub name: String,
    /// The comment block above it, joined into one paragraph.
    pub doc: String,
    /// `option (majordomus.closed)`: no key beyond the fields declared here.
    pub closed: bool,
    /// `option (majordomus.body)`: the fields are body sections, not data.
    pub body: bool,
    /// Its fields, in declaration order.
    pub fields: Vec<Field>,
}

/// One field of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The proto field name. The key it names in the document is [`Field::key`].
    pub name: String,
    /// The comment block above it, joined into one paragraph.
    pub doc: String,
    /// The declared type: a scalar (`string`, `uint32`, `int64`, `bool`) or a message or
    /// enum name declared in the same file.
    pub type_name: String,
    /// Declared `repeated`: the document carries a list.
    pub repeated: bool,
    /// The proto field number. Unused by the projections; kept so the file round-trips.
    pub number: u32,
    /// The custom options it carries.
    pub options: FieldOptions,
}

impl Field {
    /// The key this field names in the document: the declared `key` option when it carries
    /// one, otherwise the field name itself.
    pub fn key(&self) -> &str {
        self.options.key.as_deref().unwrap_or(&self.name)
    }
}

/// The custom options a field may carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldOptions {
    /// The key as written in the document, when the proto field name cannot spell it.
    pub key: Option<String>,
    /// The key must be present.
    pub required: bool,
    /// The exact string the value must equal.
    pub const_value: Option<String>,
    /// The anchored expression the value must match.
    pub pattern: Option<String>,
    /// Least length of a string; 1 means "not blank".
    pub min_length: Option<u64>,
    /// Least number of items of a repeated field.
    pub min_items: Option<u64>,
    /// Inclusive lower bound of an integer.
    pub minimum: Option<i64>,
    /// Inclusive upper bound of an integer.
    pub maximum: Option<i64>,
    /// For a body section: the heading text.
    pub heading: Option<String>,
    /// For a body section: the number of leading hashes.
    pub level: Option<u64>,
    /// For a body section: it must carry content, not only exist.
    pub non_empty: bool,
}

/// One `enum`: a closed set of strings, each with the form it is written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// The enum name as declared.
    pub name: String,
    /// The comment block above it, joined into one paragraph.
    pub doc: String,
    /// Its values, in declaration order.
    pub values: Vec<EnumValue>,
}

/// One value of an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumValue {
    /// The proto value name, SCREAMING_SNAKE_CASE.
    pub name: String,
    /// The proto value number; zero is the unspecified one.
    pub number: i64,
    /// `option (majordomus.value)`: how it is written in a document. The zero value of a
    /// proto3 enum is the unspecified one and carries none.
    pub written: Option<String>,
}

impl ProtoFile {
    /// The message with this name.
    pub fn message(&self, name: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.name == name)
    }

    /// The enum with this name.
    pub fn enumeration(&self, name: &str) -> Option<&Enum> {
        self.enums.iter().find(|e| e.name == name)
    }

    /// The `Header` message, which every document schema must declare.
    pub fn header(&self) -> Result<&Message> {
        self.message(MSG_HEADER).ok_or_else(|| Error::KindSchema {
            reason: format!("{}: declares no message {MSG_HEADER}", self.source),
        })
    }

    /// The `Body` message, when the kind describes one.
    pub fn body(&self) -> Option<&Message> {
        self.message(MSG_BODY).filter(|m| m.body)
    }

    /// The path this file's identity fixes, relative to the schema root:
    /// `majordomus.adr/v1` is `majordomus/adr/adr.v1.proto`. The identity and the path are
    /// checked against each other on load, so neither can drift from the other.
    pub fn expected_path(schema_id: &str) -> Result<String> {
        let (qualified, version) = schema_id.split_once('/').ok_or_else(|| Error::KindSchema {
            reason: format!(
                "schema id '{schema_id}' is not <vendor>.<name>/v<n>: it carries no '/'"
            ),
        })?;
        let (vendor, name) = qualified
            .rsplit_once('.')
            .ok_or_else(|| Error::KindSchema {
                reason: format!(
                    "schema id '{schema_id}' is not <vendor>.<name>/v<n>: '{qualified}' carries no '.'"
                ),
            })?;
        if vendor.is_empty() || name.is_empty() || !version.starts_with('v') || version.len() < 2 {
            return Err(Error::KindSchema {
                reason: format!("schema id '{schema_id}' is not <vendor>.<name>/v<n>"),
            });
        }
        Ok(format!("{vendor}/{name}/{name}.{version}{PROTO_SUFFIX}"))
    }

    /// The identity a path under the schema root carries, the inverse of
    /// [`Self::expected_path`]. A path that does not have the shape yields nothing.
    pub fn identity_of(relative: &str) -> Option<String> {
        let stem = relative.strip_suffix(PROTO_SUFFIX)?;
        let mut parts = stem.split('/');
        let vendor = parts.next()?;
        let name = parts.next()?;
        let file = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let version = file.strip_prefix(&format!("{name}."))?;
        Some(format!("{vendor}.{name}/{version}"))
    }
}

/// Parse one `.proto` file read from `path`, with `source` naming it in every diagnostic.
pub fn parse(source: &str, text: &str) -> Result<ProtoFile> {
    Parser::new(source, text).file()
}

/// Read and parse one `.proto` file.
pub fn read(path: &Path, source: &str) -> Result<ProtoFile> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    parse(source, &text)
}

/// Every `.proto` under a schema root, keyed by the identity each declares, parsed. A root
/// that does not exist yields nothing. A file whose declared identity disagrees with its
/// path is an error naming both.
pub fn read_dir(root: &Path, source_prefix: &str) -> Result<BTreeMap<String, ProtoFile>> {
    let mut out = BTreeMap::new();
    if !root.is_dir() {
        return Ok(out);
    }
    for relative in proto_paths(root, "")? {
        // options.proto is the vocabulary, not a document schema.
        if relative == format!("majordomus/options{PROTO_SUFFIX}") {
            continue;
        }
        let source = format!("{source_prefix}/{relative}");
        let file = read(&root.join(&relative), &source)?;
        let expected = ProtoFile::expected_path(&file.schema_id)?;
        if expected != relative {
            return Err(Error::KindSchema {
                reason: format!(
                    "{source}: declares schema id '{}', whose path is {expected}; \
                     a schema's identity and its path fix each other",
                    file.schema_id
                ),
            });
        }
        if let Some(first) = out.insert(file.schema_id.clone(), file) {
            return Err(Error::KindSchema {
                reason: format!(
                    "schema id '{}' is declared by both {} and {source}",
                    first.schema_id, first.source
                ),
            });
        }
    }
    Ok(out)
}

/// Every `.proto` under `root`, as paths relative to it, sorted.
fn proto_paths(root: &Path, prefix: &str) -> Result<Vec<String>> {
    let dir = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
        let entry = entry.map_err(|e| Error::io(&dir, e))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if entry.path().is_dir() {
            out.extend(proto_paths(root, &relative)?);
        } else if name.ends_with(PROTO_SUFFIX) {
            out.push(relative);
        }
    }
    out.sort();
    Ok(out)
}

// ---------------------------------------------------------------- the parser

struct Parser<'a> {
    source: String,
    tokens: Vec<Token<'a>>,
    at: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &str, text: &'a str) -> Self {
        Parser {
            source: source.to_string(),
            tokens: Tokenizer::new(text).collect(),
            at: 0,
        }
    }

    fn err<T>(&self, what: impl std::fmt::Display) -> Result<T> {
        let line = self.tokens.get(self.at).map(|t| t.line).unwrap_or(0);
        Err(Error::KindSchema {
            reason: format!("{}:{line}: {what}", self.source),
        })
    }

    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let t = self.tokens.get(self.at).cloned();
        if t.is_some() {
            self.at += 1;
        }
        t
    }

    /// The next token's text, whatever it is.
    fn word(&mut self) -> Result<Token<'a>> {
        match self.next() {
            Some(t) => Ok(t),
            None => self.err("unexpected end of file"),
        }
    }

    /// Consume the exact punctuation or keyword `want`.
    fn expect(&mut self, want: &str) -> Result<()> {
        let t = self.word()?;
        if t.text == want {
            Ok(())
        } else {
            self.at -= 1;
            self.err(format!("expected '{want}', found '{}'", t.text))
        }
    }

    /// Consume `want` when it is next; say whether it was.
    fn eat(&mut self, want: &str) -> bool {
        if self.peek().map(|t| t.text) == Some(want) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn file(mut self) -> Result<ProtoFile> {
        let mut package = String::new();
        let mut options: BTreeMap<String, OptionValue> = BTreeMap::new();
        let mut messages = Vec::new();
        let mut enums = Vec::new();

        while let Some(token) = self.peek().cloned() {
            match token.text {
                "syntax" => {
                    self.at += 1;
                    self.expect("=")?;
                    let v = self.word()?;
                    if v.string() != Some("proto3".to_string()) {
                        self.at -= 1;
                        return self.err("only proto3 is read here");
                    }
                    self.expect(";")?;
                }
                "package" => {
                    self.at += 1;
                    package = self.dotted_name()?;
                    self.expect(";")?;
                }
                "import" => {
                    self.at += 1;
                    self.word()?;
                    self.expect(";")?;
                }
                "option" => {
                    self.at += 1;
                    let (name, value) = self.option_assignment()?;
                    self.expect(";")?;
                    options.insert(name, value);
                }
                "message" => {
                    self.at += 1;
                    messages.push(self.message(token.doc)?);
                }
                "enum" => {
                    self.at += 1;
                    enums.push(self.enumeration(token.doc)?);
                }
                "extend" => {
                    self.at += 1;
                    // The option vocabulary itself; it declares no document.
                    self.dotted_name()?;
                    self.skip_block()?;
                }
                other => {
                    let other = other.to_string();
                    return self.err(format!("unexpected '{other}' at the top level"));
                }
            }
        }

        let schema_id = match options.get(OPT_SCHEMA_ID) {
            Some(OptionValue::Str(s)) => s.clone(),
            _ => {
                return Err(Error::KindSchema {
                    reason: format!("{}: declares no option ({OPT_SCHEMA_ID})", self.source),
                })
            }
        };
        let document_format = match options.get(OPT_DOCUMENT_FORMAT) {
            Some(OptionValue::Str(s)) => s.clone(),
            _ => {
                return Err(Error::KindSchema {
                    reason: format!(
                        "{}: declares no option ({OPT_DOCUMENT_FORMAT})",
                        self.source
                    ),
                })
            }
        };
        if document_format != "markdown" {
            return Err(Error::KindSchema {
                reason: format!(
                    "{}: document_format '{document_format}' is not 'markdown'; a YAML kind is described by JSON Schema",
                    self.source
                ),
            });
        }
        let allow_list = match options.get(OPT_ALLOW_LIST) {
            Some(OptionValue::Str(s)) => Some(s.clone()),
            _ => None,
        };

        Ok(ProtoFile {
            source: self.source,
            package,
            schema_id,
            allow_list,
            document_format,
            messages,
            enums,
        })
    }

    fn dotted_name(&mut self) -> Result<String> {
        let mut out = self.word()?.text.to_string();
        while self.eat(".") {
            out.push('.');
            out.push_str(self.word()?.text);
        }
        Ok(out)
    }

    /// `(majordomus.required) = true` or `java_package = "x"`, after the `option` keyword.
    fn option_assignment(&mut self) -> Result<(String, OptionValue)> {
        let name = if self.eat("(") {
            let n = self.dotted_name()?;
            self.expect(")")?;
            n
        } else {
            self.dotted_name()?
        };
        self.expect("=")?;
        let token = self.word()?;
        let value = if let Some(s) = token.string() {
            OptionValue::Str(s)
        } else if token.text == "true" {
            OptionValue::Bool(true)
        } else if token.text == "false" {
            OptionValue::Bool(false)
        } else if let Ok(n) = token.text.parse::<i64>() {
            OptionValue::Int(n)
        } else {
            self.at -= 1;
            return self.err(format!(
                "option value '{}' is not a string, a boolean or an integer",
                token.text
            ));
        };
        Ok((name, value))
    }

    fn message(&mut self, doc: String) -> Result<Message> {
        let name = self.word()?.text.to_string();
        self.expect("{")?;
        let mut closed = false;
        let mut body = false;
        let mut fields = Vec::new();
        loop {
            let token = match self.peek().cloned() {
                Some(t) => t,
                None => return self.err("unexpected end of file inside a message"),
            };
            match token.text {
                "}" => {
                    self.at += 1;
                    break;
                }
                "option" => {
                    self.at += 1;
                    let (name, value) = self.option_assignment()?;
                    self.expect(";")?;
                    match (name.as_str(), value) {
                        (OPT_CLOSED, OptionValue::Bool(b)) => closed = b,
                        (OPT_BODY, OptionValue::Bool(b)) => body = b,
                        (other, _) => return self.err(format!("unknown message option ({other})")),
                    }
                }
                _ => fields.push(self.field(token.doc)?),
            }
        }
        Ok(Message {
            name,
            doc,
            closed,
            body,
            fields,
        })
    }

    fn field(&mut self, doc: String) -> Result<Field> {
        let repeated = self.eat("repeated");
        if self.eat("optional") || self.eat("required") {
            return self.err(
                "proto3 label 'optional'/'required' is not read here; use (majordomus.required)",
            );
        }
        let type_name = self.dotted_name()?;
        let name = self.word()?.text.to_string();
        self.expect("=")?;
        let number_token = self.word()?;
        let number: u32 = match number_token.text.parse() {
            Ok(n) => n,
            Err(_) => {
                self.at -= 1;
                return self.err(format!(
                    "field number '{}' is not an integer",
                    number_token.text
                ));
            }
        };
        let mut options = FieldOptions::default();
        if self.eat("[") {
            loop {
                let (name, value) = self.option_assignment()?;
                self.apply_field_option(&name, value, &mut options)?;
                if !self.eat(",") {
                    break;
                }
            }
            self.expect("]")?;
        }
        self.expect(";")?;
        Ok(Field {
            name,
            doc,
            type_name,
            repeated,
            number,
            options,
        })
    }

    fn apply_field_option(
        &mut self,
        name: &str,
        value: OptionValue,
        out: &mut FieldOptions,
    ) -> Result<()> {
        match (name, value) {
            (OPT_KEY, OptionValue::Str(s)) => out.key = Some(s),
            (OPT_REQUIRED, OptionValue::Bool(b)) => out.required = b,
            (OPT_NON_EMPTY, OptionValue::Bool(b)) => out.non_empty = b,
            (OPT_CONST, OptionValue::Str(s)) => out.const_value = Some(s),
            (OPT_PATTERN, OptionValue::Str(s)) => out.pattern = Some(s),
            (OPT_HEADING, OptionValue::Str(s)) => out.heading = Some(s),
            (OPT_MIN_LENGTH, OptionValue::Int(n)) if n >= 0 => out.min_length = Some(n as u64),
            (OPT_MIN_ITEMS, OptionValue::Int(n)) if n >= 0 => out.min_items = Some(n as u64),
            (OPT_LEVEL, OptionValue::Int(n)) if n >= 0 => out.level = Some(n as u64),
            (OPT_MINIMUM, OptionValue::Int(n)) => out.minimum = Some(n),
            (OPT_MAXIMUM, OptionValue::Int(n)) => out.maximum = Some(n),
            (other, _) => {
                return self.err(format!(
                    "unknown or ill-typed field option ({other}); the vocabulary is share/schemas/majordomus/options.proto"
                ))
            }
        }
        Ok(())
    }

    fn enumeration(&mut self, doc: String) -> Result<Enum> {
        let name = self.word()?.text.to_string();
        self.expect("{")?;
        let mut values = Vec::new();
        loop {
            if self.eat("}") {
                break;
            }
            let value_name = self.word()?.text.to_string();
            self.expect("=")?;
            let number_token = self.word()?;
            let number: i64 = match number_token.text.parse() {
                Ok(n) => n,
                Err(_) => {
                    self.at -= 1;
                    return self.err(format!(
                        "enum value number '{}' is not an integer",
                        number_token.text
                    ));
                }
            };
            let mut written = None;
            if self.eat("[") {
                loop {
                    let (option_name, value) = self.option_assignment()?;
                    match (option_name.as_str(), value) {
                        (OPT_VALUE, OptionValue::Str(s)) => written = Some(s),
                        (other, _) => {
                            return self.err(format!("unknown enum value option ({other})"))
                        }
                    }
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect("]")?;
            }
            self.expect(";")?;
            values.push(EnumValue {
                name: value_name,
                number,
                written,
            });
        }
        Ok(Enum { name, doc, values })
    }

    /// Skip a `{ ... }` block whose contents this reader does not model.
    fn skip_block(&mut self) -> Result<()> {
        self.expect("{")?;
        let mut depth = 1;
        while depth > 0 {
            let t = self.word()?;
            match t.text {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }
}

/// The value of an option: the three kinds this dialect uses.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OptionValue {
    Str(String),
    Bool(bool),
    Int(i64),
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADR: &str = r#"
syntax = "proto3";
package majordomus.adr.v1;
import "majordomus/options.proto";
option (majordomus.schema_id) = "majordomus.adr/v1";
option (majordomus.allow_list) = "adr";
option (majordomus.document_format) = "markdown";

// The front matter.
message Header {
  option (majordomus.closed) = true;
  // The format version.
  string schema = 1 [(majordomus.required) = true, (majordomus.const) = "adr/v1"];
  repeated string tags = 7 [(majordomus.pattern) = "^[a-z][a-z0-9-]*$"];
  Status status = 5 [(majordomus.required) = true];
}

enum Status {
  STATUS_UNSPECIFIED = 0;
  STATUS_PROPOSED = 1 [(majordomus.value) = "proposed"];
}

message Body {
  option (majordomus.body) = true;
  Section context = 1 [(majordomus.heading) = "Context", (majordomus.level) = 2, (majordomus.required) = true, (majordomus.non_empty) = true];
}

message Section {
  string heading = 1;
  uint32 level = 2;
  bool empty = 3;
}
"#;

    fn adr() -> ProtoFile {
        parse("adr.v1.proto", ADR).expect("parses")
    }

    #[test]
    fn reads_the_file_options() {
        let f = adr();
        assert_eq!(f.package, "majordomus.adr.v1");
        assert_eq!(f.schema_id, "majordomus.adr/v1");
        assert_eq!(f.allow_list.as_deref(), Some("adr"));
        assert_eq!(f.document_format, "markdown");
    }

    #[test]
    fn reads_fields_their_options_and_their_comments() {
        let f = adr();
        let header = f.header().expect("Header");
        assert!(header.closed);
        let schema = &header.fields[0];
        assert_eq!(schema.name, "schema");
        assert_eq!(schema.doc, "The format version.");
        assert!(schema.options.required);
        assert_eq!(schema.options.const_value.as_deref(), Some("adr/v1"));
        let tags = &header.fields[1];
        assert!(tags.repeated);
        assert_eq!(tags.options.pattern.as_deref(), Some("^[a-z][a-z0-9-]*$"));
    }

    #[test]
    fn reads_an_enum_with_the_form_each_value_is_written_in() {
        let f = adr();
        let status = f.enumeration("Status").expect("Status");
        assert_eq!(status.values.len(), 2);
        assert_eq!(status.values[0].written, None);
        assert_eq!(status.values[1].written.as_deref(), Some("proposed"));
    }

    #[test]
    fn reads_the_body_sections_in_order() {
        let f = adr();
        let body = f.body().expect("Body");
        assert_eq!(body.fields[0].options.heading.as_deref(), Some("Context"));
        assert_eq!(body.fields[0].options.level, Some(2));
        assert!(body.fields[0].options.non_empty);
    }

    #[test]
    fn an_identity_and_a_path_fix_each_other() {
        assert_eq!(
            ProtoFile::expected_path("majordomus.adr/v1").unwrap(),
            "majordomus/adr/adr.v1.proto"
        );
        assert_eq!(
            ProtoFile::identity_of("majordomus/adr/adr.v1.proto").as_deref(),
            Some("majordomus.adr/v1")
        );
        assert!(ProtoFile::expected_path("adr").is_err());
        assert!(ProtoFile::expected_path("majordomus.adr").is_err());
        assert_eq!(ProtoFile::identity_of("majordomus/adr.proto"), None);
    }

    #[test]
    fn refuses_a_dialect_it_does_not_model() {
        let bad = ADR.replace("syntax = \"proto3\";", "syntax = \"proto2\";");
        assert!(parse("x.proto", &bad).is_err());
        let yaml = ADR.replace("\"markdown\"", "\"yaml\"");
        let e = parse("x.proto", &yaml).unwrap_err().to_string();
        assert!(e.contains("described by JSON Schema"), "{e}");
        let unknown = ADR.replace("(majordomus.const)", "(majordomus.nonesuch)");
        assert!(parse("x.proto", &unknown).is_err());
    }
}

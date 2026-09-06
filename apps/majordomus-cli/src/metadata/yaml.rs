//! The layer's YAML subset, as `docs/SCHEMAS.md` and the shell tool's `mj_yaml_flatten`
//! define it: `key: value`, nested maps by two-space indent, block lists (`- item`), lists
//! of maps (`- key: value` plus indented keys), inline lists `[a, b]`, single or double
//! quotes, comments. Tabs, odd indentation, anchors, multi-line scalars and flow maps are
//! refused with the line named (the shell parser reads an anchor or alias as text; this one
//! refuses it, which is the stricter reading of the same contract). A scalar is a string unless it is an unquoted integer or
//! `true`/`false`; nothing else is interpreted.
//!
//! This is not a general YAML parser and does not try to be: the repository's files are
//! written in this subset so that a person, an awk script and this executable read them
//! identically, and a construct outside it is a mistake in the file, not a gap here.

use serde_json::{Map, Number, Value};

/// A parsed scalar with the fact of its quoting kept, so that `"1"` stays a string.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scalar {
    text: String,
    quoted: bool,
}

/// One flattened line: a dotted key path and its scalar, or an empty inline list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Flat {
    Value(String, Scalar),
    EmptyList(String),
}

fn is_key(s: &str) -> Option<(&str, &str)> {
    let (k, rest) = s.split_once(':')?;
    let mut chars = k.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some((k, rest.trim_start_matches([' ', '\t'])))
}

fn unquote(v: &str) -> Scalar {
    let v = v.trim();
    if v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
    {
        return Scalar {
            text: v[1..v.len() - 1].to_string(),
            quoted: true,
        };
    }
    let mut text = v;
    if let Some(pos) = find_comment(text) {
        text = &text[..pos];
    }
    Scalar {
        text: text.trim().to_string(),
        quoted: false,
    }
}

/// Position of a ` #` that starts a trailing comment, as the shell's `[ \t]+#.*$`.
fn find_comment(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    (1..bytes.len()).find(|&i| bytes[i] == b'#' && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t'))
}

fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}.{b}")
    }
}

/// An unquoted scalar starting with `&`, `*`, `|` or `>` is an anchor, an alias or a
/// block scalar: outside the subset, and refused rather than read as text.
fn is_unsupported_construct(v: &str) -> bool {
    let v = v.trim();
    !(v.starts_with('"') || v.starts_with('\''))
        && (v.starts_with('&') || v.starts_with('*') || v.starts_with('|') || v.starts_with('>'))
}

fn emit(out: &mut Vec<Flat>, path: &str, v: &str) {
    let v = v.trim();
    if v.starts_with('[') && v.ends_with(']') {
        let inner = v[1..v.len() - 1].trim();
        if inner.is_empty() {
            out.push(Flat::EmptyList(path.to_string()));
            return;
        }
        for (i, part) in inner.split(',').enumerate() {
            out.push(Flat::Value(format!("{path}.{i}"), unquote(part)));
        }
        return;
    }
    out.push(Flat::Value(path.to_string(), unquote(v)));
}

/// Flatten text into dotted paths, mirroring `mj_yaml_flatten`.
fn flatten(text: &str) -> Result<Vec<Flat>, String> {
    use std::collections::BTreeMap;
    let mut ctx: BTreeMap<usize, String> = BTreeMap::new();
    let mut pend: BTreeMap<usize, String> = BTreeMap::new();
    let mut cnt: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    ctx.insert(0, String::new());
    for (n, raw) in text.lines().enumerate() {
        let n = n + 1;
        if raw.contains('\t') {
            return Err(format!("tab character on line {n}"));
        }
        let t = raw.trim_start_matches(' ');
        if t.is_empty() || t.starts_with('#') || t.trim_end() == "---" {
            continue;
        }
        let ind = raw.len() - t.len();
        if ind % 2 != 0 {
            return Err(format!("odd indentation on line {n}"));
        }
        let s = t;
        if let Some(item) = s.strip_prefix("- ") {
            let Some(parent) = pend.get(&ind).cloned() else {
                return Err(format!("list item without a parent key on line {n}"));
            };
            let idx = *cnt.get(&parent).unwrap_or(&0);
            cnt.insert(parent.clone(), idx + 1);
            let item = item.trim();
            let ip = format!("{parent}.{idx}");
            if let Some((k, v)) = is_key(item) {
                ctx.retain(|kk, _| *kk <= ind + 2);
                ctx.insert(ind + 2, ip.clone());
                if v.is_empty() {
                    let p = join(&ip, k);
                    pend.insert(ind + 2, p.clone());
                    pend.insert(ind + 4, p.clone());
                    ctx.insert(ind + 4, p);
                } else {
                    if is_unsupported_construct(v) {
                        return Err(format!("unsupported YAML construct on line {n}: {v}"));
                    }
                    emit(&mut out, &join(&ip, k), v);
                }
            } else {
                if is_unsupported_construct(item) {
                    return Err(format!("unsupported YAML construct on line {n}: {item}"));
                }
                emit(&mut out, &ip, item);
            }
            continue;
        }
        let Some((k, v)) = is_key(s) else {
            return Err(format!("cannot parse line {n}: {s}"));
        };
        let Some(base) = ctx.get(&ind).cloned() else {
            return Err(format!("unexpected indentation on line {n}"));
        };
        ctx.retain(|kk, _| *kk <= ind);
        pend.retain(|kk, _| *kk < ind);
        let p = join(&base, k);
        if !seen.insert(p.clone()) {
            return Err(format!("'{p}' is given twice, on line {n}"));
        }
        if v.is_empty() {
            pend.insert(ind, p.clone());
            pend.insert(ind + 2, p.clone());
            ctx.insert(ind + 2, p);
        } else {
            if is_unsupported_construct(v) {
                return Err(format!("unsupported YAML construct on line {n}: {v}"));
            }
            emit(&mut out, &p, v);
        }
    }
    Ok(out)
}

fn typed(s: &Scalar) -> Value {
    if s.quoted {
        return Value::String(s.text.clone());
    }
    match s.text.as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        t if !t.is_empty()
            && t.trim_start_matches('-')
                .bytes()
                .all(|b| b.is_ascii_digit())
            && t != "-" =>
        {
            t.parse::<i64>()
                .map(|n| Value::Number(Number::from(n)))
                .unwrap_or_else(|_| Value::String(t.to_string()))
        }
        // a decimal with digits on both sides of one point (0.5, -1.25); anything else
        // that merely looks numeric (1e3, .5, 1., 0x10) stays text, as the shell tool
        // reads it
        t if is_decimal(t) => t
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(t.to_string())),
        t => Value::String(t.to_string()),
    }
}

/// `-?[0-9]+\.[0-9]+`, and nothing wider: the one decimal form the subset types as a number.
fn is_decimal(t: &str) -> bool {
    let body = t.strip_prefix('-').unwrap_or(t);
    match body.split_once('.') {
        Some((int, frac)) => {
            !int.is_empty()
                && !frac.is_empty()
                && int.bytes().all(|b| b.is_ascii_digit())
                && frac.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

fn insert(root: &mut Map<String, Value>, path: &str, leaf: Value) -> Result<(), String> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut node: &mut Value = root.entry(segments[0].to_string()).or_insert(Value::Null);
    for seg in &segments[1..] {
        if seg.bytes().all(|b| b.is_ascii_digit()) {
            if node.is_null() {
                *node = Value::Array(Vec::new());
            }
            let Value::Array(arr) = node else {
                return Err(format!("'{path}' mixes a list with a mapping"));
            };
            let i: usize = seg.parse().map_err(|_| format!("bad index in '{path}'"))?;
            while arr.len() <= i {
                arr.push(Value::Null);
            }
            node = &mut arr[i];
        } else {
            if node.is_null() {
                *node = Value::Object(Map::new());
            }
            let Value::Object(map) = node else {
                return Err(format!("'{path}' mixes a mapping with a scalar or list"));
            };
            node = map.entry(seg.to_string()).or_insert(Value::Null);
        }
    }
    if !node.is_null() {
        return Err(format!("'{path}' is given twice"));
    }
    *node = leaf;
    Ok(())
}

/// Parse a document of the subset into an ordered mapping.
///
/// ```
/// use majordomus_cli::metadata::yaml::parse_mapping;
/// use serde_json::json;
/// let m = parse_mapping("id: project.x\nversion: 1\ntags: [a, b]\nnote: colons: are text\n").unwrap();
/// assert_eq!(m["version"], json!(1));
/// assert_eq!(m["tags"], json!(["a", "b"]));
/// assert_eq!(m["note"], json!("colons: are text"));
/// assert_eq!(parse_mapping("a:\tb\n").unwrap_err(), "tab character on line 1");
/// ```
pub fn parse_mapping(text: &str) -> Result<Map<String, Value>, String> {
    let mut root = Map::new();
    for flat in flatten(text)? {
        match flat {
            Flat::Value(path, scalar) => insert(&mut root, &path, typed(&scalar))?,
            Flat::EmptyList(path) => insert(&mut root, &path, Value::Array(Vec::new()))?,
        }
    }
    Ok(root)
}

/// Parse and deserialize into a typed value.
pub fn parse_into<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    let map = parse_mapping(text)?;
    serde_json::from_value(Value::Object(map)).map_err(|e| e.to_string())
}

/// Every leaf key path of a value, in document order, as the allow-lists are written:
/// `a.b.0.c`. An empty list is itself a leaf, so that `tags: []` flattens to `tags`.
pub fn key_paths(map: &Map<String, Value>) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in map {
        walk(k, v, &mut out);
    }
    out
}

fn walk(prefix: &str, v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                walk(&format!("{prefix}.{k}"), v, out);
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for (i, v) in a.iter().enumerate() {
                walk(&format!("{prefix}.{i}"), v, out);
            }
        }
        _ => out.push(prefix.to_string()),
    }
}

/// A scalar as the string an identity or title uses; `None` for a list or mapping.
pub fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------- rendering

/// Render a JSON value as a YAML document in the layer's subset wherever the value fits
/// it, and in a conservative quoted style wherever it does not.
///
/// The reader above is the contract this writer aims at: a document it produces from a
/// value whose keys are identifiers and whose scalars are strings, integers and booleans
/// parses back to the same value through [`parse_mapping`]. Values outside the subset —
/// a key like `$ref` or `/api/v1/peers`, a floating exponent, a null — are still written,
/// quoted so that a general YAML 1.2 parser reads them back, because the generated
/// OpenAPI and registry documents carry them and a projection that silently dropped them
/// would be a lie.
///
/// The style is fixed so that the output is byte-deterministic: two-space indent, block
/// mappings and block sequences, `{}` and `[]` for the empty collections, keys bare when
/// they are identifiers and double-quoted otherwise, and scalars plain only when reading
/// them back cannot change their type.
///
/// ```
/// use majordomus_cli::metadata::yaml::{render, parse_mapping};
/// use serde_json::json;
/// let doc = json!({ "id": "peers.list", "tags": ["peers", "coordination"], "cached": true });
/// assert_eq!(render(&doc), "id: peers.list\ntags:\n  - peers\n  - coordination\ncached: true\n");
/// assert_eq!(serde_json::Value::Object(parse_mapping(&render(&doc)).unwrap()), doc);
/// ```
pub fn render(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(map) if map.is_empty() => out.push_str("{}\n"),
        Value::Object(map) => write_mapping(map, 0, &mut out),
        Value::Array(items) if items.is_empty() => out.push_str("[]\n"),
        Value::Array(items) => write_sequence(items, 0, &mut out),
        scalar => {
            out.push_str(&scalar_literal(scalar));
            out.push('\n');
        }
    }
    out
}

/// Render a value as a YAML document under a comment banner, one `# ` line per line of
/// `banner`. The banner is a comment and never part of the data.
pub fn render_with_banner(value: &Value, banner: &str) -> String {
    let mut out = String::new();
    for line in banner.lines() {
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&render(value));
    out
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_mapping(map: &Map<String, Value>, depth: usize, out: &mut String) {
    for (key, value) in map {
        indent(depth, out);
        out.push_str(&key_literal(key));
        out.push(':');
        write_child(value, depth, out);
    }
}

fn write_sequence(items: &[Value], depth: usize, out: &mut String) {
    for item in items {
        indent(depth, out);
        out.push('-');
        match item {
            Value::Object(map) if !map.is_empty() => {
                // `- key: value`, the remaining keys aligned under it
                let mut nested = String::new();
                write_mapping(map, depth + 1, &mut nested);
                let body = nested
                    .strip_prefix(&"  ".repeat(depth + 1))
                    .unwrap_or(&nested);
                out.push(' ');
                out.push_str(body);
            }
            Value::Array(items) if !items.is_empty() => {
                out.push('\n');
                write_sequence(items, depth + 1, out);
            }
            other => {
                out.push(' ');
                out.push_str(&inline(other));
                out.push('\n');
            }
        }
    }
}

/// The right-hand side of `key:`: inline for a scalar and for an empty collection, a
/// block on the following lines for a non-empty one.
fn write_child(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            out.push('\n');
            write_mapping(map, depth + 1, out);
        }
        Value::Array(items) if !items.is_empty() => {
            out.push('\n');
            write_sequence(items, depth + 1, out);
        }
        other => {
            out.push(' ');
            out.push_str(&inline(other));
            out.push('\n');
        }
    }
}

fn inline(value: &Value) -> String {
    match value {
        Value::Object(_) => "{}".into(),
        Value::Array(_) => "[]".into(),
        scalar => scalar_literal(scalar),
    }
}

/// A YAML key: bare when it is an identifier the reader accepts, double-quoted otherwise.
/// The reader flattens nested keys into dotted paths, so a key holding a `.` is quoted
/// too — quoting does not make it readable by the subset, and it does make the document
/// readable by a general parser.
fn key_literal(key: &str) -> String {
    if is_plain_key(key) {
        key.to_string()
    } else {
        quoted(key)
    }
}

fn is_plain_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn scalar_literal(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) if is_plain_scalar(s) => s.clone(),
        Value::String(s) => quoted(s),
        other => other.to_string(),
    }
}

/// A string may be written unquoted only when reading it back yields the same string:
/// not empty, not a word or a number the reader types, no leading or trailing space, no
/// character that starts a construct, and no ` #` that the reader would cut as a comment.
fn is_plain_scalar(s: &str) -> bool {
    if s.is_empty() || s.trim() != s {
        return false;
    }
    if matches!(
        s,
        "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off"
    ) {
        return false;
    }
    if looks_numeric(s) {
        return false;
    }
    let first = s.as_bytes()[0];
    if !(first.is_ascii_alphanumeric() || first == b'_' || first == b'/' || first == b'.') {
        return false;
    }
    if s.contains(": ") || s.ends_with(':') || s.contains(" #") || s.contains('\t') {
        return false;
    }
    s.chars().all(|c| {
        !c.is_control()
            && !matches!(
                c,
                '"' | '\''
                    | '\\'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | '&'
                    | '*'
                    | '|'
                    | '>'
                    | '%'
                    | '@'
                    | '`'
            )
    })
}

fn looks_numeric(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
        && body
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
}

/// A double-quoted YAML scalar. YAML 1.2's double-quoted style takes JSON's escapes, so
/// the JSON encoding of the string is a correct YAML scalar and needs no second rule.
fn quoted(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unquoted_integers_and_decimals_are_numbers_and_everything_else_is_text() {
        let m = parse_mapping(
            "a: 10\nb: -3\nc: 0.5\nd: -1.25\ne: \"0.5\"\nf: 1e3\ng: .5\nh: 1.\ni: 0x10\nj: 1.2.3\n",
        )
        .unwrap();
        assert_eq!(m["a"], json!(10));
        assert_eq!(m["b"], json!(-3));
        assert_eq!(m["c"], json!(0.5));
        assert_eq!(m["d"], json!(-1.25));
        for k in ["e", "f", "g", "h", "i", "j"] {
            assert!(m[k].is_string(), "{k} = {:?}", m[k]);
        }
    }

    #[test]
    fn flattens_like_the_shell_tool() {
        let m = parse_mapping("id: a\nversion: 1\ndepends_on: [x@1, y@2]\ntags: []\nx-majordomus:\n  validator: v\n  tests: [t]\n").unwrap();
        assert_eq!(
            key_paths(&m),
            vec![
                "id",
                "version",
                "depends_on.0",
                "depends_on.1",
                "tags",
                "x-majordomus.validator",
                "x-majordomus.tests.0"
            ]
        );
        assert_eq!(m["version"], json!(1));
        assert_eq!(m["depends_on"], json!(["x@1", "y@2"]));
    }

    #[test]
    fn colons_and_backticks_inside_scalars_are_text() {
        let m = parse_mapping("description: The name is the only thing: no paths, no quotes.\nnote: `git status` stays clean\n").unwrap();
        assert_eq!(
            m["description"],
            json!("The name is the only thing: no paths, no quotes.")
        );
        assert_eq!(m["note"], json!("`git status` stays clean"));
    }

    #[test]
    fn block_lists_and_lists_of_maps() {
        let text = "scope:\n  - lib/a\n  - Every surface agrees: the CLI and the site\nevidence:\n  - covers: x\n    type: test\n  - covers: y\n    type: manual\n";
        let m = parse_mapping(text).unwrap();
        assert_eq!(
            m["scope"],
            json!(["lib/a", "Every surface agrees: the CLI and the site"])
        );
        assert_eq!(
            m["evidence"],
            json!([{ "covers": "x", "type": "test" }, { "covers": "y", "type": "manual" }])
        );
    }

    #[test]
    fn list_items_at_parent_indent() {
        let m = parse_mapping("sources:\n- id: a\n  kind: k\n- id: b\n  kind: k2\n").unwrap();
        assert_eq!(
            m["sources"],
            json!([{ "id": "a", "kind": "k" }, { "id": "b", "kind": "k2" }])
        );
    }

    #[test]
    fn quotes_comments_and_typing() {
        let m = parse_mapping(
            "a: \"1\"   \nb: 1 # a comment\nc: 'x: y'\nd: true\ne: 15m\nf: -3\ng: -\n",
        )
        .unwrap();
        assert_eq!(m["a"], json!("1"));
        assert_eq!(m["b"], json!(1));
        assert_eq!(m["c"], json!("x: y"));
        assert_eq!(m["d"], json!(true));
        assert_eq!(m["e"], json!("15m"));
        assert_eq!(m["f"], json!(-3));
        assert_eq!(m["g"], json!("-"));
    }

    #[test]
    fn refusals_name_the_line() {
        assert_eq!(
            parse_mapping("a:\tb\n").unwrap_err(),
            "tab character on line 1"
        );
        assert_eq!(
            parse_mapping("a:\n   b: 1\n").unwrap_err(),
            "odd indentation on line 2"
        );
        assert_eq!(
            parse_mapping("- a\n").unwrap_err(),
            "list item without a parent key on line 1"
        );
        assert_eq!(
            parse_mapping("- a: [b\n").unwrap_err(),
            "list item without a parent key on line 1"
        );
        assert_eq!(
            parse_mapping("a: &x b\nc: *x\n").unwrap_err(),
            "unsupported YAML construct on line 1: &x b"
        );
        assert_eq!(
            parse_mapping("a: |\n  text\n").unwrap_err(),
            "unsupported YAML construct on line 1: |"
        );
        assert_eq!(parse_mapping("a: '*x'\n").unwrap()["a"], json!("*x"));
        assert_eq!(
            parse_mapping("a: 1\n  b: 2\n").unwrap_err(),
            "unexpected indentation on line 2"
        );
        assert!(parse_mapping("a: 1\na: 2\n").unwrap_err().contains("twice"));
        assert!(parse_mapping("tags:\n  - a\ntags:\n  - b\n")
            .unwrap_err()
            .contains("'tags' is given twice"));
        assert!(parse_mapping("x:\n  a: 1\n  a: 2\n")
            .unwrap_err()
            .contains("'x.a' is given twice"));
    }

    // ------------------------------------------------------------ rendering

    /// Anything the writer emits from a value inside the subset must read back identical:
    /// this is the one property the two halves of this module owe each other.
    fn round_trips(doc: Value) {
        let text = render(&doc);
        let back = parse_mapping(&text).unwrap_or_else(|e| panic!("{e}\n--- rendered ---\n{text}"));
        assert_eq!(Value::Object(back), doc, "--- rendered ---\n{text}");
    }

    #[test]
    fn renders_scalars_maps_and_sequences() {
        assert_eq!(
            render(&json!({ "id": "peers.list", "count": 2, "cached": true })),
            "id: peers.list\ncount: 2\ncached: true\n"
        );
        assert_eq!(
            render(&json!({ "tags": ["a", "b"] })),
            "tags:\n  - a\n  - b\n"
        );
        assert_eq!(
            render(&json!({ "x": { "y": { "z": 1 } } })),
            "x:\n  y:\n    z: 1\n"
        );
        assert_eq!(
            render(&json!({ "items": [{ "id": "a", "n": 1 }, { "id": "b", "n": 2 }] })),
            "items:\n  - id: a\n    n: 1\n  - id: b\n    n: 2\n"
        );
        assert_eq!(render(&json!({})), "{}\n");
        assert_eq!(render(&json!([])), "[]\n");
        assert_eq!(
            render(&json!({ "empty": [], "none": {} })),
            "empty: []\nnone: {}\n"
        );
    }

    #[test]
    fn quotes_whatever_would_read_back_as_something_else() {
        // a string that looks like a number, a boolean or nothing at all
        assert_eq!(render(&json!({ "a": "1" })), "a: \"1\"\n");
        assert_eq!(render(&json!({ "a": "true" })), "a: \"true\"\n");
        assert_eq!(render(&json!({ "a": "" })), "a: \"\"\n");
        assert_eq!(render(&json!({ "a": " x " })), "a: \" x \"\n");
        // a string carrying what the reader treats as syntax
        assert_eq!(render(&json!({ "a": "k: v" })), "a: \"k: v\"\n");
        assert_eq!(render(&json!({ "a": "x #c" })), "a: \"x #c\"\n");
        assert_eq!(render(&json!({ "a": "[a, b]" })), "a: \"[a, b]\"\n");
        assert_eq!(render(&json!({ "a": "&anchor" })), "a: \"&anchor\"\n");
        assert_eq!(
            render(&json!({ "a": "line\nbreak" })),
            "a: \"line\\nbreak\"\n"
        );
        // a key outside the reader's identifier form is quoted rather than dropped
        assert_eq!(render(&json!({ "$ref": "#/x" })), "\"$ref\": \"#/x\"\n");
        assert_eq!(
            render(&json!({ "/api/v1/peers": 1 })),
            "\"/api/v1/peers\": 1\n"
        );
        assert_eq!(render(&json!({ "a.b": 1 })), "\"a.b\": 1\n");
        // null has no place in the subset and is still written
        assert_eq!(render(&json!({ "a": Value::Null })), "a: null\n");
    }

    #[test]
    fn every_shape_of_the_subset_round_trips() {
        round_trips(json!({ "id": "x", "n": 0, "neg": -3, "t": true, "f": false }));
        round_trips(json!({ "s": "1", "b": "true", "e": "", "pad": " x ", "colon": "k: v" }));
        round_trips(json!({ "tags": ["a", "b"], "empty": [] }));
        round_trips(json!({ "outer": { "inner": { "deep": "value" } } }));
        round_trips(json!({ "items": [{ "id": "a", "tags": ["x"] }, { "id": "b", "tags": [] }] }));
        round_trips(json!({ "path": "docs/generated/registry.json", "sha": "0a1b2c" }));
        round_trips(json!({ "text": "a sentence, with punctuation - and a dash" }));
        round_trips(json!({ "dec": 1.5, "negdec": -0.25 }));
    }

    /// The one shape the two halves disagree on, stated rather than hidden: an empty
    /// mapping. The reader has no `{}` — an empty document and a key with no children are
    /// both nothing to it — while the writer must emit `{}` or hand a general parser a
    /// null. Every generated document this repository writes has keys, so the divergence
    /// is at the edge of the subset and never in an artifact.
    #[test]
    fn the_empty_mapping_is_the_edge_of_the_subset() {
        assert_eq!(render(&json!({})), "{}\n");
        assert!(parse_mapping("{}").is_err());
        assert!(parse_mapping("").unwrap().is_empty());
        assert_eq!(render(&json!({ "a": {} })), "a: {}\n");
        assert_eq!(
            parse_mapping("a: {}\n").unwrap().get("a"),
            Some(&Value::String("{}".into()))
        );
    }

    #[test]
    fn a_banner_is_a_comment_and_never_data() {
        let doc = json!({ "a": 1 });
        let text = render_with_banner(&doc, "GENERATED\n\nsource: nowhere");
        assert_eq!(text, "# GENERATED\n#\n# source: nowhere\na: 1\n");
        assert_eq!(Value::Object(parse_mapping(&text).unwrap()), doc);
    }

    #[test]
    fn empty_document_is_empty_mapping() {
        assert!(parse_mapping("").unwrap().is_empty());
        assert!(parse_mapping("# only a comment\n---\n").unwrap().is_empty());
    }

    #[test]
    fn nested_maps_and_pending_keys_without_children_vanish() {
        let m = parse_mapping("context:\n  task: true\n  depth: 0\nempty:\nnext: 1\n").unwrap();
        assert_eq!(
            m,
            serde_json::from_value::<serde_json::Map<String, Value>>(
                json!({ "context": { "task": true, "depth": 0 }, "next": 1 })
            )
            .unwrap()
        );
    }
}

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
        t => Value::String(t.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

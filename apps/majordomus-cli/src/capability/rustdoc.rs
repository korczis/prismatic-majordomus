//! Rust doc comments, translated into the CommonMark every projection promises.
//!
//! A doc comment is the canonical declaration of what a type and its fields mean, and
//! [`CanonicalSchema::of`](super::CanonicalSchema::of) carries it into `description` —
//! from there into `docs/generated/registry.json`, `docs/generated/openapi.json`, MCP's
//! `inputSchema`, Swagger UI and the published site. Every one of those consumers reads
//! that field as CommonMark: OpenAPI says so in the specification, and the site hands it
//! to a Markdown renderer.
//!
//! Rustdoc is *not* CommonMark. It adds two link forms that only rustdoc can resolve:
//!
//! - the **intra-doc link**, `` [`Type::Variant`] `` — a shortcut reference link with no
//!   definition anywhere, which CommonMark renders as literal brackets around a code span;
//! - a **reference definition pointing at a path relative to the source file**, such as
//!   `[ADR 0040]: ../../../../.ai/repo/adrs/0040-….md`, which resolves from
//!   `apps/majordomus-cli/src/capability/` and from nowhere a reader will ever stand.
//!
//! Unresolved, both reach the reader as source: the site published
//! ``[`WaiverReason::ExternalDependency`]:`` and an eighty-seven character `../../../../`
//! path as body text. The prose is not wrong — it is right for rustdoc, which is the doc
//! comment's first reader — so the declaration stays as it is and the *projection*
//! translates, once, here. That is the rule a repeated semantic would break: the doc
//! comment says what a field means exactly once, and no projection restates it.
//!
//! What this does **not** touch is as important as what it does. Code blocks are left
//! byte for byte alone: a doc comment's fenced example is Rust, and `["capability"]` and
//! `[a-z][a-z0-9_-]` are an array literal and a character class, not links. A link whose
//! destination carries a URL scheme resolves for every reader and is kept.
//!
//! ```
//! use majordomus_cli::capability::rustdoc::to_commonmark;
//!
//! // an intra-doc link degrades to the code span it wrapped
//! assert_eq!(to_commonmark("see [`Waiver::Reason`] first"), "see `Waiver::Reason` first");
//!
//! // a definition only rustdoc can follow goes, and its label stays as text
//! let doc = "As [ADR 0040] says.\n\n[ADR 0040]: ../../../.ai/repo/adrs/0040-x.md";
//! assert_eq!(to_commonmark(doc), "As ADR 0040 says.");
//!
//! // a fenced example is untouched, brackets and all
//! let code = "Text.\n\n```\nlet v = [\"capability\"];\n```";
//! assert_eq!(to_commonmark(code), code);
//! ```

use std::collections::BTreeSet;

use serde_json::Value;

/// Translate one Rust doc comment into CommonMark.
///
/// Returns the text unchanged when it carries nothing rustdoc-only, so a description that
/// was already portable is not rewritten and the generated artifacts do not churn.
pub fn to_commonmark(doc: &str) -> String {
    // Nothing to do for the overwhelming majority of descriptions: a scan is far cheaper
    // than a rewrite, and this runs for every property of every schema of every capability.
    if !doc.contains('[') {
        return doc.to_string();
    }

    let lines: Vec<&str> = doc.split('\n').collect();
    let fenced = fenced_lines(&lines);

    // Pass one: find the reference definitions rustdoc alone can follow. Their labels have
    // to be known before the prose is rewritten, because `[ADR 0040]` is only safe to
    // flatten when we are also dropping the definition that gave it a destination — a
    // bracketed phrase with no definition is somebody's prose and stays as it is.
    let mut orphaned: BTreeSet<String> = BTreeSet::new();
    for (i, line) in lines.iter().enumerate() {
        if fenced[i] {
            continue;
        }
        if let Some((label, destination)) = reference_definition(line) {
            if !has_url_scheme(destination) {
                orphaned.insert(label.to_ascii_lowercase());
            }
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if fenced[i] {
            out.push((*line).to_string());
            continue;
        }
        if let Some((label, destination)) = reference_definition(line) {
            if orphaned.contains(&label.to_ascii_lowercase()) && !has_url_scheme(destination) {
                continue; // the definition is dropped, not rendered
            }
        }
        out.push(rewrite_links(line, &orphaned));
    }

    // Dropping a definition can leave the trailing blank line that separated it from the
    // prose, which would otherwise show up as a change in every rendered paragraph break.
    let mut text = out.join("\n");
    while text.ends_with('\n') || text.ends_with(' ') {
        text.truncate(text.len() - 1);
    }
    text
}

/// Which lines sit inside a fenced code block, and are therefore not prose.
///
/// The fence line itself counts as code so that an info string is never rewritten.
fn fenced_lines(lines: &[&str]) -> Vec<bool> {
    let mut inside = false;
    let mut fence = ' ';
    let mut run = 0usize;
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start();
            let (marker, count) = fence_marker(trimmed);
            match (inside, marker) {
                (false, Some(m)) => {
                    inside = true;
                    fence = m;
                    run = count;
                    true
                }
                (true, Some(m)) if m == fence && count >= run => {
                    inside = false;
                    true
                }
                (state, _) => state,
            }
        })
        .collect()
}

/// The fence character and its length, for a line that opens or closes a code block.
fn fence_marker(trimmed: &str) -> (Option<char>, usize) {
    for marker in ['`', '~'] {
        let run = trimmed.chars().take_while(|c| *c == marker).count();
        if run >= 3 {
            return (Some(marker), run);
        }
    }
    (None, 0)
}

/// A link reference definition, as `[label]: destination`, if the line is one.
fn reference_definition(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('[')?;
    let end = rest.find("]:")?;
    let label = &rest[..end];
    let destination = rest[end + 2..].trim();
    if label.is_empty() || destination.is_empty() || destination.contains(char::is_whitespace) {
        return None;
    }
    Some((label, destination))
}

/// Whether a link destination names a scheme, and so resolves for every reader.
fn has_url_scheme(destination: &str) -> bool {
    let destination = destination.trim_start_matches('<');
    match destination.find("://") {
        Some(at) => destination[..at]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'),
        None => destination.starts_with("mailto:"),
    }
}

/// Rewrite the rustdoc-only links on one line of prose.
///
/// Three forms are flattened to the text they wrap: `` [`X`](destination) `` and
/// `` [`X`] `` unconditionally, since neither destination exists outside rustdoc, and
/// `[label]` when the definition that gave it a destination has just been dropped.
fn rewrite_links(line: &str, orphaned: &BTreeSet<String>) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'[' {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        match link_at(line, i, orphaned) {
            Some((text, next)) => {
                out.push_str(text);
                i = next;
            }
            None => {
                out.push('[');
                i += 1;
            }
        }
    }
    out
}

/// The rustdoc link starting at `start`, as the text it should become and where to resume.
fn link_at<'a>(line: &'a str, start: usize, orphaned: &BTreeSet<String>) -> Option<(&'a str, usize)> {
    let rest = &line[start + 1..];
    let close = rest.find(']')?;
    let inner = &rest[..close];
    if inner.is_empty() || inner.contains('[') {
        return None;
    }
    let after = start + 1 + close + 1;
    let code_span = inner.starts_with('`') && inner.ends_with('`') && inner.len() > 1;

    // `[`X`](destination)` — an intra-doc link written the long way.
    if line[after..].starts_with('(') {
        let end = line[after..].find(')')? + after;
        let destination = &line[after + 1..end];
        if has_url_scheme(destination) {
            return None; // portable for every reader: left alone
        }
        return code_span.then_some((inner, end + 1));
    }

    // `[X][reference]` is a collapsed link, and its own definition decides it elsewhere.
    if line[after..].starts_with('[') {
        return None;
    }

    if code_span {
        // `[`X`]` — the shortcut intra-doc link, which is the common case by far.
        return Some((inner, after));
    }
    // `[ADR 0040]`, only once its rustdoc-relative definition has been dropped.
    orphaned
        .contains(&inner.to_ascii_lowercase())
        .then_some((inner, after))
}

/// Translate every `description` of a JSON Schema document, in place.
///
/// schemars writes one for the type and one for each field, at every depth the type
/// reaches, so the walk is the whole document rather than its root.
pub fn translate_descriptions(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                match child {
                    Value::String(text) if key == "description" => {
                        let translated = to_commonmark(text);
                        if &translated != text {
                            *text = translated;
                        }
                    }
                    other => translate_descriptions(other),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(translate_descriptions),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intra_doc_link_becomes_the_code_span_it_wrapped() {
        assert_eq!(
            to_commonmark("Distinct from [`WaiverReason::ExternalDependency`]: nothing else."),
            "Distinct from `WaiverReason::ExternalDependency`: nothing else."
        );
    }

    #[test]
    fn a_long_form_intra_doc_link_loses_its_unreachable_destination() {
        assert_eq!(to_commonmark("one of [`EDGES`](select::EDGES)"), "one of `EDGES`");
    }

    #[test]
    fn a_definition_relative_to_the_source_file_is_dropped_with_its_label_kept() {
        let doc = "Classified by [ADR 0040].\n\n[ADR 0040]: ../../../../.ai/repo/adrs/0040-x.md";
        assert_eq!(to_commonmark(doc), "Classified by ADR 0040.");
    }

    #[test]
    fn a_link_every_reader_can_follow_is_kept() {
        let doc = "See [the spec](https://spec.openapis.org/oas/v3.1.0).";
        assert_eq!(to_commonmark(doc), doc);
        let definition = "Text.\n\n[spec]: https://example.org/x";
        assert_eq!(to_commonmark(definition), definition);
    }

    #[test]
    fn a_fenced_example_is_left_byte_for_byte() {
        // brackets in Rust are array literals and character classes, never links
        let doc = "Prose [`Type::X`].\n\n```\nlet v = [\"capability\"];\nlet re = \"[a-z][a-z0-9_-]\";\n```";
        let out = to_commonmark(doc);
        assert!(out.contains("let v = [\"capability\"];"), "{out}");
        assert!(out.contains("[a-z][a-z0-9_-]"), "{out}");
        assert!(out.starts_with("Prose `Type::X`."), "{out}");
    }

    #[test]
    fn prose_brackets_with_no_definition_are_somebody_s_writing() {
        let doc = "The field [omitted] is not a link.";
        assert_eq!(to_commonmark(doc), doc);
    }

    #[test]
    fn text_without_a_bracket_is_returned_unchanged() {
        let doc = "Nothing to translate here at all.";
        assert_eq!(to_commonmark(doc), doc);
    }

    #[test]
    fn every_description_of_a_document_is_translated() {
        let mut value = serde_json::json!({
            "description": "A [`Root::Thing`].",
            "properties": { "field": { "description": "See [`Other::Field`]." } },
            "title": "Untouched [`Not::A::Description`]."
        });
        translate_descriptions(&mut value);
        assert_eq!(value["description"], "A `Root::Thing`.");
        assert_eq!(value["properties"]["field"]["description"], "See `Other::Field`.");
        assert_eq!(value["title"], "Untouched [`Not::A::Description`].");
    }
}

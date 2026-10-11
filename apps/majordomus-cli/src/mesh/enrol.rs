//! Enrolment: add one machine's public key to a mesh declaration's `trust.allow`.
//!
//! The declaration is a tracked file a person reads and reviews, with comments that say why
//! each machine is there, so enrolment edits its text rather than re-rendering it: the
//! comments, the order and every other key stay as they were, and the new key arrives as one
//! list item under a comment naming the machine — the shape this repository's own
//! declaration keeps by hand. The result is parsed back as a declaration before anything is
//! returned, so an edit that would leave the file unreadable, or the key unlisted, is a
//! refusal and never a written file.
//!
//! ```
//! use majordomus_cli::mesh::enrol::enrol;
//!
//! // the declaration `majordomus init` seeds: enabled, deny_unknown, nobody listed
//! let seeded = "schema: mesh/v1\nkind: mesh-declaration\nid: mesh\nenabled: true\n\
//!     trust:\n  policy: deny_unknown\n  allow: []\n";
//! let first = "11".repeat(32);
//! let second = "22".repeat(32);
//! // one machine enrols, then another: each key once, in the order they joined
//! let one = enrol(seeded, &first, "11111111 — first").unwrap().unwrap();
//! let two = enrol(&one, &second, "22222222 — second").unwrap().unwrap();
//! assert!(two.find(&first).unwrap() < two.find(&second).unwrap());
//! assert_eq!(enrol(&two, &first, "first again").unwrap(), None);
//! ```

use super::config::MeshConfig;

/// `text` with `key` listed in `trust.allow` under a `# label` comment, or `None` when the
/// key is already listed. A missing `trust:` block or `allow:` key is added; an empty
/// `allow: []` becomes a block list. Refused when the result would not parse as a
/// declaration or would not list the key.
///
/// ```
/// use majordomus_cli::mesh::enrol::enrol;
///
/// let key = "ab".repeat(32);
/// let head = "schema: mesh/v1\nkind: mesh-declaration\nid: mesh\nenabled: true\n";
/// let declared = format!("{head}# the trust list\ntrust:\n  policy: deny_unknown\n  allow: []\n");
/// let declared = declared.as_str();
/// let enrolled = enrol(declared, &key, "abababab — a machine").unwrap().unwrap();
/// assert!(enrolled.contains("# the trust list"), "the comments stay");
/// assert!(enrolled.contains(&format!("  allow:\n    # abababab — a machine\n    - \"{key}\"\n")));
/// // once is enough: a listed key changes nothing
/// assert_eq!(enrol(&enrolled, &key, "again").unwrap(), None);
/// ```
pub fn enrol(text: &str, key: &str, label: &str) -> Result<Option<String>, String> {
    if !(key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err(format!("{key} is not a public key (64 hex characters)"));
    }
    let before = parse(text)?;
    if before.trust.allow.iter().any(|k| k == key) {
        return Ok(None);
    }
    let label = label.replace(['\n', '\r'], " ");
    let item = [format!("    # {label}"), format!("    - \"{key}\"")];
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    match lines.iter().position(|l| l.trim_end() == "trust:") {
        None => {
            lines.extend(
                ["trust:", "  policy: deny_unknown", "  allow:"]
                    .iter()
                    .map(|s| s.to_string()),
            );
            lines.extend(item);
        }
        Some(trust) => {
            // the trust block: every following line that is indented, blank or a comment
            let end = lines[trust + 1..]
                .iter()
                .position(|l| !(l.is_empty() || l.starts_with(' ') || l.starts_with('#')))
                .map_or(lines.len(), |i| trust + 1 + i);
            let allow = (trust + 1..end).find(|&i| lines[i].trim_start().starts_with("allow:"));
            match allow {
                None => {
                    let mut at = end;
                    while at > trust + 1 && lines[at - 1].trim().is_empty() {
                        at -= 1;
                    }
                    let mut block = vec!["  allow:".to_string()];
                    block.extend(item);
                    lines.splice(at..at, block);
                }
                Some(i) => {
                    let inline = lines[i].trim_start()["allow:".len()..].trim().to_string();
                    if inline.is_empty() {
                        // a block list: after its last item or comment
                        let mut at = i + 1;
                        while at < end
                            && (lines[at].starts_with("    -") || lines[at].starts_with("    #"))
                        {
                            at += 1;
                        }
                        lines.splice(at..at, item);
                    } else {
                        // an inline list, `[]` or `[a, b]`: rewritten as a block list
                        let listed: Vec<String> = before
                            .trust
                            .allow
                            .iter()
                            .map(|k| format!("    - \"{k}\""))
                            .collect();
                        let mut block = vec!["  allow:".to_string()];
                        block.extend(listed);
                        block.extend(item);
                        lines.splice(i..=i, block);
                    }
                }
            }
        }
    }

    let mut after = lines.join("\n");
    after.push('\n');
    let parsed = parse(&after)?;
    if !parsed.trust.allow.iter().any(|k| k == key) {
        return Err("the edited declaration does not list the key; nothing was changed".into());
    }
    Ok(Some(after))
}

fn parse(text: &str) -> Result<MeshConfig, String> {
    let config = crate::metadata::yaml::parse_into::<MeshConfig>(text)
        .map_err(|e| format!("the declaration does not parse as mesh/v1: {e}"))?;
    if config.schema != super::config::SCHEMA_VERSION || config.kind != super::KIND {
        return Err(format!(
            "not a mesh declaration: schema {}, kind {}",
            config.schema, config.kind
        ));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const HEAD: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: mesh\nenabled: true\n";

    fn allow_of(text: &str) -> Vec<String> {
        parse(text).unwrap().trust.allow
    }

    #[test]
    fn a_block_list_gains_one_item_after_its_last_and_keeps_its_comments() {
        let other = "f".repeat(64);
        let text = format!(
            "{HEAD}trust:\n  policy: deny_unknown\n  allow:\n    # the first machine\n    - {other}\ncooperation:\n  heartbeat_seconds: 5\n"
        );
        let after = enrol(&text, KEY, "01234567 — host").unwrap().unwrap();
        assert_eq!(allow_of(&after), [other.clone(), KEY.to_string()]);
        assert!(after.contains("    # the first machine\n"));
        assert!(
            after.ends_with("cooperation:\n  heartbeat_seconds: 5\n"),
            "{after}"
        );
        assert_eq!(enrol(&after, KEY, "x").unwrap(), None, "idempotent");
    }

    #[test]
    fn a_missing_trust_block_or_allow_key_is_added() {
        let after = enrol(HEAD, KEY, "host").unwrap().unwrap();
        assert_eq!(allow_of(&after), [KEY]);
        let text =
            format!("{HEAD}trust:\n  policy: deny_unknown\n\nmulticast:\n  enabled: false\n");
        let after = enrol(&text, KEY, "host").unwrap().unwrap();
        assert_eq!(allow_of(&after), [KEY]);
        assert!(after.contains("  allow:\n    # host\n"), "{after}");
        assert!(
            !parse(&after).unwrap().multicast.enabled,
            "the rest is untouched"
        );
    }

    #[test]
    fn an_inline_list_becomes_a_block_list_with_every_key() {
        let other = "e".repeat(64);
        let text = format!("{HEAD}trust:\n  policy: deny_unknown\n  allow: [\"{other}\"]\n");
        let after = enrol(&text, KEY, "host").unwrap().unwrap();
        assert_eq!(allow_of(&after), [other, KEY.to_string()]);
    }

    #[test]
    fn a_bad_key_or_an_unreadable_declaration_is_refused() {
        assert!(enrol(HEAD, "not-a-key", "x").is_err());
        assert!(enrol("schema: mesh/v1\nkind: other\n", KEY, "x").is_err());
    }
}

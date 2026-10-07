//! A tool named in prose is a tool the registry projects.
//!
//! An MCP tool's name is declared once, on its capability, and every listing of tools is
//! derived from the registry. Prose is the exception nothing derives: the manual, the
//! bootstrap files every worker reads first, the provider templates those are generated
//! from, the site's templates. A tool renamed or removed leaves its old name standing in
//! all of them, and the reader who trusts the document calls a tool that answers
//! `unknown tool`.
//!
//! So the authored files are read, every token shaped like a tool name is collected, and
//! each must be a tool of the registry or one of the few tokens below that only look like
//! one, each with the reason it is not. Generated documents are not read: they cannot
//! disagree with the registry, and `generate --check` holds them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use majordomus_cli::capability::{builtin, CapabilityRegistry};

/// Tokens with the shape of a tool name that are not tools, and why each is here.
const NOT_TOOLS: &[(&str, &str)] = &[
    ("majordomus_cli", "the crate's name in Rust paths"),
    (
        "majordomus_executable",
        "a variable of the command templates in docs/COMMANDS.md",
    ),
    (
        "majordomus_plan_done",
        "docs/COCKPIT_IDE_AUDIT.md quotes the server refusing it as an unknown tool",
    ),
];

fn repository() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // the crate also builds alone, from a package that carries no repository around it
    root.join("docs/MCP.md").is_file().then_some(root)
}

fn walk(dir: &Path, suffix: &str, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, suffix, into);
        } else if path.to_string_lossy().ends_with(suffix) {
            into.push(path);
        }
    }
}

/// The authored files a reader takes tool names from. `docs/generated` is derived and is
/// deliberately not here.
fn authored(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in [
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        "apps/majordomus-cli/README.md",
    ] {
        files.push(root.join(name));
    }
    if let Ok(entries) = std::fs::read_dir(root.join("docs")) {
        files.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md")),
        );
    }
    walk(&root.join(".ai/repo/providers"), ".tmpl", &mut files);
    walk(&root.join(".ai/repo/rules/project"), ".md", &mut files);
    walk(&root.join(".ai/repo/workflows"), ".md", &mut files);
    walk(&root.join("site/templates"), ".html", &mut files);
    files.sort();
    files
}

/// Every token of `text` shaped like a tool name: `majordomus_` and then lower-case
/// letters, digits and underscores, not continuing an identifier on its left.
fn tool_shaped(text: &str) -> BTreeSet<String> {
    const PREFIX: &str = "majordomus_";
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
    let mut found = BTreeSet::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(PREFIX) {
        let start = from + at;
        let mut end = start + PREFIX.len();
        while end < bytes.len() && word(bytes[end]) {
            end += 1;
        }
        let continues = start > 0
            && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        // `majordomus_*` is a way of writing "every tool", not a tool
        if !continues && end > start + PREFIX.len() {
            found.insert(text[start..end].trim_end_matches('_').to_string());
        }
        from = end;
    }
    found
}

#[test]
fn every_tool_named_in_authored_prose_is_a_tool_of_the_registry() {
    let Some(root) = repository() else {
        return;
    };
    let registry = CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .build()
        .unwrap();
    let tools: BTreeSet<String> = registry
        .iter()
        .filter_map(|c| c.exposure.mcp.as_ref()?.tool.clone())
        .collect();
    assert!(!tools.is_empty(), "the registry projects no tool at all");

    let files = authored(&root);
    let mut read = 0;
    let mut named = 0;
    let mut unknown = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        read += 1;
        for token in tool_shaped(&text) {
            if tools.contains(&token) {
                named += 1;
            } else if !NOT_TOOLS.iter().any(|(t, _)| *t == token) {
                let shown = file.strip_prefix(&root).unwrap_or(file).display();
                unknown.push(format!("{token} in {shown}"));
            }
        }
    }
    // a walk that found nothing would pass for the wrong reason
    assert!(read > 10, "only {read} authored file(s) were read");
    assert!(named > 10, "only {named} tool name(s) were found in prose");
    assert!(
        unknown.is_empty(),
        "prose names {} tool(s) the registry does not project — rename them to the tool \
         that exists, or add the token to NOT_TOOLS with the reason it is not one:\n  {}",
        unknown.len(),
        unknown.join("\n  ")
    );
}

/// An exemption that stopped being needed is a hole: the token it covers could come back
/// as a real, wrong tool name and pass.
#[test]
fn every_exempted_token_still_appears_and_is_still_not_a_tool() {
    let Some(root) = repository() else {
        return;
    };
    let registry = CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .build()
        .unwrap();
    let mut seen = BTreeSet::new();
    for file in authored(&root) {
        if let Ok(text) = std::fs::read_to_string(&file) {
            seen.extend(tool_shaped(&text));
        }
    }
    for (token, why) in NOT_TOOLS {
        assert!(
            registry.by_mcp_tool(token).is_none(),
            "{token} is exempted as '{why}' and is a tool of the registry"
        );
        assert!(
            seen.contains(*token),
            "{token} is exempted as '{why}' and no authored file names it any more"
        );
    }
}

#[test]
fn the_scanner_finds_a_tool_name_and_nothing_that_only_contains_one() {
    let found = tool_shaped(
        "call `majordomus_peers`, then majordomus_plan_transition; not xmajordomus_no, \
         not the pattern majordomus_* and not majordomus_ alone.",
    );
    let expected: BTreeSet<String> = ["majordomus_peers", "majordomus_plan_transition"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(found, expected);
}

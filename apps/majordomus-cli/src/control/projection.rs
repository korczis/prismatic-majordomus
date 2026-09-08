//! Names on other surfaces, computed rather than kept.
//!
//! A canonical command has one identity and several spellings: the words a person types,
//! the recipe `just` runs, the tool an MCP client lists, the action the Cockpit offers,
//! the page the documentation publishes. Every one of those is a function of the identity
//! and of the surface's own naming rules, so this module is an algorithm and never a
//! table. Two commands whose spellings collide on one surface are a defect the graph
//! reports with both ends named, not a silent winner.
//!
//! ```
//! use majordomus_cli::control::projection::{just_recipe, mcp_tool, docs_route};
//! assert_eq!(just_recipe(&["worktree".into(), "create".into()]), "worktree-create");
//! assert_eq!(mcp_tool(&["capabilities".into(), "list".into()]), "majordomus_capabilities_list");
//! assert_eq!(docs_route(&["bench".into(), "coverage".into()]), "/docs/cli/bench/coverage/");
//! ```

use serde::{Deserialize, Serialize};

use schemars::JsonSchema;

/// Words `just` will not accept as a recipe name, or would read as something else. A
/// recipe that would land on one of these is prefixed rather than renamed by hand.
const JUST_RESERVED: &[&str] = &[
    "alias", "assert", "else", "export", "false", "if", "import", "mod", "set", "shell", "true",
    "unexport",
];

/// The `just` recipe name for a command path: the words joined by a hyphen.
///
/// A leading digit, a leading hyphen or a reserved word would make an unusable recipe, so
/// the name is prefixed with `mj-` in those cases. The rule is stated once here; the
/// projection is regenerated, so a name never has to be remembered.
pub fn just_recipe(path: &[String]) -> String {
    let joined: String = path
        .iter()
        .map(|w| w.replace('_', "-"))
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let unusable = joined.is_empty()
        || joined.starts_with('-')
        || joined.starts_with(|c: char| c.is_ascii_digit())
        || JUST_RESERVED.contains(&joined.as_str());
    if unusable {
        format!("mj-{joined}")
    } else {
        joined
    }
}

/// The MCP tool name for a command path: the words joined by an underscore, under the
/// executable's own prefix, which is what every existing tool of this registry is named.
pub fn mcp_tool(path: &[String]) -> String {
    let mut s = String::from("majordomus");
    for word in path {
        s.push('_');
        s.push_str(&word.replace('-', "_"));
    }
    s
}

/// The Cockpit's action identity: the canonical id itself, because the Cockpit addresses
/// by identity and renders the label from the summary.
pub fn cockpit_action(id: &str) -> String {
    id.to_string()
}

/// The documentation route of a command path. The prefix belongs to the command line's
/// own reference, and is the one `cli::route` already publishes.
pub fn docs_route(path: &[String]) -> String {
    let mut s = String::from(crate::cli::ROUTE_PREFIX);
    for word in path {
        s.push('/');
        s.push_str(word);
    }
    s.push('/');
    s
}

/// Every spelling of one command, computed together so that a consumer never assembles
/// one of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Projections {
    /// The words after `majordomus`.
    pub cli: Vec<String>,
    /// The recipe `just` runs, when the command is bridged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub just: Option<String>,
    /// The MCP tool, when a machine surface may call it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<String>,
    /// The HTTP route, when the capability behind it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    /// The Cockpit action, when a machine surface may call it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cockpit: Option<String>,
    /// The page the documentation publishes.
    pub docs: String,
}

/// One surface, for a diagnostic that has to say where a collision happened.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// The command line of the executable.
    Cli,
    /// The `just` bridge.
    Just,
    /// MCP.
    Mcp,
    /// HTTP.
    Http,
    /// The Cockpit.
    Cockpit,
    /// The generated documentation.
    Docs,
}

impl Surface {
    /// The word a diagnostic prints.
    pub fn label(self) -> &'static str {
        match self {
            Surface::Cli => "cli",
            Surface::Just => "just",
            Surface::Mcp => "mcp",
            Surface::Http => "http",
            Surface::Cockpit => "cockpit",
            Surface::Docs => "docs",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn a_reserved_word_is_prefixed_rather_than_renamed_by_hand() {
        assert_eq!(just_recipe(&path(&["mod"])), "mj-mod");
        assert_eq!(just_recipe(&path(&["set"])), "mj-set");
        assert_eq!(just_recipe(&path(&["serve"])), "serve");
    }

    #[test]
    fn a_name_just_cannot_parse_is_never_produced() {
        for words in [vec!["9lives"], vec!["a b"], vec!["a$b"], vec![""]] {
            let name = just_recipe(&path(&words));
            assert!(
                !name.starts_with(|c: char| c.is_ascii_digit()),
                "{name} starts with a digit"
            );
            let body = name.trim_start_matches("mj-");
            assert!(
                body.is_empty()
                    || body
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "{name} carries a character just cannot parse"
            );
        }
    }

    #[test]
    fn the_mcp_name_matches_what_the_registry_already_produces() {
        // the existing tools are majordomus_<module>_<capability>; the algorithm agrees
        assert_eq!(
            mcp_tool(&path(&["web", "surfaces"])),
            "majordomus_web_surfaces"
        );
        assert_eq!(mcp_tool(&path(&["repository"])), "majordomus_repository");
    }
}

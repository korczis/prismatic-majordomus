//! One highlighter for every code block, on every surface (ADR 0166 §7).
//!
//! The published site is built by Zola 0.23, and Zola highlights with giallo 0.5.2: VS Code's
//! TextMate grammars, matched by a VS Code theme, written as classes. This module is the same
//! engine at the same version, configured as the site configures it, so a block the Cockpit or
//! a report renders carries the markup the site gives the same block — the same spans, the
//! same classes — because the same program produced both. There is no second tokeniser to
//! keep in step.
//!
//! # One theme, one stylesheet, colours from the tokens
//!
//! A giallo theme maps scopes to colours, and in class mode giallo numbers the distinct
//! colours of the theme in the order of their hex spelling: a token gets `z-<n>`, and the
//! theme's own stylesheet says what `z-<n>` looks like. The theme written here
//! ([`theme_json`]) uses that numbering as a vocabulary rather than as a palette. Its colours
//! are sentinels — `#000001` for the first kind of [`KINDS`], `#000002` for the second — so the
//! class a token gets names the *kind* of token it is (`z-2` is a keyword on every surface and
//! under either theme), and nothing in the theme is a colour anybody chose.
//!
//! The colour is chosen once, in `share/design/tokens.yaml`, as a role per kind
//! (`code-keyword`, light and dark). [`stylesheet`] writes `share/design/code.css`, which says
//! `.z-2 { color: var(--mj-code-keyword); }`: a custom property the generated surface sheet
//! already resolves per theme. So one stylesheet serves both themes, every surface that
//! imports it (the site, the UI, the Cockpit, the handbook) colours code identically, and the
//! contrast measurement (`design.contrast`) holds the code roles to WCAG AA like every other
//! foreground, because `code.css` is one of the stylesheets it reads.
//!
//! The theme is written for Zola as `site/highlighting/majordomus.json` by
//! `majordomus generate`; the executable registers the same bytes in-process, and a test
//! holds the two equal.

use std::sync::OnceLock;

use giallo::{
    parse_markdown_fence, DataAttrPosition, ExtraHtmlContent, HighlightOptions, HtmlRenderer,
    Registry, ThemeVariant,
};
use serde_json::{json, Value};

use crate::design::DesignSystem;

/// The theme's name, as the site's configuration names it (`theme = "majordomus"`). giallo
/// keys themes by their lowercased name, so it is written lowercase.
pub(crate) const THEME: &str = "majordomus";

/// The prefix Zola gives every class in class mode, and that this module gives them too.
pub(crate) const CLASS_PREFIX: &str = "z-";

/// The theme file, written for Zola by `majordomus generate` and named by
/// `site/config.toml` (`extra_themes`). Relative to the repository root.
pub(crate) const THEME_FILE: &str = "site/highlighting/majordomus.json";

/// The one stylesheet of highlighted code, imported by every Tailwind build and inlined by
/// the handbook. Relative to the repository root.
pub(crate) const STYLESHEET: &str = "share/design/code.css";

/// One kind of token the theme distinguishes: the design role that colours it, the
/// TextMate scopes that select it, and the font style it adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Kind {
    /// The design role (`share/design/tokens.yaml`, `roles:`) whose colour the kind takes.
    pub role: &'static str,
    /// The scope selectors, in VS Code's theme syntax.
    pub scopes: &'static [&'static str],
    /// `italic`, `bold`, or empty.
    pub font_style: &'static str,
}

/// The kinds, in class order: the first is `z-1`. Appending a kind is safe; reordering or
/// removing one renumbers every class after it, which is a change to the published markup.
pub(crate) const KINDS: &[Kind] = &[
    Kind {
        role: "code-comment",
        scopes: &["comment", "punctuation.definition.comment", "markup.quote"],
        font_style: "italic",
    },
    Kind {
        role: "code-keyword",
        scopes: &[
            "keyword",
            "storage",
            "storage.type",
            "storage.modifier",
            "keyword.control",
            "markup.list punctuation.definition.list",
        ],
        font_style: "",
    },
    Kind {
        role: "code-string",
        scopes: &[
            "string",
            "string.unquoted",
            "markup.inline.raw",
            "markup.fenced_code",
            "markup.raw",
            "markup.inserted",
            "punctuation.definition.string",
        ],
        font_style: "",
    },
    Kind {
        role: "code-constant",
        scopes: &[
            "constant",
            "constant.numeric",
            "constant.language",
            "constant.character",
            "constant.other",
            "support.constant",
            "variable.other.constant",
        ],
        font_style: "",
    },
    Kind {
        role: "code-function",
        scopes: &[
            "entity.name.function",
            "support.function",
            "entity.name.command",
            "meta.diff.range",
            "meta.diff.header",
            "markup.underline.link",
            "markup.heading",
            "entity.name.section.markdown",
        ],
        font_style: "",
    },
    Kind {
        role: "code-type",
        scopes: &[
            "entity.name.type",
            "entity.name.class",
            "entity.name.namespace",
            "entity.name.section",
            "entity.other.inherited-class",
            "support.type",
            "support.class",
        ],
        font_style: "",
    },
    Kind {
        role: "code-tag",
        scopes: &[
            "entity.name.tag",
            "entity.other.attribute-name",
            "support.type.property-name",
            "meta.object-literal.key",
            "variable.other.key",
            "variable.other.normal",
            "variable.other.special",
            "variable.other.positional",
            "variable.other.bracket",
            "variable.parameter",
            "markup.deleted",
        ],
        font_style: "",
    },
];

/// Scopes a broader selector above would colour and that read best in the text colour: an
/// operator is a `keyword` to the grammar and noise to a reader. They take the theme's
/// default foreground, which no class is written for.
const PLAIN: &[&str] = &[
    "keyword.operator",
    "punctuation.separator",
    "punctuation.terminator",
    "storage.type.annotation",
];

/// Font styles that carry meaning in prose-like grammars (Markdown), independent of colour.
const STYLES: &[(&str, &str)] = &[
    ("markup.bold", "bold"),
    ("markup.italic", "italic"),
    ("markup.heading", "bold"),
    ("markup.strikethrough", "strikethrough"),
];

/// The theme's default foreground: no class is written for a token that keeps it.
const DEFAULT_FOREGROUND: &str = "#000000";

/// The sentinel colour of the kind at `index` (0-based): `#000001` for the first. Zero-padded
/// to one width, so giallo's ordering by hex spelling is the order of [`KINDS`].
fn sentinel(index: usize) -> String {
    format!("#{:06x}", index + 1)
}

/// The theme, as the VS Code theme JSON giallo and Zola read.
///
/// The colours are sentinels (see the module documentation): what this file decides is which
/// scope is which kind, never how a kind looks.
pub(crate) fn theme_value() -> Value {
    let mut rules = Vec::new();
    for (index, kind) in KINDS.iter().enumerate() {
        let mut settings = json!({ "foreground": sentinel(index) });
        if !kind.font_style.is_empty() {
            settings["fontStyle"] = json!(kind.font_style);
        }
        rules.push(json!({
            "name": kind.role,
            "scope": kind.scopes,
            "settings": settings,
        }));
    }
    rules.push(json!({
        "name": "plain",
        "scope": PLAIN,
        "settings": { "foreground": DEFAULT_FOREGROUND },
    }));
    for (scope, style) in STYLES {
        rules.push(json!({
            "name": format!("style {style}"),
            "scope": [scope],
            "settings": { "fontStyle": style },
        }));
    }
    json!({
        "name": THEME,
        "type": "light",
        "colors": {
            "editor.foreground": DEFAULT_FOREGROUND,
            "editor.background": "#ffffff",
        },
        "tokenColors": rules,
    })
}

/// The theme as text, the bytes the executable registers.
pub(crate) fn theme_json() -> String {
    serde_json::to_string_pretty(&theme_value()).unwrap_or_default() + "\n"
}

/// The stylesheet of highlighted code: every class giallo writes, coloured from the design's
/// roles. Refused when a kind names a role the declaration does not hold, because the
/// stylesheet would otherwise name a custom property that resolves to nothing.
pub(crate) fn stylesheet(design: &DesignSystem) -> Result<String, String> {
    let mut missing = Vec::new();
    for kind in KINDS {
        if design.roles.get(kind.role).is_none() {
            missing.push(kind.role);
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "the highlighter colours code with role(s) the design does not declare: {}",
            missing.join(", ")
        ));
    }
    let p = CLASS_PREFIX;
    let mut css = String::new();
    css.push_str(
        "/* Highlighted code (ADR 0166 §7). One engine writes these classes on every surface:\n   \
         giallo, through Zola on the site and through the executable everywhere else. A class\n   \
         names the kind of token, never a colour; the colour is the design's role for that\n   \
         kind, which the surface sheet resolves for the theme in force. */\n",
    );
    css.push_str(&format!(
        ".{p}code {{\n  color: var(--mj-fg);\n  background-color: var(--mj-sunken);\n}}\n"
    ));
    // a highlighted line states its text with its ground, so the ground is a pair and not a
    // container every other foreground of the design is measured on
    css.push_str(&format!(
        ".{p}hl {{\n  color: var(--mj-fg);\n  background-color: var(--mj-accent-soft);\n}}\n"
    ));
    for (index, kind) in KINDS.iter().enumerate() {
        css.push_str(&format!(
            ".{p}{} {{ color: var(--mj-{}); }}\n",
            index + 1,
            kind.role
        ));
    }
    for (class, rule) in [
        ("b", "font-weight: bold;"),
        ("i", "font-style: italic;"),
        ("u", "text-decoration: underline;"),
        ("s", "text-decoration: line-through;"),
        ("us", "text-decoration: underline line-through;"),
    ] {
        css.push_str(&format!(".{p}{class} {{ {rule} }}\n"));
    }
    Ok(css)
}

/// The engine, loaded once per process: giallo's built-in grammars and the one theme.
///
/// giallo registers a theme from a file and from nothing else, so the theme's bytes are
/// written to a file of this process's own in the temporary directory, read, and removed.
fn registry() -> Result<&'static Registry, &'static str> {
    static ENGINE: OnceLock<Result<Registry, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut registry = Registry::builtin().map_err(|e| e.to_string())?;
            let path = std::env::temp_dir().join(format!(
                "majordomus-highlight-theme-{}.json",
                std::process::id()
            ));
            std::fs::write(&path, theme_json()).map_err(|e| format!("{}: {e}", path.display()))?;
            let added = registry.add_theme_from_path(&path);
            let _ = std::fs::remove_file(&path);
            added.map_err(|e| e.to_string())?;
            registry.link_grammars();
            Ok(registry)
        })
        .as_ref()
        .map_err(String::as_str)
}

/// A code block as the site renders it: `info` is what follows the opening fence of a
/// Markdown block (`yaml`, `rust,linenos`, or empty), `code` its content. The markup is
/// giallo's, configured as `site/config.toml` configures Zola: class mode with the `z-`
/// prefix, the one theme, an unknown language drawn as plain text.
///
/// Should the engine itself be unavailable, the block is drawn as Zola draws one it could not
/// highlight: escaped, in a bare `<pre><code>`.
pub(crate) fn code_block(info: &str, code: &str) -> String {
    let fence = parse_markdown_fence(info);
    let plain = || {
        format!(
            "<pre><code>{}</code></pre>\n",
            crate::cockpit::html::escape(code)
        )
    };
    let Ok(registry) = registry() else {
        return plain();
    };
    let renderer = HtmlRenderer {
        other_metadata: fence.rest,
        data_attr_position: DataAttrPosition::default(),
        css_class_prefix: Some(CLASS_PREFIX.to_string()),
        extra_html_content: ExtraHtmlContent::default(),
        add_color_scheme: false,
    };
    let options =
        HighlightOptions::new(&fence.lang, ThemeVariant::Single(THEME)).fallback_to_plain(true);
    match registry.highlight(code, &options) {
        Ok(highlighted) => renderer.render(&highlighted, &fence.options),
        Err(_) => plain(),
    }
}

/// The fence language of a file, read from its name: what a Markdown author would write
/// after the backticks to show that file. Empty — plain text — for anything else.
pub(crate) fn language_of(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    match extension.to_ascii_lowercase().as_str() {
        "md" => "markdown",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "jsonl" => "jsonl",
        "toml" => "toml",
        "rs" => "rust",
        "sh" => "bash",
        "js" | "mjs" => "javascript",
        "css" => "css",
        "html" => "html",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_is_highlighted_in_class_mode_with_the_sites_prefix() {
        let html = code_block("yaml", "id: I1\ntitle: \"one\"  # why\n");
        assert!(
            html.starts_with(r#"<pre class="giallo z-code" ><code data-lang="yaml">"#),
            "{html}"
        );
        // a key, a string and a comment are three kinds, each with its own class
        let tag = format!(r#"class="z-{}""#, index_of("code-tag"));
        let string = format!(r#"class="z-{}"#, index_of("code-string"));
        let comment = format!(r#"class="z-{} z-i""#, index_of("code-comment"));
        assert!(html.contains(&tag), "no key class in {html}");
        assert!(html.contains(&string), "no string class in {html}");
        assert!(html.contains(&comment), "no comment class in {html}");
        // and no colour is written inline: the stylesheet decides
        assert!(!html.contains("style="), "{html}");
    }

    fn index_of(role: &str) -> usize {
        KINDS.iter().position(|k| k.role == role).expect("a kind") + 1
    }

    #[test]
    fn an_unknown_language_is_plain_text_and_markup_is_escaped() {
        let html = code_block("no-such-language", "<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("giallo-l"), "{html}");
        let empty = code_block("", "plain words");
        assert!(empty.contains("plain words"), "{empty}");
    }

    #[test]
    fn fence_options_reach_the_renderer() {
        let html = code_block("rust,linenos,hl_lines=1", "fn main() {}\n");
        assert!(html.contains("giallo-ln"), "line numbers: {html}");
        assert!(html.contains("z-hl"), "highlighted line: {html}");
    }

    #[test]
    fn every_kind_has_its_own_sentinel_in_class_order() {
        let theme = theme_value();
        let rules = theme["tokenColors"].as_array().expect("rules");
        for (index, kind) in KINDS.iter().enumerate() {
            let rule = rules
                .iter()
                .find(|r| r["name"] == kind.role)
                .expect("a rule per kind");
            assert_eq!(rule["settings"]["foreground"], sentinel(index));
        }
        let mut sorted: Vec<String> = (0..KINDS.len()).map(sentinel).collect();
        sorted.sort();
        assert_eq!(sorted, (0..KINDS.len()).map(sentinel).collect::<Vec<_>>());
        assert_eq!(theme["name"], THEME);
        assert!(theme_json().ends_with("}\n"));
    }

    #[test]
    fn the_stylesheet_colours_every_kind_from_its_role() {
        let design = DesignSystem::compiled().expect("the compiled design");
        let css = stylesheet(design).expect("every role is declared");
        for (index, kind) in KINDS.iter().enumerate() {
            assert!(
                css.contains(&format!(
                    ".z-{} {{ color: var(--mj-{}); }}",
                    index + 1,
                    kind.role
                )),
                "{css}"
            );
        }
        assert!(css.contains(".z-code {"));
        assert!(
            !css.contains('#'),
            "a colour literal in the stylesheet: {css}"
        );
        crate::design::render::well_formed(&css).expect("well formed");
    }

    #[test]
    fn a_role_the_design_does_not_declare_is_refused() {
        let design = DesignSystem::compiled().expect("the compiled design");
        let mut pruned = design.clone();
        pruned.roles.0.retain(|(name, _)| name != "code-keyword");
        let err = stylesheet(&pruned).expect_err("refused");
        assert!(err.contains("code-keyword"), "{err}");
    }

    #[test]
    fn a_files_language_is_read_from_its_name() {
        assert_eq!(language_of("docs/A.md"), "markdown");
        assert_eq!(language_of(".ai/repo/project/issues/I1.yaml"), "yaml");
        assert_eq!(language_of("x.YML"), "yaml");
        assert_eq!(language_of("a/b.json"), "json");
        assert_eq!(language_of("Cargo.toml"), "toml");
        assert_eq!(language_of("src/lib.rs"), "rust");
        assert_eq!(language_of("bin/tool"), "");
        assert_eq!(language_of("scripts/x.sh"), "bash");
        assert_eq!(language_of("a.jsonl"), "jsonl");
        assert_eq!(language_of("share/ui/ui.js"), "javascript");
        assert_eq!(language_of("share/design/code.css"), "css");
        assert_eq!(language_of("site/templates/base.html"), "html");
    }
}

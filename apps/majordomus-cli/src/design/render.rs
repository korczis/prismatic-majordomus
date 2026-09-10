//! The projections of the design system: every stylesheet, dataset and document a surface
//! consumes, rendered from one [`DesignSystem`]. Nothing here chooses a value; every
//! function is a pure rendering of the declaration, and `majordomus generate design`
//! writes what they return behind the provenance banner every generated file carries.
//!
//! The projections, and what reads each:
//!
//! | file | what | read by |
//! |---|---|---|
//! | `share/design/theme.css` | the Tailwind `@theme`, and Flowbite's names as synonyms | both Tailwind builds |
//! | `share/design/surface.css` | every role and status colour as `--mj-*`, light and dark | both Tailwind builds |
//! | `share/design/status.css` | one selector group per state word | both Tailwind builds |
//! | `apps/majordomus-cli/src/web/tokens.css` | the same tokens, no Tailwind | the executable's own pages |
//! | `apps/majordomus-cli/src/design/tokens.yaml` | the declaration itself | the executable |
//! | `site/data/registry/design.json` | the vocabulary, the theme contract, the widths | the site's templates and the probes |
//! | `docs/generated/design.{json,yaml,md}` | the inventory | a reader |

use serde_json::{json, Map, Value};

use super::{DesignSystem, Resolved, TokenKind, PREFIX, SITE_SCHEMA, SOURCE};

/// The Tailwind theme both builds import.
pub const THEME_CSS: &str = "share/design/theme.css";
/// The semantic surface both builds import.
pub const SURFACE_CSS: &str = "share/design/surface.css";
/// The status vocabulary both builds import.
pub const STATUS_CSS: &str = "share/design/status.css";
/// The tokens compiled into the executable for the pages without Tailwind.
pub const TOKENS_CSS: &str = "apps/majordomus-cli/src/web/tokens.css";
/// The declaration compiled into the executable.
pub const COMPILED_YAML: &str = "apps/majordomus-cli/src/design/tokens.yaml";
/// The mark compiled into the executable for the Cockpit's shell.
pub const COMPILED_MARK: &str = "apps/majordomus-cli/src/cockpit/logo-mark.svg";
/// The dataset the site's templates read.
pub const SITE_JSON: &str = "site/data/registry/design.json";
/// The stem of the documents under `docs/generated/`.
pub const DOCUMENT: &str = "design";

/// Where a projection says it came from.
pub const SOURCE_LINE: &str = "share/design/tokens.yaml, the one declaration of the design";

/// The generated files a token of this kind reaches. Answered per kind rather than per
/// token: a role reaches every stylesheet by construction, and pretending to compute it
/// would be computing a constant.
pub fn projections_of(kind: TokenKind) -> Vec<String> {
    let common = [
        COMPILED_YAML,
        "docs/generated/design.json",
        "docs/generated/design.yaml",
        "docs/generated/design.md",
    ];
    let own: &[&str] = match kind {
        TokenKind::Font | TokenKind::Type | TokenKind::Tracking | TokenKind::Radius => {
            &[THEME_CSS, TOKENS_CSS]
        }
        TokenKind::Role | TokenKind::Status => {
            &[THEME_CSS, SURFACE_CSS, STATUS_CSS, TOKENS_CSS, SITE_JSON]
        }
        TokenKind::State => &[STATUS_CSS, SITE_JSON],
        TokenKind::Layout | TokenKind::Motion => &[SURFACE_CSS, TOKENS_CSS],
        TokenKind::Theme => &[SURFACE_CSS, TOKENS_CSS, SITE_JSON],
        TokenKind::Palette => &[],
    };
    own.iter()
        .chain(common.iter())
        .map(|s| s.to_string())
        .collect()
}

// -------------------------------------------------------------------- declarations

fn decl(out: &mut String, indent: &str, name: &str, value: &str) {
    out.push_str(indent);
    out.push_str(name);
    out.push_str(": ");
    out.push_str(value);
    out.push_str(";\n");
}

/// The declarations that belong to Tailwind's namespaces: type stacks, the named scale,
/// the letter-spacings, the radii. In `@theme` for a Tailwind build; on `:root` for a page
/// without one.
fn theme_declarations(design: &DesignSystem) -> Vec<(String, String)> {
    let mut out = vec![
        ("--font-sans".to_string(), design.font.sans.clone()),
        ("--font-mono".to_string(), design.font.mono.clone()),
    ];
    for (name, step) in design.type_.scale.iter() {
        out.push((format!("--text-{name}"), step.size.clone()));
        out.push((
            format!("--text-{name}--line-height"),
            step.leading.to_string(),
        ));
    }
    for (name, named) in design.type_.tracking.iter() {
        out.push((format!("--tracking-{name}"), named.value.clone()));
    }
    for (name, named) in design.radius.iter() {
        out.push((format!("--radius-{name}"), named.value.clone()));
    }
    out
}

/// The theme-independent `--mj-*` declarations: the fingerprint, layout, motion.
fn constant_declarations(design: &DesignSystem) -> Vec<(String, String)> {
    let mut out = vec![(
        format!("{PREFIX}design"),
        format!("\"{}\"", design.short_fingerprint()),
    )];
    for (name, named) in design.layout.iter() {
        out.push((format!("{PREFIX}{name}"), named.value.clone()));
    }
    for (name, named) in design.motion.iter() {
        out.push((format!("{PREFIX}motion-{name}"), named.value.clone()));
    }
    out
}

/// Every colour of one theme: the roles, then each status's text, ground and border.
fn colour_declarations(design: &DesignSystem, dark: bool) -> Vec<(String, String)> {
    let css = |r: Option<Resolved>| r.map(|r| r.css).unwrap_or_default();
    let mut out = Vec::new();
    for (name, role) in design.roles.iter() {
        let reference = if dark { &role.dark } else { &role.light };
        out.push((
            format!("{PREFIX}{name}"),
            css(design.resolve(reference, dark)),
        ));
    }
    for (name, status) in design.status.roles.iter() {
        for (suffix, pair) in [
            ("", &status.fg),
            ("-bg", &status.bg),
            ("-line", &status.line),
        ] {
            let reference = if dark { &pair.dark } else { &pair.light };
            out.push((
                format!("{PREFIX}{name}{suffix}"),
                css(design.resolve(reference, dark)),
            ));
        }
    }
    out
}

fn block(out: &mut String, indent: &str, declarations: &[(String, String)]) {
    for (name, value) in declarations {
        decl(out, indent, name, value);
    }
}

// --------------------------------------------------------------------- stylesheets

/// `share/design/theme.css`: the Tailwind `@theme`, and Flowbite's vocabulary as synonyms.
pub fn theme_css(design: &DesignSystem) -> String {
    let mut out = String::new();
    out.push_str("@theme {\n");
    block(&mut out, "  ", &theme_declarations(design));
    out.push_str("}\n");
    if !design.alias.flowbite.is_empty() {
        out.push_str(
            "/* The site's vocabulary. Flowbite's semantic names, declared as synonyms of the roles\n   so that `text-heading` and `--mj-fg` are one value. Unlayered and after Flowbite's own\n   `.dark` block, which is unlayered too, so these win in both themes. */\n",
        );
        out.push_str(&format!(":root, .{} {{\n", design.theme.class));
        for (name, target) in design.alias.flowbite.iter() {
            let css = design.colour_css(target).unwrap_or_default();
            decl(
                &mut out,
                "  ",
                &format!("--color-{name}"),
                &format!("var({css})"),
            );
        }
        out.push_str("}\n");
    }
    out
}

/// `share/design/surface.css`: the semantic surface in the base layer, light on `:root`
/// and dark under the theme class. The dark block is a class selector *after* the root
/// one, not `:where(.dark)`: `:where()` has no specificity, `:root` has some, and the two
/// meet on the same element, so the earlier arrangement never let a dark value win.
pub fn surface_css(design: &DesignSystem) -> String {
    let mut out = String::new();
    out.push_str("@layer base {\n  :root {\n");
    decl(&mut out, "    ", "color-scheme", "light");
    block(&mut out, "    ", &constant_declarations(design));
    block(&mut out, "    ", &colour_declarations(design, false));
    out.push_str(&format!("  }}\n  .{} {{\n", design.theme.class));
    decl(&mut out, "    ", "color-scheme", "dark");
    block(&mut out, "    ", &colour_declarations(design, true));
    out.push_str("  }\n}\n");
    out
}

/// `share/design/status.css`: the vocabulary. For each status, one selector group over the
/// status and every word filed under it, setting the three indirection properties the
/// shared primitives read; and one swatch class per colour token for the inspector.
pub fn status_css(design: &DesignSystem) -> String {
    let mut out = String::new();
    out.push_str("@layer components {\n");
    for (name, _) in design.status.roles.iter() {
        let mut words: Vec<&str> = vec![name];
        if let Some(filed) = design.status.states.get(name) {
            words.extend(filed.iter().map(String::as_str).filter(|w| *w != name));
        }
        let selectors: Vec<String> = words
            .iter()
            .flat_map(|w| {
                [
                    format!(".mj-badge--{w}"),
                    format!(".mj-alert--{w}"),
                    format!(".mj-status--{w}"),
                ]
            })
            .collect();
        out.push_str("  ");
        out.push_str(&selectors.join(",\n  "));
        out.push_str(" {\n");
        decl(
            &mut out,
            "    ",
            &format!("{PREFIX}status-fg"),
            &format!("var({PREFIX}{name})"),
        );
        decl(
            &mut out,
            "    ",
            &format!("{PREFIX}status-bg"),
            &format!("var({PREFIX}{name}-bg)"),
        );
        decl(
            &mut out,
            "    ",
            &format!("{PREFIX}status-line"),
            &format!("var({PREFIX}{name}-line)"),
        );
        out.push_str("  }\n");
    }
    out.push_str("  /* one swatch per colour token, for the design inspector */\n");
    let mut swatches: Vec<String> = design.roles.keys().map(str::to_string).collect();
    for name in design.status.roles.keys() {
        swatches.push(name.to_string());
        swatches.push(format!("{name}-bg"));
        swatches.push(format!("{name}-line"));
    }
    for token in swatches {
        out.push_str(&format!(
            "  .mj-swatch--{token} {{ background: var({PREFIX}{token}); }}\n"
        ));
    }
    out.push_str("}\n");
    out
}

/// `apps/majordomus-cli/src/web/tokens.css`: every token as a plain custom property for a
/// page with no Tailwind. Dark is answered twice — by the media query, because a report is
/// often opened from the filesystem where no script has run, and by the theme class the
/// site and the Cockpit switch — with `.light` as the escape hatch a page that must stay
/// light (the Swagger shell) uses.
pub fn tokens_css(design: &DesignSystem) -> String {
    let mut out = String::new();
    out.push_str(":root {\n");
    decl(&mut out, "  ", "color-scheme", "light dark");
    block(&mut out, "  ", &theme_declarations(design));
    block(&mut out, "  ", &constant_declarations(design));
    block(&mut out, "  ", &colour_declarations(design, false));
    out.push_str("}\n@media (prefers-color-scheme: dark) {\n  :root:not(.light) {\n");
    block(&mut out, "    ", &colour_declarations(design, true));
    out.push_str(&format!("  }}\n}}\n:root.{} {{\n", design.theme.class));
    block(&mut out, "  ", &colour_declarations(design, true));
    out.push_str("}\n");
    out
}

/// Whether a rendered stylesheet can be written: every declaration has a value and every
/// line's quotes balance. The generator this replaces emitted both defects and the CSS
/// parser dropped them in silence, so nothing downstream could notice.
/// `a_declaration_with_no_value_or_an_unbalanced_quote_is_never_written` below is the
/// example.
pub fn well_formed(css: &str) -> Result<(), String> {
    for (i, line) in css.lines().enumerate() {
        if line.matches('"').count() % 2 == 1 {
            return Err(format!("line {}: unbalanced quote: {}", i + 1, line.trim()));
        }
        let trimmed = line.trim();
        if trimmed.starts_with("--") && (trimmed.ends_with(": ;") || trimmed.ends_with(":;")) {
            return Err(format!(
                "line {}: a declaration with no value: {trimmed}",
                i + 1
            ));
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------- datasets

/// `site/data/registry/design.json`: what the site's templates and the browser probes read.
/// The vocabulary as a flat word-to-status map, the theme contract with the bootstrap
/// statement, the audit widths, the fingerprint. Index-independent, so both passes of
/// the derivation graph agree byte for byte.
pub fn site_document(design: &DesignSystem, version: &str) -> Value {
    let mut states = Map::new();
    for (role, words) in design.status.states.iter() {
        states.insert(role.to_string(), Value::String(role.to_string()));
        for word in words {
            states.insert(word.clone(), Value::String(role.to_string()));
        }
    }
    let roles: Vec<Value> = design
        .roles
        .iter()
        .map(|(name, role)| {
            json!({
                "name": name,
                "css": format!("{PREFIX}{name}"),
                "about": role.about,
                "light": design.resolve(&role.light, false).map(|r| r.literal),
                "dark": design.resolve(&role.dark, true).map(|r| r.literal),
            })
        })
        .collect();
    let statuses: Vec<Value> = design
        .status
        .roles
        .iter()
        .map(|(name, status)| {
            json!({
                "name": name,
                "about": status.about,
                "fg": format!("{PREFIX}{name}"),
                "bg": format!("{PREFIX}{name}-bg"),
                "line": format!("{PREFIX}{name}-line"),
                "light": design.resolve(&status.fg.light, false).map(|r| r.literal),
                "dark": design.resolve(&status.fg.dark, true).map(|r| r.literal),
            })
        })
        .collect();
    json!({
        "schema": SITE_SCHEMA,
        "generated": crate::generate::json_banner(SOURCE_LINE),
        "generator": format!("majordomus-cli {version}"),
        "fingerprint": design.fingerprint(),
        "design": design.short_fingerprint(),
        "source": SOURCE,
        "identity": design.identity,
        "theme": {
            "class": design.theme.class,
            "storage_key": design.theme.storage_key,
            "bootstrap": design.theme_bootstrap(),
        },
        "font": design.font,
        "roles": roles,
        "statuses": statuses,
        "states": Value::Object(states),
        "viewports": design.viewports,
    })
}

/// `docs/generated/design.{json,yaml}`: the inventory and the declaration behind it.
pub fn document(design: &DesignSystem) -> Value {
    let tokens = design.tokens();
    let mut counts = Map::new();
    for kind in TokenKind::ALL {
        counts.insert(
            kind.as_str().to_string(),
            Value::from(tokens.iter().filter(|t| t.kind == *kind).count()),
        );
    }
    json!({
        "fingerprint": design.fingerprint(),
        "design": design.short_fingerprint(),
        "source": SOURCE,
        "projections": [
            THEME_CSS, SURFACE_CSS, STATUS_CSS, TOKENS_CSS, COMPILED_YAML, COMPILED_MARK, SITE_JSON,
            "docs/generated/design.json", "docs/generated/design.yaml", "docs/generated/design.md",
        ],
        "counts": Value::Object(counts),
        "tokens": tokens,
        "declaration": design,
    })
}

/// `docs/generated/design.md`: the inventory a reader gets.
pub fn reference_markdown(design: &DesignSystem) -> String {
    let mut out = String::new();
    out.push_str("# The design system\n\n");
    out.push_str(&format!(
        "One declaration, `{SOURCE}`, projected into every surface. Fingerprint `{}` (`--mj-design: \"{}\"` on every page that carries it). Explain any token with `majordomus_design_explain`, `GET /api/v1/design/explain?token=<name>` or the Cockpit's Design page.\n\n",
        design.fingerprint(),
        design.short_fingerprint()
    ));
    out.push_str("## Type\n\n| stack | value |\n|---|---|\n");
    out.push_str(&format!(
        "| `--font-sans` | {} |\n",
        code(&design.font.sans)
    ));
    out.push_str(&format!(
        "| `--font-mono` | {} |\n\n",
        code(&design.font.mono)
    ));
    out.push_str("| step | size / leading | utility | for |\n|---|---|---|---|\n");
    for (name, step) in design.type_.scale.iter() {
        out.push_str(&format!(
            "| `--text-{name}` | {} / {} | `text-{name}` | {} |\n",
            step.size, step.leading, step.about
        ));
    }
    for (name, named) in design.type_.tracking.iter() {
        out.push_str(&format!(
            "| `--tracking-{name}` | {} | `tracking-{name}` | {} |\n",
            named.value, named.about
        ));
    }
    out.push_str("\n## Roles\n\nEach role is one custom property; its value follows the theme.\n\n| role | light | dark | Flowbite names | for |\n|---|---|---|---|---|\n");
    for token in design.tokens().iter().filter(|t| t.kind == TokenKind::Role) {
        let part = &token.parts[0];
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            part.css,
            reference(&part.light),
            reference(&part.dark),
            if token.aliases.is_empty() {
                "—".to_string()
            } else {
                token
                    .aliases
                    .iter()
                    .map(|a| format!("`{a}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            token.about
        ));
    }
    out.push_str("\n## Status\n\nFive meanings; every state word any surface renders is filed under one of them and coloured by it alone.\n\n| status | text (light / dark) | ground | border | words |\n|---|---|---|---|---|\n");
    for token in design
        .tokens()
        .iter()
        .filter(|t| t.kind == TokenKind::Status)
    {
        let part = |p: &str| token.parts.iter().find(|x| x.part == p).cloned();
        let fg = part("fg").unwrap();
        let bg = part("bg").unwrap();
        let line = part("line").unwrap();
        out.push_str(&format!(
            "| `{}` | {} / {} | `{}` | `{}` | {} |\n",
            fg.css,
            reference(&fg.light),
            reference(&fg.dark),
            bg.css,
            line.css,
            token
                .states
                .iter()
                .map(|w| format!("`{w}`"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    out.push_str("\n## Layout, radius, motion\n\n| token | value | for |\n|---|---|---|\n");
    for token in design.tokens().iter().filter(|t| {
        matches!(
            t.kind,
            TokenKind::Layout | TokenKind::Radius | TokenKind::Motion
        )
    }) {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            token.css[0],
            token.value.clone().unwrap_or_default(),
            token.about
        ));
    }
    out.push_str(&format!(
        "\n## Theme\n\nThe class `{}` on the root element means dark; the choice is stored under `{}`. Every surface runs the same pre-paint statement, generated from those two values.\n\n## Audit widths\n\n{}\n\n## Palette\n\nInternal. A role or a status names an entry; nothing else does.\n\n| entry | value |\n|---|---|\n",
        design.theme.class,
        design.theme.storage_key,
        design.viewports.iter().map(|w| format!("{w}px")).collect::<Vec<_>>().join(", ")
    ));
    for (name, value) in design.palette.iter() {
        out.push_str(&format!("| `{name}` | `{value}` |\n"));
    }
    out
}

fn code(value: &str) -> String {
    format!("`{}`", value.replace('`', "'"))
}

fn reference(r: &Resolved) -> String {
    if r.css == r.literal {
        format!("`{}`", r.reference)
    } else {
        format!("`{}` → `{}`", r.reference, r.literal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design() -> DesignSystem {
        DesignSystem::compiled().expect("compiled").clone()
    }

    #[test]
    fn a_declaration_with_no_value_or_an_unbalanced_quote_is_never_written() {
        assert!(well_formed(":root { --a: 1; }").is_ok());
        assert!(well_formed("--a: ;").is_err());
        assert!(well_formed("--a: \"x;").is_err());
    }

    #[test]
    fn every_stylesheet_is_well_formed_and_carries_the_fingerprint() {
        let d = design();
        for (name, css) in [
            ("theme", theme_css(&d)),
            ("surface", surface_css(&d)),
            ("status", status_css(&d)),
            ("tokens", tokens_css(&d)),
        ] {
            well_formed(&css).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let stamp = format!("--mj-design: \"{}\";", d.short_fingerprint());
        assert!(surface_css(&d).contains(&stamp));
        assert!(tokens_css(&d).contains(&stamp));
    }

    #[test]
    fn the_dark_block_is_a_class_after_the_root_block() {
        let css = surface_css(&design());
        let root = css.find(":root {").expect("root");
        let dark = css.find("\n  .dark {").expect("dark");
        assert!(dark > root);
        assert!(
            !css.contains(":where(.dark)"),
            "the arrangement that never won"
        );
    }

    #[test]
    fn the_theme_aliases_every_flowbite_name_to_a_role() {
        let d = design();
        let css = theme_css(&d);
        assert!(css.contains("--color-heading: var(--mj-fg);"));
        assert!(css.contains("--color-success-soft: var(--mj-ok-bg);"));
        assert!(css.contains(":root, .dark {"));
        assert!(css.contains("--text-meta: 10px;"));
        assert!(css.contains("--text-meta--line-height: 1.4;"));
    }

    #[test]
    fn the_status_sheet_styles_every_word_under_its_meaning() {
        let d = design();
        let css = status_css(&d);
        assert!(css.contains(".mj-badge--succeeded"));
        assert!(css.contains(".mj-status--stale"));
        assert!(css.contains("--mj-status-fg: var(--mj-ok);"));
        assert!(css.contains(".mj-swatch--accent { background: var(--mj-accent); }"));
    }

    #[test]
    fn the_inline_block_answers_dark_by_media_and_by_class() {
        let css = tokens_css(&design());
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains(":root:not(.light)"));
        assert!(css.contains(":root.dark {"));
        assert!(css.contains("--font-sans:"));
        assert!(css.contains("--mj-measure: 62rem;"));
    }

    #[test]
    fn the_site_dataset_files_every_word() {
        let d = design();
        let doc = site_document(&d, "0.0.0");
        assert_eq!(doc["schema"], SITE_SCHEMA);
        assert_eq!(doc["states"]["succeeded"], "ok");
        assert_eq!(doc["states"]["ok"], "ok");
        assert_eq!(doc["theme"]["storage_key"], "color-theme");
        assert!(doc["theme"]["bootstrap"]
            .as_str()
            .unwrap()
            .contains("color-theme"));
        assert_eq!(doc["viewports"][0], 320);
    }

    #[test]
    fn a_token_added_to_the_declaration_reaches_every_projection_unregistered() {
        // the zero-registration property: a new role and a new state word, added to the
        // declaration alone, appear in every stylesheet, the site dataset, the inventory
        // and `explain`, with no consumer edited
        let mut d = design();
        d.roles.0.push((
            "overlay".into(),
            super::super::Role {
                about: "a scrim behind a dialog".into(),
                light: "gray-900".into(),
                dark: "gray-50".into(),
            },
        ));
        d.status
            .states
            .0
            .iter_mut()
            .find(|(k, _)| k == "warn")
            .unwrap()
            .1
            .push("provisional".into());
        assert!(d.validate().is_empty(), "{:?}", d.validate());
        assert!(surface_css(&d).contains("--mj-overlay: oklch(21% 0.034 264.665);"));
        assert!(tokens_css(&d).contains("--mj-overlay:"));
        assert!(status_css(&d).contains(".mj-swatch--overlay"));
        assert!(status_css(&d).contains(".mj-badge--provisional"));
        assert_eq!(site_document(&d, "0")["states"]["provisional"], "warn");
        assert_eq!(
            d.explain("provisional").unwrap().role.as_deref(),
            Some("warn")
        );
        assert!(reference_markdown(&d).contains("`--mj-overlay`"));
        assert!(reference_markdown(&d).contains("`provisional`"));
        assert_ne!(d.fingerprint(), design().fingerprint());
    }
}

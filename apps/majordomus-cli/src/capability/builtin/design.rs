//! The `design` module: the design system as a capability.
//!
//! Four questions, answered from the declaration compiled into this executable — the
//! same one every stylesheet was projected from: what the design is (its fingerprint, its
//! identity, its theme contract, where it is projected), what tokens it holds, what one
//! token means, and whether the colours it pairs can be read on one another. The MCP tool,
//! the resource, the HTTP route, the OpenAPI operation and the Cockpit's Design page are
//! projections of these; none of them holds a token of its own.
//!
//! Three of the four read nothing at all: the design is the tool's, not the supervised
//! repository's, and it ships inside the binary. Whether the *served* stylesheets carry the
//! same fingerprint is a question for the page (`--mj-design` against
//! `design.system.design`), which the Cockpit asks on every load.
//!
//! `design.contrast` is the exception, and reads only the distribution's own share
//! directory: the pair a ratio is computed for is stated by the primitives that consume the
//! declaration, and those stylesheets ship beside `kinds.yaml` and the obligation
//! vocabulary. It is located the way every other reader locates it
//! ([`crate::share::Share`]), never discovered in the supervised repository.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::design::contrast::{self, ContrastReport};
use crate::design::{render, DesignSystem, Fonts, Identity, Token, TokenKind, SOURCE};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the design is read as an MCP resource.
pub const DESIGN_URI: &str = "majordomus://design";

/// One generated projection of the design and what it is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionView {
    /// Repository-relative path.
    pub path: String,
    /// What reads it.
    pub read_by: String,
}

/// The theme contract as a client reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ThemeView {
    /// The class on the root element that means dark.
    pub class: String,
    /// The `localStorage` key the person's choice is kept under.
    pub storage_key: String,
    /// The one pre-paint statement every surface runs.
    pub bootstrap: String,
}

/// How many of each kind of token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TokenTallies {
    /// Raw palette entries.
    pub palette: usize,
    /// Semantic roles.
    pub roles: usize,
    /// Status meanings.
    pub statuses: usize,
    /// State words filed under a status.
    pub states: usize,
    /// Steps of the type scale.
    pub type_steps: usize,
    /// Flowbite names declared as synonyms.
    pub aliases: usize,
}

/// The design system: what it is and where it goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DesignReport {
    /// SHA-256 of the declaration's canonical form.
    pub fingerprint: String,
    /// The first twelve digits, as every stylesheet carries them in `--mj-design`.
    pub design: String,
    /// The format version of the declaration.
    pub schema: u64,
    /// Where the declaration lives, repository-relative.
    pub source: String,
    /// Who this is.
    pub identity: Identity,
    /// The two type stacks.
    pub font: Fonts,
    /// The theme contract.
    pub theme: ThemeView,
    /// The widths a browser audit measures every page at.
    pub viewports: Vec<u32>,
    /// How many of each kind of token.
    pub tallies: TokenTallies,
    /// Every generated file the declaration is projected into.
    pub projections: Vec<ProjectionView>,
}

/// The input of `design.tokens`: every token, or one kind of them.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TokensInput {
    /// Only tokens of this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TokenKind>,
}

impl BenchmarkCases for TokensInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", TokensInput { kind: None }),
            NamedCase::new(
                "roles",
                TokensInput {
                    kind: Some(TokenKind::Role),
                },
            ),
        ]
    }
}

/// The tokens, listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TokenList {
    /// The fingerprint of the declaration they came from.
    pub fingerprint: String,
    /// How many are listed.
    pub total: usize,
    /// The tokens, in the inventory's order.
    pub tokens: Vec<Token>,
}

/// The input of `design.explain`: one token by name.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplainTokenInput {
    /// A role (`fg`), a status (`ok`), a state word (`succeeded`), a type step (`meta`), a
    /// palette entry (`gray-600`), or the custom property any of them becomes (`--mj-fg`).
    pub token: String,
}

impl BenchmarkCases for ExplainTokenInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("role", ExplainTokenInput { token: "fg".into() }),
            NamedCase::new(
                "state",
                ExplainTokenInput {
                    token: "succeeded".into(),
                },
            ),
        ]
    }
}

fn compiled() -> Result<&'static DesignSystem, CapabilityError> {
    DesignSystem::compiled().map_err(|reason| {
        CapabilityError::Internal(format!(
            "the design declaration compiled into this executable is invalid: {reason}"
        ))
    })
}

fn system(_: &Context, _: Empty) -> Result<DesignReport, CapabilityError> {
    let design = compiled()?;
    let states = design.status.states.iter().map(|(_, w)| w.len()).sum();
    let read_by = |path: &str| -> &'static str {
        match path {
            render::THEME_CSS | render::SURFACE_CSS | render::STATUS_CSS => {
                "both Tailwind builds: the site's and the Cockpit's"
            }
            render::TOKENS_CSS => "the pages the executable renders itself, and the Swagger shell",
            render::COMPILED_YAML => "this executable: the design capabilities and the Cockpit",
            render::COMPILED_MARK => "the Cockpit's shell",
            render::SITE_JSON => "the site's templates and the browser probes",
            _ => "a reader",
        }
    };
    Ok(DesignReport {
        fingerprint: design.fingerprint(),
        design: design.short_fingerprint(),
        schema: design.schema,
        source: SOURCE.into(),
        identity: design.identity.clone(),
        font: design.font.clone(),
        theme: ThemeView {
            class: design.theme.class.clone(),
            storage_key: design.theme.storage_key.clone(),
            bootstrap: design.theme_bootstrap(),
        },
        viewports: design.viewports.clone(),
        tallies: TokenTallies {
            palette: design.palette.len(),
            roles: design.roles.len(),
            statuses: design.status.roles.len(),
            states,
            type_steps: design.type_.scale.len(),
            aliases: design.alias.flowbite.len(),
        },
        projections: [
            render::THEME_CSS,
            render::SURFACE_CSS,
            render::STATUS_CSS,
            render::TOKENS_CSS,
            render::COMPILED_YAML,
            render::COMPILED_MARK,
            render::SITE_JSON,
            "docs/generated/design.json",
            "docs/generated/design.yaml",
            "docs/generated/design.md",
        ]
        .iter()
        .map(|p| ProjectionView {
            path: p.to_string(),
            read_by: read_by(p).to_string(),
        })
        .collect(),
    })
}

fn tokens(_: &Context, input: TokensInput) -> Result<TokenList, CapabilityError> {
    let design = compiled()?;
    let tokens: Vec<Token> = design
        .tokens()
        .into_iter()
        .filter(|t| input.kind.is_none_or(|k| t.kind == k))
        .collect();
    Ok(TokenList {
        fingerprint: design.fingerprint(),
        total: tokens.len(),
        tokens,
    })
}

fn explain(_: &Context, input: ExplainTokenInput) -> Result<Token, CapabilityError> {
    let design = compiled()?;
    if input.token.trim().is_empty() {
        return Err(CapabilityError::Refused(
            "name a token: a role, a status, a state word, a type step, or its custom property"
                .into(),
        ));
    }
    design.explain(&input.token).ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no token named '{}'; `design.tokens` lists every one",
            input.token
        ))
    })
}

/// The stylesheets that state which foreground is rendered on which ground, read from the
/// distribution's share directory. A distribution that ships none states no pair, which is
/// reported as a source that was not found rather than as a clean measurement.
fn consumers(root: &std::path::Path) -> Result<Vec<(String, String)>, CapabilityError> {
    let share = crate::share::Share::locate(None, root).map_err(|e| {
        CapabilityError::Internal(format!(
            "the distribution's share directory was not found: {e}"
        ))
    })?;
    let mut out = Vec::new();
    for name in contrast::CONSUMERS {
        let path = share.dir().join(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => out.push((format!("share/{name}"), text)),
            Err(e) => {
                return Err(CapabilityError::Internal(format!(
                    "{}: {e}; the design's own primitives are what state which colour is rendered on which ground",
                    path.display()
                )))
            }
        }
    }
    Ok(out)
}

fn readable(ctx: &Context, _: Empty) -> Result<ContrastReport, CapabilityError> {
    let design = compiled()?;
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    Ok(contrast::measure(design, &consumers(&root)?))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "design",
        title: "Design system",
        description: "The one declaration of how every surface of this tool looks — the semantic roles, the status vocabulary, the type scale, the theme contract — as this executable carries it: its fingerprint, its tokens, what any one of them means, and whether the colours it pairs are readable on one another. The stylesheets the site and the Cockpit load, the tokens the executable's own pages compile in, and the dataset the site's templates read are all projections of it; a page compares its `--mj-design` with this fingerprint to know whether it is wearing the design this executable was built with.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "design.system",
                title: "The design system",
                description: "What the design is: the fingerprint every stylesheet carries, the identity, the type stacks, the theme contract with the pre-paint statement every surface runs, the audit widths, how many tokens of each kind, and every generated file the declaration is projected into.",
                input: Empty,
                output: DesignReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_design".into()),
                        resource: Some(McpResource { uri: DESIGN_URI.into(), name: "design".into() }),
                    }),
                    http: get("/api/v1/design"),
                    cli: None,
                },
                tags: ["design", "ui", "introspection"],
                handler: system,
            },
            capability! {
                id: "design.tokens",
                title: "The design tokens",
                description: "Every token of the design, explained: roles with their light and dark values, statuses with their text, ground and border and the state words filed under them, the type scale, the layout values, the theme contract, the palette. Narrow it to one kind.",
                input: TokensInput,
                output: TokenList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_design_tokens"),
                    http: get("/api/v1/design/tokens"),
                    cli: None,
                },
                tags: ["design", "ui", "introspection"],
                handler: tokens,
            },
            capability! {
                id: "design.explain",
                title: "What a design token means",
                description: "One token by name or by the custom property it becomes: what it is for, what it resolves to in each theme, which Flowbite names are synonyms of it, which state words it colours, and which generated files it reaches.",
                input: ExplainTokenInput,
                output: Token,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_design_explain"),
                    http: get("/api/v1/design/explain"),
                    cli: None,
                },
                tags: ["design", "ui", "provenance", "introspection"],
                handler: explain,
            },
            capability! {
                id: "design.contrast",
                title: "Whether the declared colours can be read",
                description: "Every foreground the design puts on a ground, in both themes, measured against WCAG 2.1 AA: the pair, the palette entries behind it, the ratio and the threshold. The pairs are not a list — they are derived from the declaration, which files each status's text with its own ground, and from the primitives that consume it, where a rule that sets a colour and a background states a pair and a rule that sets only a colour states a foreground that lands on every ground a container sets. A pair below the threshold is a finding that names the role, the ground, the theme, the measured ratio and the required one.",
                input: Empty,
                output: ContrastReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_design_contrast"),
                    http: get("/api/v1/design/contrast"),
                    cli: None,
                },
                tags: ["design", "ui", "accessibility", "introspection"],
                handler: readable,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "design");
        let expected: &[(&str, &str, &str)] = &[
            ("design.system", "majordomus_design", "/api/v1/design"),
            (
                "design.tokens",
                "majordomus_design_tokens",
                "/api/v1/design/tokens",
            ),
            (
                "design.explain",
                "majordomus_design_explain",
                "/api/v1/design/explain",
            ),
            (
                "design.contrast",
                "majordomus_design_contrast",
                "/api/v1/design/contrast",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost its HTTP route"
            );
        }
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .expect("the design is readable as a resource");
        assert_eq!(resource.uri, DESIGN_URI);
    }

    #[test]
    fn the_report_carries_the_fingerprint_every_stylesheet_carries() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let report = system(&ctx, Empty::default()).expect("a report");
        let design = DesignSystem::compiled().unwrap();
        assert_eq!(report.fingerprint, design.fingerprint());
        assert_eq!(report.design, design.short_fingerprint());
        assert!(report
            .projections
            .iter()
            .any(|p| p.path == render::SURFACE_CSS));
        assert!(report.tallies.states > 0);
        assert_eq!(report.theme.storage_key, design.theme.storage_key);
    }

    #[test]
    fn the_declared_colours_are_measured_against_the_standard() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        // a synthetic repository carries no share directory of its own, so the answer is
        // either the measurement of this distribution's primitives or the sentence saying
        // which stylesheet was missing — never a clean verdict over nothing
        match readable(&ctx, Empty::default()) {
            Ok(report) => {
                assert!(report.measured > 0, "no pair was derived");
                assert_eq!(report.minimum_text, contrast::MINIMUM_TEXT);
                assert!(
                    report.readable,
                    "the declaration pairs colours that cannot be read:\n{}",
                    report.findings.join("\n")
                );
            }
            Err(CapabilityError::Internal(message)) => {
                assert!(message.contains("share"), "{message}");
            }
            Err(other) => panic!("unexpected refusal: {other:?}"),
        }
    }

    #[test]
    fn tokens_can_be_narrowed_to_a_kind() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let all = tokens(&ctx, TokensInput { kind: None }).unwrap();
        let roles = tokens(
            &ctx,
            TokensInput {
                kind: Some(TokenKind::Role),
            },
        )
        .unwrap();
        assert!(roles.total < all.total);
        assert!(roles.tokens.iter().all(|t| t.kind == TokenKind::Role));
        assert_eq!(roles.total, roles.tokens.len());
    }

    #[test]
    fn explaining_a_token_that_does_not_exist_says_so() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        match explain(
            &ctx,
            ExplainTokenInput {
                token: "nothing-like-this".into(),
            },
        ) {
            Err(CapabilityError::NotFound(message)) => {
                assert!(message.contains("nothing-like-this"))
            }
            other => panic!("{other:?}"),
        }
        match explain(&ctx, ExplainTokenInput { token: "  ".into() }) {
            Err(CapabilityError::Refused(_)) => {}
            other => panic!("{other:?}"),
        }
        let ok = explain(
            &ctx,
            ExplainTokenInput {
                token: "--mj-ok-bg".into(),
            },
        )
        .expect("a status colour is a token");
        assert_eq!(ok.kind, TokenKind::Status);
    }
}

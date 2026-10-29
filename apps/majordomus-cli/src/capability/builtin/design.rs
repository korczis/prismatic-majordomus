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
//! `design.kit` and `design.kit.adopt` are about the supervised repository rather than
//! this tool's own design (I4334, ADR 0230): the first judges the repository's user
//! interface against a kit profile the distribution ships (`share/design/kits/`), signal by
//! signal, and the second declares that profile in the repository — its rule, the
//! enforcement entries that run the check in CI and before every push, and `ui.kit` in the
//! policy's `ui:` block, which the design licence declares and both read. The judgement is
//! [`crate::design::kit`]'s; these declare it once.
//!
//! `design.contrast` is the exception, and reads only the distribution's own share
//! directory: the pair a ratio is computed for is stated by the primitives that consume the
//! declaration, and those stylesheets ship beside `kinds.yaml` and the obligation
//! vocabulary. It is located the way every other reader locates it
//! ([`crate::share::Share`]), never discovered in the supervised repository.
//!
//! `design.licence` is the one that reads the supervised repository, and the one file
//! beyond it the caller names: whether a public tree holds a licensed kit's bytes, judged
//! from kit-evidence/v1 fingerprints ([`crate::design::licence`]). It has no HTTP route: it
//! opens a file by the path its input gives, and that is a local caller's act.
//!
//! `design.kit` has none for the same reason. Its input `evidence` names a file the process
//! opens, and a query parameter that names a file on the server would hand any HTTP caller,
//! a remote one included, an oracle over the server's files. The registry binds one input
//! type to every transport and cannot withhold one field from one of them, so the judgement
//! is the command line's and the MCP tool's: the command line for CI and the pre-push hook,
//! MCP for an agent beside them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, CliExposure, Exposure, McpExposure, McpResource, Stability,
    WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::design::contrast::{self, ContrastReport};
use crate::design::kit::{self, AdoptError, AdoptReport, KitReport};
use crate::design::licence::{self, LicenceError, LicenceReport};
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

/// The input of `design.kit`: the licence evidence a licensed marker is judged by, or none
/// to read the policy's.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KitInput {
    /// A kit-evidence/v1 file, as the caller names it: in CI the file the launcher decoded
    /// `MAJORDOMUS_KIT_EVIDENCE_<KIT>` into, through the shell's `mj_fleet_design_judge`.
    /// Absent reads the policy's `ui.evidence` entries under `$HOME`. Taken on the command
    /// line and over MCP only: a file this process opens is never named over HTTP.
    /// A file whose first line `# kit: <id>` names another kit than the profile judges
    /// nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

impl BenchmarkCases for KitInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("policy", KitInput { evidence: None }),
            // a file of the caller's: in a tree that adopts no kit it is never opened
            NamedCase::new(
                "evidence",
                KitInput {
                    evidence: Some("kit-evidence.tsv".into()),
                },
            ),
        ]
    }
}

/// The input of `design.kit.adopt`: which kit profile to declare in the repository.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KitAdoptInput {
    /// A kit profile of the distribution, `share/design/kits/<profile>.yaml`.
    pub profile: String,
    /// Report what would be written, and write nothing.
    #[serde(default)]
    pub dry_run: bool,
}

impl BenchmarkCases for KitAdoptInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "dry-run",
            KitAdoptInput {
                profile: kit::DEFAULT_PROFILE.into(),
                dry_run: true,
            },
        )]
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

fn kit_share(ctx: &Context, root: &std::path::Path) -> Result<std::path::PathBuf, CapabilityError> {
    crate::share::Share::locate(ctx.index.share.as_deref(), root)
        .map(|s| s.dir().to_path_buf())
        .map_err(|e| {
            CapabilityError::Internal(format!(
                "the distribution's share directory, which holds the kit profiles, was not found: {e}"
            ))
        })
}

fn kit_check(ctx: &Context, input: KitInput) -> Result<KitReport, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let share = kit_share(ctx, &root)?;
    let evidence = input.evidence.as_deref().map(std::path::Path::new);
    kit::inspect(&root, &share, evidence).map_err(CapabilityError::Internal)
}

fn kit_adopt(ctx: &Context, input: KitAdoptInput) -> Result<AdoptReport, CapabilityError> {
    if input.profile.trim().is_empty() {
        return Err(CapabilityError::InvalidInput(
            "name a kit profile of share/design/kits/, such as flowbite-pro".into(),
        ));
    }
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let share = kit_share(ctx, &root)?;
    kit::adopt(&root, &share, input.profile.trim(), input.dry_run).map_err(|e| match e {
        AdoptError::NotFound(m) => CapabilityError::NotFound(m),
        AdoptError::Refused(m) => CapabilityError::Refused(m),
        AdoptError::Internal(m) => CapabilityError::Internal(m),
    })
}

fn readable(ctx: &Context, _: Empty) -> Result<ContrastReport, CapabilityError> {
    let design = compiled()?;
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    Ok(contrast::measure(design, &consumers(&root)?))
}

/// The input of `design.licence`: the evidence file, or none to read the policy's.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenceInput {
    /// A kit-evidence/v1 file, as the caller names it (CI points this at the file it decoded
    /// `MAJORDOMUS_KIT_EVIDENCE_<KIT>` into). Absent reads the policy's `ui.evidence`
    /// entries under `$HOME`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

impl BenchmarkCases for LicenceInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("policy", LicenceInput { evidence: None })]
    }
}

fn licensed(ctx: &Context, input: LicenceInput) -> Result<LicenceReport, CapabilityError> {
    let repository = ctx.repository().map_err(|e| {
        CapabilityError::Internal(format!("the repository could not be re-read: {e}"))
    })?;
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let tightened = std::env::var(licence::VISIBILITY_ENV).ok();
    licence::assess(
        &repository,
        input.evidence.as_deref(),
        home.as_deref(),
        tightened.as_deref(),
    )
    .map_err(|e| match e {
        LicenceError::Evidence(m) => CapabilityError::InvalidInput(m),
        LicenceError::Policy(m) => CapabilityError::Refused(m),
        LicenceError::Tree(m) => CapabilityError::Internal(m),
    })
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
            capability! {
                id: "design.licence",
                title: "Whether a public tree holds a licensed kit's files",
                description: "The never-publish gate: the raw bytes of every tracked file, committed bundles included (staged, committed where that differs, and the working copy where it differs), and of every untracked file under each `ui.publish` directory, read through symbolic links and into nested repositories as a deploy archives it, compared whole, exactly and with every run of whitespace made one space, against the kit-evidence/v1 fingerprints of the declared kit's licensed files. `refused` names each file and, where the evidence says, the licensed source; `not_judged` is a public tree that declares a kit, tracks a UI file and has no evidence here, and fails like `refused`; a `ui:` block that cannot be used as written (an absolute `ui.publish` or `ui.evidence` entry) is `not_judged` too, naming the entry; `inactive` is no kit declared, a private tree without evidence, or no UI file; `active` is judged and clean, or private. The evidence is the file the input names, else the policy's `~`-relative `ui.evidence` entries; an error names the file and the line, never its content. Near-copies and fragments inlined in a larger file are not judged: the evidence holds whole-file fingerprints only.",
                input: LicenceInput,
                output: LicenceReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_design_licence"),
                    http: None,
                    cli: Some(CliExposure { path: vec!["design".into(), "licence".into()] }),
                },
                tags: ["design", "ui", "licence", "governance"],
                handler: licensed,
            },
            capability! {
                id: "design.kit",
                title: "Whether the interface is built on the kit",
                description: "The supervised repository's user interface judged against the kit profile its policy's `ui.kit` names (flowbite-pro when it names none), from markers in the tree and never the kit's own source: S0 whether there is an interface at all and its surfaces (the root's manifest or the one in assets/, every workspace the root manifest or pnpm-workspace.yaml declares, every nested app manifest with markup in its own subtree outside examples, fixtures, tests, vendor, docs, deps and priv; Phoenix LiveView in mix.exs, HEEx or HTML templates, a Next.js root layout; published pages: a root index.html, pages under site/, www/, public/, under a directory a Pages workflow or a deploy or build script publishes, and under docs/ when it is published), any other manifest with UI packages being a notice and never a finding; S1 the kit's package in each surface, its React binding for React and Next.js, and the kit in use in the surface's own files (an attribute the kit's JavaScript drives, a flowbite-react component, a call of the kit's API), because wiring alone passes nothing; S2 the Tailwind plugin and source (Tailwind 4's @plugin and @source, Tailwind 3's config), a Play CDN or a CDN-only kit being a finding; S3 the kit's JavaScript behind every data-drawer, dropdown, modal, tabs, collapse and tooltip attribute, a script that reads them itself being a re-implementation; S4 no other kit (daisyUI, Bootstrap, Preline, TW Elements, shadcn/ui and Radix, Petal Components, Material UI, Ant Design, Bulma, Chakra UI, Foundation, Materialize), Alpine.js reported and not refused; S5 the shell on every root layout: a fixed top navbar, an aside labelled Sidebar that both drawer attributes name, a breadcrumb, and the page, surface and border roles, a published site's layout being judged by a navbar and a footer instead; the layouts, the roles and the marketing layouts are the profile's alone. Each finding names its signal, path, evidence and remedy; each exemption the policy declares under `ui.exempt`, exactly `{path, because}`, moves every finding at its path and names its reason and how many findings it covered. A licensed kit's profile declares its licensed markers (a distribution directory, a manifest named after a Pro product, a dependency on a Pro package), and where the tree carries one the design licence decides it: active passes, while inactive, refused or not judged is a finding no exemption moves, since a variant the judge cannot decide is never a pass. The licence evidence is the kit-evidence/v1 file the input names (in CI the file the launcher decoded: `majordomus design kit` run where a `MAJORDOMUS_KIT_EVIDENCE_<KIT>` variable is exported goes through the shell's `mj_fleet_design_judge`, which decodes the kit's variable into a private file, passes it and removes it; this capability reads a file and never the environment), else the policy's `~`-relative `ui.evidence` entries; a file whose first line `# kit: <id>` names another kit is not judged, and `licence.reason` says why the licence answered as it did, naming a file and a line at most and never what the evidence holds. A repository without an interface passes vacuously; a policy that cannot be read is `bad_policy` and judges nothing; a policy that still carries the retired top-level `design_kit:` block earns a notice naming `ui.kit` as its replacement. Reads the repository's files (in a Git work tree the tracked ones and the untracked ones Git does not ignore; a submodule or a nested repository is named in a notice and judged in its own repository), skipping dependencies and build output and listing vendored directories by path only; no build and no network. It has no HTTP route, as the design licence has none: the evidence input names a file this process opens, which is a local caller's act, so it is taken on the command line and over MCP only.",
                input: KitInput,
                output: KitReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_design_kit"),
                    // `evidence` names a file this process opens: a local caller's input,
                    // never a query parameter (as for design.licence)
                    http: None,
                    cli: Some(CliExposure { path: vec!["design".into(), "kit".into()] }),
                },
                tags: ["design", "ui", "governance"],
                handler: kit_check,
            },
            capability! {
                id: "design.kit.adopt",
                kind: CapabilityKind::Command,
                title: "Declare the kit in the repository",
                description: "Writes what makes `design.kit` the repository's gate, and nothing that is there already: the profile's rule (project.ui-uses-flowbite-pro for flowbite-pro) into .ai/repo/rules/project/ from the distribution's template, `ui.kit` naming the profile in .ai/repo/policy.yaml (set in the `ui:` block when it names no kit, the block appended when there is none; exemptions under `ui.exempt` are the repository's to declare), and two enforcement entries running `majordomus design kit --check`, design-kit-on-push wired by the pre-push hook and design-kit wired by the repository's CI workflow when it has one. Idempotent: a second adoption reports every file unchanged. The rule's last section, 'In this repository', is the repository's own: a later adoption brings the rest of the rule up to the template and never overwrites that section, and a rule that differs from the template and has no such section is kept and named, a `ui.kit` naming another profile is refused, and a policy that would not read back is never written. `dry_run` reports the same and writes nothing. Wiring the hook and the workflow step is left to the repository, and `majordomus doctor` names each entry until it is wired.",
                input: KitAdoptInput,
                output: AdoptReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["design".into(), "kit".into(), "adopt".into()] }),
                },
                tags: ["design", "ui", "governance"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: kit_adopt,
            }
            .writes_repository(),
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
        let expected: &[(&str, &str, Option<&str>)] = &[
            ("design.system", "majordomus_design", Some("/api/v1/design")),
            (
                "design.tokens",
                "majordomus_design_tokens",
                Some("/api/v1/design/tokens"),
            ),
            (
                "design.explain",
                "majordomus_design_explain",
                Some("/api/v1/design/explain"),
            ),
            (
                "design.contrast",
                "majordomus_design_contrast",
                Some("/api/v1/design/contrast"),
            ),
            // each opens the file its input names, so it is a local caller's: no route
            ("design.licence", "majordomus_design_licence", None),
            ("design.kit", "majordomus_design_kit", None),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let mut want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        want.push("design.kit.adopt");
        assert_eq!(ids, want);
        let adopt = &m.capabilities[6].capability;
        assert_eq!(adopt.kind, CapabilityKind::Command);
        assert_eq!(
            adopt.execution.effect,
            crate::capability::Effect::RepositoryMutation
        );
        assert!(
            adopt.exposure.mcp.is_none() && adopt.exposure.http.is_none(),
            "writing the repository is the command line's alone"
        );
        assert_eq!(
            adopt.exposure.cli.as_ref().unwrap().path,
            ["design", "kit", "adopt"]
        );
        assert_eq!(
            m.capabilities[5]
                .capability
                .exposure
                .cli
                .as_ref()
                .unwrap()
                .path,
            ["design", "kit"]
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                *path,
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

    /// `evidence` names a file this process opens, so no HTTP caller names one: the kit
    /// judge has no route, whatever the query, while the executor the command line and MCP
    /// call through still takes the input.
    #[test]
    fn the_kit_judge_takes_no_evidence_over_http() {
        use crate::capability::HttpMethod;
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        for method in [HttpMethod::Get, HttpMethod::Post] {
            assert!(
                ctx.registry.by_http(method, "/api/v1/design/kit").is_none(),
                "design.kit is routed for {}",
                method.as_str()
            );
        }
        let router = crate::http::Router::new(ctx.clone(), "test").with_ui(None);
        for target in [
            "/api/v1/design/kit",
            "/api/v1/design/kit?evidence=%2Fetc%2Fhosts",
        ] {
            let r = router.handle(&crate::http::Request::parse_target(
                "GET",
                target,
                Vec::new(),
            ));
            assert_eq!(r.status, 404, "{target}: {}", r.body.text());
        }
        // the command line and the MCP tool still take it
        let kit = ctx
            .registry
            .get("design.kit")
            .expect("design.kit is declared");
        assert!(kit.exposure.mcp.is_some() && kit.exposure.cli.is_some());
        let (properties, _) = kit.input.properties();
        assert!(
            properties.iter().any(|(name, _)| name == "evidence"),
            "the input lost its evidence"
        );
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
    fn the_licence_gate_is_reached_as_design_licence_and_skips_a_tree_without_a_kit() {
        let m = module();
        let gate = m
            .capabilities
            .iter()
            .map(|e| &e.capability)
            .find(|c| c.id.as_str() == "design.licence")
            .expect("the licence gate is declared");
        assert_eq!(
            gate.exposure.cli.as_ref().unwrap().path,
            ["design", "licence"]
        );
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        // the synthetic policy has no ui: block, which declares no kit
        let report = licensed(&ctx, LicenceInput::default()).expect("an answer");
        assert_eq!(report.verdict, licence::Verdict::Inactive);
        assert!(!report.fails);
        assert!(report.kit.is_empty());
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

    /// A context over `repo` whose index carries the distribution's share, as a real
    /// invocation's does, so that the kit's capabilities judge rather than refuse.
    fn context_with_share(repo: &crate::synthetic::SyntheticRepository) -> std::sync::Arc<Context> {
        let mut index = repo.index().expect("an index");
        index.share = Some(crate::synthetic::crate_share());
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .with_index(&index)
            .build()
            .expect("a registry");
        std::sync::Arc::new(Context::new(
            std::sync::Arc::new(index),
            std::sync::Arc::new(registry),
        ))
    }

    #[test]
    fn the_kit_is_judged_in_the_repository_the_context_reads() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = context_with_share(&repo);
        // a synthetic repository has no interface: the rule holds vacuously
        let report = kit_check(&ctx, KitInput::default()).expect("a judgement");
        assert_eq!(report.verdict, kit::KitVerdict::NoUi, "{report:#?}");
        assert!(!report.ui);
        assert!(report.passes);
        assert_eq!(report.profile, kit::DEFAULT_PROFILE);
        let refused = kit_adopt(
            &ctx,
            KitAdoptInput {
                profile: " ".into(),
                dry_run: true,
            },
        );
        assert!(matches!(refused, Err(CapabilityError::InvalidInput(_))));
        let cases = KitAdoptInput::benchmark_cases(&CaseContext { index: &ctx.index });
        assert!(
            cases.iter().all(|c| c.input.dry_run),
            "a benchmark never writes"
        );
    }

    #[test]
    fn adoption_maps_each_refusal_to_its_category() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = context_with_share(&repo);
        let unknown = kit_adopt(
            &ctx,
            KitAdoptInput {
                profile: "no-such-kit".into(),
                dry_run: true,
            },
        );
        assert!(
            matches!(&unknown, Err(CapabilityError::NotFound(m)) if m.contains("no-such-kit")),
            "{unknown:?}"
        );
        // the synthetic layer carries .ai/repo/policy.yaml, so a dry run plans and writes nothing
        let policy = repo.root().join(kit::POLICY);
        let before = std::fs::read_to_string(&policy).expect("the synthetic policy");
        let report = kit_adopt(
            &ctx,
            KitAdoptInput {
                profile: kit::DEFAULT_PROFILE.into(),
                dry_run: true,
            },
        )
        .expect("a dry run adoption");
        assert!(report.dry_run);
        assert_eq!(report.profile, kit::DEFAULT_PROFILE);
        assert!(!report.writes.is_empty());
        assert!(!repo
            .root()
            .join(".ai/repo/rules/project/ui-uses-flowbite-pro.v1.md")
            .exists());
        assert_eq!(std::fs::read_to_string(&policy).unwrap(), before);
        // a policy that does not read is the repository's to fix: refused
        std::fs::write(&policy, "a:\n\tb: 1\n").unwrap();
        let broken = kit_adopt(
            &ctx,
            KitAdoptInput {
                profile: kit::DEFAULT_PROFILE.into(),
                dry_run: true,
            },
        );
        assert!(
            matches!(broken, Err(CapabilityError::Refused(_))),
            "{broken:?}"
        );
    }
}

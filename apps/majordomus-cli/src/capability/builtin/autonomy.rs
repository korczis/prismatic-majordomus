//! The `autonomy` module: how much a coding-agent session in this checkout may do unasked,
//! read from the facts rather than assumed (I2790). The judgement is [`crate::autonomy`]'s;
//! this declares it once, and the command line, MCP, HTTP and the benchmark follow.
//!
//! Every capability here reads. None writes a setting: granting autonomy is the person's
//! act, and the profile exists so that a session does not grant its own.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::autonomy::{self, AutonomyProfile, AutonomyStatus, CommandDecision, Facts, Friction};
use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The longest window `autonomy.friction` reads, in days.
const MAX_DAYS: u32 = 90;

/// Which command to judge, and under which profile.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AutonomyExplainInput {
    /// The shell command, as a session would run it.
    pub(crate) command: String,
    /// The profile to judge it under; the checkout's own, as `autonomy.status` derives it,
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) profile: Option<AutonomyProfile>,
}

impl BenchmarkCases for AutonomyExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "routine",
                AutonomyExplainInput {
                    command: "cargo test --lib autonomy".into(),
                    profile: Some(AutonomyProfile::Trusted),
                },
            ),
            NamedCase::new(
                "boundary",
                AutonomyExplainInput {
                    command: "git push --force origin master".into(),
                    profile: Some(AutonomyProfile::Isolated),
                },
            ),
        ]
    }
}

/// The window to count.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AutonomyFrictionInput {
    /// How many days back to count, 1 to 90; 7 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) since_days: Option<u32>,
}

impl BenchmarkCases for AutonomyFrictionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "one-day",
            AutonomyFrictionInput {
                since_days: Some(1),
            },
        )]
    }
}

fn root(ctx: &Context) -> std::path::PathBuf {
    std::path::PathBuf::from(&ctx.index.repository.root)
}

fn autonomy_status(ctx: &Context, _: Empty) -> Result<AutonomyStatus, CapabilityError> {
    Ok(autonomy::status(&Facts::observe(&root(ctx))))
}

fn autonomy_explain(
    ctx: &Context,
    input: AutonomyExplainInput,
) -> Result<CommandDecision, CapabilityError> {
    let profile = match input.profile {
        Some(p) => p,
        None => autonomy::status(&Facts::observe(&root(ctx))).profile,
    };
    Ok(autonomy::explain(&input.command, profile))
}

fn autonomy_friction(
    ctx: &Context,
    input: AutonomyFrictionInput,
) -> Result<Friction, CapabilityError> {
    let days = input.since_days.unwrap_or(7);
    if days == 0 || days > MAX_DAYS {
        return Err(CapabilityError::InvalidInput(format!(
            "since_days must be 1 to {MAX_DAYS}, not {days}"
        )));
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let projects = home
        .as_ref()
        .map(|h| h.join(".claude/projects"))
        .unwrap_or_default();
    Ok(autonomy::friction(
        &projects,
        &root(ctx),
        autonomy::days_ago(days),
        home.as_deref(),
    ))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "autonomy",
        title: "Autonomy",
        description: "How much a coding-agent session in this checkout may do without a person's approval, read from the facts: the client's settings layers, the evidence for hard isolation, the person's own trust in the checkout, and what the permission layer actually cost. Fails closed; reads only; never grants itself anything.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "autonomy.status",
                title: "The autonomy this checkout supports",
                description: "The Claude Code settings layers (managed, local, project, user: presence, default mode, rule counts, hooks, never rule text), the isolation evidence (container markers, the client's sandbox, credential variables by name and credential files), the person's trust-dialog acceptance of the checkout, and the profile those facts support — conservative, balanced, trusted or isolated — with its recommended permission mode, the reason for it and the fact that refused every stronger one. isolated (bypassPermissions) needs a container marker and no credential in reach; trusted (auto) needs the person's acceptance, which the repository cannot check in; an unreadable layer or a managed default mode is a ceiling.",
                input: Empty,
                output: AutonomyStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_autonomy"),
                    http: get("/api/v1/autonomy"),
                    cli: Some(CliExposure { path: vec!["autonomy".into(), "status".into()] }),
                },
                tags: ["autonomy", "permissions", "security"],
                handler: autonomy_status,
            },
            capability! {
                id: "autonomy.explain",
                title: "Judge one shell command",
                description: "What a static permission rule may do with one shell command under a profile: allow (it runs unasked), defer (the client's own classifier or prompt decides) or require_approval (a person approves it under every profile), with its risk class, the reason and the table entry that matched. A compound command — chaining, a pipe, a redirection, a substitution, a leading assignment, find -exec — never earns an allow; history rewrites, destructive removal, privilege and external effects require approval even inside isolation.",
                input: AutonomyExplainInput,
                output: CommandDecision,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_autonomy_explain"),
                    http: get("/api/v1/autonomy/explain"),
                    cli: Some(CliExposure { path: vec!["autonomy".into(), "explain".into()] }),
                },
                tags: ["autonomy", "permissions", "security"],
                handler: autonomy_explain,
            },
            capability! {
                id: "autonomy.friction",
                title: "What the permission layer cost",
                description: "Counted from Claude Code's own transcripts of this repository's checkouts (the primary and every <repository>-wt/ worktree) over the last since_days days: tool calls, and every refusal by kind (classifier_denial, hook_block, safety_check, user_rejection), by the classifier's stated reason and by tool, with the rate per thousand calls. Counts only: no command, path or payload leaves a transcript. A prompt the person approved leaves no trace and is not counted.",
                input: AutonomyFrictionInput,
                output: Friction,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_autonomy_friction"),
                    http: get("/api/v1/autonomy/friction"),
                    cli: Some(CliExposure { path: vec!["autonomy".into(), "friction".into()] }),
                },
                tags: ["autonomy", "permissions", "measurement"],
                handler: autonomy_friction,
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
        assert_eq!(m.id.as_str(), "autonomy");
        let got: Vec<(&str, Option<&str>, Option<&str>)> = m
            .capabilities
            .iter()
            .map(|e| {
                let x = &e.capability.exposure;
                (
                    e.capability.id.as_str(),
                    x.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                    x.http.as_ref().map(|h| h.path.as_str()),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    "autonomy.status",
                    Some("majordomus_autonomy"),
                    Some("/api/v1/autonomy")
                ),
                (
                    "autonomy.explain",
                    Some("majordomus_autonomy_explain"),
                    Some("/api/v1/autonomy/explain")
                ),
                (
                    "autonomy.friction",
                    Some("majordomus_autonomy_friction"),
                    Some("/api/v1/autonomy/friction")
                ),
            ]
        );
    }

    fn context() -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![module()])
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    #[test]
    fn every_capability_answers_through_the_registry() {
        let (_repo, ctx) = context();
        let s: AutonomyStatus = serde_json::from_value(
            ctx.execute("autonomy.status", serde_json::json!({}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(s.schema, autonomy::STATUS_SCHEMA);
        assert_eq!(s.recommended_mode, s.profile.mode());

        let d: CommandDecision = serde_json::from_value(
            ctx.execute(
                "autonomy.explain",
                serde_json::json!({ "command": "git reset --hard" }),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(d.decision, autonomy::Decision::RequireApproval);
        assert_eq!(
            d.profile, s.profile,
            "absent, the checkout's own profile judges"
        );

        let f: Friction = serde_json::from_value(
            ctx.execute("autonomy.friction", serde_json::json!({ "since_days": 1 }))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(f.schema, autonomy::FRICTION_SCHEMA);
    }

    #[test]
    fn a_window_outside_its_bounds_is_refused() {
        let (_repo, ctx) = context();
        for days in [0, MAX_DAYS + 1] {
            let e = ctx
                .execute(
                    "autonomy.friction",
                    serde_json::json!({ "since_days": days }),
                )
                .unwrap_err();
            assert!(e.to_string().contains("since_days must be"), "{e}");
        }
    }

    #[test]
    fn every_benchmark_case_is_an_input_the_capability_answers() {
        let (_repo, ctx) = context();
        let c = CaseContext { index: &ctx.index };
        for case in AutonomyExplainInput::benchmark_cases(&c) {
            let input = serde_json::to_value(&case.input).unwrap();
            assert!(
                ctx.execute("autonomy.explain", input).is_ok(),
                "{}",
                case.name
            );
        }
        for case in AutonomyFrictionInput::benchmark_cases(&c) {
            let input = serde_json::to_value(&case.input).unwrap();
            assert!(
                ctx.execute("autonomy.friction", input).is_ok(),
                "{}",
                case.name
            );
        }
    }
}

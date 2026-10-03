//! The handlers and inputs of the cross-machine half of the `continuity` module: status,
//! records, plan, publish, sync and resume. The declarations are in
//! [`super::continuity::module`], beside `continuity.state`, so that one module answers
//! both "what is this checkout holding" and "what can it continue from elsewhere"; the
//! domain is [`crate::continuity`], and every handler here is a call into it.
//!
//! Like `continuity.state`, these answers are served, never published: they name this
//! device and this checkout's local state, so they have no projection into `docs/generated`
//! content or the site.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::continuity::{self, Machine};

fn root(ctx: &Context) -> PathBuf {
    PathBuf::from(&ctx.index.repository.root)
}

fn mesh(ctx: &Context) -> Option<crate::mesh::config::MeshConfig> {
    super::mesh::declaration(ctx).and_then(Result::ok)
}

fn reading<'a>(ctx: &Context, root: &'a Path) -> Result<Machine<'a>, CapabilityError> {
    Machine::open_read(root, mesh(ctx)).map_err(CapabilityError::Refused)
}

fn writing<'a>(ctx: &Context, root: &'a Path) -> Result<Machine<'a>, CapabilityError> {
    Machine::open(root, mesh(ctx)).map_err(CapabilityError::Refused)
}

/// `continuity.status`.
pub fn status(ctx: &Context, _: super::Empty) -> Result<continuity::Status, CapabilityError> {
    let root = root(ctx);
    continuity::status(&reading(ctx, &root)?).map_err(CapabilityError::Internal)
}

/// `continuity.records`.
pub fn records(ctx: &Context, _: super::Empty) -> Result<continuity::Records, CapabilityError> {
    let root = root(ctx);
    continuity::records(&reading(ctx, &root)?).map_err(CapabilityError::Internal)
}

/// The input of `continuity.plan` and `continuity.resume`: which record, by id or a prefix
/// of one. Absent, the one handover another device published that this checkout has not
/// resumed — on this branch when there is one there.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity_transfer::RecordInput;
/// let i: RecordInput = serde_json::from_str(r#"{"record":"a1b2c3"}"#).unwrap();
/// assert_eq!(i.record.as_deref(), Some("a1b2c3"));
/// assert!(serde_json::from_str::<RecordInput>(r#"{"force":true}"#).is_err());
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordInput {
    /// A record id, or a unique prefix of one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
}

impl BenchmarkCases for RecordInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", RecordInput { record: None }),
            NamedCase::new(
                "by-prefix",
                RecordInput {
                    record: Some("0000".into()),
                },
            ),
        ]
    }
}

/// `continuity.plan`.
pub fn plan(ctx: &Context, input: RecordInput) -> Result<continuity::ResumePlan, CapabilityError> {
    let root = root(ctx);
    continuity::plan(&reading(ctx, &root)?, input.record.as_deref())
        .map_err(CapabilityError::Internal)
}

/// `continuity.resume`.
pub fn resume(ctx: &Context, input: RecordInput) -> Result<continuity::Resumed, CapabilityError> {
    let root = root(ctx);
    continuity::resume(&reading(ctx, &root)?, input.record.as_deref())
        .map_err(CapabilityError::Refused)
}

/// The input of `continuity.publish`.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity_transfer::PublishInput;
/// let i: PublishInput = serde_json::from_str(r##"{"issue":"#184"}"##).unwrap();
/// assert_eq!(i.issue.as_deref(), Some("#184"));
/// assert!(i.handover.is_none(), "the newest handover by default");
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishInput {
    /// The file name of a handover record under `.ai/local/state/handovers/`; the newest
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover: Option<String>,
    /// The issue the work belongs to (`#184`, `I0042`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
}

impl BenchmarkCases for PublishInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", PublishInput::default())]
    }
}

/// `continuity.publish`.
pub fn publish(
    ctx: &Context,
    input: PublishInput,
) -> Result<continuity::Published, CapabilityError> {
    let root = root(ctx);
    continuity::locate_handover(&root, input.handover.as_deref())
        .map_err(CapabilityError::Refused)?;
    continuity::publish(
        &writing(ctx, &root)?,
        &continuity::PublishRequest {
            handover: input.handover,
            issue: input.issue,
            milestone: input.milestone,
        },
    )
    .map_err(CapabilityError::Refused)
}

/// The input of `continuity.sync`.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity_transfer::SyncInput;
/// let i: SyncInput = serde_json::from_str(r#"{"remote":"origin"}"#).unwrap();
/// assert_eq!(i.remote.as_deref(), Some("origin"));
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SyncInput {
    /// The git remote; the current branch's, else `origin`, when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

impl BenchmarkCases for SyncInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", SyncInput::default())]
    }
}

/// `continuity.sync`.
pub fn sync(ctx: &Context, input: SyncInput) -> Result<continuity::Synced, CapabilityError> {
    let root = root(ctx);
    continuity::sync(&reading(ctx, &root)?, input.remote.as_deref())
        .map_err(CapabilityError::Refused)
}

/// The input of `continuity.device`.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity_transfer::DeviceInput;
/// let i: DeviceInput = serde_json::from_str(r#"{"label":"mac-mini"}"#).unwrap();
/// assert_eq!(i.label.as_deref(), Some("mac-mini"));
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceInput {
    /// A label to give this device (`macbook-pro`, `mac-mini`): letters, digits, `.`, `_`,
    /// `-`. The key, and so the identity, is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl BenchmarkCases for DeviceInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", DeviceInput::default())]
    }
}

/// `continuity.device`.
pub fn device(_: &Context, input: DeviceInput) -> Result<continuity::DeviceView, CapabilityError> {
    continuity::device(input.label.as_deref()).map_err(CapabilityError::Refused)
}

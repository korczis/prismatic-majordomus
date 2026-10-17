//! The server's half of browser compute (ADR 0170): a browser computes with the portable core
//! (`majordomus-core`, as wasm32) and returns a claim, and this module recomputes it with the
//! same core and says whether the two agree.
//!
//! A verification is a read. It records nothing — no execution, no ledger entry, no journal
//! event — so ADR 0126 admits it from any caller, and nothing a browser returns becomes part
//! of the repository through it. Agreement among browsers is never proof; only this
//! recomputation is. What the browser computed on travels with the claim, so the server
//! never has to trust where it came from.
//!
//! A claim's whole life, as the server sees it: a browser digests a document, the server
//! recomputes the digest from the same text, and a claim that differs by one digit is wrong.
//!
//! ```
//! use majordomus_cli::capability::builtin::compute::{verify_claim, ComputeFunction, ComputeVerdict};
//! let document = r#"{"task":"t-1","inputs":[3,1,2]}"#;
//! // what the browser's wasm32 module returns: the same core, the same answer
//! let claimed = majordomus_core::digest_of_json_text(document).unwrap();
//! let answer = verify_claim(ComputeFunction::CanonicalDigest, document, &claimed);
//! assert_eq!(answer.verdict, ComputeVerdict::Match);
//! let forged = format!("{}0", &claimed[..63]);
//! let forged = if forged == claimed { format!("{}1", &claimed[..63]) } else { forged };
//! let answer = verify_claim(ComputeFunction::CanonicalDigest, document, &forged);
//! assert_eq!(answer.verdict, ComputeVerdict::Mismatch);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{mcp, post};

/// The largest input a claim may carry, in bytes. A browser task is bounded; a larger one is
/// answered `invalid` with this limit named, never read.
pub const MAX_INPUT_BYTES: usize = 1 << 20;

/// A function of the portable core whose result a browser may claim.
///
/// ```
/// use majordomus_cli::capability::builtin::compute::ComputeFunction;
/// let f: ComputeFunction = serde_json::from_str(r#""canonical_digest""#).unwrap();
/// assert_eq!(f, ComputeFunction::CanonicalDigest);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComputeFunction {
    /// SHA-256, as lowercase hex, of a JSON document's canonical bytes: the digest the mesh
    /// signs and the executor keys its cache by.
    #[default]
    CanonicalDigest,
}

/// A browser's claim: what it computed, on what, and what it got.
///
/// ```
/// use majordomus_cli::capability::builtin::compute::{ComputeFunction, ComputeVerifyInput};
/// let claim: ComputeVerifyInput =
///     serde_json::from_str(r#"{"input":"[1]","claimed":"00"}"#).unwrap();
/// assert_eq!(claim.function, ComputeFunction::CanonicalDigest, "the function defaults");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComputeVerifyInput {
    /// The function the claim was computed with.
    #[serde(default)]
    pub function: ComputeFunction,
    /// The input the browser computed on, as JSON text, at most [`MAX_INPUT_BYTES`].
    pub input: String,
    /// The result the browser claims: for `canonical_digest`, 64 lowercase hex characters.
    pub claimed: String,
}

impl BenchmarkCases for ComputeVerifyInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let input = r#"{"b":[1,2,{"d":true}],"a":"x"}"#.to_string();
        let claimed = majordomus_core::digest_of_json_text(&input).unwrap_or_default();
        vec![
            NamedCase::new(
                "match",
                ComputeVerifyInput {
                    function: ComputeFunction::CanonicalDigest,
                    input: input.clone(),
                    claimed,
                },
            ),
            NamedCase::new(
                "mismatch",
                ComputeVerifyInput {
                    function: ComputeFunction::CanonicalDigest,
                    input,
                    claimed: "0".repeat(64),
                },
            ),
        ]
    }
}

/// What the server's recomputation says of a claim.
///
/// ```
/// use majordomus_cli::capability::builtin::compute::ComputeVerdict;
/// assert_eq!(serde_json::to_string(&ComputeVerdict::Mismatch).unwrap(), r#""mismatch""#);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComputeVerdict {
    /// The server computed the same result from the same input.
    Match,
    /// The server computed a different result: the claim is wrong, whoever made it.
    Mismatch,
    /// The claim could not be judged: its input is not what the function takes, or its
    /// result is not the shape the function returns. `reason` says which.
    Invalid,
}

/// The server's answer to a claim.
///
/// ```
/// use majordomus_cli::capability::builtin::compute::{
///     verify_claim, ComputeFunction, ComputeVerification,
/// };
/// let answer: ComputeVerification = verify_claim(ComputeFunction::CanonicalDigest, "[1]", "00");
/// assert_eq!(answer.reason.as_deref(), Some("the claimed result is not 64 lowercase hex characters"));
/// assert_eq!(answer.input_bytes, 3);
/// assert!(answer.recomputed.is_some(), "the input was read, so the server says what it computed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComputeVerification {
    /// The function the claim named.
    pub function: ComputeFunction,
    /// Whether the server's recomputation agrees.
    pub verdict: ComputeVerdict,
    /// The result the claim carried, as given.
    pub claimed: String,
    /// What the server computed, when the input could be read.
    pub recomputed: Option<String>,
    /// Why the claim could not be judged, when the verdict is `invalid`.
    pub reason: Option<String>,
    /// How many bytes of input the claim carried.
    pub input_bytes: usize,
}

/// Recompute a claim with the portable core and judge it. It never fails: a claim that
/// cannot be judged is answered `invalid` with its reason.
///
/// ```
/// use majordomus_cli::capability::builtin::compute::{verify_claim, ComputeFunction, ComputeVerdict};
/// let digest = majordomus_core::digest_of_json_text(r#"{"a":1}"#).unwrap();
/// let same = verify_claim(ComputeFunction::CanonicalDigest, r#"{ "a" : 1 }"#, &digest);
/// assert_eq!(same.verdict, ComputeVerdict::Match);
/// let other = verify_claim(ComputeFunction::CanonicalDigest, r#"{"a":2}"#, &digest);
/// assert_eq!(other.verdict, ComputeVerdict::Mismatch);
/// let broken = verify_claim(ComputeFunction::CanonicalDigest, "{not json", &digest);
/// assert_eq!(broken.verdict, ComputeVerdict::Invalid);
/// ```
pub fn verify_claim(function: ComputeFunction, input: &str, claimed: &str) -> ComputeVerification {
    let answer = |verdict, recomputed, reason| ComputeVerification {
        function,
        verdict,
        claimed: claimed.to_string(),
        recomputed,
        reason,
        input_bytes: input.len(),
    };
    if input.len() > MAX_INPUT_BYTES {
        return answer(
            ComputeVerdict::Invalid,
            None,
            Some(format!(
                "the input is {} bytes; a claim carries at most {MAX_INPUT_BYTES}",
                input.len()
            )),
        );
    }
    let recomputed = match function {
        ComputeFunction::CanonicalDigest => majordomus_core::digest_of_json_text(input),
    };
    let recomputed = match recomputed {
        Ok(digest) => digest,
        Err(e) => {
            return answer(
                ComputeVerdict::Invalid,
                None,
                Some(format!("the input is not JSON: {e}")),
            )
        }
    };
    let shaped = claimed.len() == 64
        && claimed
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !shaped {
        return answer(
            ComputeVerdict::Invalid,
            Some(recomputed),
            Some("the claimed result is not 64 lowercase hex characters".into()),
        );
    }
    let verdict = if recomputed == claimed {
        ComputeVerdict::Match
    } else {
        ComputeVerdict::Mismatch
    };
    answer(verdict, Some(recomputed), None)
}

fn verify(_: &Context, input: ComputeVerifyInput) -> Result<ComputeVerification, CapabilityError> {
    Ok(verify_claim(input.function, &input.input, &input.claimed))
}

/// The module.
pub(crate) fn module() -> ModuleDescriptor {
    module! {
        id: "compute",
        title: "Browser compute",
        description: "The server's half of browser compute (ADR 0170): a browser computes with the portable core, the same code this executable links, and returns a claim; the server recomputes it on the same input and says whether they agree. A verification records nothing, so any caller may ask, and agreement among browsers is never proof.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "compute.verify",
                title: "Whether a browser's result is what the server computes",
                description: "Recompute a claimed result of a portable-core function (today `canonical_digest`: SHA-256 of a JSON document's canonical bytes) on the input the claim carries, with the same core the browser ran as wasm32, and answer `match`, `mismatch`, or `invalid` with the reason when the input is not JSON, exceeds 1 MiB or the result is not the function's shape. It records nothing: no execution, ledger entry or journal event, and the claim never enters a record.",
                input: ComputeVerifyInput,
                output: ComputeVerification,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_compute_verify"),
                    http: post("/api/v1/compute/verify"),
                    cli: None,
                },
                tags: ["compute", "browser", "verification"],
                handler: verify,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_is_one_read_capability_over_mcp_and_http() {
        let m = module();
        assert_eq!(m.id.as_str(), "compute");
        assert_eq!(m.capabilities.len(), 1);
        let c = &m.capabilities[0];
        assert_eq!(c.capability.id.as_str(), "compute.verify");
        assert_eq!(
            c.capability.execution.effect,
            crate::capability::Effect::Read
        );
    }

    #[test]
    fn a_claim_is_judged_by_recomputing_it() {
        let input = r#"{"z":1,"a":[true,null]}"#;
        let digest = majordomus_core::digest_of_json_text(input).expect("JSON");
        let f = ComputeFunction::CanonicalDigest;

        let same = verify_claim(f, r#"{ "a": [true, null], "z": 1 }"#, &digest);
        assert_eq!(same.verdict, ComputeVerdict::Match);
        assert_eq!(same.recomputed.as_deref(), Some(digest.as_str()));
        assert_eq!(same.reason, None);

        let wrong = verify_claim(f, input, &"f".repeat(64));
        assert_eq!(wrong.verdict, ComputeVerdict::Mismatch);
        assert_eq!(wrong.recomputed.as_deref(), Some(digest.as_str()));

        let upper = verify_claim(f, input, &digest.to_uppercase());
        assert_eq!(upper.verdict, ComputeVerdict::Invalid);
        assert!(upper
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("64 lowercase hex")));

        let broken = verify_claim(f, "{nope", &digest);
        assert_eq!(broken.verdict, ComputeVerdict::Invalid);
        assert_eq!(broken.recomputed, None);
        assert!(broken
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("not JSON")));
    }

    #[test]
    fn an_input_past_the_bound_is_refused_unread() {
        let big = format!("\"{}\"", "a".repeat(MAX_INPUT_BYTES));
        let v = verify_claim(ComputeFunction::CanonicalDigest, &big, &"0".repeat(64));
        assert_eq!(v.verdict, ComputeVerdict::Invalid);
        assert_eq!(v.recomputed, None);
        assert_eq!(v.input_bytes, big.len());
        assert!(v.reason.as_deref().is_some_and(|r| r.contains("at most")));
    }

    #[test]
    fn every_benchmark_case_is_judged_as_named() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let cases = ComputeVerifyInput::benchmark_cases(&CaseContext::of(&ctx));
        let verdicts: Vec<_> = cases
            .iter()
            .map(|c| {
                (
                    c.name,
                    verify(&ctx, c.input.clone()).expect("an answer").verdict,
                )
            })
            .collect();
        assert_eq!(
            verdicts,
            vec![
                ("match", ComputeVerdict::Match),
                ("mismatch", ComputeVerdict::Mismatch)
            ]
        );
    }
}

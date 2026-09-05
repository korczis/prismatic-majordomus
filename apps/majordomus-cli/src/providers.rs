//! The provider projections: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` and whatever else the
//! policy's `projections[]` declares, rendered from the provider templates and the
//! canonical policy. They are one-way: the policy and the templates are the sources, the
//! targets are caches that `generate --check` compares and `generate` rewrites.
//!
//! The rendering is byte-identical to the shell tool's `majordomus update`, so both agree
//! on every stamp: a file-mode target starts with a stamp naming the policy hash and the
//! hash of the content below it; a region-mode target carries both in its begin marker and
//! the rest of the host document is left alone.

use std::path::Path;

use crate::error::{Error, Result};
use crate::generate::Artifact;
use crate::policy::{
    is_safe_relative, repository_providers_dir, sha256_hex, LoadedPolicy, ProjectionMode,
};
use crate::repository::Repository;
use crate::share::Share;

/// The region markers of a region-mode target.
pub const REGION_BEGIN: &str = "<!-- majordomus:begin";
/// The closing marker of a region-mode target.
pub const REGION_END: &str = "<!-- majordomus:end -->";

/// The template of a provider: the repository's own under `.ai/repo/providers/`, else the
/// distribution's under `share/providers/`.
pub fn template_path(
    repository: &Repository,
    share: &Share,
    provider: &str,
) -> Option<std::path::PathBuf> {
    let name = format!("{provider}.tmpl");
    let own = repository_providers_dir(repository).join(&name);
    if own.is_file() {
        return Some(own);
    }
    let shipped = share.dir().join("providers").join(&name);
    shipped.is_file().then_some(shipped)
}

/// Every provider projection the policy declares, rendered.
///
/// A target outside the repository, a provider without a template, a token the policy
/// cannot fill, and an `always_loaded` target over the budget are errors: a projection
/// that cannot be produced is a defect of the declaration, not something to skip.
pub fn artifacts(
    repository: &Repository,
    share: &Share,
    policy: &LoadedPolicy,
) -> Result<Vec<Artifact>> {
    let mut out = Vec::with_capacity(policy.policy.projections.len());
    for projection in &policy.policy.projections {
        let target = &projection.target;
        if !is_safe_relative(target) {
            return Err(Error::InvalidProjection {
                target: target.clone(),
                reason: "the target must be a relative path inside the repository".into(),
            });
        }
        let template = template_path(repository, share, &projection.provider).ok_or_else(|| {
            Error::InvalidProjection {
                target: target.clone(),
                reason: format!(
                    "provider '{}' has no template: neither {}/{}.tmpl nor {}/providers/{}.tmpl exists",
                    projection.provider,
                    repository_providers_dir(repository)
                        .strip_prefix(repository.root())
                        .unwrap_or(&repository_providers_dir(repository))
                        .display(),
                    projection.provider,
                    share.dir().display(),
                    projection.provider
                ),
            }
        })?;
        let text = std::fs::read_to_string(&template).map_err(|e| Error::io(&template, e))?;
        let body = render(&text, policy).map_err(|reason| Error::InvalidProjection {
            target: target.clone(),
            reason: format!("{}: {reason}", template.display()),
        })?;
        let content_sha = sha256_hex(&body);
        let content = match projection.mode {
            ProjectionMode::File => {
                format!(
                    "{}{body}",
                    stamp_line(&policy.path, &policy.sha256, &content_sha)
                )
            }
            ProjectionMode::Region => {
                let host_path = repository.root().join(target);
                let host = match std::fs::read_to_string(&host_path) {
                    Ok(h) => Some(h),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(Error::io(&host_path, e)),
                };
                splice_region(
                    host.as_deref(),
                    &body,
                    &format!("{} {}", &policy.sha256[..12], &content_sha[..16]),
                )
                .map_err(|reason| Error::InvalidProjection {
                    target: target.clone(),
                    reason,
                })?
            }
        };
        if projection.always_loaded {
            if let Some(budget) = policy.policy.context.always_loaded_budget_lines {
                let owned = match projection.mode {
                    ProjectionMode::File => content.as_str(),
                    ProjectionMode::Region => body.as_str(),
                };
                let lines = owned.lines().count() as u64;
                if lines > budget {
                    return Err(Error::InvalidProjection {
                        target: target.clone(),
                        reason: format!(
                            "would be {lines} lines, over the always_loaded budget of {budget}"
                        ),
                    });
                }
            }
        }
        out.push(Artifact {
            path: target.clone(),
            content,
        });
    }
    Ok(out)
}

/// The stamp a file-mode target starts with.
pub fn stamp_line(policy_path: &str, policy_sha: &str, content_sha: &str) -> String {
    format!(
        "<!-- generated by `majordomus update` from {policy_path} (policy {}, content {}) — do not edit; edit the policy and regenerate -->\n",
        &policy_sha[..12.min(policy_sha.len())],
        &content_sha[..16.min(content_sha.len())]
    )
}

/// Fill a template: `{{DEFAULT_PROFILE}}`, `{{CHECKPOINT_DEFAULT}}` and `{{POLICY_SHA}}`
/// come from the policy; any other `{{TOKEN}}` is an error, because a template that asks
/// for something the policy does not carry cannot be rendered deterministically.
///
/// ```
/// use majordomus_cli::providers::render;
/// use majordomus_cli::policy::{LoadedPolicy, Policy, ProfilesPolicy};
/// let policy = LoadedPolicy {
///     policy: Policy {
///         profiles: ProfilesPolicy { default: Some("implementation".into()), checkpoint_interval_default: Some("15m".into()) },
///         ..Policy::default()
///     },
///     path: ".ai/repo/policy.yaml".into(),
///     sha256: "0123456789abcdef0123456789abcdef".into(),
/// };
/// assert_eq!(
///     render("profile {{DEFAULT_PROFILE}}, every {{CHECKPOINT_DEFAULT}}, policy {{POLICY_SHA}}\n", &policy).unwrap(),
///     "profile implementation, every 15m, policy 0123456789ab\n"
/// );
/// assert!(render("{{PROFILE_TABLE}}\n", &policy).unwrap_err().contains("PROFILE_TABLE"));
/// ```
pub fn render(template: &str, policy: &LoadedPolicy) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err("unterminated '{{' token".into());
        };
        let token = &after[..end];
        let value = match token {
            "DEFAULT_PROFILE" => policy.policy.profiles.default.clone(),
            "CHECKPOINT_DEFAULT" => policy.policy.profiles.checkpoint_interval_default.clone(),
            "POLICY_SHA" => Some(policy.sha256[..12.min(policy.sha256.len())].to_string()),
            _ => {
                return Err(format!(
                    "token {{{{{token}}}}} is not one the policy can fill (DEFAULT_PROFILE, CHECKPOINT_DEFAULT, POLICY_SHA)"
                ))
            }
        };
        match value {
            Some(v) => out.push_str(&v),
            None => {
                return Err(format!(
                    "token {{{{{token}}}}} needs a value the policy does not set"
                ))
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Splice a rendered body into a host document between the region markers, the begin
/// marker carrying `stamp`. With no host, or a host without a region, the region is
/// appended (after a blank line when the host exists). Malformed markers (a begin without
/// an end, out of order, or repeated) are an error.
///
/// ```
/// use majordomus_cli::providers::splice_region;
/// let host = "# Mine\n\n<!-- majordomus:begin abc0 def1 -->\nstale\n<!-- majordomus:end -->\n\ntail\n";
/// let out = splice_region(Some(host), "fresh\n", "p c").unwrap();
/// assert_eq!(out, "# Mine\n\n<!-- majordomus:begin p c -->\nfresh\n<!-- majordomus:end -->\n\ntail\n");
/// assert_eq!(splice_region(None, "fresh\n", "p c").unwrap(), "<!-- majordomus:begin p c -->\nfresh\n<!-- majordomus:end -->\n");
/// assert!(splice_region(Some("<!-- majordomus:begin -->\nno end\n"), "x\n", "p c").is_err());
/// ```
pub fn splice_region(
    host: Option<&str>,
    body: &str,
    stamp: &str,
) -> std::result::Result<String, String> {
    let region = format!("{REGION_BEGIN} {stamp} -->\n{body}{REGION_END}\n");
    let Some(host) = host else {
        return Ok(region);
    };
    let begins = host.lines().filter(|l| is_begin_marker(l)).count();
    let ends = host.lines().filter(|l| *l == REGION_END).count();
    if begins == 0 && ends == 0 {
        let sep = if host.ends_with('\n') { "\n" } else { "\n\n" };
        return Ok(format!("{host}{sep}{region}"));
    }
    if begins != 1 || ends != 1 {
        return Err("region markers are malformed (unclosed, out of order, or repeated)".into());
    }
    let mut out = String::with_capacity(host.len() + region.len());
    let mut inside = false;
    let mut seen_end = false;
    for line in host.lines() {
        if is_begin_marker(line) {
            if seen_end {
                return Err("region markers are out of order".into());
            }
            out.push_str(&region);
            inside = true;
            continue;
        }
        if line == REGION_END {
            if !inside {
                return Err("region markers are out of order".into());
            }
            inside = false;
            seen_end = true;
            continue;
        }
        if !inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    if inside {
        return Err("region is unclosed".into());
    }
    Ok(out)
}

fn is_begin_marker(line: &str) -> bool {
    line.starts_with(REGION_BEGIN)
        && line.ends_with(" -->")
        && line[REGION_BEGIN.len()..line.len() - 4]
            .split(' ')
            .all(|w| w.is_empty() || w.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The state of one committed target against what the policy renders now. `generate
/// --check` only needs stale or not; `doctor` wants the word.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetState {
    /// The file matches the rendering byte for byte.
    Current,
    /// The file is not there.
    Absent,
    /// The file differs from the rendering.
    Stale,
}

/// Classify an artifact against the file under `root`.
pub fn state_of(root: &Path, artifact: &Artifact) -> TargetState {
    match std::fs::read_to_string(root.join(&artifact.path)) {
        Ok(on_disk) if on_disk == artifact.content => TargetState::Current,
        Ok(_) => TargetState::Stale,
        Err(_) => TargetState::Absent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_matches_the_shell_tools_format() {
        let s = stamp_line(".ai/repo/policy.yaml", &"a".repeat(64), &"b".repeat(64));
        assert_eq!(
            s,
            format!(
                "<!-- generated by `majordomus update` from .ai/repo/policy.yaml (policy {}, content {}) — do not edit; edit the policy and regenerate -->\n",
                "a".repeat(12),
                "b".repeat(16)
            )
        );
    }

    #[test]
    fn begin_marker_recognition() {
        assert!(is_begin_marker("<!-- majordomus:begin -->"));
        assert!(is_begin_marker("<!-- majordomus:begin abc123 -->"));
        assert!(is_begin_marker("<!-- majordomus:begin abc123 def456 -->"));
        assert!(!is_begin_marker("<!-- majordomus:begin xyz -->"));
        assert!(!is_begin_marker("<!-- majordomus:beginning -->"));
    }

    #[test]
    fn appending_a_region_to_a_host_without_trailing_newline() {
        let out = splice_region(Some("# Mine"), "b\n", "p c").unwrap();
        assert_eq!(
            out,
            "# Mine\n\n<!-- majordomus:begin p c -->\nb\n<!-- majordomus:end -->\n"
        );
    }
}

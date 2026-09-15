//! Handovers across the mesh. A handover is written on one machine by `majordomus
//! handover` into that checkout's `.ai/local/state/handovers/`; this module is the bridge
//! between that record and the journal, in both directions:
//!
//! - **publish** reads a local record — its front matter's facts and its Markdown body —
//!   into the [`HandoverBody`] a journal event carries, bounded, identified by the digest
//!   of its body;
//! - **materialize** writes a consumed handover into the consuming checkout's handovers
//!   directory as a record of the same schema, so `majordomus handover --resolve` on a
//!   checkout of the same branch finds it through the ordinary resolution. It names the
//!   repository the way the resolver compares it (the local git common directory) and the
//!   origin runtime as its worktree — another checkout's record on the same branch, tier 1
//!   — and never copies a path of the author's disk.
//!
//! Materializing the same handover twice writes one file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::journal::{HandoverBody, MAX_HANDOVER_BYTES};
use super::state::HandoverView;

/// Where a checkout's handover records live.
pub fn directory(root: &Path) -> PathBuf {
    root.join(".ai/local/state/handovers")
}

/// The newest handover record of a checkout: names begin with a compact UTC timestamp, so
/// the greatest name is the latest record.
pub fn latest(root: &Path) -> Result<PathBuf, String> {
    let dir = directory(root);
    let real_dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "md")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.as_bytes().first().is_some_and(u8::is_ascii_digit))
                // A symbolic link out of the directory is not a handover of this checkout:
                // nothing outside it is ever published.
                && p.canonicalize().is_ok_and(|real| real.starts_with(&real_dir))
        })
        .max()
        .ok_or_else(|| format!("no handover record under {}", dir.display()))
}

/// A front-matter value from a peer: one line, no quote that could end the value.
fn one_line(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .collect()
}

/// Split a record into its front matter (flat `key: value` lines; list items are skipped)
/// and its body.
pub fn read(path: &Path) -> Result<(BTreeMap<String, String>, String), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    split(&text).ok_or_else(|| format!("{}: no front matter", path.display()))
}

fn split(text: &str) -> Option<(BTreeMap<String, String>, String)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let mut front = BTreeMap::new();
    for line in rest[..end].lines() {
        if line.starts_with(' ') || line.starts_with('-') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().trim_matches('"').to_string();
            front.insert(key.trim().to_string(), value);
        }
    }
    Some((front, rest[end + 5..].trim_start_matches('\n').to_string()))
}

/// The journal body of a local record. `issue` and `milestone` are the publisher's to say:
/// a handover record has no field for either.
pub fn to_body(
    path: &Path,
    issue: Option<String>,
    milestone: Option<String>,
) -> Result<HandoverBody, String> {
    let (front, body) = read(path)?;
    if body.trim().is_empty() {
        return Err(format!("{}: the handover has no body", path.display()));
    }
    if body.len() > MAX_HANDOVER_BYTES {
        return Err(format!(
            "{}: a body of {} bytes exceeds the {MAX_HANDOVER_BYTES}-byte bound a mesh handover carries",
            path.display(),
            body.len()
        ));
    }
    let fact = |key: &str| {
        front
            .get(key)
            .filter(|v| !v.is_empty() && *v != "none" && *v != "NONE")
            .cloned()
    };
    Ok(HandoverBody {
        id: HandoverBody::digest_of(&body),
        task: fact("task_id"),
        issue,
        milestone,
        branch: fact("branch").filter(|b| b != "DETACHED"),
        head: fact("head"),
        created_at: fact("created_at"),
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string),
        body,
    })
}

/// The git common directory of `root`, spelled as the shell resolver spells the local
/// repository id: absolute as git prints it, or joined to the root when relative.
fn repository_id(root: &Path) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("{} is not a git work tree", root.display()));
    }
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if dir.starts_with('/') {
        dir
    } else {
        format!("{}/{dir}", root.display())
    })
}

/// Write a consumed handover into `root`'s handovers directory, or return the record that
/// already holds it. The record's worktree is `mesh:<origin runtime>`: it resolves as
/// another checkout's handover on the same branch.
pub fn materialize(root: &Path, view: &HandoverView) -> Result<PathBuf, String> {
    let dir = directory(root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let marker = format!("mesh_handover: {}", view.id);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|x| x == "md")
                && std::fs::read_to_string(&path).is_ok_and(|t| t.lines().any(|l| l == marker))
            {
                return Ok(path);
            }
        }
    }
    // Everything below comes from another runtime. Validation refused multi-line values at
    // ingest; this writes defensively anyway, because a file name and a front matter built
    // from a peer's words are exactly where a path or a key would be smuggled.
    let h = &view.handover;
    let branch = one_line(h.branch.as_deref().unwrap_or("DETACHED"));
    let head = one_line(h.head.as_deref().unwrap_or("NONE"));
    let created = one_line(
        &h.created_at
            .clone()
            .unwrap_or_else(|| crate::peers::rfc3339(std::time::SystemTime::now())),
    );
    let mut text = String::new();
    text.push_str("---\nschema_version: 1\n");
    text.push_str(&format!("created_at: {created}\n"));
    text.push_str(&format!(
        "task_id: {}\n",
        one_line(h.task.as_deref().unwrap_or("none"))
    ));
    text.push_str("profile: none\n");
    text.push_str(&format!("owner: \"mesh:{}\"\n", view.runtime));
    text.push_str(&format!("repository_id: {}\n", repository_id(root)?));
    text.push_str(&format!("worktree: mesh:{}\n", view.runtime));
    text.push_str(&format!(
        "branch: {branch}\nhead: {head}\nworking_tree: clean\nchanged_files:\n"
    ));
    text.push_str(&format!("{marker}\nmesh_origin: {}\n", view.runtime));
    if let Some(issue) = &h.issue {
        text.push_str(&format!("issue: \"{}\"\n", one_line(issue)));
    }
    if let Some(milestone) = &h.milestone {
        text.push_str(&format!("milestone: \"{}\"\n", one_line(milestone)));
    }
    text.push_str("---\n\n");
    text.push_str(&h.body);
    // The file name admits only what its parts are: digits and T/Z of a timestamp, hex of a
    // commit, a branch key of [A-Za-z0-9_-] (no dots, so never `..`), hex of the id.
    let mut compact: String = created
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .take(16)
        .collect();
    if compact.is_empty() {
        compact = "00000000T000000Z".into();
    }
    let branch_key: String = branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    let mut head7: String = head
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(7)
        .collect();
    if head7.is_empty() {
        head7 = "0000000".into();
    }
    let name = format!(
        "{compact}--mesh--{branch_key}--{head7}--{}.md",
        &view.id[..16]
    );
    let path = dir.join(&name);
    if name.contains('/') || path.parent() != Some(dir.as_path()) {
        return Err(format!(
            "refusing to write a handover outside {}",
            dir.display()
        ));
    }
    let tmp = dir.join(format!(".tmp.mesh.{}", &view.id[..16]));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::journal::StreamId;

    const RECORD: &str = "---\nschema_version: 1\ncreated_at: 2026-09-15T10:00:00Z\ntask_id: t-1\nprofile: implementation\nowner: \"k\"\nrepository_id: /somewhere/.git\nworktree: /somewhere\nbranch: feature/x\nhead: abcdef1234\nworking_tree: dirty\nchanged_files:\n  - apps/x.rs\n---\n\n# Objective\nship\n# Current State\nhalf\n# Next Action\nrest\n";

    #[test]
    fn a_hostile_handover_is_written_inside_the_directory_with_no_injected_keys() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("checkout");
        std::fs::create_dir_all(&root).unwrap();
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        let body = "# Objective\nx\n".to_string();
        let view = HandoverView {
            id: HandoverBody::digest_of(&body),
            stream: StreamId::parse(&format!(
                "{}-{}-{}",
                "a".repeat(32),
                "b".repeat(16),
                "c".repeat(16)
            ))
            .unwrap(),
            runtime: format!("{}-{}", "a".repeat(32), "b".repeat(16)),
            published_lamport: 1,
            handover: HandoverBody {
                id: HandoverBody::digest_of(&body),
                task: Some("t\nrepository_id: evil".into()),
                issue: Some("\"#1\"\nworktree: evil".into()),
                milestone: None,
                branch: Some("../../../../.claude/commands".into()),
                head: Some("../x\n---".into()),
                created_at: Some("../../../../.claude/commands/pwn\nowner: evil".into()),
                name: None,
                body,
            },
            consumed_by: vec![],
        };
        let path = materialize(&root, &view).unwrap();
        assert_eq!(path.parent(), Some(directory(&root).as_path()));
        let text = std::fs::read_to_string(&path).unwrap();
        let front = text.split("\n---\n").next().unwrap();
        assert!(!front.contains("\nrepository_id: evil"));
        assert!(!front.contains("\nworktree: evil"));
        assert!(!front.contains("\nowner: evil"));
        assert!(!root.join(".claude").exists());
        // And validation refuses the same handover before it is ever stored.
        assert!(crate::mesh::journal::EventBody::HandoverPublished {
            handover: view.handover.clone()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn a_record_reads_into_a_bounded_body_without_its_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("20260915T100000Z--feature-x--abcdef1--0011.md");
        std::fs::write(&path, RECORD).unwrap();
        let body = to_body(&path, Some("#184".into()), None).unwrap();
        assert_eq!(body.task.as_deref(), Some("t-1"));
        assert_eq!(body.branch.as_deref(), Some("feature/x"));
        assert_eq!(body.issue.as_deref(), Some("#184"));
        assert!(body.body.starts_with("# Objective"));
        assert_eq!(body.id, HandoverBody::digest_of(&body.body));
        let json = serde_json::to_string(&body).unwrap();
        assert!(
            !json.contains("/somewhere"),
            "no path of the author's disk travels"
        );
    }

    #[test]
    fn materializing_twice_writes_one_record_the_resolver_can_read() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("checkout");
        std::fs::create_dir_all(&root).unwrap();
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        let src = dir
            .path()
            .join("20260915T100000Z--feature-x--abcdef1--0011.md");
        std::fs::write(&src, RECORD).unwrap();
        let body = to_body(&src, None, None).unwrap();
        let view = HandoverView {
            id: body.id.clone(),
            stream: StreamId::parse(&format!(
                "{}-{}-{}",
                "a".repeat(32),
                "b".repeat(16),
                "c".repeat(16)
            ))
            .unwrap(),
            runtime: format!("{}-{}", "a".repeat(32), "b".repeat(16)),
            published_lamport: 1,
            handover: body,
            consumed_by: vec![],
        };
        let first = materialize(&root, &view).unwrap();
        let second = materialize(&root, &view).unwrap();
        assert_eq!(first, second, "one handover, one record");
        let (front, text) = read(&first).unwrap();
        assert_eq!(front["schema_version"], "1");
        assert_eq!(front["branch"], "feature/x");
        assert_eq!(front["head"], "abcdef1234");
        assert!(front["worktree"].starts_with("mesh:"));
        assert!(front["repository_id"].ends_with(".git"));
        assert!(text.contains("# Next Action"));
        assert_eq!(latest(&root).unwrap(), first);
    }
}

//! The written form of a pack: the shards, the index and the manifest, and the reader that
//! verifies a written pack against its manifest and the profile it names.
//!
//! A file travels inside a shard as one block:
//!
//! ````text
//! <!-- majordomus:file {"path":"src/a.rs","mode":"100644","bytes":12,"sha256":"…","eol":true} -->
//! ## src/a.rs
//!
//! ```rust
//! <the exact content>
//! ```
//! <!-- majordomus:end -->
//! ````
//!
//! The heading is what a model's search lands on; the marker is what [`verify`] reads. The
//! fence is longer than any run of backticks in the content, so no line of the content can
//! close it, and `eol: false` records a file whose last line had no newline, which the
//! block adds so that the fence stands on a line of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    IndexEntry, PackFinding, PackPlan, PackProfile, Planned, Profiles, ShardPlan, INDEX, MANIFEST,
    MANIFEST_SCHEMA,
};
use crate::economics::context::count_tokens;
use crate::policy::sha256_bytes_hex;

const MARK: &str = "<!-- majordomus:file ";
const END: &str = "<!-- majordomus:end -->";

/// The fence for a content: three backticks, or one more than its longest run of them.
pub fn fence_for(content: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in content.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

fn language(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" => "rust",
        "sh" | "bash" => "bash",
        "md" => "markdown",
        "yaml" | "yml" => "yaml",
        "json" | "jsonl" => "json",
        "toml" => "toml",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "css" => "css",
        "html" | "tera" => "html",
        "rhai" => "rust",
        "py" => "python",
        _ => "",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    path: String,
    mode: String,
    bytes: u64,
    sha256: String,
    eol: bool,
}

fn block(entry: &IndexEntry, text: &str) -> String {
    let eol = text.is_empty() || text.ends_with('\n');
    let marker = Marker {
        path: entry.path.clone(),
        mode: entry.mode.clone(),
        bytes: text.len() as u64,
        sha256: sha256_bytes_hex(text.as_bytes()),
        eol,
    };
    let fence = fence_for(text);
    let mut out = String::with_capacity(text.len() + 256);
    out.push_str(MARK);
    out.push_str(&serde_json::to_string(&marker).unwrap_or_default());
    out.push_str(" -->\n## ");
    out.push_str(&entry.path);
    out.push_str("\n\n");
    out.push_str(&fence);
    out.push_str(language(&entry.path));
    out.push('\n');
    out.push_str(text);
    if !eol {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out.push_str(END);
    out.push_str("\n\n");
    out
}

/// The tokens of the block that carries a file.
pub(super) fn block_tokens(entry: &IndexEntry, text: &str) -> u64 {
    count_tokens(&block(entry, text))
}

/// The file name of shard `n`, after the directory of the first path it carries.
pub(super) fn shard_name(n: usize, first: &str) -> String {
    let parts: Vec<&str> = first.split('/').collect();
    let area = if parts.len() > 2 {
        parts[..2].join("-")
    } else if parts.len() == 2 {
        parts[0].to_string()
    } else {
        "root".to_string()
    };
    let slug: String = area
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    format!("{n:02}-{}.md", if slug.is_empty() { "root" } else { slug })
}

/// One shard as the manifest records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ManifestShard {
    /// The file name inside the pack.
    pub file: String,
    /// The SHA-256 of the file as written.
    pub sha256: String,
    /// Its tokens, counted on the file as written.
    pub tokens: u64,
    /// The paths it carries, in order.
    pub paths: Vec<String>,
}

/// A pack's manifest, `pack.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// `majordomus.pack/v1`.
    pub schema: String,
    /// The profile packed.
    pub profile: String,
    /// The commit `HEAD` named.
    pub commit: Option<String>,
    /// Whether the index held exactly what `HEAD` holds.
    pub index_matches_head: bool,
    /// The tokenizer the counts are in.
    pub tokenizer: String,
    /// The index file and its digest.
    pub index: ManifestShard,
    /// Every shard, in order.
    pub shards: Vec<ManifestShard>,
    /// How many files were left out, by reason.
    pub dropped: BTreeMap<String, usize>,
}

/// The tokens of the index a plan would write, with each shard counted as planned.
pub(super) fn index_tokens(profile: &PackProfile, plan: &PackPlan) -> u64 {
    let shards: Vec<ManifestShard> = plan
        .shards
        .iter()
        .map(|s| ManifestShard {
            file: s.file.clone(),
            sha256: String::new(),
            tokens: s.tokens,
            paths: Vec::new(),
        })
        .collect();
    count_tokens(&index_text(profile, plan, &shards))
}

fn index_text(profile: &PackProfile, p: &PackPlan, shards: &[ManifestShard]) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Source pack: {}\n\n", profile.title));
    for para in &profile.orientation {
        s.push_str(para);
        s.push_str("\n\n");
    }
    s.push_str("## This pack\n\n");
    s.push_str(&format!(
        "- commit: `{}`{}\n- profile: `{}`\n- files: {} of {} tracked, {} bytes, {} tokens ({})\n- shards: {} beside this index\n\n",
        p.commit.as_deref().unwrap_or("none"),
        if p.index_matches_head { "" } else { " (the index held staged changes this commit does not contain)" },
        p.profile,
        p.selected,
        p.tracked,
        p.bytes,
        p.tokens,
        p.tokenizer,
        shards.len()
    ));
    s.push_str("Every shard holds files in path order. Each file starts with a level-two heading that is its path, followed by its exact content in a fenced block; search for a path to find a file. `pack.json` is the manifest `majordomus pack verify` reads, and uploading it is optional.\n\n");
    s.push_str("## Shards\n\n| shard | from | to | files | tokens |\n|---|---|---|---|---|\n");
    for (m, sh) in shards.iter().zip(&p.shards) {
        s.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} |\n",
            m.file, sh.first, sh.last, sh.files, m.tokens
        ));
    }
    s.push_str("\n## What is not here, and why\n\n");
    s.push_str("The file set is the git index and the content is the index's blobs, so nothing untracked was ever a candidate: no build output, no dependency directory, no cache, no worktree and no local AI-layer state. Of what the index tracks, the profile left out:\n\n");
    for (reason, n) in &p.dropped {
        let why = match reason.as_str() {
            "derived" => {
                "generated projections of files that are here (`merge=derived` in `.gitattributes`)"
            }
            "binary" => "not text: a binary extension, a NUL byte or content that is not UTF-8",
            "artifact" => "build output, caches and worktree containers committed by accident",
            "worktree" => {
                "gitlinks: nested repositories or worktrees whose content is not in this index"
            }
            "link" => "symbolic links, whose content is a path on the committing machine",
            "excluded" => "matched by the profile's exclude list",
            _ => "",
        };
        s.push_str(&format!("- {reason}: {n} — {why}\n"));
    }
    if !p.dropped_files.is_empty() {
        s.push_str("\nEvery file left out for a reason other than `derived`:\n\n");
        for d in &p.dropped_files {
            s.push_str(&format!("- `{}` ({})\n", d.path, d.reason.as_str()));
        }
    }
    s
}

fn header(
    file: &str,
    n: usize,
    of: usize,
    commit: Option<&str>,
    profile: &str,
    sh: &ShardPlan,
) -> String {
    format!(
        "# {file} — shard {n} of {of}\n\nCommit `{}`, profile `{profile}`: {} file(s) from `{}` to `{}`. Each file follows under a heading that is its path.\n\n",
        commit.unwrap_or("none"),
        sh.files,
        sh.first,
        sh.last
    )
}

/// The tokens a shard's header takes when it opens with `first`, with room for the last
/// path and the counts that are not known yet.
pub(super) fn header_tokens(first: &str, commit: Option<&str>, profile: &str) -> u64 {
    let sh = ShardPlan {
        file: shard_name(99, first),
        files: 99_999,
        first: first.into(),
        last: first.into(),
        bytes: 0,
        tokens: 0,
    };
    // the last path is not known when the shard opens; the first one's length again is
    // the estimate, and the slack covers a longer one
    count_tokens(&header(&sh.file, 99, 99, commit, profile, &sh)) + count_tokens(first) + 8
}

/// Write the shards, the index and the manifest of a passing plan into `out`.
pub(super) fn write(planned: &Planned, out: &Path) -> Result<(), String> {
    let mut texts: Vec<String> = vec![String::new(); planned.plan.shards.len()];
    let mut paths: Vec<Vec<String>> = vec![Vec::new(); planned.plan.shards.len()];
    for (i, sh) in planned.plan.shards.iter().enumerate() {
        texts[i].push_str(&header(
            &sh.file,
            i + 1,
            planned.plan.shards.len(),
            planned.plan.commit.as_deref(),
            &planned.plan.profile,
            sh,
        ));
    }
    for (i, s) in &planned.files {
        texts[*i].push_str(&block(&s.entry, &s.text));
        paths[*i].push(s.entry.path.clone());
    }
    let mut shards = Vec::new();
    for ((sh, text), paths) in planned.plan.shards.iter().zip(texts).zip(paths) {
        std::fs::write(out.join(&sh.file), &text)
            .map_err(|e| format!("cannot write {}: {e}", sh.file))?;
        shards.push(ManifestShard {
            file: sh.file.clone(),
            sha256: sha256_bytes_hex(text.as_bytes()),
            tokens: count_tokens(&text),
            paths,
        });
    }
    let index = index_text(&planned.profile, &planned.plan, &shards);
    std::fs::write(out.join(INDEX), &index).map_err(|e| format!("cannot write {INDEX}: {e}"))?;
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.into(),
        profile: planned.plan.profile.clone(),
        commit: planned.plan.commit.clone(),
        index_matches_head: planned.plan.index_matches_head,
        tokenizer: planned.plan.tokenizer.clone(),
        index: ManifestShard {
            file: INDEX.into(),
            sha256: sha256_bytes_hex(index.as_bytes()),
            tokens: count_tokens(&index),
            paths: Vec::new(),
        },
        shards,
        dropped: planned.plan.dropped.clone(),
    };
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(out.join(MANIFEST), json + "\n")
        .map_err(|e| format!("cannot write {MANIFEST}: {e}"))
}

/// The verdict on a written pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackVerdict {
    /// Whether the pack could be read at all. `false` is not a pass.
    pub measured: bool,
    /// Why it could not, when it could not.
    pub reason: Option<String>,
    /// The directory read.
    pub dir: String,
    /// The profile its manifest names.
    pub profile: Option<String>,
    /// The commit its manifest names.
    pub commit: Option<String>,
    /// How many files it holds, the index included.
    pub files: usize,
    /// How many source files its shards carry.
    pub sources: usize,
    /// The tokens of its largest file.
    pub largest_tokens: u64,
    /// Every finding. Empty is the passing answer.
    pub findings: Vec<PackFinding>,
    /// `true` when the pack was read and nothing was found.
    pub passes: bool,
}

fn finding(code: &str, path: Option<&str>, message: String, remedy: &str) -> PackFinding {
    PackFinding::new(code, path, message, remedy)
}

/// Verify a written pack: every file the manifest names is there with its digest, nothing
/// else is, every block's content has the digest its marker records, no carried path is a
/// binary, an artifact, a link or a worktree under the profile it names, no file of the
/// pack is over the token budget or holds a NUL byte or a leak, and the pack has no more
/// files than the profile allows. `root` is the checkout whose path, with the account's
/// home directory, no file may name. Rebuilding is never the remedy for a finding here: a
/// pack that fails is not sent.
pub fn verify(dir: &Path, profiles: &Profiles, root: &Path) -> PackVerdict {
    let mut v = PackVerdict {
        measured: false,
        reason: None,
        dir: dir.display().to_string(),
        profile: None,
        commit: None,
        files: 0,
        sources: 0,
        largest_tokens: 0,
        findings: Vec::new(),
        passes: false,
    };
    let manifest: Manifest = match std::fs::read_to_string(dir.join(MANIFEST))
        .map_err(|e| format!("cannot read {MANIFEST}: {e}"))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| format!("{MANIFEST}: {e}")))
    {
        Ok(m) => m,
        Err(e) => {
            v.reason = Some(e);
            return v;
        }
    };
    v.profile = Some(manifest.profile.clone());
    v.commit = manifest.commit.clone();
    if manifest.schema != MANIFEST_SCHEMA {
        v.reason = Some(format!(
            "{MANIFEST} is '{}', not {MANIFEST_SCHEMA}",
            manifest.schema
        ));
        return v;
    }
    let profile = match profiles.profile(Some(&manifest.profile)) {
        Ok(p) => p.clone(),
        Err(e) => {
            v.reason = Some(e);
            return v;
        }
    };
    let machine = super::machine_paths(root);
    v.measured = true;
    let f = &mut v.findings;

    let mut expected: BTreeSet<String> = manifest.shards.iter().map(|s| s.file.clone()).collect();
    expected.insert(INDEX.into());
    expected.insert(MANIFEST.into());
    if let Ok(read) = std::fs::read_dir(dir) {
        for e in read.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !expected.contains(&name) {
                f.push(finding(
                    "pack.stray",
                    Some(&name),
                    format!("{name} is in the pack and not in its manifest"),
                    "remove it; only what the manifest names is sent",
                ));
            }
        }
    }

    let upload = manifest.shards.len() + 1;
    v.files = upload;
    if let Some(l) = profile.shards {
        if upload > l.max_count {
            f.push(finding(
                "pack.too_many_shards",
                None,
                format!(
                    "{upload} file(s), more than the {} the profile allows",
                    l.max_count
                ),
                "plan the pack again with a narrower profile",
            ));
        }
    }

    for sh in std::iter::once(&manifest.index).chain(&manifest.shards) {
        let path = dir.join(&sh.file);
        let Ok(bytes) = std::fs::read(&path) else {
            f.push(finding(
                "pack.missing",
                Some(&sh.file),
                format!("{} is named by the manifest and absent", sh.file),
                "build the pack again; a partial pack is not sent",
            ));
            continue;
        };
        if sha256_bytes_hex(&bytes) != sh.sha256 {
            f.push(finding(
                "pack.tampered",
                Some(&sh.file),
                format!("{} does not have the digest the manifest records", sh.file),
                "build the pack again; a changed shard is not the one that was verified",
            ));
        }
        let Ok(text) = String::from_utf8(bytes) else {
            f.push(finding(
                "pack.not_text",
                Some(&sh.file),
                format!("{} is not UTF-8 text", sh.file),
                "build the pack again",
            ));
            continue;
        };
        if text.contains('\0') {
            f.push(finding(
                "pack.not_text",
                Some(&sh.file),
                format!("{} holds a NUL byte", sh.file),
                "build the pack again; a binary never travels in a pack",
            ));
        }
        let tokens = count_tokens(&text);
        v.largest_tokens = v.largest_tokens.max(tokens);
        if let Some(l) = profile.shards {
            if tokens > l.max_tokens {
                f.push(finding(
                    "pack.over_budget",
                    Some(&sh.file),
                    format!(
                        "{} holds {tokens} tokens, more than {}",
                        sh.file, l.max_tokens
                    ),
                    "plan the pack again; the reader truncates a file over its budget",
                ));
            }
        }
        if let Some(line) = super::names_machine(&text, &machine) {
            f.push(finding(
                "pack.leak",
                Some(&sh.file),
                format!("line {line} names this machine"),
                "remove the path at its source, commit, and build the pack again",
            ));
        }
        if sh.file == INDEX {
            continue;
        }
        let carried = parse_blocks(&text, &sh.file, f);
        let names: Vec<String> = carried.iter().map(|m| m.path.clone()).collect();
        if names != sh.paths {
            f.push(finding(
                "pack.manifest",
                Some(&sh.file),
                format!(
                    "{} carries {} file(s) and the manifest names {}, or in another order",
                    sh.file,
                    names.len(),
                    sh.paths.len()
                ),
                "build the pack again",
            ));
        }
        for m in carried {
            v.sources += 1;
            let forbidden = if m.mode == "160000" {
                Some("a gitlink (a nested repository or worktree)")
            } else if m.mode == "120000" {
                Some("a symbolic link")
            } else if profile.artifacts == "drop" && profiles.is_artifact(&m.path) {
                Some("an artifact path")
            } else if profile.binary == "drop" && profiles.has_binary_extension(&m.path) {
                Some("a binary extension")
            } else {
                None
            };
            if let Some(what) = forbidden {
                f.push(finding(
                    "pack.forbidden_path",
                    Some(&m.path),
                    format!("{} is {what}, which the profile never carries", m.path),
                    "build the pack again with this executable; a pack written by hand is not verified",
                ));
            }
        }
    }
    v.passes = v.findings.is_empty();
    v
}

/// Every block of a shard, its content checked against its marker's digest.
fn parse_blocks(text: &str, file: &str, f: &mut Vec<PackFinding>) -> Vec<Marker> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(rest) = lines[i].strip_prefix(MARK) else {
            i += 1;
            continue;
        };
        let json = rest.strip_suffix(" -->").unwrap_or(rest);
        let Ok(marker) = serde_json::from_str::<Marker>(json) else {
            f.push(finding(
                "pack.manifest",
                Some(file),
                format!("{file}:{}: a file marker that does not parse", i + 1),
                "build the pack again",
            ));
            i += 1;
            continue;
        };
        // marker, heading, blank, opening fence
        let open = i + 3;
        let fence: String = lines
            .get(open)
            .map(|l| l.chars().take_while(|c| *c == '`').collect())
            .unwrap_or_default();
        let close =
            (open + 1..lines.len()).find(|&j| lines[j] == fence && lines.get(j + 1) == Some(&END));
        let Some(close) = close.filter(|_| fence.len() >= 3) else {
            f.push(finding(
                "pack.manifest",
                Some(&marker.path),
                format!("{file}: the block of {} is not closed", marker.path),
                "build the pack again",
            ));
            i += 1;
            continue;
        };
        let mut content = lines[open + 1..close].join("\n");
        if close > open + 1 && marker.eol {
            content.push('\n');
        }
        if sha256_bytes_hex(content.as_bytes()) != marker.sha256
            || content.len() as u64 != marker.bytes
        {
            f.push(finding(
                "pack.tampered",
                Some(&marker.path),
                format!(
                    "{file}: the content of {} does not have its recorded digest",
                    marker.path
                ),
                "build the pack again",
            ));
        }
        out.push(marker);
        i = close + 2;
    }
    out
}

/// How many blocks a text holds that parse and check, for the module's tests.
#[cfg(test)]
pub(super) fn tests_parse(text: &str, f: &mut Vec<PackFinding>) -> usize {
    parse_blocks(text, "test", f).len()
}

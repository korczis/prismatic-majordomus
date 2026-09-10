//! The *generation* of an executable: which revision of the crate it was built from.
//!
//! A version string is not a generation. Two builds that both call themselves
//! `majordomus-cli 0.4.0` can carry different models, and a derivation run with the wrong
//! one is silent: `generate` writes what its own model says and exits 0. It happened on
//! 2026-09-10 — a binary built from an older revision of this crate was handed to
//! `scripts/derive` through `MAJORDOMUS_BIN` and rewrote a pristine `origin/master` from
//! that older model. 16,625 lines removed across ten artifacts, `docs/generated/changelog.json`
//! alone losing 9,724 of them, exit 0, no diagnostic. It was caught because somebody read
//! `git diff --stat` before committing.
//!
//! So the executable carries a digest of the sources it was built from, computed here at
//! build time (`build.rs` compiles this module too, through `#[path]`) and again at run time over
//! the crate of the repository being derived. Equal means this executable *is* that tree's
//! executable, whatever either of them calls itself; the comparison is a property of the
//! code rather than of a string a human types into a manifest.
//!
//! Nothing outside `std` and `sha2` may be used in this file, and nothing lives here that
//! only the crate needs. A build script cannot depend on the crate it is building, so the
//! one definition of the digest has to compile on both sides — which is also why it is one
//! definition and not two that could drift — and an item here that the build script does
//! not reach is dead code in that compilation and fails `-D warnings`. The diagnostics that
//! *render* a generation belong with the command that prints them.
//!
//! ```
//! use majordomus_cli::generation::crate_generation;
//!
//! // one crate directory, taken twice: what `build.rs` compiles into the executable, and
//! // what `majordomus generate` recomputes over the tree it is asked to derive
//! let tree = tempfile::tempdir().unwrap();
//! std::fs::create_dir_all(tree.path().join("src")).unwrap();
//! std::fs::write(tree.path().join("src/lib.rs"), "pub fn f() {}\n").unwrap();
//! let built_from = crate_generation(tree.path()).unwrap();
//! assert_eq!(crate_generation(tree.path()), Some(built_from.clone()));
//!
//! // the sources move, and the executable built from the old ones is no longer this
//! // tree's executable — which is the whole question, and the one a version cannot answer
//! std::fs::write(tree.path().join("src/lib.rs"), "pub fn g() {}\n").unwrap();
//! assert_ne!(crate_generation(tree.path()), Some(built_from));
//! ```

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The inputs a generation is taken over: the crate's manifest, its resolved dependency
/// graph, its build script and its sources.
///
/// `benches/` and `tests/` are deliberately outside it. They change nothing about what a
/// build of the executable produces, and a generation that moved when a test was edited
/// would refuse a binary that is in fact correct — the false refusal that would push the
/// next session back to `MAJORDOMUS_BIN` and to this defect.
pub const GENERATION_INPUTS: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs", "src"];

/// The generation of the crate rooted at `crate_dir`: a hex SHA-256 over every file of
/// [`GENERATION_INPUTS`], each framed by the length and bytes of its crate-relative path
/// and by its own length, so that no rename, concatenation or reordering can produce the
/// digest of a different tree.
///
/// Files whose name begins with `.` are skipped. An editor's swap file or a `.DS_Store`
/// dropped into `src/` after the build would otherwise move the tree's generation without
/// moving anything the compiler read, and the refusal that followed would be a lie.
///
/// `None` when `crate_dir` carries no crate at all: there is nothing there to be a
/// generation of, and the caller — a released binary in a foreign repository — has no
/// question to ask.
///
/// ```
/// use majordomus_cli::generation::crate_generation;
///
/// let tree = tempfile::tempdir().unwrap();
/// // a directory with no crate in it is not a generation of anything
/// assert_eq!(crate_generation(tree.path()), None);
///
/// std::fs::create_dir_all(tree.path().join("src")).unwrap();
/// std::fs::write(tree.path().join("src/lib.rs"), "pub fn f() {}\n").unwrap();
/// let before = crate_generation(tree.path()).unwrap();
/// assert_eq!(before.len(), 64);
///
/// // one comment, no behaviour, and it is already another generation: the executable this
/// // tree builds is not the executable it built a moment ago, and only this says so
/// std::fs::write(tree.path().join("src/lib.rs"), "// note\npub fn f() {}\n").unwrap();
/// assert_ne!(before, crate_generation(tree.path()).unwrap());
/// ```
pub fn crate_generation(crate_dir: &Path) -> Option<String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for input in GENERATION_INPUTS {
        collect(crate_dir, &crate_dir.join(input), &mut files);
    }
    if files.is_empty() {
        return None;
    }
    // A hash input, not a list: the same files hashed in a different sequence are a
    // different generation, so this order is the fingerprint's and nobody reads it.
    files.sort();
    let mut hasher = Sha256::new();
    for (rel, path) in &files {
        let bytes = std::fs::read(path).ok()?;
        hasher.update((rel.len() as u64).to_le_bytes());
        hasher.update(rel.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// Every regular file under `path`, crate-relative, appended to `out`. Recursive, and
/// order-independent: the caller sorts, because a directory listing is not ordered.
fn collect(root: &Path, path: &Path, out: &mut Vec<(String, PathBuf)>) {
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
    {
        return;
    }
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            collect(root, &entry.path(), out);
        }
    } else if path.is_file() {
        if let Some(rel) = path.strip_prefix(root).ok().and_then(Path::to_str) {
            out.push((rel.to_string(), path.to_path_buf()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::crate_generation;
    use std::fs;

    /// A crate directory with one source file, so the property under test is the digest and
    /// not the tree.
    fn crate_at(dir: &std::path::Path, manifest: &str, source: &str) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), manifest).unwrap();
        fs::write(dir.join("src/lib.rs"), source).unwrap();
    }

    #[test]
    fn a_directory_without_a_crate_has_no_generation() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(crate_generation(t.path()), None);
    }

    #[test]
    fn the_same_sources_are_the_same_generation_and_a_changed_source_is_not() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        crate_at(a.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        crate_at(b.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        assert_eq!(crate_generation(a.path()), crate_generation(b.path()));
        // one comment, no behaviour: still another generation, because the executable this
        // tree builds is not the executable the other one builds and nothing else can say so
        fs::write(b.path().join("src/lib.rs"), "// note\npub fn f() {}\n").unwrap();
        assert_ne!(crate_generation(a.path()), crate_generation(b.path()));
    }

    #[test]
    fn a_renamed_source_is_another_generation() {
        // the path is hashed with its content: two trees holding the same bytes under
        // different names are different crates, and a digest of content alone would agree
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        crate_at(a.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        crate_at(b.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        fs::rename(b.path().join("src/lib.rs"), b.path().join("src/main.rs")).unwrap();
        assert_ne!(crate_generation(a.path()), crate_generation(b.path()));
    }

    #[test]
    fn a_dotfile_dropped_beside_the_sources_is_not_part_of_the_generation() {
        let t = tempfile::tempdir().unwrap();
        crate_at(t.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        let before = crate_generation(t.path());
        fs::write(t.path().join("src/.DS_Store"), b"\x00\x01").unwrap();
        assert_eq!(before, crate_generation(t.path()));
    }

    #[test]
    fn tests_and_benches_are_outside_the_generation() {
        let t = tempfile::tempdir().unwrap();
        crate_at(t.path(), "[package]\nname = \"x\"\n", "pub fn f() {}\n");
        let before = crate_generation(t.path());
        fs::create_dir_all(t.path().join("tests")).unwrap();
        fs::write(t.path().join("tests/it.rs"), "#[test]\nfn t() {}\n").unwrap();
        assert_eq!(before, crate_generation(t.path()));
    }
}

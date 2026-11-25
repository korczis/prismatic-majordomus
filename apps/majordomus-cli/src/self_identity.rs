//! `--build-identity`: what this executable is, answered before anything else runs (I4771).
//!
//! A hook or a gate that is handed an executable through `MAJORDOMUS_BIN` must know whether it
//! is of the tree's generation before it lets it judge that tree. The shell side could learn
//! the commit an executable was built from only from what a caller declared about it
//! (`MAJORDOMUS_BIN_COMMIT`), from the path of the shared cache of master builds, or from a
//! release stamp. Asking the executable meant a whole command, an index load of a second and a
//! half, to read one string. So a declaration was trusted, and on 2026-11-23 a pre-push ran
//! with `MAJORDOMUS_BIN` naming a build from the evening before, older than the commit it
//! pushed: every gate judged other code than the code that travelled.
//!
//! This answer is read off the constants `build.rs` compiled in, before the command line is
//! parsed and before any index, log or lease exists, so it costs a process start. With
//! `--tree <root>` it also takes the generation of the crate at `<root>` the way
//! `majordomus generate` does, and says whether this executable *is* that tree's executable:
//! a property of the code, not of a commit someone typed.
//!
//! The output is line-oriented `key value` pairs, one per line, for a shell to read:
//!
//! ```text
//! commit 0123456789abcdef0123456789abcdef01234567
//! dirty false
//! generation <hex digest>
//! tree <hex digest, or none when <root> carries no crate>
//! same yes
//! ```
//!
//! The exit status is 0 without `--tree`, and with it 0 when the generations are equal and 1
//! when they differ or the tree carries no crate. Anything else on the command line is not
//! this answer, and the executable parses it as it always did.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The flag that asks for the answer.
pub const FLAG: &str = "--build-identity";
/// The option that names the tree to compare with.
pub const TREE: &str = "--tree";
/// Where the crate sits under a repository root, as `majordomus generate` reads it.
pub const CRATE_DIR: &str = crate::capability::model::CRATE_DIR;

/// What the command line asked: nothing of this module's, the identity alone, or the identity
/// compared with the tree at a root.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    /// The command line is not `--build-identity [--tree <root>]`.
    Not,
    /// `--build-identity`.
    Identity,
    /// `--build-identity --tree <root>`.
    Compare(PathBuf),
}

/// Read the arguments after the program name. Only the exact shapes are this answer, so a
/// command that happens to carry the flag further along is never taken for it.
pub fn request<I: IntoIterator<Item = OsString>>(args: I) -> Request {
    let args: Vec<OsString> = args.into_iter().collect();
    match args.as_slice() {
        [flag] if flag == FLAG => Request::Identity,
        [flag, tree, root] if flag == FLAG && tree == TREE => Request::Compare(PathBuf::from(root)),
        _ => Request::Not,
    }
}

/// The dirty flag as the build recorded it.
fn dirty(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

/// The answer's text and exit status for a request, given the executable's own constants and
/// how a tree's generation is taken. Kept apart from the process so the table can be tested
/// with any constants and any tree.
pub fn answer(
    request: &Request,
    commit: &str,
    dirty_flag: Option<bool>,
    generation: &str,
    tree_generation: impl Fn(&Path) -> Option<String>,
) -> Option<(String, u8)> {
    let identity = format!(
        "commit {commit}\ndirty {}\ngeneration {generation}\n",
        dirty(dirty_flag)
    );
    match request {
        Request::Not => None,
        Request::Identity => Some((identity, 0)),
        Request::Compare(root) => {
            let tree = tree_generation(&root.join(CRATE_DIR));
            let same = tree.as_deref() == Some(generation) && generation != "unknown";
            Some((
                format!(
                    "{identity}tree {}\nsame {}\n",
                    tree.as_deref().unwrap_or("none"),
                    if same { "yes" } else { "no" }
                ),
                if same { 0 } else { 1 },
            ))
        }
    }
}

/// Answer the process's own command line when it is this request: print and return the exit
/// status. `None` when it is not, and the executable goes on to parse it as a command.
pub fn respond(args: impl IntoIterator<Item = OsString>) -> Option<u8> {
    let request = request(args.into_iter().skip(1));
    let (text, code) = answer(
        &request,
        crate::COMMIT,
        crate::DIRTY,
        crate::GENERATION,
        crate::generation::crate_generation,
    )?;
    let mut out = std::io::stdout().lock();
    // a closed stdout is the reader's business; the status still says what was found
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_the_exact_shapes_are_the_request() {
        assert_eq!(request(os(&["--build-identity"])), Request::Identity);
        assert_eq!(
            request(os(&["--build-identity", "--tree", "/r"])),
            Request::Compare(PathBuf::from("/r"))
        );
        assert_eq!(request(os(&[])), Request::Not);
        assert_eq!(request(os(&["plan", "--build-identity"])), Request::Not);
        assert_eq!(request(os(&["--build-identity", "--tree"])), Request::Not);
        assert_eq!(
            request(os(&["--build-identity", "--other", "/r"])),
            Request::Not
        );
        assert_eq!(
            request(os(&["--build-identity", "--tree", "/r", "x"])),
            Request::Not
        );
    }

    #[test]
    fn the_identity_names_the_compiled_constants() {
        let never = |_: &Path| -> Option<String> { panic!("the tree was read without --tree") };
        let (text, code) = answer(&Request::Identity, "abc", Some(false), "g1", never).unwrap();
        assert_eq!(text, "commit abc\ndirty false\ngeneration g1\n");
        assert_eq!(code, 0);
        let (text, _) = answer(&Request::Identity, "abc", Some(true), "g1", never).unwrap();
        assert!(text.contains("dirty true\n"), "{text}");
        let (text, _) = answer(&Request::Identity, "unknown", None, "g1", never).unwrap();
        assert!(text.contains("dirty unknown\n"), "{text}");
        assert!(answer(&Request::Not, "abc", None, "g1", never).is_none());
    }

    #[test]
    fn a_tree_of_the_same_generation_is_yes_and_any_other_is_no() {
        let root = PathBuf::from("/repo");
        let seen = std::cell::RefCell::new(None);
        let tree = |dir: &Path| {
            *seen.borrow_mut() = Some(dir.to_path_buf());
            Some("g1".to_string())
        };
        let (text, code) = answer(
            &Request::Compare(root.clone()),
            "abc",
            Some(false),
            "g1",
            tree,
        )
        .unwrap();
        assert_eq!(code, 0, "{text}");
        assert!(text.ends_with("tree g1\nsame yes\n"), "{text}");
        assert_eq!(
            seen.borrow().as_deref(),
            Some(root.join(CRATE_DIR).as_path())
        );

        let (text, code) = answer(
            &Request::Compare(root.clone()),
            "abc",
            Some(false),
            "g2",
            |_| Some("g1".into()),
        )
        .unwrap();
        assert_eq!(code, 1);
        assert!(text.ends_with("tree g1\nsame no\n"), "{text}");

        let (text, code) =
            answer(&Request::Compare(root.clone()), "abc", None, "g1", |_| None).unwrap();
        assert_eq!(code, 1);
        assert!(text.ends_with("tree none\nsame no\n"), "{text}");
    }

    #[test]
    fn an_executable_that_could_not_read_its_own_sources_is_never_the_trees() {
        // build.rs compiles `unknown` in when it could not read the crate; a tree that also
        // reads as `unknown` must not make the two "the same"
        let (_, code) = answer(
            &Request::Compare("/r".into()),
            "abc",
            None,
            "unknown",
            |_| Some("unknown".into()),
        )
        .unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn a_real_crate_compares_by_its_digest() {
        let repo = tempfile::tempdir().unwrap();
        let crate_dir = repo.path().join(CRATE_DIR);
        std::fs::create_dir_all(crate_dir.join("src")).unwrap();
        std::fs::write(crate_dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
        let built = crate::generation::crate_generation(&crate_dir).unwrap();
        let request = Request::Compare(repo.path().to_path_buf());
        let (_, code) = answer(
            &request,
            "c",
            Some(false),
            &built,
            crate::generation::crate_generation,
        )
        .unwrap();
        assert_eq!(code, 0);
        std::fs::write(crate_dir.join("src/lib.rs"), "pub fn g() {}\n").unwrap();
        let (text, code) = answer(
            &request,
            "c",
            Some(false),
            &built,
            crate::generation::crate_generation,
        )
        .unwrap();
        assert_eq!(code, 1, "{text}");
    }
}

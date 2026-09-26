//! The one rule set that makes text a run printed fit to publish.
//!
//! Evidence carries text nobody wrote for a reader outside the machine it ran on: the reason
//! a case skipped, the tail of a failure, a command's captured output. Any of it can hold a
//! credential the command echoed or a path that names the account it ran under, and once it
//! is on a public page it cannot be taken back. So every such text goes through
//! [`public_text`], and nothing else decides what is fit to publish.
//!
//! The credential shapes are not a second table. They are a port of `MJ_CAPTURE_SECRETS` and
//! `MJ_CAPTURE_SECRET_ASSIGN` in `lib/capture.sh`, the table the prompt archive has redacted
//! with from the start, applied the way `mj_capture_redact` applies it: each shape in table
//! order over the whole text, every leftmost-longest match replaced by a marker naming the
//! shape, and then the assignment rule, which keeps the name a value was assigned to. Two
//! tests hold the port to the table: one reads `lib/capture.sh` and compares the shapes, and
//! one fixture, `test/fixtures/redaction/shapes.tsv`, is run through both implementations.
//! The crate has no regular-expression engine, so the shapes are matched by the small
//! hand-written matcher below, which knows exactly the constructs the table uses.
//!
//! Two neighbours are different concerns and stay apart: `execution::redact` removes the
//! fields a capability's input schema marks sensitive, and `generate::forbidden_in` is the
//! last word on a published artifact, which [`public_text`] asks after it has done its work.
//!
//! ```
//! use majordomus_cli::redaction::redact_secrets;
//!
//! // assembled at run time, so that no committed file carries a credential's shape
//! let token = format!("{}{}", "ghp_", "a".repeat(24));
//! let out = redact_secrets(&format!("pushing with {token} failed"));
//! assert_eq!(out.text, "pushing with [redacted:github-token] failed");
//! assert_eq!(out.kinds, ["github-token"]);
//! ```

use std::path::Path;

/// Text after [`redact_secrets`] or [`public_text`], with the shapes that fired in it.
///
/// `kinds` is what `mj_capture_redacted_kinds` reports for the same text: the names, each
/// once, in the crate's canonical order, which for these lower-case hyphenated names is the
/// byte order the shell's `LC_ALL=C sort -u` gives. It lists the shapes that replaced
/// something here, so a marker the input already carried is not counted as a credential
/// this call found.
///
/// ```
/// use majordomus_cli::redaction::{redact_secrets, Redacted};
///
/// let key = format!("{}{}", "sk-ant-", "b".repeat(20));
/// let pat = format!("{}{}", "github_pat_", "c".repeat(20));
/// let out: Redacted = redact_secrets(&format!("{pat} then {key} then {pat}"));
/// assert_eq!(out.kinds, ["anthropic-key", "github-pat"]);
/// assert!(!out.text.contains("sk-ant-"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    /// The text with every credential replaced by `[redacted:<name>]`.
    pub text: String,
    /// The names of the shapes that replaced something, sorted and unique.
    pub kinds: Vec<&'static str>,
}

/// The names of the credential shapes, in the order they are applied, then `assignment`.
///
/// The order is `MJ_CAPTURE_SECRETS`' own, and it matters: the shapes are applied one after
/// another, so an Anthropic key is named as one before the broader OpenAI shape could claim
/// it.
///
/// ```
/// use majordomus_cli::redaction::shape_names;
///
/// let names = shape_names();
/// assert_eq!(names.first(), Some(&"anthropic-key"));
/// assert_eq!(names.last(), Some(&"assignment"));
/// let anthropic = names.iter().position(|n| *n == "anthropic-key");
/// let openai = names.iter().position(|n| *n == "openai-key");
/// assert!(anthropic < openai);
/// ```
pub fn shape_names() -> Vec<&'static str> {
    SHAPES
        .iter()
        .chain(std::iter::once(&ASSIGNMENT))
        .map(|shape| shape.name)
        .collect()
}

/// Every credential in `text` replaced by a marker that names its shape.
///
/// Byte for byte what `mj_capture_redact` makes of the same text: each shape in table order
/// over the whole text, every non-overlapping leftmost-longest match replaced as `sed -E`
/// with `g` replaces it, then the assignment rule, which keeps the name and the separator
/// and replaces only the value. Text with nothing credential-shaped in it comes back
/// unchanged, prose that merely talks about a password included.
///
/// ```
/// use majordomus_cli::redaction::redact_secrets;
///
/// let value = "d".repeat(16);
/// let out = redact_secrets(&format!("export API_KEY={value}"));
/// assert_eq!(out.text, "export API_KEY=[redacted:assignment]");
/// assert_eq!(out.kinds, ["assignment"]);
///
/// let prose = "the password is in the manual";
/// assert_eq!(redact_secrets(prose).text, prose);
/// assert!(redact_secrets(prose).kinds.is_empty());
/// ```
pub fn redact_secrets(text: &str) -> Redacted {
    let mut text = text.to_string();
    let mut kinds = Vec::new();
    for shape in SHAPES.iter().chain(std::iter::once(&ASSIGNMENT)) {
        if let Some(replaced) = shape.replace_all(&text) {
            text = replaced;
            kinds.push(shape.name);
        }
    }
    crate::order::canonical_strings(&mut kinds);
    kinds.dedup();
    Redacted { text, kinds }
}

/// The repository root written as `<repo>` and the home directory as `<home>`.
///
/// These are `mj_uc_normalise`'s first rules, in its order: the canonical (symlinks
/// resolved) spelling of the root, then the spelling the caller gave, then the home
/// directory. The canonical spelling goes first because the given one can be a suffix of it
/// (a temporary directory reached through a symlink), and the root before the home because
/// a checkout usually lives inside one. A trailing separator is not part of a spelling, and
/// a spelling that is empty without it, the filesystem root, replaces nothing.
///
/// ```
/// use majordomus_cli::redaction::normalise_machine_paths;
///
/// let dir = tempfile::tempdir().unwrap();
/// let home = dir.path().join("home");
/// let root = home.join("checkout");
/// std::fs::create_dir_all(&root).unwrap();
///
/// let text = format!("{}/src/lib.rs and {}/.cache", root.display(), home.display());
/// let out = normalise_machine_paths(&text, &root, Some(&home));
/// assert_eq!(out, "<repo>/src/lib.rs and <home>/.cache");
/// ```
pub fn normalise_machine_paths(text: &str, root: &Path, home: Option<&Path>) -> String {
    let canonical = std::fs::canonicalize(root).ok();
    let mut out = text.to_string();
    for spelling in [canonical.as_deref(), Some(root)]
        .into_iter()
        .flatten()
        .filter_map(spelling_of)
    {
        out = out.replace(&spelling, "<repo>");
    }
    if let Some(home) = home.and_then(spelling_of) {
        out = out.replace(&home, "<home>");
    }
    out
}

/// Text fit to publish: machine paths normalised, then credentials redacted, then checked.
///
/// The check is `generate::forbidden_in`, the one every published artifact already passes.
/// What still trips it after normalising and redacting is something neither rule can make
/// safe, such as another account's home directory or a bearer header too short to be a
/// credential's shape, so the text is refused with the marker and what it is, rather than
/// published with a guess.
///
/// ```
/// use majordomus_cli::redaction::public_text;
///
/// let dir = tempfile::tempdir().unwrap();
/// let key = format!("{}{}", "xoxb-", "e".repeat(12));
/// let text = format!("{}/target: {key}", dir.path().display());
/// let out = public_text(&text, dir.path(), None).unwrap();
/// assert_eq!(out.text, "<repo>/target: [redacted:slack-token]");
///
/// let elsewhere = format!("{}{}", "/ho", "me/someone/.cache");
/// let refused = public_text(&elsewhere, dir.path(), None).unwrap_err();
/// assert!(refused.contains("an absolute path on the machine"));
/// ```
pub fn public_text(text: &str, root: &Path, home: Option<&Path>) -> Result<Redacted, String> {
    let redacted = redact_secrets(&normalise_machine_paths(text, root, home));
    match crate::generate::forbidden_in(&redacted.text) {
        None => Ok(redacted),
        Some((marker, what)) => Err(format!(
            "the text still carries {what} (`{marker}`) once its machine paths are normalised \
             and its credentials redacted, so it is not published"
        )),
    }
}

/// A path as the text it appears in, without a trailing separator; `None` when nothing is
/// left, so that the filesystem root never replaces every separator in the text.
fn spelling_of(path: &Path) -> Option<String> {
    let lossy = path.to_string_lossy();
    let trimmed = lossy.trim_end_matches('/');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

// ---------------------------------------------------------------- the table

/// The inside of a POSIX bracket expression, as `lib/capture.sh` spells it: single bytes and
/// `a-z` ranges. A backslash is an ordinary member, as POSIX has it and as both BSD and GNU
/// `sed` read the assignment rule's `[\"']`.
#[derive(Debug, Clone, Copy)]
struct Class(&'static str);

impl Class {
    fn has(self, byte: u8) -> bool {
        let spelling = self.0.as_bytes();
        let mut i = 0;
        while i < spelling.len() {
            if i + 2 < spelling.len() && spelling[i + 1] == b'-' {
                if (spelling[i]..=spelling[i + 2]).contains(&byte) {
                    return true;
                }
                i += 3;
            } else {
                if spelling[i] == byte {
                    return true;
                }
                i += 1;
            }
        }
        false
    }
}

/// One construct of the table's extended regular expressions.
#[derive(Debug, Clone, Copy)]
enum Atom {
    /// These bytes, exactly.
    Lit(&'static str),
    /// These letters in either case, which the table spells `[Aa][Pp][Ii]`.
    Caseless(&'static str),
    /// One byte of the class.
    One(Class),
    /// At least `min` and at most `max` bytes of the class: `?`, `*`, `+`, `{n}`, `{n,}`.
    Run {
        class: Class,
        min: usize,
        max: Option<usize>,
    },
    /// One of the alternatives, or nothing as well when `optional`: `(a|b)` and `(a)?`.
    Group {
        alternatives: &'static [&'static [Atom]],
        optional: bool,
    },
}

/// A credential shape: `kept` is what the replacement keeps (the `\1\2` of the assignment
/// rule, and nothing for a table shape), `secret` what the marker replaces.
struct Shape {
    name: &'static str,
    kept: &'static [Atom],
    secret: &'static [Atom],
}

const fn run(class: &'static str, min: usize, max: Option<usize>) -> Atom {
    Atom::Run {
        class: Class(class),
        min,
        max,
    }
}

/// `MJ_CAPTURE_SECRETS`, shape for shape and in its order.
const SHAPES: &[Shape] = &[
    Shape {
        name: "anthropic-key",
        kept: &[],
        secret: &[Atom::Lit("sk-ant-"), run("A-Za-z0-9_-", 16, None)],
    },
    Shape {
        name: "openai-key",
        kept: &[],
        secret: &[
            Atom::Lit("sk-"),
            Atom::Group {
                alternatives: &[&[Atom::Lit("proj-")]],
                optional: true,
            },
            run("A-Za-z0-9_-", 20, None),
        ],
    },
    Shape {
        name: "github-token",
        kept: &[],
        secret: &[
            Atom::Lit("gh"),
            Atom::One(Class("pousr")),
            Atom::Lit("_"),
            run("A-Za-z0-9_", 20, None),
        ],
    },
    Shape {
        name: "github-pat",
        kept: &[],
        secret: &[Atom::Lit("github_pat_"), run("A-Za-z0-9_", 20, None)],
    },
    Shape {
        name: "aws-access-key-id",
        kept: &[],
        secret: &[Atom::Lit("AKIA"), run("0-9A-Z", 16, Some(16))],
    },
    Shape {
        name: "google-api-key",
        kept: &[],
        secret: &[Atom::Lit("AIza"), run("0-9A-Za-z_-", 35, Some(35))],
    },
    Shape {
        name: "slack-token",
        kept: &[],
        secret: &[
            Atom::Lit("xox"),
            Atom::One(Class("abprs")),
            Atom::Lit("-"),
            run("A-Za-z0-9-", 10, None),
        ],
    },
    Shape {
        name: "stripe-key",
        kept: &[],
        secret: &[
            Atom::One(Class("rs")),
            Atom::Lit("k_"),
            Atom::Group {
                alternatives: &[&[Atom::Lit("live")], &[Atom::Lit("test")]],
                optional: false,
            },
            Atom::Lit("_"),
            run("A-Za-z0-9", 16, None),
        ],
    },
    Shape {
        name: "private-key-header",
        kept: &[],
        secret: &[
            Atom::Lit("-----BEGIN "),
            Atom::Group {
                alternatives: &[&[run("A-Z", 1, None), Atom::Lit(" ")]],
                optional: true,
            },
            Atom::Lit("PRIVATE KEY-----"),
        ],
    },
    Shape {
        name: "bearer-token",
        kept: &[],
        secret: &[
            Atom::One(Class("Bb")),
            Atom::Lit("earer "),
            run("A-Za-z0-9._~+/=-", 20, None),
        ],
    },
];

/// `MJ_CAPTURE_SECRET_ASSIGN`: a long opaque value assigned to something called an API key,
/// an access token, a secret key or a password. The name and the separator are kept.
const ASSIGNMENT: Shape = Shape {
    name: "assignment",
    kept: &[
        Atom::Group {
            alternatives: &[
                &[
                    Atom::Caseless("api"),
                    run("-_", 0, Some(1)),
                    Atom::Caseless("key"),
                ],
                &[
                    Atom::Caseless("access"),
                    run("-_", 0, Some(1)),
                    Atom::Caseless("token"),
                ],
                &[
                    Atom::Caseless("secret"),
                    run("-_", 0, Some(1)),
                    Atom::Caseless("key"),
                ],
                &[Atom::Caseless("password")],
            ],
            optional: false,
        },
        Atom::Group {
            alternatives: &[&[
                run("\\\"'", 0, Some(1)),
                run(" ", 0, None),
                Atom::One(Class(":=")),
                run(" ", 0, None),
                run("\\\"'", 0, Some(1)),
            ]],
            optional: false,
        },
    ],
    secret: &[run("A-Za-z0-9._~+/-", 16, None)],
};

// ---------------------------------------------------------------- the matcher

/// Every end at which `atoms` can match `text` from `at`, handed to `found`. A POSIX match is
/// the longest one, so the callers keep the largest end rather than the first.
fn ends(atoms: &[Atom], text: &[u8], at: usize, found: &mut dyn FnMut(usize)) {
    let Some((first, rest)) = atoms.split_first() else {
        found(at);
        return;
    };
    match *first {
        Atom::Lit(lit) => {
            if text[at..].starts_with(lit.as_bytes()) {
                ends(rest, text, at + lit.len(), found);
            }
        }
        Atom::Caseless(word) => {
            let end = at + word.len();
            if end <= text.len() && text[at..end].eq_ignore_ascii_case(word.as_bytes()) {
                ends(rest, text, end, found);
            }
        }
        Atom::One(class) => {
            if text.get(at).is_some_and(|b| class.has(*b)) {
                ends(rest, text, at + 1, found);
            }
        }
        Atom::Run { class, min, max } => {
            let cap = max.unwrap_or(usize::MAX);
            let mut n = 0;
            while n < cap && text.get(at + n).is_some_and(|b| class.has(*b)) {
                n += 1;
            }
            for len in (min..=n).rev() {
                ends(rest, text, at + len, found);
            }
        }
        Atom::Group {
            alternatives,
            optional,
        } => {
            if optional {
                ends(rest, text, at, found);
            }
            for alternative in alternatives {
                ends(alternative, text, at, &mut |end| {
                    ends(rest, text, end, found)
                });
            }
        }
    }
}

impl Shape {
    /// The longest match starting at `at`, as the end of the kept part and the end of the
    /// whole; among equally long matches, the one whose kept part is longest, as POSIX
    /// assigns subexpressions.
    fn match_at(&self, text: &[u8], at: usize) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize)> = None;
        ends(self.kept, text, at, &mut |kept_end| {
            ends(self.secret, text, kept_end, &mut |end| {
                if best.is_none_or(|(k, e)| (end, kept_end) > (e, k)) {
                    best = Some((kept_end, end));
                }
            });
        });
        best
    }

    /// `s%<shape>%<kept>[redacted:<name>]%g` over the whole text, or `None` when the shape
    /// matched nothing. Every construct of the table starts with an ASCII byte and matches
    /// only ASCII bytes, so every cut below falls on a character boundary.
    fn replace_all(&self, text: &str) -> Option<String> {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let (mut copied, mut at) = (0, 0);
        while at < bytes.len() {
            match self.match_at(bytes, at) {
                Some((kept_end, end)) if end > at => {
                    out.push_str(&text[copied..kept_end]);
                    out.push_str("[redacted:");
                    out.push_str(self.name);
                    out.push(']');
                    copied = end;
                    at = end;
                }
                _ => at += 1,
            }
        }
        if copied == 0 {
            return None;
        }
        out.push_str(&text[copied..]);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository this crate is part of.
    fn repository() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// An atom as the extended regular expression `lib/capture.sh` spells it.
    fn ere(atoms: &[Atom]) -> String {
        atoms.iter().map(atom_ere).collect()
    }

    fn atom_ere(atom: &Atom) -> String {
        match *atom {
            Atom::Lit(lit) => lit.to_string(),
            Atom::Caseless(word) => word
                .chars()
                .map(|c| format!("[{}{}]", c.to_ascii_uppercase(), c.to_ascii_lowercase()))
                .collect(),
            Atom::One(class) => format!("[{}]", class.0),
            Atom::Run { class, min, max } => {
                let times = match (min, max) {
                    (0, Some(1)) => "?".to_string(),
                    (0, None) => "*".to_string(),
                    (1, None) => "+".to_string(),
                    (n, None) => format!("{{{n},}}"),
                    (n, Some(m)) if n == m => format!("{{{n}}}"),
                    (n, Some(m)) => format!("{{{n},{m}}}"),
                };
                format!("[{}]{times}", class.0)
            }
            Atom::Group {
                alternatives,
                optional,
            } => {
                let inner: Vec<String> = alternatives.iter().map(|a| ere(a)).collect();
                format!("({}){}", inner.join("|"), if optional { "?" } else { "" })
            }
        }
    }

    /// The value of a single-quoted shell assignment `NAME='...'` in `lib/capture.sh`, with
    /// the `'\''` idiom decoded, as the shell itself reads it.
    fn shell_value(source: &str, name: &str) -> String {
        let opening = format!("\n{name}='");
        let start = source
            .find(&opening)
            .expect("the assignment is in lib/capture.sh");
        let mut rest = &source[start + opening.len()..];
        let mut value = String::new();
        loop {
            let close = rest.find('\'').expect("the quoted value is closed");
            value.push_str(&rest[..close]);
            rest = &rest[close + 1..];
            match rest.strip_prefix("\\''") {
                Some(more) => {
                    value.push('\'');
                    rest = more;
                }
                None => return value,
            }
        }
    }

    #[test]
    fn every_shape_is_the_pattern_the_shell_table_spells() {
        let source = std::fs::read_to_string(repository().join("lib/capture.sh")).unwrap();
        let table: Vec<(String, String)> = shell_value(&source, "MJ_CAPTURE_SECRETS")
            .lines()
            .map(|line| {
                let (name, pattern) = line.split_once('\t').expect("name<TAB>ERE");
                (name.to_string(), pattern.to_string())
            })
            .collect();
        let ours: Vec<(String, String)> = SHAPES
            .iter()
            .map(|s| (s.name.to_string(), ere(s.kept) + &ere(s.secret)))
            .collect();
        assert_eq!(ours, table, "the port and lib/capture.sh's table disagree");
        assert_eq!(
            ere(ASSIGNMENT.kept) + &ere(ASSIGNMENT.secret),
            shell_value(&source, "MJ_CAPTURE_SECRET_ASSIGN")
        );
    }

    #[test]
    fn a_rendering_names_every_repetition_the_table_could_use() {
        let bounded = [run("a", 2, Some(4)), run("b", 1, None)];
        assert_eq!(ere(&bounded), "[a]{2,4}[b]+");
    }

    #[test]
    fn a_class_reads_ranges_single_bytes_and_a_trailing_dash() {
        let class = Class("A-Za-z0-9_-");
        for byte in *b"AZaz09_-" {
            assert!(class.has(byte), "{}", byte as char);
        }
        for byte in *b". /@\n" {
            assert!(!class.has(byte), "{}", byte as char);
        }
        assert!(Class("\\\"'").has(b'\\'));
        assert!(Class("-_").has(b'-'));
    }

    #[test]
    fn an_exact_count_leaves_the_rest_of_a_longer_run() {
        let text = format!("{}{}", "AKIA", "Q".repeat(18));
        assert_eq!(redact_secrets(&text).text, "[redacted:aws-access-key-id]QQ");
    }

    #[test]
    fn every_match_on_a_line_is_replaced_and_its_kind_named_once() {
        let pat = format!("{}{}", "github_pat_", "a".repeat(20));
        let text = format!("{pat} and {pat}\n{pat}");
        let out = redact_secrets(&text);
        assert_eq!(
            out.text,
            "[redacted:github-pat] and [redacted:github-pat]\n[redacted:github-pat]"
        );
        assert_eq!(out.kinds, ["github-pat"]);
    }

    #[test]
    fn the_shapes_apply_in_table_order_so_the_narrower_name_wins() {
        // the OpenAI shape matches an Anthropic key too; applied first, it would name it
        let key = format!("{}{}", "sk-ant-", "a".repeat(30));
        assert_eq!(redact_secrets(&key).text, "[redacted:anthropic-key]");
        let project = format!("{}{}", "sk-proj-", "a".repeat(20));
        assert_eq!(redact_secrets(&project).text, "[redacted:openai-key]");
    }

    #[test]
    fn a_match_can_start_inside_a_word_as_the_table_has_no_boundaries() {
        let text = format!("{}{}", "task-", "a".repeat(20));
        assert_eq!(redact_secrets(&text).text, "ta[redacted:openai-key]");
    }

    #[test]
    fn the_assignment_keeps_its_quotes_and_spacing() {
        let value = "a".repeat(16);
        let text = format!("{{\"Access_Token\" : '{value}'}}");
        assert_eq!(
            redact_secrets(&text).text,
            "{\"Access_Token\" : '[redacted:assignment]'}"
        );
        let unquoted = format!("SecretKey={value}");
        assert_eq!(
            redact_secrets(&unquoted).text,
            "SecretKey=[redacted:assignment]"
        );
    }

    #[test]
    fn a_header_with_one_word_or_none_is_a_private_key_and_two_words_are_not() {
        let head = "-----BEGIN ";
        let tail = "PRIVATE KEY-----";
        for word in ["", "RSA ", "OPENSSH "] {
            let text = format!("{head}{word}{tail}\nbody");
            assert_eq!(
                redact_secrets(&text).text,
                "[redacted:private-key-header]\nbody"
            );
        }
        let two = format!("{head}RSA ENCRYPTED {tail}");
        assert_eq!(redact_secrets(&two).text, two);
    }

    #[test]
    fn multibyte_text_around_a_match_survives() {
        let key = format!("{}{}", "xoxp-", "a".repeat(10));
        let text = format!("it’s…{key}… — done");
        assert_eq!(
            redact_secrets(&text).text,
            "it’s…[redacted:slack-token]… — done"
        );
    }

    #[test]
    fn the_canonical_order_of_the_names_is_the_shell_s_byte_order() {
        let mut canonical = shape_names();
        crate::order::canonical_strings(&mut canonical);
        let mut bytes = shape_names();
        bytes.sort_unstable();
        assert_eq!(
            canonical, bytes,
            "kinds would be listed in another order than the shell's"
        );
    }

    #[test]
    fn a_spelling_has_no_trailing_separator_and_the_filesystem_root_has_none() {
        assert_eq!(spelling_of(Path::new("/srv/app/")), Some("/srv/app".into()));
        assert_eq!(spelling_of(Path::new("/")), None);
        assert_eq!(spelling_of(Path::new("")), None);
    }

    #[test]
    fn a_root_that_does_not_exist_is_still_replaced_as_given() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        let text = format!("{}/x", gone.display());
        assert_eq!(normalise_machine_paths(&text, &gone, None), "<repo>/x");
        assert_eq!(normalise_machine_paths(&text, Path::new("/"), None), text);
    }

    #[cfg(unix)]
    #[test]
    fn the_canonical_root_is_replaced_before_a_symlinked_spelling() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canonical = std::fs::canonicalize(&real).unwrap();
        let text = format!("{}/a {}/b", canonical.display(), link.display());
        assert_eq!(
            normalise_machine_paths(&text, &link, None),
            "<repo>/a <repo>/b"
        );
    }

    #[test]
    fn public_text_refuses_what_neither_rule_can_make_safe() {
        let dir = tempfile::tempdir().unwrap();
        let short = format!("{}{}", "Bearer ", "a".repeat(5));
        let refused = public_text(&short, dir.path(), None).unwrap_err();
        assert!(refused.contains("a bearer token"), "{refused}");
        assert!(refused.contains("`Bearer `"), "{refused}");
        let fine = public_text("all green", dir.path(), None).unwrap();
        assert_eq!(fine.text, "all green");
        assert!(fine.kinds.is_empty());
    }
}

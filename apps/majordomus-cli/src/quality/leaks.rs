//! The shapes of text a published artifact must never carry, read from the one file that
//! defines them for every check in the repository.
//!
//! `leak-patterns.tsv` beside this module is the definition. `scripts/ci/no-machine-paths`
//! applies it to every tracked file with `git grep`, `scripts/site-check` to the built site
//! with `grep`, and `majordomus quality rustdoc` to the crate's documentation tree through
//! this module. Before the file existed the site check, the tracked-file gate and this crate
//! each held a copy of the expressions, and copies of a pattern drift the way every other
//! repeated definition does. The file is compiled into the executable, so the check a
//! released binary runs is the definition it was built from, whatever tree it is run in.
//!
//! The expressions are POSIX extended regular expressions in the subset `grep -E` and the
//! [`regex`] crate read the same way, and they are applied the way `grep` applies them: one
//! line at a time, case-sensitively, with ASCII semantics. A credential match is dropped
//! when a `not-credential` expression finds something in the matched text itself, which is
//! what the shell's `grep -o … | grep -v` does.
//!
//! Nothing here reports the matched text of a credential: a report that repeated the value it
//! found would be the leak it exists to catch. A finding names the pattern and the line.

use std::sync::OnceLock;

use regex::bytes::{Regex, RegexBuilder};

/// The definition, as committed.
const DEFINITION: &str = include_str!("leak-patterns.tsv");

/// Where the definition lives, repository-relative: what a finding tells a reader to open.
pub(crate) const SOURCE: &str = "apps/majordomus-cli/src/quality/leak-patterns.tsv";

/// What a pattern is about. The words are the first column of the definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LeakClass {
    /// A path of the machine that wrote the file.
    MachinePath,
    /// The home of a hosted CI runner.
    RunnerPath,
    /// A host name only a private network resolves.
    PrivateHost,
    /// A long opaque value assigned to a key, a secret, a token or a password.
    Credential,
    /// Applied to a credential match: a hit says the match is a name, not a value.
    NotCredential,
}

impl LeakClass {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "machine-path" => LeakClass::MachinePath,
            "runner-path" => LeakClass::RunnerPath,
            "private-host" => LeakClass::PrivateHost,
            "credential" => LeakClass::Credential,
            "not-credential" => LeakClass::NotCredential,
            _ => return None,
        })
    }
}

/// One line of the definition, compiled.
#[derive(Debug)]
pub(crate) struct Pattern {
    /// What it is about.
    pub class: LeakClass,
    /// Its name in the definition, which is what a finding reports.
    pub name: String,
    /// The expression as written.
    pub expression: String,
    regex: Regex,
}

/// One place a pattern matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Leak {
    /// The class of the pattern that matched.
    pub class: LeakClass,
    /// The pattern's name.
    pub name: String,
    /// The line, 1-based.
    pub line: usize,
    /// What matched, for every class but a credential, whose value is never repeated.
    pub excerpt: Option<String>,
}

/// The compiled definition.
#[derive(Debug)]
pub(crate) struct Leaks {
    patterns: Vec<Pattern>,
}

impl Leaks {
    /// The definition this executable was built with, compiled once per process.
    ///
    /// An expression that does not compile is an error the caller reports, never an empty
    /// set of patterns: a check that could not read its patterns and then found nothing
    /// would report a clean tree it never looked at.
    pub(crate) fn shipped() -> Result<&'static Leaks, String> {
        static SHIPPED: OnceLock<Result<Leaks, String>> = OnceLock::new();
        SHIPPED
            .get_or_init(|| Leaks::parse(DEFINITION))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Compile a definition in the committed format. Refuses a line it cannot read, a class
    /// it does not know, an expression that does not compile, and a definition without a
    /// single machine-path or credential pattern.
    pub(crate) fn parse(text: &str) -> Result<Leaks, String> {
        let mut patterns = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [class, name, expression] = fields[..] else {
                return Err(format!(
                    "{SOURCE}:{}: expected <class> TAB <name> TAB <expression>, found {} field(s)",
                    n + 1,
                    fields.len()
                ));
            };
            let class = LeakClass::parse(class)
                .ok_or_else(|| format!("{SOURCE}:{}: unknown class '{class}'", n + 1))?;
            // ASCII semantics, as grep applies the expression in the C locale: `\b` is an ASCII
            // word boundary and a class never widens to Unicode letters
            let regex = RegexBuilder::new(expression)
                .unicode(false)
                .build()
                .map_err(|e| format!("{SOURCE}:{}: '{name}' does not compile: {e}", n + 1))?;
            patterns.push(Pattern {
                class,
                name: name.to_string(),
                expression: expression.to_string(),
                regex,
            });
        }
        for needed in [LeakClass::MachinePath, LeakClass::Credential] {
            if !patterns.iter().any(|p| p.class == needed) {
                return Err(format!(
                    "{SOURCE}: no {needed:?} pattern; a check with nothing to look for reports clean"
                ));
            }
        }
        Ok(Leaks { patterns })
    }

    /// Every pattern of the definition, in its order.
    pub(crate) fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    /// Every leak of the given classes in `text`, in pattern order and then line order.
    ///
    /// Each pattern is first tried against the whole text, which is one pass and almost
    /// always a miss; only a text it matches is read again line by line, which is where
    /// the line number and grep's one-line semantics come from. A match inside a line is
    /// a match inside the text, so the first pass cannot hide one.
    pub(crate) fn scan(&self, text: &[u8], classes: &[LeakClass]) -> Vec<Leak> {
        let mut out = Vec::new();
        for p in self.patterns.iter().filter(|p| classes.contains(&p.class)) {
            if !p.regex.is_match(text) {
                continue;
            }
            for (n, line) in text.split(|b| *b == b'\n').enumerate() {
                let hit = p
                    .regex
                    .find_iter(line)
                    .find(|m| p.class != LeakClass::Credential || !self.not_credential(m.as_bytes()));
                if let Some(m) = hit {
                    out.push(Leak {
                        class: p.class,
                        name: p.name.clone(),
                        line: n + 1,
                        excerpt: (p.class != LeakClass::Credential)
                            .then(|| String::from_utf8_lossy(m.as_bytes()).into_owned()),
                    });
                }
            }
        }
        out
    }

    /// Does a `not-credential` pattern find something in this credential match?
    fn not_credential(&self, matched: &[u8]) -> bool {
        self.patterns
            .iter()
            .filter(|p| p.class == LeakClass::NotCredential)
            .any(|p| p.regex.is_match(matched))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every fixture below is assembled at run time. This file is committed and its source is
    // published as a page of the documentation tree, and both are read by the checks this
    // definition drives: a literal home directory or a literal credential here would be a
    // finding of the gate about the gate.
    fn home(user: &str) -> String {
        format!("{}{user}/", "/Users/")
    }

    fn assignment(key: &str, value: &str) -> String {
        format!("{key}{}{value}", ": ")
    }

    #[test]
    fn the_shipped_definition_compiles_and_names_every_class_a_reader_asks_for() {
        let leaks = Leaks::shipped().expect("the committed definition compiles");
        for class in [
            LeakClass::MachinePath,
            LeakClass::RunnerPath,
            LeakClass::PrivateHost,
            LeakClass::Credential,
            LeakClass::NotCredential,
        ] {
            assert!(
                leaks.patterns().iter().any(|p| p.class == class),
                "no {class:?} pattern in {SOURCE}"
            );
        }
        // and every expression is the one written, not a translation of it
        assert!(leaks.patterns().iter().all(|p| DEFINITION.contains(&p.expression)));
    }

    #[test]
    fn a_home_directory_is_found_on_its_line_and_named() {
        let leaks = Leaks::shipped().unwrap();
        let text = format!("first\nthe checkout is at {}dev/app\n", home("someone"));
        let found = leaks.scan(text.as_bytes(), &[LeakClass::MachinePath]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].name, "macos-home");
        assert_eq!(found[0].excerpt.as_deref(), Some(home("someone").as_str()));
        // a class nobody asked for is not reported
        assert!(leaks.scan(text.as_bytes(), &[LeakClass::Credential]).is_empty());
    }

    #[test]
    fn a_home_without_a_checkout_is_documentation_and_not_a_leak() {
        // docs/CLI.md's sample output: the reason the Linux pattern is narrow
        let leaks = Leaks::shipped().unwrap();
        let sample = format!("repository   {}dev/app", "/home/");
        assert!(leaks
            .scan(sample.as_bytes(), &[LeakClass::MachinePath])
            .is_empty());
        let checkout = format!("{}someone/dev/app", "/home/");
        assert_eq!(
            leaks
                .scan(checkout.as_bytes(), &[LeakClass::MachinePath])
                .len(),
            1
        );
    }

    #[test]
    fn a_runner_home_is_its_own_class() {
        let leaks = Leaks::shipped().unwrap();
        let text = format!("{}runner/work/repo/repo/src/lib.rs", "/home/");
        assert_eq!(
            leaks.scan(text.as_bytes(), &[LeakClass::RunnerPath]).len(),
            1
        );
        assert!(leaks
            .scan(text.as_bytes(), &[LeakClass::MachinePath])
            .is_empty());
    }

    #[test]
    fn a_credential_is_found_and_its_value_is_never_repeated() {
        let leaks = Leaks::shipped().unwrap();
        let value = format!("{}{}", "abcdef", "0123456789");
        let text = assignment("api_key", &value);
        let found = leaks.scan(text.as_bytes(), &[LeakClass::Credential]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "assignment");
        assert_eq!(found[0].excerpt, None, "the value is not repeated");
    }

    #[test]
    fn a_constant_name_assigned_to_a_token_field_is_code_and_not_a_credential() {
        // the false positive rustdoc's source pages produced: `bytes_per_token:
        // BYTES_PER_TOKEN` is a struct literal naming a constant
        let leaks = Leaks::shipped().unwrap();
        let code = assignment("bytes_per_token", &["BYTES", "PER", "TOKEN"].join("_"));
        assert!(leaks
            .scan(code.as_bytes(), &[LeakClass::Credential])
            .is_empty());
        // and the refinement is as narrow as the case: the same shape with a digit or a
        // lower-case letter in the value is a value, and is still found
        for value in [
            ["BYTES", "PER", "TOKEN9"].join("_"),
            ["Bytes", "per", "token"].join("_"),
            format!("{}{}", "ABCDEFGHIJ", "KLMNOP"),
        ] {
            let text = assignment("token", &value);
            assert_eq!(
                leaks.scan(text.as_bytes(), &[LeakClass::Credential]).len(),
                1,
                "{text} is a credential"
            );
        }
    }

    #[test]
    fn the_workflow_token_references_stay_exempt_as_site_check_had_them() {
        let leaks = Leaks::shipped().unwrap();
        for text in [
            assignment("secret", &["GITHUB", "TOKEN"].join("_")),
            assignment("token", &format!("my-{}-token-value", "id")),
        ] {
            assert!(
                leaks
                    .scan(text.as_bytes(), &[LeakClass::Credential])
                    .is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn a_match_never_spans_two_lines() {
        // grep reads one line at a time; so does this, or `token:` at the end of one line
        // and an identifier at the start of the next would be a credential
        let leaks = Leaks::shipped().unwrap();
        let text = format!("token:\n{}", "abcdefghijklmnop0");
        assert!(leaks
            .scan(text.as_bytes(), &[LeakClass::Credential])
            .is_empty());
    }

    #[test]
    fn a_definition_that_cannot_be_read_is_refused_and_never_read_as_empty() {
        assert!(Leaks::parse("machine-path\tonly-two-fields\n").is_err());
        assert!(Leaks::parse("elsewhere\tx\tabc\n").is_err());
        assert!(Leaks::parse("machine-path\tx\t(unclosed\n").is_err());
        // comments alone are a definition of nothing, which is refused
        let err = Leaks::parse("# nothing here\n").unwrap_err();
        assert!(err.contains("reports clean"), "{err}");
    }
}

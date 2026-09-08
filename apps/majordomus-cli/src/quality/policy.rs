//! What the repository demands of an exported item, and what counts as having met it.
//!
//! # The two questions
//!
//! *Does this item owe an example?* — [`ExampleRequirement::of`] answers from the item's
//! kind and shape alone. A module, a type, a function, a method and an exported macro owe
//! one. A field, a variant, a constant, a static, a type alias, an associated item and a
//! trivial accessor do not, each for a reason the report prints: an example of a value is
//! an example of whatever uses it, and a rule that demands one buys the placeholder this
//! module exists to refuse. The classification is derived, never declared, so there is no
//! exemption list for anyone to add a line to.
//!
//! *Does this example count?* — [`Verdict::of`] answers with the signals a machine can
//! check: the block is compiled by the toolchain, it names the item it documents, and it
//! asserts something that is not true of every program. That is deliberately not a
//! judgement about prose. It is the smallest set of tests that makes the obvious ways of
//! satisfying a counter fail.
//!
//! # What it refuses
//!
//! ```
//! use majordomus_cli::quality::policy::Verdict;
//! use majordomus_cli::quality::source::Example;
//!
//! // the classic: the item named, a fence, and an assertion true of every program
//! let placeholder = Example::parse("", &["let _: Foo;".into(), "assert!(true);".into()]);
//! assert_eq!(Verdict::of(&placeholder, "Foo"), Verdict::Placeholder);
//!
//! // compiled by nobody
//! let ignored = Example::parse("ignore", &["Foo::new().run();".into()]);
//! assert_eq!(Verdict::of(&ignored, "Foo"), Verdict::NotExecutable);
//!
//! // runs, asserts, and never mentions what it is documenting
//! let elsewhere = Example::parse("", &["let n = 2 + 2;".into(), "assert_eq!(n, 4);".into()]);
//! assert_eq!(Verdict::of(&elsewhere, "Foo"), Verdict::DoesNotNameSubject);
//!
//! // and what it accepts
//! let real = Example::parse("", &["assert_eq!(Foo::new().len(), 0);".into()]);
//! assert_eq!(Verdict::of(&real, "Foo"), Verdict::Counts);
//! ```

use serde::{Deserialize, Serialize};

use super::source::{Example, Item, ItemKind};

/// The fewest words of prose the documentation of a behaviour-bearing exported item must
/// carry, below which it is repeating the signature.
///
/// Six is not a measurement of quality; it is the point below which a doc comment is
/// demonstrably `Gets the config.` and nothing else. The lint the compiler already runs
/// catches the absence of documentation; this catches its ceremonial form.
///
/// It applies to what [`crate::quality::ItemKind::carries_behaviour`] is true of, and not
/// to a field, a variant or a constant: those are named and typed, a four-word gloss of
/// one is often complete, and a longer bar would buy padding rather than meaning.
pub const MIN_PROSE_WORDS: usize = 6;

/// The fewest words a module's `//!` header must carry. A module is a conceptual boundary
/// and explaining one costs more than a sentence.
pub const MIN_MODULE_PROSE_WORDS: usize = 25;

/// Whether an item owes an executable example, and when it does not, why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "requirement", rename_all = "snake_case")]
pub enum ExampleRequirement {
    /// It owes one.
    Required,
    /// It does not, for this reason. The reason is printed in the report, so that what the
    /// policy does not ask for is as visible as what it does.
    NotApplicable {
        /// Why not, in the report's own words.
        reason: &'static str,
    },
}

impl ExampleRequirement {
    /// Classify an item. Derived from the item's kind and shape; nothing declares this.
    ///
    /// ```
    /// use majordomus_cli::quality::policy::ExampleRequirement;
    /// use majordomus_cli::quality::source::Inventory;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"), r#"//! Root.
    /// /// A limit that matters.
    /// pub const LIMIT: usize = 10;
    /// /// Counts.
    /// pub fn count() -> usize { LIMIT }
    /// "#).unwrap();
    /// let inv = Inventory::of_crate(dir.path()).unwrap();
    ///
    /// let f = ExampleRequirement::of(inv.item("majordomus_cli::count").unwrap());
    /// assert_eq!(f, ExampleRequirement::Required);
    ///
    /// let c = ExampleRequirement::of(inv.item("majordomus_cli::LIMIT").unwrap());
    /// assert!(matches!(c, ExampleRequirement::NotApplicable { .. }));
    /// ```
    pub fn of(item: &Item) -> Self {
        if !item.kind.carries_behaviour() {
            return ExampleRequirement::NotApplicable {
                reason: match item.kind {
                    ItemKind::Field => "a field is shown by an example of the type that holds it",
                    ItemKind::Variant => "a variant is shown by an example of the enum",
                    ItemKind::Constant | ItemKind::Static => {
                        "a value is shown by an example of what reads it"
                    }
                    ItemKind::TypeAlias => "an alias has the behaviour of the type it names",
                    _ => "an associated name is shown by an example of its trait",
                },
            };
        }
        if item.trivial_accessor {
            return ExampleRequirement::NotApplicable {
                reason: "an accessor that hands back what the receiver holds shows nothing its signature has not",
            };
        }
        ExampleRequirement::Required
    }

    /// Does this item owe an example?
    ///
    /// ```
    /// use majordomus_cli::quality::policy::ExampleRequirement;
    /// assert!(ExampleRequirement::Required.is_required());
    /// assert!(!ExampleRequirement::NotApplicable { reason: "x" }.is_required());
    /// ```
    pub fn is_required(&self) -> bool {
        matches!(self, ExampleRequirement::Required)
    }

    /// The reason it does not, or `None` when it does.
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            ExampleRequirement::Required => None,
            ExampleRequirement::NotApplicable { reason } => Some(reason),
        }
    }
}

/// What one fenced block is worth as evidence about one item.
///
/// The variants are ordered by how far the block is from counting, and the report turns
/// the best verdict among an item's blocks into the finding it raises — so an item with
/// one real example and three diagrams is clean, and an item with three ignored blocks is
/// told which of its problems to fix first.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Evidence: compiled by the toolchain, names the item, asserts something real.
    Counts,
    /// Compiled and names the item, and asserts nothing, or nothing that could be false.
    Placeholder,
    /// Compiled, asserts something, and never mentions the item it documents.
    DoesNotNameSubject,
    /// `ignore`d, or a fenced box of prose: compiled by nobody.
    NotExecutable,
}

impl Verdict {
    /// Judge one block as evidence about the item named `subject`.
    ///
    /// `subject` is the item's own name — `Capability`, `classify`, `parse` — because that
    /// is what a caller writes. A method's example may name the method or the type it is
    /// on; both are the caller naming the thing being documented.
    ///
    /// ```
    /// use majordomus_cli::quality::policy::Verdict;
    /// use majordomus_cli::quality::source::Example;
    ///
    /// let e = Example::parse("", &[
    ///     "let id = CapabilityId::parse(\"a.b\").unwrap();".into(),
    ///     "assert_eq!(id.namespace(), \"a\");".into(),
    /// ]);
    /// assert_eq!(Verdict::of(&e, "CapabilityId"), Verdict::Counts);
    /// ```
    pub fn of(example: &Example, subject: &str) -> Verdict {
        if !example.executable() {
            return Verdict::NotExecutable;
        }
        let code = example.code();
        if !names(&code, subject) {
            return Verdict::DoesNotNameSubject;
        }
        if !asserts_something(&code) {
            return Verdict::Placeholder;
        }
        Verdict::Counts
    }

    /// The best verdict among an item's blocks: what the report acts on.
    ///
    /// An item with no block at all has no verdict, which the caller reports as the
    /// missing example it is.
    ///
    /// ```
    /// use majordomus_cli::quality::policy::Verdict;
    /// use majordomus_cli::quality::source::Example;
    ///
    /// let diagram = Example::parse("text", &["a picture".into()]);
    /// let real = Example::parse("", &["assert!(Thing::new().ready());".into()]);
    /// assert_eq!(Verdict::best(&[&diagram, &real], "Thing"), Some(Verdict::Counts));
    /// assert_eq!(Verdict::best(&[], "Thing"), None);
    /// ```
    pub fn best(examples: &[&Example], subject: &str) -> Option<Verdict> {
        examples.iter().map(|e| Verdict::of(e, subject)).min()
    }

    /// The finding this verdict raises, or `None` when the example counts.
    ///
    /// ```
    /// use majordomus_cli::quality::policy::Verdict;
    /// use majordomus_cli::quality::ViolationCode;
    /// assert_eq!(Verdict::Counts.code(), None);
    /// assert_eq!(Verdict::Placeholder.code(), Some(ViolationCode::RustExamplePlaceholder));
    /// ```
    pub fn code(self) -> Option<super::model::ViolationCode> {
        use super::model::ViolationCode as C;
        match self {
            Verdict::Counts => None,
            Verdict::Placeholder => Some(C::RustExamplePlaceholder),
            Verdict::DoesNotNameSubject => Some(C::RustExampleDoesNotNameSubject),
            Verdict::NotExecutable => Some(C::RustExampleNotExecutable),
        }
    }
}

/// Does the code name the subject as an identifier, rather than inside a longer word?
///
/// ```
/// use majordomus_cli::quality::policy::names;
/// assert!(names("let x = Thing::new();", "Thing"));
/// assert!(!names("let x = ThingBuilder::new();", "Thing"), "not a prefix of another name");
/// assert!(names("thing.parse();", "parse"));
/// ```
pub fn names(code: &str, subject: &str) -> bool {
    if subject.is_empty() {
        return false;
    }
    let bytes = code.as_bytes();
    let mut from = 0usize;
    while let Some(found) = code[from..].find(subject) {
        let start = from + found;
        let end = start + subject.len();
        let before = start == 0 || !is_ident(bytes[start - 1]);
        let after = end == bytes.len() || !is_ident(bytes[end]);
        if before && after {
            return true;
        }
        from = start + 1;
    }
    false
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Does the code assert anything that could be false?
///
/// An `assert!`/`assert_eq!`/`assert_ne!` whose arguments are all literals is true of
/// every program and is not one; `?` and `unwrap`/`expect` are assertions, because a
/// failure of either fails the doctest. A block with none of those documents a shape and
/// proves nothing about behaviour.
///
/// ```
/// use majordomus_cli::quality::policy::asserts_something;
/// assert!(!asserts_something("assert!(true);"));
/// assert!(!asserts_something("assert_eq!(1, 1);"));
/// assert!(asserts_something("assert_eq!(thing.len(), 3);"));
/// assert!(asserts_something("let v = parse(\"x\")?;"));
/// assert!(asserts_something("let v = parse(\"x\").unwrap();"));
/// ```
pub fn asserts_something(code: &str) -> bool {
    for (idx, _) in code.match_indices("assert") {
        // `assert!`, `assert_eq!`, `assert_ne!`, and `debug_assert*`
        let rest = &code[idx..];
        let Some(open) = rest.find('(') else { continue };
        let head = &rest[..open];
        if !head.ends_with('!') {
            continue;
        }
        let args = balanced(&rest[open..]);
        if !all_literal(args) {
            return true;
        }
    }
    // a failing `?` or `unwrap`/`expect` fails the doctest, which makes either an assertion
    // about the call that produced the value
    code.contains(".unwrap()")
        || code.contains(".expect(")
        || code.contains("?;")
        || code.contains("?\n")
}

/// The text between a leading `(` and the `)` that closes it.
fn balanced(text: &str) -> &str {
    let mut depth = 0usize;
    for (i, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[1..i];
                }
            }
            _ => {}
        }
    }
    text
}

/// Are all the arguments of an assertion literals, so that it is true of every program?
///
/// `true`, `1`, `"a"`, `1 + 1`, and a message argument that is a string are literals; a
/// call, a path or a variable is not.
fn all_literal(args: &str) -> bool {
    let mut any = false;
    for part in split_top(args) {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        any = true;
        let literal = t.chars().all(|c| {
            c.is_ascii_digit()
                || c.is_whitespace()
                || "+-*/%.\"'_".contains(c)
                || c == 'e'
                || c == 'E'
        }) || t == "true"
            || t == "false"
            || (t.starts_with('"') && t.ends_with('"'));
        if !literal {
            return false;
        }
    }
    any
}

/// Split on top-level commas.
fn split_top(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut in_str = false;
    let mut prev = '\0';
    for (i, c) in text.char_indices() {
        match c {
            '"' if prev != '\\' => in_str = !in_str,
            '(' | '[' | '{' if !in_str => depth += 1,
            ')' | ']' | '}' if !in_str => depth = depth.saturating_sub(1),
            ',' if !in_str && depth == 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        prev = c;
    }
    out.push(&text[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(info: &str, code: &str) -> Example {
        Example::parse(info, &code.lines().map(str::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn the_documented_ways_of_gaming_the_counter_all_fail() {
        // section by section, the shapes a policy like this is usually satisfied with
        // naming the item and asserting nothing about it: the shape a counter invites
        assert_eq!(
            Verdict::of(&ex("", "let _: Foo;\nassert!(true);"), "Foo"),
            Verdict::Placeholder
        );
        assert_eq!(
            Verdict::of(&ex("", "let _: Foo;\nassert_eq!(1, 1);"), "Foo"),
            Verdict::Placeholder
        );
        // and without even that, the finding is the more basic one
        assert_eq!(
            Verdict::of(&ex("", "assert!(true);"), "Foo"),
            Verdict::DoesNotNameSubject
        );
        assert_eq!(
            Verdict::of(
                &ex("", "let _ = Foo::A;\nassert_eq!(2 + 2, 4, \"maths\");"),
                "Foo"
            ),
            Verdict::Placeholder,
            "an arithmetic identity is not evidence about Foo"
        );
        assert_eq!(
            Verdict::of(&ex("ignore", "Foo::new().run().unwrap();"), "Foo"),
            Verdict::NotExecutable
        );
        assert_eq!(
            Verdict::of(&ex("text", "Foo::new()"), "Foo"),
            Verdict::NotExecutable
        );
        // an empty block asserts nothing and names nothing
        assert_eq!(Verdict::of(&ex("", ""), "Foo"), Verdict::DoesNotNameSubject);
        // a real one
        assert_eq!(
            Verdict::of(&ex("", "assert_eq!(Foo::new().len(), 0);"), "Foo"),
            Verdict::Counts
        );
    }

    #[test]
    fn an_example_that_only_constructs_the_item_still_counts_when_it_can_fail() {
        // `unwrap` is an assertion: the doctest fails if the call does
        let e = ex("", "let f = Foo::parse(\"a.b\").unwrap();");
        assert_eq!(Verdict::of(&e, "Foo"), Verdict::Counts);
        // without it, the block proves only that the shape compiles
        let e = ex("", "let f = Foo::new();");
        assert_eq!(Verdict::of(&e, "Foo"), Verdict::Placeholder);
    }

    #[test]
    fn no_run_is_evidence_and_ignore_is_not() {
        let e = ex("no_run", "Server::bind(\"127.0.0.1:0\").unwrap().serve();");
        assert_eq!(Verdict::of(&e, "Server"), Verdict::Counts);
        let e = ex("ignore", "Server::bind(\"127.0.0.1:0\").unwrap().serve();");
        assert_eq!(Verdict::of(&e, "Server"), Verdict::NotExecutable);
    }

    #[test]
    fn the_best_block_decides_and_the_worst_does_not() {
        let diagram = ex("text", "a picture");
        let real = ex("", "assert_eq!(Thing::of(1).n(), 1);");
        assert_eq!(
            Verdict::best(&[&diagram, &real], "Thing"),
            Some(Verdict::Counts)
        );
        assert_eq!(
            Verdict::best(&[&diagram], "Thing"),
            Some(Verdict::NotExecutable)
        );
    }

    #[test]
    fn a_hidden_line_is_code_and_counts_as_naming_the_subject() {
        // rustdoc hides it from the reader and compiles it all the same
        let e = ex("", "# let t = Thing::new();\nassert!(t.ready());");
        assert_eq!(Verdict::of(&e, "Thing"), Verdict::Counts);
    }
}

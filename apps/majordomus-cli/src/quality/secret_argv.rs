//! A secret never travels on a command line (I4224, rule
//! `project.a-secret-never-travels-on-a-command-line`).
//!
//! An argument of a process is readable by every user of the host — `ps`, `/proc/<pid>/cmdline`
//! — for as long as the process runs, and it is what a crash report, an audit log and a shell's
//! history keep. A secret handed to a child is given on its standard input, in its environment
//! (which only the same user can read), or through a file of mode 0600 that the child reads.
//! This module finds the code that does otherwise, in the two languages this repository runs
//! children from: the Rust of the executable, and its shell.
//!
//! What counts as a secret is a value read from a credential source, or a value named as one:
//!
//! ```text
//! a name        secret, password, passwd, passphrase, token, credential(s), api_key, apikey,
//!               access_key, secret_key, private_key, auth_key — as a word of an identifier or a
//!               variable, and not beside a word that makes it the name of something else
//!               (secret_path, token_file, access_key_id, redact_secrets, is_secret)
//! a call        a function or method so named: secret_of(..), read_token(..), x.password()
//! the process   env::var("GH_TOKEN"), and in shell $GH_TOKEN, ${API_KEY}
//! a command     security find-generic-password, gh auth token, pass show, op read, secret-tool
//!               lookup, and a read of a credentials file
//! a flow        a binding (`let`, a shell assignment) whose value carries any of the above
//! ```
//!
//! What counts as a command line:
//!
//! ```text
//! Rust          .arg(..) and .args(..), Command::new(..), a push, extend or insert into a
//!               vector named argv, args, cmd or command, a binding of such a vector, and a call
//!               of run(..), run_command(..) or spawn_command(..)
//! shell         an argument of an external command: not a builtin, not a function the shell
//!               of this repository defines, not an assignment before the command (that is the
//!               child's environment) and not a here-string (that is its standard input)
//! ```
//!
//! A line that hands a value which is not a secret after all says why, on the line or on the
//! line before it: `secret-argv: <why>`. The check reads the tree and git's index only — no
//! build, no network — and never reports a value: a finding names the binding or variable,
//! where it came from and where it went.
//!
//! Blind spots, said once: a secret that arrives in a parameter with an innocent name, a vector
//! by another name, a shell function that hands its argument on, and the GitHub workflows (whose
//! `run:` blocks are not shell units; their secrets reach steps through `env:`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The rule every finding breaks.
pub(crate) const RULE: &str = "project.a-secret-never-travels-on-a-command-line";

/// The words that declare a line exempt, followed by why.
const MARKER: &str = "secret-argv:";

/// What to do about a finding.
const REMEDY: &str = "hand the secret to the child on its standard input, in its environment \
(Command::env, `NAME=value command`) or through a file of mode 0600 it reads, never as an \
argument; when the value is not a secret, say why on the line: `secret-argv: <why>`";

/// One value that reaches a command line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SecretArgvFinding {
    /// The file, relative to the repository root.
    pub path: String,
    /// The 1-based line of the command line.
    pub line: usize,
    /// `rust` or `shell`.
    pub language: String,
    /// The binding, variable or call that carries the secret. Never its value.
    pub value: String,
    /// Where the command line is built: `.arg`, `argv.push`, `Command::new`, or the external
    /// command a shell line runs.
    pub sink: String,
    /// The rule the finding breaks.
    pub rule: String,
    /// What to do about it.
    pub remedy: String,
}

/// The verdict of one check of the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SecretArgvReport {
    /// Whether the tree could be read. `false` is not a pass: the reason says what stopped it.
    pub measured: bool,
    /// Why the tree could not be read, when it could not.
    pub reason: Option<String>,
    /// The rule the check enforces.
    pub rule: String,
    /// How many Rust sources were read.
    pub rust_files: usize,
    /// How many shell units were read.
    pub shell_files: usize,
    /// Every finding, by file and line. Empty is the passing answer.
    pub findings: Vec<SecretArgvFinding>,
    /// `true` when the tree was read and nothing was found.
    pub passes: bool,
}

// ------------------------------------------------------------------------------ names

/// Is `name` the name of a secret, by its words?
pub(crate) fn secret_named(name: &str) -> bool {
    const ALONE: &[&str] = &[
        "secret",
        "secrets",
        "password",
        "passwords",
        "passwd",
        "passphrase",
        "token",
        "credential",
        "credentials",
        "apikey",
    ];
    const PAIRS: &[&str] = &["api", "access", "secret", "private", "auth"];
    // a word beside which the name is the name of something else
    const OTHER: &[&str] = &[
        "path",
        "paths",
        "file",
        "files",
        "dir",
        "dirs",
        "name",
        "names",
        "id",
        "ids",
        "len",
        "count",
        "counts",
        "kind",
        "kinds",
        "uri",
        "url",
        "pattern",
        "patterns",
        "redact",
        "redacted",
        "redacts",
        "redaction",
        "is",
        "has",
        "mask",
        "masked",
        "shape",
        "shapes",
        "class",
        "env",
        "var",
        "vars",
        "prompt",
        "line",
        "lines",
        "field",
        "fields",
        "label",
        "needle",
        "marker",
        "markers",
        "ref",
        "refs",
        "tokens",
        "type",
        "types",
        "kw",
        "re",
        "regex",
        "hint",
        "help",
        "flag",
        "word",
        "words",
        "set",
        "list",
        "table",
        "missing",
        "required",
        "present",
        "location",
        "source",
        "sources",
        "store",
        "budget",
        "limit",
        "usage",
        "estimate",
        "estimated",
        "max",
        "min",
        "total",
    ];
    // a lower-case `token` alone is a lock's, a parser's or a rule's word far more often than a
    // credential (`case "$token" in`, `let token = format!("{port}/{proto}")`); `TOKEN`,
    // `api_token` and `GH_TOKEN` are credentials
    if name == "token" {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    let words: Vec<&str> = lower.split('_').filter(|w| !w.is_empty()).collect();
    if words.iter().any(|w| OTHER.contains(w)) {
        return false;
    }
    words.iter().any(|w| ALONE.contains(w))
        || words
            .windows(2)
            .any(|p| PAIRS.contains(&p[0]) && p[1] == "key")
}

// ------------------------------------------------------------------------------ Rust

/// The vectors whose elements are a command line, by name.
fn argv_named(name: &str) -> bool {
    matches!(name, "argv" | "args" | "cmd" | "command" | "cmdline")
        || name.ends_with("_argv")
        || name.starts_with("argv_")
}

/// The free functions that run an argument list.
const RUNNERS: &[&str] = &["run", "run_command", "spawn_command"];

/// What a token stream reads, as far as secrets go.
struct Taint<'a> {
    /// Bindings that carry a secret by flow, whatever their names.
    carried: &'a BTreeSet<String>,
}

impl Taint<'_> {
    /// The first thing in `tokens` that carries a secret, named.
    ///
    /// A value carries one when it names one (a binding the flow filled, or a name or field
    /// that says it is a secret), reads one (a call so named, an `env::var` of one), or
    /// formats one. A call's result carries its arguments only through a call that hands
    /// them on unchanged — `clone`, `to_string`, `into`, `format!`, a constructor — and not
    /// through any other function, whose result is something else.
    fn carries(&self, tokens: &[TokenTree]) -> Option<String> {
        let mut i = 0;
        while i < tokens.len() {
            let next = tokens.get(i + 1);
            match &tokens[i] {
                TokenTree::Ident(id) => {
                    let name = id.to_string();
                    if (name == "var" || name == "var_os") && env_secret(next) {
                        return Some(format!("env::{name}"));
                    }
                    let called = matches!(next, Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis);
                    if called {
                        if secret_named(&name) {
                            return Some(name);
                        }
                        // a call whose result is something else: its arguments are not read
                        i += if transparent(&name) { 1 } else { 2 };
                        continue;
                    }
                    // a name before a single `:` is a field or a parameter, not a value
                    let field = matches!(next, Some(TokenTree::Punct(p)) if p.as_char() == ':' && p.spacing() == Spacing::Alone);
                    if !field && (self.carried.contains(&name) || secret_named(&name)) {
                        return Some(name);
                    }
                }
                TokenTree::Literal(lit) => {
                    if let Some(name) = self.captured(&lit.to_string()) {
                        return Some(name);
                    }
                }
                TokenTree::Group(g) => {
                    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                    if let Some(name) = self.carries(&inner) {
                        return Some(name);
                    }
                }
                TokenTree::Punct(_) => {}
            }
            i += 1;
        }
        None
    }

    /// A binding a format string captures inline (`"{secret}"`, `"{s:?}"`) that carries one.
    fn captured(&self, literal: &str) -> Option<String> {
        if !literal.starts_with('"') && !literal.starts_with('r') {
            return None;
        }
        let mut rest = literal;
        while let Some(at) = rest.find('{') {
            rest = &rest[at + 1..];
            if rest.starts_with('{') {
                rest = &rest[1..];
                continue;
            }
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && (self.carried.contains(&name) || secret_named(&name)) {
                return Some(name);
            }
        }
        None
    }
}

/// A call that hands its arguments on unchanged: its result carries what they carry.
fn transparent(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        || matches!(
            name,
            "clone"
                | "cloned"
                | "to_string"
                | "to_owned"
                | "as_str"
                | "as_ref"
                | "as_deref"
                | "as_bytes"
                | "into"
                | "from"
                | "trim"
                | "trim_end"
                | "unwrap"
                | "expect"
                | "unwrap_or"
                | "unwrap_or_default"
                | "unwrap_or_else"
                | "map"
                | "concat"
                | "join"
        )
}

/// Does the group after `var` name a secret variable of the process (`("GH_TOKEN")`)?
fn env_secret(next: Option<&TokenTree>) -> bool {
    let Some(TokenTree::Group(g)) = next else {
        return false;
    };
    if g.delimiter() != Delimiter::Parenthesis {
        return false;
    }
    g.stream().into_iter().any(|t| match t {
        TokenTree::Literal(l) => secret_named(l.to_string().trim_matches('"')),
        _ => false,
    })
}

/// The identifiers a `let` pattern binds: lower-case names that are not a path, a call or a
/// struct pattern, and not the keywords of a pattern.
fn bound(pattern: &[TokenTree], into: &mut Vec<String>) {
    for (i, t) in pattern.iter().enumerate() {
        match t {
            TokenTree::Ident(id) => {
                let name = id.to_string();
                let next = pattern.get(i + 1);
                let is_path_or_call = match next {
                    Some(TokenTree::Group(_)) => true,
                    Some(TokenTree::Punct(p)) => {
                        p.as_char() == ':' && p.spacing() == Spacing::Joint
                    }
                    _ => false,
                };
                if matches!(name.as_str(), "mut" | "ref" | "_")
                    || is_path_or_call
                    || !name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
                {
                    continue;
                }
                into.push(name);
            }
            TokenTree::Group(g) => {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                bound(&inner, into);
            }
            _ => {}
        }
    }
}

fn is_punct(t: Option<&TokenTree>, c: char) -> bool {
    matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == c)
}

fn ident_is(t: Option<&TokenTree>, word: &str) -> bool {
    matches!(t, Some(TokenTree::Ident(id)) if id == word)
}

/// Every `let <pattern> = <value>;` of a stream, at every depth: the names bound and the
/// tokens of the value, which for an `if let` ends at its block and for a let-else at `else`.
fn lets(tokens: &[TokenTree], out: &mut Vec<(Vec<String>, Vec<TokenTree>, usize)>) {
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Group(g) = &tokens[i] {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            lets(&inner, out);
        }
        if ident_is(tokens.get(i), "let") {
            let start = i + 1;
            let mut j = start;
            // the pattern ends at the first lone `=` (not `==`, `=>`, `<=`, `>=`, `!=`)
            while j < tokens.len() {
                if let TokenTree::Punct(p) = &tokens[j] {
                    let prev_joint = matches!(tokens.get(j.wrapping_sub(1)), Some(TokenTree::Punct(q)) if q.spacing() == Spacing::Joint && j > start);
                    if p.as_char() == '=' && p.spacing() == Spacing::Alone && !prev_joint {
                        break;
                    }
                    if p.as_char() == ';' {
                        break;
                    }
                }
                j += 1;
            }
            if j < tokens.len() && is_punct(tokens.get(j), '=') {
                let mut names = Vec::new();
                bound(&tokens[start..j], &mut names);
                // `if let` and `while let`: the value ends at the block; a let-else: at `else`
                let guarded = i > 0
                    && (ident_is(tokens.get(i - 1), "if") || ident_is(tokens.get(i - 1), "while"));
                let mut k = j + 1;
                let mut conditional = false;
                while k < tokens.len() && !is_punct(tokens.get(k), ';') {
                    if guarded
                        && matches!(&tokens[k], TokenTree::Group(g) if g.delimiter() == Delimiter::Brace)
                    {
                        break;
                    }
                    conditional |=
                        ident_is(tokens.get(k), "if") || ident_is(tokens.get(k), "match");
                    if !conditional && ident_is(tokens.get(k), "else") {
                        break;
                    }
                    k += 1;
                }
                let line = tokens[i].span().start().line;
                out.push((names, tokens[j + 1..k].to_vec(), line));
                // the value itself may hold further lets (a block); they were read above
                i = k;
                continue;
            }
        }
        i += 1;
    }
}

/// The bindings of a file that carry a secret by flow, to a fixed point.
fn carried(tokens: &[TokenTree]) -> BTreeSet<String> {
    let mut all = Vec::new();
    lets(tokens, &mut all);
    let mut carried = BTreeSet::new();
    loop {
        let fresh: Vec<String> = {
            let taint = Taint { carried: &carried };
            all.iter()
                .filter(|(_, value, _)| taint.carries(value).is_some())
                .flat_map(|(names, _, _)| names.iter())
                .filter(|n| !carried.contains(*n))
                .cloned()
                .collect()
        };
        if fresh.is_empty() {
            return carried;
        }
        carried.extend(fresh);
    }
}

/// Every place a Rust stream builds a command line out of a secret: `(line, value, sink)`.
fn rust_sinks(tokens: &[TokenTree], taint: &Taint<'_>, out: &mut Vec<(usize, String, String)>) {
    let group_after = |k: usize| match tokens.get(k) {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => Some(g),
        _ => None,
    };
    for (i, t) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = t {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            rust_sinks(&inner, taint, out);
        }
        let TokenTree::Ident(id) = t else { continue };
        let word = id.to_string();
        let line = id.span().start().line;
        let after_dot = i > 0 && is_punct(tokens.get(i - 1), '.');
        let after_fn = i > 0 && ident_is(tokens.get(i - 1), "fn");
        let read = |g: &proc_macro2::Group| {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            taint.carries(&inner)
        };
        let mut hit = |sink: String, g: &proc_macro2::Group| {
            if let Some(value) = read(g) {
                out.push((line, value, sink));
            }
        };
        // .arg(x), .args(x)
        if after_dot && (word == "arg" || word == "args") {
            if let Some(g) = group_after(i + 1) {
                hit(format!(".{word}"), g);
            }
            continue;
        }
        // Command::new(x)
        if word == "Command"
            && is_punct(tokens.get(i + 1), ':')
            && is_punct(tokens.get(i + 2), ':')
            && ident_is(tokens.get(i + 3), "new")
        {
            if let Some(g) = group_after(i + 4) {
                hit("Command::new".into(), g);
            }
            continue;
        }
        // argv.push(x), args.extend(x), cmd.insert(i, x)
        if !after_dot && argv_named(&word) && is_punct(tokens.get(i + 1), '.') {
            if let Some(TokenTree::Ident(m)) = tokens.get(i + 2) {
                let m = m.to_string();
                if matches!(m.as_str(), "push" | "extend" | "insert" | "append") {
                    if let Some(g) = group_after(i + 3) {
                        hit(format!("{word}.{m}"), g);
                    }
                }
            }
            continue;
        }
        // run(&[..]), run_command(&argv, stdin): the argument list is the first argument
        if !after_dot && !after_fn && RUNNERS.contains(&word.as_str()) {
            if let Some(g) = group_after(i + 1) {
                let first: Vec<TokenTree> = g
                    .stream()
                    .into_iter()
                    .take_while(|t| !matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
                    .collect();
                if let Some(value) = taint.carries(&first) {
                    out.push((line, value, word.clone()));
                }
            }
        }
    }
}

/// The `let argv = <value>` bindings of a command line built out of a secret.
fn rust_argv_lets(tokens: &[TokenTree], taint: &Taint<'_>, out: &mut Vec<(usize, String, String)>) {
    let mut all = Vec::new();
    lets(tokens, &mut all);
    for (names, value, line) in all {
        if let Some(name) = names.iter().find(|n| argv_named(n)) {
            if let Some(v) = taint.carries(&value) {
                out.push((line, v, format!("let {name}")));
            }
        }
    }
}

/// The body of every `fn` of a stream, at every depth of `impl`, `mod` and `trait` blocks. A
/// function is the scope of a flow: a name filled from a secret in one function says nothing
/// about the same name in another. A function nested in a body is part of that body.
fn fn_bodies(tokens: &[TokenTree], out: &mut Vec<Vec<TokenTree>>) {
    let mut i = 0;
    while i < tokens.len() {
        if ident_is(tokens.get(i), "fn") && matches!(tokens.get(i + 1), Some(TokenTree::Ident(_))) {
            let mut j = i + 2;
            while j < tokens.len() && !is_punct(tokens.get(j), ';') {
                if let TokenTree::Group(g) = &tokens[j] {
                    if g.delimiter() == Delimiter::Brace {
                        out.push(g.stream().into_iter().collect());
                        break;
                    }
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            fn_bodies(&inner, out);
        }
        i += 1;
    }
}

/// Every finding of one Rust source. A source that does not lex is an error, never a pass.
pub(crate) fn scan_rust(path: &str, text: &str) -> Result<Vec<SecretArgvFinding>, String> {
    let stream = TokenStream::from_str(text).map_err(|e| format!("{path}: does not lex: {e}"))?;
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut bodies = Vec::new();
    fn_bodies(&tokens, &mut bodies);
    let mut hits = Vec::new();
    for body in &bodies {
        let carried = carried(body);
        let taint = Taint { carried: &carried };
        rust_sinks(body, &taint, &mut hits);
        rust_argv_lets(body, &taint, &mut hits);
    }
    Ok(findings(path, "rust", text, hits))
}

// ------------------------------------------------------------------------------ shell

/// The builtins and keywords of the shells this repository writes: running one starts no
/// process, so its words reach no command line.
const BUILTINS: &[&str] = &[
    "echo",
    "printf",
    "test",
    "[",
    "[[",
    "]]",
    "export",
    "local",
    "declare",
    "readonly",
    "typeset",
    "read",
    "set",
    "unset",
    "case",
    "esac",
    "if",
    "then",
    "elif",
    "else",
    "fi",
    "while",
    "until",
    "do",
    "done",
    "for",
    "in",
    "return",
    "exit",
    ":",
    "true",
    "false",
    "shift",
    "cd",
    "pushd",
    "popd",
    "trap",
    "wait",
    "let",
    "eval",
    ".",
    "source",
    "break",
    "continue",
    "getopts",
    "hash",
    "type",
    "ulimit",
    "umask",
    "alias",
    "unalias",
    "mapfile",
    "readarray",
    "shopt",
    "caller",
    "printf%s",
    "!",
    "{",
    "}",
    "((",
    "function",
];

/// Words that run the command after them.
const PREFIXES: &[&str] = &[
    "command", "exec", "env", "nice", "nohup", "time", "sudo", "builtin", "then", "do", "else",
    "elif", "if", "while", "until", "!", "{",
];

fn credential_command() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"security[[:space:]]+find-(generic|internet)-password|\bgh[[:space:]]+auth[[:space:]]+token\b|\bpass[[:space:]]+show\b|\bop[[:space:]]+read\b|\bsecret-tool[[:space:]]+lookup\b|aws_secret_access_key|/credentials\b",
        )
        .expect("the credential-command expression compiles")
    })
}

/// The variables a shell word references: `$NAME`, `${NAME...}`.
fn references(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = word.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == b'$' {
            let mut j = i + 1;
            if j < b.len() && b[j] == b'{' {
                j += 1;
                // ${#name}, ${!name}
                while j < b.len() && (b[j] == b'#' || b[j] == b'!') {
                    j += 1;
                }
            }
            let start = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j > start && !b[start].is_ascii_digit() {
                out.push(word[start..j].to_string());
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

/// Is a shell variable the name of a secret? The same words as a Rust name.
fn shell_secret_named(name: &str) -> bool {
    secret_named(name)
}

/// One logical line of a shell unit: its first line and its text, continuations joined and
/// here-document bodies (a child's standard input) left out.
fn logical_lines(text: &str) -> Vec<(usize, String)> {
    static HEREDOC: OnceLock<Regex> = OnceLock::new();
    let heredoc = HEREDOC.get_or_init(|| {
        Regex::new(r#"<<-?[[:space:]]*['"]?([A-Za-z_][A-Za-z0-9_]*)['"]?"#)
            .expect("the here-document expression compiles")
    });
    let mut out = Vec::new();
    let mut pending: Option<(usize, String)> = None;
    let mut until: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let n = n + 1;
        if let Some(word) = &until {
            if raw.trim() == word {
                until = None;
            }
            continue;
        }
        let (start, mut joined) = pending.take().unwrap_or((n, String::new()));
        joined.push_str(raw);
        if let Some(stripped) = joined.strip_suffix('\\') {
            pending = Some((start, format!("{stripped} ")));
            continue;
        }
        // a here-string is not a here-document
        let scan = joined.replace("<<<", "   ");
        if let Some(c) = heredoc.captures(&scan) {
            until = Some(c[1].to_string());
        }
        out.push((start, joined));
    }
    if let Some(p) = pending {
        out.push(p);
    }
    out
}

/// The simple commands of one logical line: every pipeline element, list element and command
/// substitution apart, each a string of words. A command substitution's own text is one of
/// them, and stands as `$(…)` in the command it is part of.
fn simple_commands(line: &str) -> Vec<String> {
    enum Frame {
        Cmd(String),
        Dq,
    }
    fn buffer(stack: &mut [Frame]) -> &mut String {
        for f in stack.iter_mut().rev() {
            if let Frame::Cmd(b) = f {
                return b;
            }
        }
        unreachable!("the bottom frame is a command")
    }
    let cmd_depth = |stack: &[Frame]| stack.iter().filter(|f| matches!(f, Frame::Cmd(_))).count();
    let chars: Vec<char> = line.chars().collect();
    let mut stack = vec![Frame::Cmd(String::new())];
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let in_dq = matches!(stack.last(), Some(Frame::Dq));
        if c == '\\' {
            let b = buffer(&mut stack);
            b.push(c);
            if let Some(n) = next {
                b.push(n);
            }
            i += 2;
            continue;
        }
        if c == '$' && next == Some('(') && chars.get(i + 2) != Some(&'(') {
            stack.push(Frame::Cmd(String::new()));
            i += 2;
            continue;
        }
        if in_dq {
            if c == '"' {
                stack.pop();
            }
            buffer(&mut stack).push(c);
            i += 1;
            continue;
        }
        match c {
            '\'' => {
                let b = buffer(&mut stack);
                b.push(c);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    b.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    b.push('\'');
                }
            }
            '"' => {
                buffer(&mut stack).push(c);
                stack.push(Frame::Dq);
            }
            '#' if buffer(&mut stack)
                .chars()
                .last()
                .is_none_or(|l| l.is_whitespace()) =>
            {
                break;
            }
            '$' if next == Some('{') => {
                let b = buffer(&mut stack);
                while i < chars.len() {
                    b.push(chars[i]);
                    if chars[i] == '}' {
                        break;
                    }
                    i += 1;
                }
            }
            ')' if cmd_depth(&stack) > 1 => {
                // the end of a command substitution: its text is a command of its own
                while matches!(stack.last(), Some(Frame::Dq)) {
                    stack.pop();
                }
                if let Some(Frame::Cmd(b)) = stack.pop() {
                    out.push(b);
                }
                buffer(&mut stack).push_str("$(…)");
            }
            '|' | ';' | '&' | '(' | ')' | '`' => {
                let b = std::mem::take(buffer(&mut stack));
                out.push(b);
            }
            _ => buffer(&mut stack).push(c),
        }
        i += 1;
    }
    for f in stack {
        if let Frame::Cmd(b) = f {
            out.push(b);
        }
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

/// The words of a simple command, split on blanks outside quotes.
fn words(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                cur.push(c);
            }
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                cur.push(c);
            }
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn assignment(word: &str) -> Option<(&str, &str)> {
    let eq = word.find('=')?;
    let name = word[..eq].trim_end_matches('+');
    let valid = !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then(|| (name, &word[eq + 1..]))
}

/// The functions a shell unit defines: `name() {` and `function name`.
fn functions(text: &str, into: &mut BTreeSet<String>) {
    static DEF: OnceLock<Regex> = OnceLock::new();
    let def = DEF.get_or_init(|| {
        Regex::new(
            r"(?m)^[[:space:]]*(function[[:space:]]+)?([A-Za-z_][A-Za-z0-9_:.-]*)[[:space:]]*\(\)",
        )
        .expect("the function expression compiles")
    });
    for c in def.captures_iter(text) {
        into.insert(c[2].to_string());
    }
}

/// One shell command, judged: the program it runs and the first argument that carries a
/// secret, when it runs one and an argument does.
fn shell_sink(
    command: &str,
    tainted: &BTreeSet<String>,
    defined: &BTreeSet<String>,
) -> Option<(String, String)> {
    let ws = words(command);
    let mut i = 0;
    // assignments before the command are its environment, not its argv
    while i < ws.len() && (assignment(&ws[i]).is_some() || PREFIXES.contains(&ws[i].as_str())) {
        if ws[i] == "env" || ws[i] == "sudo" || ws[i] == "nice" {
            i += 1;
            while i < ws.len() && ws[i].starts_with('-') {
                // the options that take a value
                let takes = matches!(ws[i].as_str(), "-u" | "-n" | "-g" | "-C");
                i += if takes { 2 } else { 1 };
            }
            continue;
        }
        i += 1;
    }
    let program = ws
        .get(i)?
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string();
    if program.is_empty()
        || BUILTINS.contains(&program.as_str())
        || defined.contains(&program)
        || program.starts_with("$(")
    {
        return None;
    }
    let mut skip_next = false;
    for w in &ws[i + 1..] {
        if skip_next {
            skip_next = false;
            continue;
        }
        // a here-string is the child's standard input, a redirection target is a file
        if w.starts_with("<<<")
            || w.starts_with('<')
            || w.starts_with('>')
            || w.contains(">&")
            || w.starts_with("2>")
            || w.starts_with("&>")
        {
            let bare = matches!(w.as_str(), "<<<" | "<" | ">" | ">>" | "2>" | "2>>" | "&>");
            skip_next = bare;
            continue;
        }
        if let Some(v) = references(w).into_iter().find(|v| tainted.contains(v)) {
            return Some((v, program));
        }
    }
    None
}

/// Every finding of one shell unit. `defined` is every function the scanned shell defines.
pub(crate) fn scan_shell(
    path: &str,
    text: &str,
    defined: &BTreeSet<String>,
) -> Vec<SecretArgvFinding> {
    let lines = logical_lines(text);
    let commands: Vec<(usize, &str, Vec<String>)> = lines
        .iter()
        .map(|(n, l)| (*n, l.as_str(), simple_commands(l)))
        .collect();
    // the variables named as secrets, and those a flow fills from one, to a fixed point
    let mut tainted: BTreeSet<String> = BTreeSet::new();
    for (_, _, cmds) in &commands {
        for c in cmds {
            for w in words(c) {
                for r in references(&w) {
                    if shell_secret_named(&r) {
                        tainted.insert(r);
                    }
                }
                if let Some((name, _)) = assignment(&w) {
                    if shell_secret_named(name) {
                        tainted.insert(name.to_string());
                    }
                }
            }
        }
    }
    loop {
        let before = tainted.len();
        for (_, line, cmds) in &commands {
            let from_command = credential_command().is_match(line);
            for c in cmds {
                for w in words(c) {
                    let Some((name, value)) = assignment(&w) else {
                        continue;
                    };
                    let carries = references(value).iter().any(|r| tainted.contains(r))
                        || (from_command && value.contains("$(…)"));
                    if carries && !tainted.contains(name) {
                        tainted.insert(name.to_string());
                    }
                }
            }
        }
        if tainted.len() == before {
            break;
        }
    }
    let mut hits = Vec::new();
    for (n, _, cmds) in &commands {
        for c in cmds {
            if let Some((value, program)) = shell_sink(c, &tainted, defined) {
                hits.push((*n, format!("${value}"), program));
            }
        }
    }
    findings(path, "shell", text, hits)
}

// ------------------------------------------------------------------------------ the tree

/// The findings of one file, a line that declares why left out, one per line and sink.
fn findings(
    path: &str,
    language: &str,
    text: &str,
    hits: Vec<(usize, String, String)>,
) -> Vec<SecretArgvFinding> {
    let lines: Vec<&str> = text.lines().collect();
    let declared = |n: usize| {
        let has = |k: usize| {
            k >= 1
                && lines.get(k - 1).is_some_and(|l| {
                    l.find(MARKER)
                        .is_some_and(|at| !l[at + MARKER.len()..].trim().is_empty())
                })
        };
        has(n) || has(n.wrapping_sub(1))
    };
    let mut seen: BTreeMap<(usize, String), SecretArgvFinding> = BTreeMap::new();
    for (line, value, sink) in hits {
        if declared(line) {
            continue;
        }
        seen.entry((line, sink.clone()))
            .or_insert_with(|| SecretArgvFinding {
                path: path.to_string(),
                line,
                language: language.to_string(),
                value,
                sink,
                rule: RULE.to_string(),
                remedy: REMEDY.to_string(),
            });
    }
    seen.into_values().collect()
}

/// The Rust sources of the tree as git would commit them, `test/` left out (a fixture there
/// is a deliberate example of what is refused).
fn rust_sources(root: &Path) -> Result<Vec<String>, String> {
    let out = crate::git::read_only(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ])
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let paths: BTreeSet<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .filter(|p| !p.starts_with("test/"))
        .collect();
    Ok(paths.into_iter().collect())
}

/// Check the tree at `root`: every Rust source and every shell unit.
pub(crate) fn check(root: &Path) -> SecretArgvReport {
    let unmeasured = |reason: String| SecretArgvReport {
        measured: false,
        reason: Some(reason),
        rule: RULE.to_string(),
        rust_files: 0,
        shell_files: 0,
        findings: Vec::new(),
        passes: false,
    };
    let rust = match rust_sources(root) {
        Ok(r) => r,
        Err(e) => return unmeasured(e),
    };
    let shell = match crate::automation::enumerate(root) {
        Ok(s) => s,
        Err(e) => return unmeasured(e),
    };
    let mut all = Vec::new();
    let mut read = 0;
    for path in &rust {
        // a file listed and gone (deleted, not yet staged) has nothing to say
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        read += 1;
        match scan_rust(path, &text) {
            Ok(f) => all.extend(f),
            Err(e) => return unmeasured(e),
        }
    }
    let texts: Vec<(String, String)> = shell
        .iter()
        .filter_map(|u| {
            std::fs::read_to_string(root.join(&u.path))
                .ok()
                .map(|t| (u.path.clone(), t))
        })
        .collect();
    let mut defined = BTreeSet::new();
    for (_, t) in &texts {
        functions(t, &mut defined);
    }
    for (path, t) in &texts {
        all.extend(scan_shell(path, t, &defined));
    }
    let passes = all.is_empty();
    SecretArgvReport {
        measured: true,
        reason: None,
        rule: RULE.to_string(),
        rust_files: read,
        shell_files: texts.len(),
        findings: all,
        passes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust(src: &str) -> Vec<(usize, String, String)> {
        scan_rust("x.rs", src)
            .unwrap()
            .into_iter()
            .map(|f| (f.line, f.value, f.sink))
            .collect()
    }

    fn shell(src: &str) -> Vec<(usize, String, String)> {
        scan_shell("x.sh", src, &BTreeSet::new())
            .into_iter()
            .map(|f| (f.line, f.value, f.sink))
            .collect()
    }

    #[test]
    fn a_name_is_a_secret_by_its_words_and_not_beside_another_name() {
        for yes in [
            "secret",
            "api_token",
            "GH_TOKEN",
            "password",
            "AWS_SECRET_ACCESS_KEY",
            "api_key",
            "private_key",
            "credentials",
            "passphrase",
            "apikey",
        ] {
            assert!(secret_named(yes), "{yes}");
        }
        for no in [
            "secret_path",
            "token_file",
            "access_key_id",
            "redact_secrets",
            "is_secret",
            "tokens",
            "key",
            "MJ_SESSION_KEY",
            "monkey",
            "secretary",
            "credential_dir",
            "",
        ] {
            assert!(!secret_named(no), "{no}");
        }
        assert!(shell_secret_named("GH_TOKEN") && shell_secret_named("secret"));
        assert!(!shell_secret_named("token"), "a parser's token");
        assert!(shell_secret_named("api_token"));
        assert!(!secret_named("token") && secret_named("TOKEN"));
        assert!(!shell_secret_named("path"));
    }

    /// The shape I3996 removed from cache_provision.rs: a secret read from the credential
    /// file and pushed onto the argv of `mc admin user add`.
    #[test]
    fn the_provisioning_shape_before_i3996_is_refused_and_after_it_passes() {
        let before = r#"
fn step(cred: &Path) {
    let Some(s) = secret_of(&cred) else { return };
    let mut argv = mcv("admin user add vtminio mesh-cache-mbp");
    argv.push(s.clone());
    push(report, line, argv, Some(&s));
}
"#;
        assert_eq!(rust(before), [(5, "s".into(), "argv.push".into())]);
        let after = r#"
fn step(cred: &Path) {
    let Some(s) = secret_of(&cred) else { return };
    let argv = mcv("admin user add vtminio mesh-cache-mbp");
    push(report, line, argv, Some(&s));
}
"#;
        assert_eq!(rust(after), []);
    }

    #[test]
    fn every_rust_sink_is_seen_and_the_environment_and_stdin_are_not_sinks() {
        let src = r#"
fn f(api_token: &str) {
    Command::new("gh").arg("api").arg(api_token);
    let t = std::env::var("GH_TOKEN").unwrap();
    let header = format!("Authorization: Bearer {t}");
    Command::new("curl").args(["-H", &header]);
    Command::new(password_of(x));
    let argv = vec!["mc".to_string(), t.clone()];
    run(&["chown".into(), secret.clone()]);
    cmd.insert(0, cfg.password.clone());
    Command::new("mc").env("MC_HOST_x", &t).stdin(Stdio::piped());
    Command::new("mc").arg(path).arg(&token_file);
}
"#;
        let got = rust(src);
        let sinks: Vec<&str> = got.iter().map(|(_, _, s)| s.as_str()).collect();
        assert_eq!(
            sinks,
            [
                ".arg",
                ".args",
                "Command::new",
                "let argv",
                "run",
                "cmd.insert"
            ],
            "{got:?}"
        );
        assert!(got.iter().all(|(l, _, _)| *l != 11 && *l != 12), "{got:?}");
        assert_eq!(
            got[1].1, "header",
            "the flow is followed through the format"
        );
    }

    #[test]
    fn a_rust_line_that_says_why_is_not_a_finding_and_a_bare_marker_is_not_a_reason() {
        let src = "fn f() {\n    // secret-argv: a fixture's placeholder\n    Command::new(\"x\").arg(private_key);\n    Command::new(\"x\").arg(secret_key); // secret-argv:\n}\n";
        let got = rust(src);
        assert_eq!(got, [(4, "secret_key".into(), ".arg".into())]);
    }

    #[test]
    fn a_rust_source_that_does_not_lex_is_an_error() {
        assert!(scan_rust("bad.rs", "fn f() { \"unterminated }").is_err());
    }

    #[test]
    fn a_fn_named_run_is_a_definition_and_a_method_named_arg_on_a_field_is_not_a_push() {
        assert_eq!(
            rust("pub fn run(secret: &str) {}\nfn g() { x.argv.push(secret); }"),
            []
        );
    }

    #[test]
    fn every_shell_sink_is_seen_and_builtins_env_and_stdin_are_not() {
        let src = r#"#!/bin/sh
curl -H "Authorization: Bearer $GH_TOKEN" https://api.invalid
key=$(security find-generic-password -s x -w)
mc alias set x http://h access "$key"
printf '%s\n' "$GH_TOKEN" | mc admin user add x id
GH_TOKEN="$GH_TOKEN" gh api /user
mc admin user add x id <<<"$key"
echo "$secret" > "$file"
if gh api -f "token=$API_TOKEN"; then :; fi
out="$(curl -u "user:$password" https://h.invalid)"
cat <<EOF
$GH_TOKEN
EOF
env -u X curl --data "$secret" \
  https://h.invalid
"#;
        let got = shell(src);
        assert_eq!(
            got,
            [
                (2, "$GH_TOKEN".into(), "curl".into()),
                (4, "$key".into(), "mc".into()),
                (9, "$API_TOKEN".into(), "gh".into()),
                (10, "$password".into(), "curl".into()),
                (14, "$secret".into(), "curl".into()),
            ]
        );
    }

    #[test]
    fn a_shell_function_is_not_an_external_command_and_a_comment_is_not_code() {
        let mut defined = BTreeSet::new();
        functions("mj_key() {\n  :\n}\nfunction other {\n}\n", &mut defined);
        assert!(defined.contains("mj_key"));
        let src = "mj_key \"$GH_TOKEN\"\n# curl -u \"$password\"\ncurl x # \"$password\"\n";
        assert!(scan_shell("x.sh", src, &defined).is_empty());
    }

    #[test]
    fn a_shell_line_that_says_why_is_not_a_finding() {
        let src =
            "# secret-argv: a fixture's placeholder, not a credential\ncurl -u \"$password\" h\n";
        assert_eq!(shell(src), []);
    }

    #[test]
    fn the_words_and_references_of_a_command_are_read_as_the_shell_reads_them() {
        assert_eq!(words("a 'b c' \"d $e\" f"), ["a", "'b c'", "\"d $e\"", "f"]);
        assert_eq!(references("${#TOKEN}x$A_B$1\\$NOT"), ["TOKEN", "A_B"]);
        assert_eq!(assignment("A+=1"), Some(("A", "1")));
        assert_eq!(assignment("--x=1"), None);
        assert_eq!(
            simple_commands("a | b; c && d $(e \"$(f)\") 'g;h'"),
            ["a ", " b", " c ", "f", "e \"$(…)\"", " d $(…) 'g;h'"]
        );
        assert_eq!(logical_lines("a \\\nb\nc").len(), 2);
    }

    #[test]
    fn this_repository_hands_no_secret_to_a_command_line() {
        // the crate's own tree, read the way the gate reads it
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = check(&root);
        assert!(report.measured, "{:?}", report.reason);
        assert!(
            report.rust_files > 100 && report.shell_files > 10,
            "{report:?}"
        );
        assert!(report.passes, "{:#?}", report.findings);
    }

    #[test]
    fn a_tree_git_cannot_read_is_unmeasured_and_never_a_pass() {
        let dir = tempfile::tempdir().unwrap();
        let report = check(dir.path());
        assert!(!report.measured && !report.passes);
        assert!(report.reason.is_some());
    }
}

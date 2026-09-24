//! Claim governance for token economics: no number about tokens, context or cost reaches a
//! reader unless the evidence produced it.
//!
//! Three checks, all deterministic and all run by `majordomus economics check`:
//!
//! - **Bound claims.** The methodology binds claims of `docs/CLAIMS.yaml` to metrics. A
//!   bound claim may be `guaranteed` only while its metric stands where the binding says —
//!   `verified` for sampled live evidence meeting the publication rule, `measured` for
//!   deterministic evidence — and while every suite behind that metric is current. Evidence
//!   that goes stale takes the guarantee with it.
//! - **Unbound claims.** A `guaranteed` claim whose sentence says that tokens, context or
//!   cost go down has to be bound to a metric, or nothing would ever check it. A claim id
//!   that appears twice is refused too: a binding, a page and a gate would each read
//!   whichever copy they found first.
//! - **Unsupported quantities in prose.** Every hand-written document, page and claim
//!   sentence is scanned for a quantity (a percentage, "N times fewer", "half the tokens") in
//!   the same sentence as a word of the economics vocabulary. Repository prose is
//!   hard-wrapped, so the scan reads paragraphs, not lines: a sentence wrapped over two lines
//!   is one sentence. Numbers about tokens are generated from evidence into generated
//!   artifacts; typed by hand, they are refused.
//!
//! The check fails closed. Declarations it cannot read, a claims matrix it cannot parse and
//! a repository git cannot list are findings, never an empty and therefore clean result.
//!
//! ```
//! use majordomus_cli::economics::claims::unsupported_quantity;
//! assert!(unsupported_quantity("Majordomus saves 50% of tokens.").is_some());
//! assert!(unsupported_quantity("Majordomus cuts cost by 3x.").is_some());
//! assert!(unsupported_quantity("The suite takes 50% longer on Linux.").is_none(), "no economics word");
//! assert!(unsupported_quantity("The context budget is 300 lines.").is_none(), "no quantity");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{EconomicsFreshness, EconomicsMetric, EconomicsMetricStatus, EconomicsSummary};

/// Words that make a quantity a statement about token economics.
const VOCABULARY: &[&str] = &[
    "token", "context", "cost", "costs", "spend", "spending", "cheaper", "saving", "savings",
    "save", "saves", "bill", "billing", "usage", "prompt",
];

/// Words that say something went down. With the vocabulary they make a sentence a savings
/// claim: a fraction word counts as a quantity only then, and a guaranteed claim that says
/// it has to be bound to a metric. The inflections past the base list (`saved`, `lowers`,
/// `halves`) are here because "it halves the token bill" says what "it cuts" says.
const SAVING: &[&str] = &[
    "reduce",
    "reduces",
    "reduced",
    "reducing",
    "reduction",
    "save",
    "saves",
    "saved",
    "saving",
    "savings",
    "cheaper",
    "fewer",
    "less",
    "lower",
    "lowers",
    "lowered",
    "cuts",
    "cut",
    "cutting",
    "halve",
    "halves",
    "halved",
    "halving",
];

/// Multiplier and fraction words. "The other half of the context" is not a claim, and
/// "runs twice ... the provider's usage" is not one either: these are quantities only in a
/// sentence that also says something went down, and only within [`NEAR`] words of a word of
/// the vocabulary.
const FRACTIONS: &[&str] = &[
    "half", "halve", "halves", "halved", "halving", "twice", "double", "doubled", "doubles",
    "triple", "tripled", "third", "thirds", "quarter", "quarters",
];

/// Fraction words that are also ordinals or nouns: after `the` they count ("the third
/// session", "the last quarter") rather than divide, and are not a quantity.
const ORDINALS: &[&str] = &["third", "quarter"];

/// How many words apart a fraction word and the vocabulary may stand and still be read as
/// one statement ("halves what a session spends").
const NEAR: usize = 6;

/// Characters that join the parts of a compound: `total-token` and `token-saving` are two
/// words each, and each part counts.
const DASHES: &[char] = &['-', '\u{2010}', '\u{2011}', '\u{2013}', '\u{2014}'];

/// Hand-written text the scan reads: prose, pages, site data and the provider templates the
/// instruction files are generated from. Generated files are skipped by provenance.
const PROSE: &[&str] = &[
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    "docs/*.md",
    "docs/**/*.md",
    "site/content-src/**",
    "site/data/*.toml",
    "site/templates/**",
    ".ai/repo/**/*.md",
    "share/providers/*.tmpl",
    ".ai/repo/providers/*.tmpl",
];

/// Directories written entirely by `scripts/generate-site-data`; their sources, under
/// `site/content-src/`, are scanned.
const GENERATED_DIRS: &[&str] = &["site/data/generated/", "site/content/"];

/// The manifest of every artifact `majordomus generate` writes, by path.
const MANIFEST: &str = "docs/generated/artifacts.json";

/// How a generator stamps the first line of what it writes.
const STAMPS: &[&str] = &[
    "<!-- generated by",
    "# GENERATED FILE",
    "<!-- GENERATED FILE",
    "// GENERATED FILE",
];

/// The claims matrix, whose claim sentences are prose too.
const CLAIMS: &str = "docs/CLAIMS.yaml";

/// One thing the check refuses. `detail` names the remedy, so a finding can be acted on
/// without reading the methodology. A finding about a claim or a file rather than a line
/// carries no `line`, and its JSON omits the key. The kinds:
///
/// - `unsupported_quantity`: a number typed into hand-written prose next to the economics
///   vocabulary.
/// - `claim_unsupported`: a claim binding that does not hold — the bound claim is missing
///   from `docs/CLAIMS.yaml`, or it is `guaranteed` while its metric does not stand where
///   the binding says or its evidence is not current.
/// - `claim_unbound`: a `guaranteed` claim says tokens, context or cost go down and no
///   binding ties it to a metric.
/// - `claim_duplicate`: one claim id appears more than once in `docs/CLAIMS.yaml`.
/// - `declarations_unreadable`: the benchmark declarations cannot be read, so no binding
///   can be shown to hold.
/// - `claims_unreadable`: `docs/CLAIMS.yaml` exists and cannot be read or parsed.
/// - `scan_failed`: the prose could not be listed or a file in it could not be read, so
///   nothing can be said to be clean.
///
/// ```
/// use majordomus_cli::economics::claims::EconomicsFinding;
/// let finding = EconomicsFinding {
///     kind: "claim_unsupported".into(),
///     path: "docs/CLAIMS.yaml".into(),
///     line: None,
///     detail: "set the claim to planned, or record the evidence".into(),
/// };
/// let json = serde_json::to_value(&finding).unwrap();
/// assert!(json.get("line").is_none(), "no line, no key");
/// assert_eq!(serde_json::from_value::<EconomicsFinding>(json).unwrap(), finding);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsFinding {
    /// `unsupported_quantity`, `claim_unsupported`, `claim_unbound`, `claim_duplicate`,
    /// `declarations_unreadable`, `claims_unreadable` or `scan_failed`.
    pub kind: String,
    /// Repository-relative path.
    pub path: String,
    /// 1-based line, when the finding is about a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// What was found, and what to do about it.
    pub detail: String,
}

/// The answer of `economics.check`, the same over the CLI, HTTP and MCP: how much was read
/// and every refusal. `ok` is true exactly when `findings` is empty. `scanned` counts the
/// hand-written files read, `docs/CLAIMS.yaml` included and generated files excluded;
/// `claims_checked` counts the methodology's claim bindings, whatever their status.
///
/// ```
/// use majordomus_cli::economics::claims::{check, EconomicsCheckReport};
/// let dir = tempfile::tempdir().unwrap();
/// let git = std::process::Command::new("git")
///     .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
///     .arg("-C").arg(dir.path()).args(["init", "-q"]).status().unwrap();
/// assert!(git.success());
/// let report: EconomicsCheckReport = check(dir.path());
/// assert!(report.ok && report.findings.is_empty(), "an empty repository claims nothing");
/// let json = serde_json::to_value(&report).unwrap();
/// assert_eq!(json["claims_checked"], 0);
/// assert_eq!(serde_json::from_value::<EconomicsCheckReport>(json).unwrap(), report);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsCheckReport {
    /// No finding.
    pub ok: bool,
    /// Files scanned.
    pub scanned: usize,
    /// Bound claims checked.
    pub claims_checked: usize,
    /// Every finding.
    pub findings: Vec<EconomicsFinding>,
}

/// The input of `economics.check`: nothing to narrow; the check is over the repository.
/// Unknown fields are refused rather than ignored, so a caller who tries to scope the check
/// to a path learns that it cannot be scoped instead of reading a partial verdict as whole.
///
/// ```
/// use majordomus_cli::economics::claims::EconomicsCheckInput;
/// let input: EconomicsCheckInput = serde_json::from_str("{}").unwrap();
/// assert_eq!(input, EconomicsCheckInput::default());
/// let narrowed = serde_json::from_str::<EconomicsCheckInput>(r#"{"path":"docs"}"#);
/// assert!(narrowed.is_err(), "the check is over the whole repository");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomicsCheckInput {}

/// Comparative words that make `3 x` or `3 ×` a multiplier rather than a count.
const COMPARATIVES: &[&str] = &[
    "fewer",
    "less",
    "cheaper",
    "smaller",
    "lower",
    "more",
    "faster",
    "reduction",
    "saving",
    "savings",
];

/// Where `inner`, a slice of `outer`, starts in it.
fn offset(outer: &str, inner: &str) -> usize {
    inner.as_ptr() as usize - outer.as_ptr() as usize
}

/// Apostrophes a possessive or a contraction is written with.
const APOSTROPHES: &[char] = &['\'', '\u{2019}'];

/// The plain words of a sentence with where each starts: whitespace-separated tokens split
/// into their parts at a dash, each part with surrounding punctuation trimmed, a possessive
/// `'s` dropped, and lowercased. The dash is split first, so `tokens—40%` still has its
/// word and `token's` is `token`. A part with other punctuation inside
/// (`validate:context`, `context/v1`, `a.b`, `v1`) is an identifier, not a word, and never
/// counts as vocabulary.
fn words(sentence: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for token in sentence.split_whitespace() {
        for part in token.split(|c: char| DASHES.contains(&c)) {
            let t = part.trim_matches(|c: char| !c.is_alphanumeric());
            let t = t
                .strip_suffix(['s', 'S'])
                .and_then(|x| x.strip_suffix(APOSTROPHES))
                .unwrap_or(t);
            if t.is_empty()
                || !t
                    .chars()
                    .all(|c| c.is_alphabetic() || APOSTROPHES.contains(&c))
            {
                continue;
            }
            out.push((offset(sentence, t), t.to_lowercase()));
        }
    }
    out
}

fn vocabulary(word: &str) -> bool {
    VOCABULARY
        .iter()
        .any(|v| word == *v || (word.starts_with(v) && word.len() <= v.len() + 1))
}

fn economic(sentence: &str) -> bool {
    words(sentence).iter().any(|(_, w)| vocabulary(w))
}

/// Words that say cost went down with nothing else in the sentence: "sessions are cheaper"
/// and "the savings are real" are about cost already.
const SAVING_ALONE: &[&str] = &["cheaper", "savings"];

/// A sentence that uses the economics vocabulary and says something went down. The two have
/// to be two words: "saves" is in both lists, and "Majordomus saves every decision" says
/// nothing about tokens, while "saves tokens" does. Only a word of [`SAVING_ALONE`] is both
/// at once.
fn saving_claim(sentence: &str) -> bool {
    let w = words(sentence);
    let saving: Vec<usize> = (0..w.len())
        .filter(|&k| SAVING.contains(&w[k].1.as_str()))
        .collect();
    let vocab: Vec<usize> = (0..w.len()).filter(|&k| vocabulary(&w[k].1)).collect();
    w.iter().any(|(_, x)| SAVING_ALONE.contains(&x.as_str()))
        || saving.iter().any(|s| vocab.iter().any(|v| v != s))
}

/// Abbreviations whose full stop does not end a sentence: "cuts tokens, e.g. by 40%" is one
/// sentence, and splitting it would hide the number from the word it is about.
const ABBREVIATIONS: &[&str] = &[
    "e.g.", "i.e.", "etc.", "vs.", "cf.", "approx.", "ca.", "incl.", "resp.", "viz.",
];

/// Whether the full stop at byte `i` of `text` closes one of the [`ABBREVIATIONS`] inside
/// a sentence. `etc.` ends as many sentences as it continues, so it continues one only when
/// the next word does not start with a capital.
fn abbreviation(text: &str, i: usize) -> bool {
    let word = text[..=i]
        .rsplit(|c: char| c.is_whitespace() || matches!(c, '(' | '[' | '"' | '*' | '_' | '`'))
        .next()
        .unwrap_or("")
        .to_lowercase();
    let capital_next = text[i + 1..]
        .trim_start()
        .chars()
        .next()
        .is_some_and(char::is_uppercase);
    ABBREVIATIONS.contains(&word.as_str()) && !(word == "etc." && capital_next)
}

/// The sentences of a text with where each starts: split after `.`, `!` or `?` followed by
/// a space, or by closing quotes, brackets or emphasis and then a space, except after an
/// abbreviation. A quantity and the vocabulary have to meet in one sentence to be a
/// statement about token economics.
fn sentence_spans(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if matches!(b[i], b'.' | b'!' | b'?') && !(b[i] == b'.' && abbreviation(text, i)) {
            let mut j = i + 1;
            while j < b.len() && matches!(b[j], b'"' | b'\'' | b')' | b']' | b'*' | b'_') {
                j += 1;
            }
            if b.get(j) == Some(&b' ') {
                out.push((start, &text[start..j]));
                start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out.push((start, &text[start..]));
    out
}

/// Whether the digit at `i` continues something else: a word, an identifier or a decimal.
/// Emphasis markers before a number (`**50%**`, `__50%__`) do not glue it to anything.
fn glued(chars: &[(usize, char)], i: usize) -> bool {
    let Some(&(_, prev)) = i.checked_sub(1).and_then(|k| chars.get(k)) else {
        return false;
    };
    if prev.is_alphanumeric() || prev == '.' {
        return true;
    }
    if prev == '_' {
        let mut k = i - 1;
        while k > 0 && matches!(chars[k - 1].1, '_' | '*') {
            k -= 1;
        }
        return k > 0 && chars[k - 1].1.is_alphanumeric();
    }
    false
}

/// The first numeric quantity in a sentence and where it starts: a number followed by `%`,
/// `％`, `percent`, `per cent`, `percentage points` or `pp`, or a multiplier (`3x`,
/// `3 × cheaper`, `2 times fewer`).
fn numeric_quantity(sentence: &str) -> Option<(usize, String)> {
    let chars: Vec<(usize, char)> = sentence.char_indices().collect();
    let at = |k: usize| chars.get(k).map(|&(_, c)| c);
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].1.is_ascii_digit() || glued(&chars, i) {
            i += 1;
            continue;
        }
        let start = i;
        while at(i).is_some_and(|c| c.is_ascii_digit())
            || (matches!(at(i), Some('.' | ',')) && at(i + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            i += 1;
        }
        let number: String = chars[start..i].iter().map(|&(_, c)| c).collect();
        let position = chars[start].0;
        let attached = at(i);
        let mut j = i;
        while matches!(at(j), Some('*' | '_')) {
            j += 1;
        }
        while at(j).is_some_and(char::is_whitespace) {
            j += 1;
        }
        let rest: String = chars[j..]
            .iter()
            .map(|&(_, c)| c)
            .collect::<String>()
            .to_lowercase();
        let next_word = |from: usize| -> String {
            rest.chars()
                .skip(from)
                .collect::<String>()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        };
        let ends_word = |len: usize| rest.chars().nth(len).is_none_or(|c| !c.is_alphanumeric());
        // `0 %}` closes a template tag; it is not a percentage
        if (rest.starts_with('%') && !rest[1..].starts_with('}')) || rest.starts_with('\u{ff05}') {
            return Some((position, format!("{number}%")));
        }
        if rest.starts_with("percentage point") {
            return Some((position, format!("{number} percentage points")));
        }
        if rest.starts_with("percent") {
            return Some((position, format!("{number} percent")));
        }
        if rest.starts_with("per cent") && ends_word(8) {
            return Some((position, format!("{number} per cent")));
        }
        if rest.starts_with("pp") && ends_word(2) {
            return Some((position, format!("{number} pp")));
        }
        if rest.starts_with('x') || rest.starts_with('×') {
            let glued_sign = matches!(attached, Some('x') | Some('×')) && ends_word(1);
            let compared = COMPARATIVES.contains(&next_word(1).as_str());
            if glued_sign || compared {
                return Some((position, format!("{number}x")));
            }
        }
        if rest.starts_with("times ") && COMPARATIVES.contains(&next_word(5).as_str()) {
            return Some((position, format!("{number} times")));
        }
    }
    None
}

/// The first multiplier or fraction word of a sentence that says something went down, with
/// a word of the vocabulary near it. "Both halves" and "two halves" are a noun, not a verb,
/// and "the third" is an ordinal, not a fraction.
fn fraction_quantity(sentence: &str) -> Option<(usize, String)> {
    let w = words(sentence);
    if !w.iter().any(|(_, x)| SAVING.contains(&x.as_str())) {
        return None;
    }
    let before = |k: usize| k.checked_sub(1).map(|p| w[p].1.as_str());
    let noun = |k: usize| {
        (w[k].1 == "halves" && matches!(before(k), Some("both" | "two")))
            || (ORDINALS.contains(&w[k].1.as_str()) && before(k) == Some("the"))
    };
    let near = |k: usize| {
        w[k.saturating_sub(NEAR)..(k + NEAR + 1).min(w.len())]
            .iter()
            .any(|(_, x)| vocabulary(x))
    };
    (0..w.len())
        .find(|&k| FRACTIONS.contains(&w[k].1.as_str()) && !noun(k) && near(k))
        .map(|k| w[k].clone())
}

/// The first quantity of an economics sentence, numeric or a word, and where it starts.
fn quantity(sentence: &str) -> Option<(usize, String)> {
    if !economic(sentence) {
        return None;
    }
    match (numeric_quantity(sentence), fraction_quantity(sentence)) {
        (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// The quantity in `line` that states something about token economics, if there is one: a
/// number followed by `%`, `％`, `percent`, `per cent`, `percentage points` or `pp`, or a
/// multiplier (`3x`, `3 × cheaper`, `2 times fewer`), in a sentence that also uses a plain
/// word of the economics vocabulary; or a multiplier or fraction word (`half`, `twice`,
/// `a third`) in a sentence that also says something went down. This reads one line as it
/// stands; [`prose_quantities`] reads a whole file, with wrapped sentences joined.
///
/// ```
/// use majordomus_cli::economics::claims::unsupported_quantity;
/// assert_eq!(unsupported_quantity("Uses 3x fewer tokens.").as_deref(), Some("3x"));
/// assert_eq!(unsupported_quantity("Cuts context by 60 percent.").as_deref(), Some("60 percent"));
/// assert_eq!(unsupported_quantity("Cuts total-token use by **50%**.").as_deref(), Some("50%"));
/// assert_eq!(unsupported_quantity("It halves the token bill.").as_deref(), Some("halves"));
/// // a fraction with nothing going down is not a claim
/// assert_eq!(unsupported_quantity("The other half of the context is rules."), None);
/// // the number and the vocabulary have to meet in one sentence
/// assert_eq!(unsupported_quantity("Coverage is 80%. Tokens are counted."), None);
/// // an identifier is not a word of the vocabulary
/// assert_eq!(unsupported_quantity("validate:context took 50% longer"), None);
/// ```
pub fn unsupported_quantity(line: &str) -> Option<String> {
    sentence_spans(line)
        .into_iter()
        .find_map(|(_, s)| quantity(s))
        .map(|(_, q)| q)
}

/// A quantity [`prose_quantities`] found: the 1-based line the quantity starts on, the
/// quantity as written, and whether it stands in a fenced code block. A quantity in a code
/// block is still a finding — a pasted benchmark output is a number about tokens a person
/// typed into the document — and the flag lets the finding say where it is.
///
/// ```
/// use majordomus_cli::economics::claims::{prose_quantities, ProseQuantity};
/// let found = prose_quantities("README.md", "```\ntokens: 40% fewer\n```\n");
/// assert_eq!(found, vec![ProseQuantity { line: 2, quantity: "40%".into(), code: true }]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProseQuantity {
    /// 1-based line the quantity starts on.
    pub line: usize,
    /// The quantity, as the finding names it (`40%`, `3x`, `half`).
    pub quantity: String,
    /// In a fenced code block.
    pub code: bool,
}

/// How a file's lines group into paragraphs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    /// Markdown and the templates written like it, HTML templates included.
    Markdown,
    /// TOML: every key starts a paragraph, and consecutive comment lines are one.
    Toml,
    /// YAML: only the `claim:` and `note:` values are prose.
    Yaml,
}

fn syntax(path: &str) -> Syntax {
    if path.ends_with(".toml") {
        Syntax::Toml
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        Syntax::Yaml
    } else {
        Syntax::Markdown
    }
}

/// Lines read as one piece of prose, each with its 1-based number and the text it
/// contributes.
struct Paragraph<'a> {
    lines: Vec<(usize, &'a str)>,
    code: bool,
}

/// A fence opening or closing a code block: its character and its length.
fn fence(t: &str) -> Option<(char, usize)> {
    let c = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

fn list_item(t: &str) -> bool {
    if let Some(rest) = t.strip_prefix(['-', '*', '+']) {
        return rest.starts_with(' ');
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))
}

fn heading(t: &str) -> bool {
    let rest = t.trim_start_matches('#');
    rest.len() < t.len() && (rest.is_empty() || rest.starts_with(' '))
}

fn thematic_break(t: &str) -> bool {
    t.chars().filter(|c| !c.is_whitespace()).count() >= 3
        && t.chars()
            .all(|c| matches!(c, '-' | '*' | '_' | '=') || c.is_whitespace())
}

fn key_line(t: &str, separator: char) -> bool {
    t.split_once(separator).is_some_and(|(k, v)| {
        let k = k.trim();
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '"'))
            && (separator == '=' || v.is_empty() || v.starts_with(' '))
    })
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The paragraphs of Markdown or TOML: consecutive non-blank lines joined. A fenced code
/// block is a paragraph of its own; a table row, a heading and a TOML table header are
/// each a paragraph of their own; a list item, a front-matter or TOML key and the first of
/// a run of TOML comments start a new one.
fn text_paragraphs(text: &str, syntax: Syntax) -> Vec<Paragraph<'_>> {
    let mut out = Vec::new();
    let mut current: Vec<(usize, &str)> = Vec::new();
    let mut current_comment = false;
    let mut code: Vec<(usize, &str)> = Vec::new();
    let mut open: Option<(char, usize)> = None;
    let mut front =
        syntax == Syntax::Markdown && text.lines().next().map(str::trim_end) == Some("---");
    fn flush<'a>(out: &mut Vec<Paragraph<'a>>, lines: &mut Vec<(usize, &'a str)>, code: bool) {
        if !lines.is_empty() {
            out.push(Paragraph {
                lines: std::mem::take(lines),
                code,
            });
        }
    }
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let t = raw.trim();
        if let Some((c, len)) = open {
            if fence(t)
                .is_some_and(|(c2, len2)| c2 == c && len2 >= len && t.chars().all(|x| x == c))
            {
                flush(&mut out, &mut code, true);
                open = None;
            } else if !t.is_empty() {
                code.push((n, t));
            }
            continue;
        }
        if front {
            if n > 1 && t == "---" {
                flush(&mut out, &mut current, false);
                front = false;
            } else if n > 1 && !t.is_empty() {
                if key_line(t, ':') || list_item(t) {
                    flush(&mut out, &mut current, false);
                }
                current.push((n, t));
            } else if t.is_empty() {
                flush(&mut out, &mut current, false);
            }
            continue;
        }
        if t.is_empty() {
            flush(&mut out, &mut current, false);
            continue;
        }
        if syntax == Syntax::Markdown {
            if let Some(f) = fence(t) {
                flush(&mut out, &mut current, false);
                open = Some(f);
                continue;
            }
            if thematic_break(t) {
                flush(&mut out, &mut current, false);
                continue;
            }
            if t.starts_with('|') || heading(t) {
                flush(&mut out, &mut current, false);
                out.push(Paragraph {
                    lines: vec![(n, t)],
                    code: false,
                });
                continue;
            }
            if list_item(t) {
                flush(&mut out, &mut current, false);
            }
            current.push((n, t));
            continue;
        }
        // TOML
        if let Some(comment) = t.strip_prefix('#') {
            let comment = comment.trim();
            if !current_comment || list_item(comment) {
                flush(&mut out, &mut current, false);
            }
            current_comment = true;
            if !comment.is_empty() {
                current.push((n, comment));
            }
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') && !t.contains('=') && !t.contains('"') {
            flush(&mut out, &mut current, false);
            current_comment = false;
            continue;
        }
        if current_comment || key_line(t, '=') {
            flush(&mut out, &mut current, false);
        }
        current_comment = false;
        current.push((n, t));
    }
    flush(&mut out, &mut current, false);
    flush(&mut out, &mut code, true);
    out
}

/// The paragraphs of YAML prose: each `claim:` and `note:` value, with the lines a plain or
/// block scalar continues on — the ones indented deeper than its key — joined to it.
fn yaml_paragraphs(text: &str) -> Vec<Paragraph<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let mut t = raw.trim_start();
        let mut key_indent = indent(raw);
        if let Some(rest) = t.strip_prefix("- ") {
            key_indent += 2 + indent(rest);
            t = rest.trim_start();
        }
        let value = t.strip_prefix("claim:").or_else(|| t.strip_prefix("note:"));
        let Some(value) = value else {
            i += 1;
            continue;
        };
        let value = value.trim();
        let mut paragraph = Vec::new();
        if !value.is_empty() && !value.chars().all(|c| matches!(c, '>' | '|' | '-' | '+')) {
            paragraph.push((i + 1, value));
        }
        i += 1;
        while i < lines.len() {
            let next = lines[i];
            let nt = next.trim();
            if nt.is_empty() || nt.starts_with('#') || indent(next) <= key_indent {
                break;
            }
            paragraph.push((i + 1, nt));
            i += 1;
        }
        if !paragraph.is_empty() {
            out.push(Paragraph {
                lines: paragraph,
                code: false,
            });
        }
    }
    out
}

/// Every sentence of a paragraph with a quantity in it, as the line the quantity starts on
/// and the quantity.
fn paragraph_quantities(p: &Paragraph<'_>) -> Vec<ProseQuantity> {
    let mut joined = String::new();
    let mut starts: Vec<(usize, usize)> = Vec::new();
    for &(n, text) in &p.lines {
        if !joined.is_empty() {
            joined.push(' ');
        }
        starts.push((joined.len(), n));
        joined.push_str(text);
    }
    sentence_spans(&joined)
        .into_iter()
        .filter_map(|(at, s)| quantity(s).map(|(within, q)| (at + within, q)))
        .map(|(position, quantity)| ProseQuantity {
            line: starts
                .iter()
                .rev()
                .find(|(start, _)| *start <= position)
                .map_or(p.lines[0].0, |&(_, n)| n),
            quantity,
            code: p.code,
        })
        .collect()
}

/// Every unsupported quantity in the text of the file at `path`, read paragraph by
/// paragraph so that a sentence hard-wrapped over several lines is read whole, with the line
/// each quantity starts on. How lines group is decided by the file's kind: Markdown and
/// templates join consecutive non-blank lines, with each fenced code block, table row and
/// heading a paragraph of its own and each list item starting a new one; TOML starts a
/// paragraph at every key; YAML reads only the `claim:` and `note:` values, with their
/// continuation lines. [`unsupported_quantity`] is the sentence-level test applied.
///
/// ```
/// use majordomus_cli::economics::claims::prose_quantities;
/// let text = "# Pitch\n\nWith Majordomus a session spends\n40% fewer tokens than without.\n";
/// let found = prose_quantities("docs/PITCH.md", text);
/// assert_eq!((found[0].line, found[0].quantity.as_str()), (4, "40%"), "the line it starts on");
/// // list items and table rows do not run into each other
/// let text = "- Tokens are counted\n- 40% of tests are slow\n\n| token |\n| 40% |\n";
/// assert!(prose_quantities("README.md", text).is_empty());
/// // a claim wrapped in YAML is one sentence too
/// let yaml = "claims:\n  - id: x\n    claim: Majordomus saves\n      40% of tokens\n    status: planned\n";
/// assert_eq!(prose_quantities("docs/CLAIMS.yaml", yaml)[0].line, 4);
/// ```
pub fn prose_quantities(path: &str, text: &str) -> Vec<ProseQuantity> {
    let paragraphs = match syntax(path) {
        Syntax::Yaml => yaml_paragraphs(text),
        s => text_paragraphs(text, s),
    };
    paragraphs.iter().flat_map(paragraph_quantities).collect()
}

/// The paths the generation manifest lists as generated artifacts. An absent or unreadable
/// manifest exempts nothing, which scans more rather than less.
fn generated_artifacts(root: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(MANIFEST))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| {
            v.get("artifacts").and_then(|a| a.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.get("path").and_then(|p| p.as_str()).map(str::to_string))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Whether a file is generated, decided by provenance and never by what its prose says:
/// listed in the generation manifest, under a directory the site generator owns entirely,
/// or stamped by its generator on its first line. A hand-written paragraph that mentions
/// "generated by" is still hand-written.
fn is_generated(path: &str, text: &str, manifest: &BTreeSet<String>) -> bool {
    let first = text
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches('\u{feff}')
        .trim_start();
    manifest.contains(path)
        || GENERATED_DIRS.iter().any(|g| path.starts_with(g))
        || STAMPS.iter().any(|s| first.starts_with(s))
}

fn scan_failed(path: &str, detail: String) -> EconomicsFinding {
    EconomicsFinding {
        kind: "scan_failed".into(),
        path: path.into(),
        line: None,
        detail,
    }
}

/// Read a file as text for the scan: `Ok(None)` when it is not there to claim anything or is
/// binary, and an error for anything else, which the scan reports rather than skips.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(bytes) if bytes.contains(&0) => Ok(None),
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Scan the repository's hand-written prose for unsupported quantities, and return how many
/// files were read with every finding.
///
/// Only tracked files are read: the prose set comes from `git ls-files`. A root git cannot
/// list is a `scan_failed` finding, not an empty and therefore clean scan; so is a tracked
/// file that exists and cannot be read. A generated file is skipped and not counted, and
/// "generated" is decided by provenance only: listed in `docs/generated/artifacts.json`,
/// under `site/data/generated/` or `site/content/`, or stamped by its generator on its first
/// line. A quantity in a fenced code block is still a finding, and its detail says it is in
/// one. In `docs/CLAIMS.yaml`, read whether tracked or not, only `claim:` and `note:` values
/// count. Files are read paragraph by paragraph ([`prose_quantities`]).
///
/// ```
/// use majordomus_cli::economics::claims::scan_prose;
/// use std::process::Command;
/// let dir = tempfile::tempdir().unwrap();
/// let root = dir.path();
/// std::fs::create_dir_all(root.join("docs/generated")).unwrap();
/// std::fs::write(root.join("README.md"), "Install it.\nIt saves 40% of tokens.\n").unwrap();
/// let stamp = "<!-- generated by majordomus economics -->\n";
/// std::fs::write(root.join("docs/generated/economics.md"), format!("{stamp}It saves 40% of tokens.\n")).unwrap();
/// std::fs::write(root.join("docs/generated/notes.md"), "It saves 40% of tokens.\n").unwrap();
/// let git = |a: &[&str]| {
///     Command::new("git")
///         .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
///         .arg("-C").arg(root).args(a).output().unwrap()
/// };
/// git(&["init", "-q"]);
/// git(&["add", "-A"]);
/// let (scanned, findings) = scan_prose(root);
/// assert_eq!(scanned, 2, "the stamped report is not hand-written prose");
/// let at: Vec<_> = findings.iter().map(|f| (f.path.as_str(), f.line)).collect();
/// assert_eq!(at, [("README.md", Some(2)), ("docs/generated/notes.md", Some(1))], "no stamp, no exemption");
///
/// let outside = tempfile::tempdir().unwrap();
/// let (_, findings) = scan_prose(outside.path());
/// assert_eq!(findings[0].kind, "scan_failed", "nothing listed is not nothing found");
/// ```
pub fn scan_prose(root: &Path) -> (usize, Vec<EconomicsFinding>) {
    let mut scanned = 0;
    let mut findings = Vec::new();
    match crate::git::ls_files_any(root, PROSE) {
        Err(e) => findings.push(scan_failed(
            ".",
            format!(
                "git could not list the tracked prose ({e}): nothing was scanned, so nothing can be said to be free of typed numbers about tokens"
            ),
        )),
        Ok(files) => {
            let manifest = generated_artifacts(root);
            for path in files {
                let text = match read_text(&root.join(&path)) {
                    Ok(Some(t)) => t,
                    Ok(None) => continue,
                    Err(e) => {
                        findings.push(scan_failed(&path, format!("{path} could not be read ({e}), so it was not scanned")));
                        continue;
                    }
                };
                if is_generated(&path, &text, &manifest) {
                    continue;
                }
                scanned += 1;
                for q in prose_quantities(&path, &text) {
                    findings.push(EconomicsFinding {
                        kind: "unsupported_quantity".into(),
                        path: path.clone(),
                        line: Some(q.line),
                        detail: format!(
                            "{} next to the economics vocabulary{}: numbers about tokens, context or cost are generated from recorded evidence (majordomus economics), never typed",
                            q.quantity,
                            if q.code { " in a code block" } else { "" }
                        ),
                    });
                }
            }
        }
    }
    // the claim sentences of the claims matrix are prose too
    match read_text(&root.join(CLAIMS)) {
        Ok(None) => {}
        Err(e) => findings.push(scan_failed(
            CLAIMS,
            format!("{CLAIMS} could not be read ({e}), so its claim sentences were not scanned"),
        )),
        Ok(Some(text)) => {
            scanned += 1;
            for q in prose_quantities(CLAIMS, &text) {
                findings.push(EconomicsFinding {
                    kind: "unsupported_quantity".into(),
                    path: CLAIMS.into(),
                    line: Some(q.line),
                    detail: format!(
                        "{} in a claim sentence next to the economics vocabulary: a claim about tokens is bound to a metric in the economics methodology instead",
                        q.quantity
                    ),
                });
            }
        }
    }
    (scanned, findings)
}

/// One claim of the claims matrix, as the checks need it.
struct Claim {
    id: String,
    status: String,
    sentence: String,
}

/// The claims matrix as the checks read it: the claims in order, and the line of every
/// `id:` in the claims list by id, for findings a person can jump to.
struct Matrix {
    claims: Vec<Claim>,
    lines: BTreeMap<String, Vec<usize>>,
}

/// Read `docs/CLAIMS.yaml`: `Ok(None)` when there is no claims matrix, an error when there is
/// one and it cannot be read or parsed.
fn read_claims(root: &Path) -> Result<Option<Matrix>, String> {
    let text = match std::fs::read_to_string(root.join(CLAIMS)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let map = crate::metadata::yaml::parse_mapping(&text)?;
    let field = |c: &serde_json::Value, k: &str| {
        c.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let claims = match map.get("claims") {
        None => Vec::new(),
        Some(v) => v
            .as_array()
            .ok_or("`claims` is not a list")?
            .iter()
            .map(|c| Claim {
                id: field(c, "id"),
                status: field(c, "status"),
                sentence: field(c, "claim"),
            })
            .collect(),
    };
    // Only the `id:` of a claim itself counts, at the indentation of the claims list's
    // items: an `id:` in a list nested inside a claim is someone else's.
    let mut lines: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut inside = false;
    let mut item: Option<usize> = None;
    for (i, raw) in text.lines().enumerate() {
        if !raw.starts_with([' ', '-', '#']) && !raw.trim().is_empty() {
            inside = raw.split(" #").next().unwrap_or("").trim_end() == "claims:";
            item = None;
            continue;
        }
        let mut t = raw.trim_start();
        let mut key = indent(raw);
        if let Some(rest) = t.strip_prefix("- ") {
            key += 2 + indent(rest);
            t = rest.trim_start();
            item.get_or_insert(key);
        }
        if let (true, Some(id)) = (inside && item == Some(key), t.strip_prefix("id:")) {
            let id = id
                .split(" #")
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(['\'', '"']);
            lines.entry(id.to_string()).or_default().push(i + 1);
        }
    }
    Ok(Some(Matrix { claims, lines }))
}

fn claim_finding(kind: &str, line: Option<usize>, detail: String) -> EconomicsFinding {
    EconomicsFinding {
        kind: kind.into(),
        path: CLAIMS.into(),
        line,
        detail,
    }
}

/// The status of a metric as its JSON spells it (`not_measured`), or `absent`.
fn status_name(metric: Option<&EconomicsMetric>) -> String {
    metric
        .and_then(|m| serde_json::to_value(m.status).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "absent".into())
}

/// The suites a metric rests on that are not current, with their freshness; every suite
/// the metric names has to be in the summary and current, and a metric that names none
/// rests on nothing.
fn not_current(metric: &EconomicsMetric, summary: &EconomicsSummary) -> Vec<String> {
    let names: Vec<&str> = metric
        .suite
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return vec!["no suite".into()];
    }
    names
        .into_iter()
        .filter_map(|n| match summary.suites.iter().find(|s| s.id == n) {
            Some(s) if s.freshness == EconomicsFreshness::Current => None,
            Some(s) => Some(format!(
                "{n} {}",
                serde_json::to_value(s.freshness)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default()
            )),
            None => Some(format!("{n} undeclared")),
        })
        .collect()
}

/// Check the claims matrix against the methodology's bindings and `summary`, and return how
/// many bindings the methodology declares with every finding.
///
/// A binding whose claim `docs/CLAIMS.yaml` does not have is a finding. A claim that is not
/// `guaranteed` promises nothing and is left alone. A guaranteed claim stands only while its
/// metric does — `verified` with a publishable verdict, or `measured` (or better) — and
/// while every suite behind the metric is current. A guaranteed claim whose sentence says
/// tokens, context or cost go down and that no binding names is `claim_unbound`, whether or
/// not a methodology is declared; a claim id used twice is `claim_duplicate`. Declarations
/// that cannot be read are `declarations_unreadable`, one finding per error, and a claims
/// matrix that cannot be parsed is `claims_unreadable`: the check fails closed.
///
/// ```
/// use majordomus_cli::economics::{claims::check_claims, summarize};
/// let dir = tempfile::tempdir().unwrap();
/// let summary = summarize(dir.path(), &Default::default());
/// let (checked, findings) = check_claims(dir.path(), &summary);
/// assert_eq!(checked, 0, "no methodology, no bindings");
/// assert!(findings.is_empty());
///
/// // with no methodology, a guaranteed savings claim has nothing to stand on
/// std::fs::create_dir(dir.path().join("docs")).unwrap();
/// let claims = "claims:\n  - id: cheap\n    claim: Majordomus reduces token spend\n    status: guaranteed\n";
/// std::fs::write(dir.path().join("docs/CLAIMS.yaml"), claims).unwrap();
/// let (_, findings) = check_claims(dir.path(), &summary);
/// assert_eq!((findings[0].kind.as_str(), findings[0].line), ("claim_unbound", Some(2)));
/// ```
pub fn check_claims(root: &Path, summary: &EconomicsSummary) -> (usize, Vec<EconomicsFinding>) {
    let mut findings = Vec::new();
    let bindings = match super::load(root) {
        Ok(Some(decl)) => Some(decl.methodology.claims),
        Ok(None) => Some(Vec::new()),
        Err(errors) => {
            let errors = if errors.is_empty() {
                vec!["the declarations could not be read".to_string()]
            } else {
                errors
            };
            for e in errors {
                let path = e
                    .split_once(": ")
                    .map(|(p, _)| p)
                    .filter(|p| p.starts_with(super::DIR))
                    .unwrap_or(super::DIR)
                    .to_string();
                findings.push(EconomicsFinding {
                    kind: "declarations_unreadable".into(),
                    path,
                    line: None,
                    detail: format!("{e}: the claim bindings cannot be read, so no bound claim can be shown to stand"),
                });
            }
            None
        }
    };
    let Matrix { claims, lines } = match read_claims(root) {
        Ok(Some(m)) => m,
        Ok(None) => Matrix {
            claims: Vec::new(),
            lines: BTreeMap::new(),
        },
        Err(e) => {
            findings.push(claim_finding(
                "claims_unreadable",
                None,
                format!("{CLAIMS} could not be read ({e}), so no claim in it can be checked"),
            ));
            return (bindings.map_or(0, |b| b.len()), findings);
        }
    };
    let line_of = |id: &str, nth: usize| lines.get(id).and_then(|l| l.get(nth)).copied();

    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &claims {
        let count = seen.entry(c.id.as_str()).or_default();
        *count += 1;
        if *count > 1 {
            findings.push(claim_finding(
                "claim_duplicate",
                line_of(&c.id, *count - 1),
                format!(
                    "claim id {} appears more than once in {CLAIMS}: a binding, a page and a gate would each read whichever copy they found first; give each claim its own id",
                    c.id
                ),
            ));
        }
    }

    let Some(bindings) = bindings else {
        return (0, findings);
    };
    let guaranteed = |id: &str| {
        claims
            .iter()
            .any(|c| c.id == id && c.status == "guaranteed")
    };
    for b in &bindings {
        if !claims.iter().any(|c| c.id == b.claim) {
            findings.push(claim_finding(
                "claim_unsupported",
                None,
                format!(
                    "the methodology binds claim {} to {}, and {CLAIMS} has no such claim",
                    b.claim, b.metric
                ),
            ));
            continue;
        }
        if !guaranteed(&b.claim) {
            continue;
        }
        let metric = summary.metrics.iter().find(|m| m.id == b.metric);
        let stale = metric.map(|m| not_current(m, summary)).unwrap_or_default();
        let stands = match (b.requires.as_str(), metric.map(|m| m.status)) {
            ("verified", Some(EconomicsMetricStatus::Verified)) => {
                summary.verdict.publishable && stale.is_empty()
            }
            (
                "measured",
                Some(EconomicsMetricStatus::Measured | EconomicsMetricStatus::Verified),
            ) => stale.is_empty(),
            _ => false,
        };
        if !stands {
            let mut why = Vec::new();
            if !stale.is_empty() {
                why.push(format!("evidence not current: {}", stale.join(", ")));
            }
            if b.requires == "verified" && !summary.verdict.publishable {
                why.push("verdict not publishable".to_string());
            }
            let why = if why.is_empty() {
                String::new()
            } else {
                format!(" ({})", why.join("; "))
            };
            findings.push(claim_finding(
                "claim_unsupported",
                line_of(&b.claim, 0),
                format!(
                    "claim {} is guaranteed, but its metric {} is {} and must be {} and current{why}: set the claim to planned, or record the evidence",
                    b.claim,
                    b.metric,
                    status_name(metric),
                    b.requires
                ),
            ));
        }
    }

    let mut reported = BTreeSet::new();
    for c in &claims {
        let bound = bindings.iter().any(|b| b.claim == c.id);
        let says_saving = sentence_spans(&c.sentence)
            .into_iter()
            .any(|(_, s)| saving_claim(s));
        if c.status == "guaranteed" && says_saving && !bound && reported.insert(c.id.as_str()) {
            findings.push(claim_finding(
                "claim_unbound",
                line_of(&c.id, 0),
                format!(
                    "claim {} is guaranteed and says tokens, context or cost go down, and no binding in {}/methodology.yaml ties it to a metric: bind it, or set it to planned",
                    c.id,
                    super::DIR
                ),
            ));
        }
    }
    (bindings.len(), findings)
}

/// Run every check over the repository at `root`: the prose scan of [`scan_prose`], and the
/// claims of [`check_claims`] against a summary computed afresh by
/// [`crate::economics::summarize`], never a cached one. It never fails as a call: what
/// cannot be read is a finding, and the report is `ok` exactly when no check found anything.
///
/// ```
/// use majordomus_cli::economics::claims::check;
/// let dir = tempfile::tempdir().unwrap();
/// let git = std::process::Command::new("git")
///     .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
///     .arg("-C").arg(dir.path()).args(["init", "-q"]).status().unwrap();
/// assert!(git.success());
/// std::fs::create_dir(dir.path().join("docs")).unwrap();
/// let claims = "claims:\n  - id: savings\n    claim: Majordomus saves 40% of tokens.\n";
/// std::fs::write(dir.path().join("docs/CLAIMS.yaml"), claims).unwrap();
/// let report = check(dir.path());
/// assert!(!report.ok);
/// assert_eq!(report.findings[0].kind, "unsupported_quantity");
/// assert_eq!(report.findings[0].line, Some(3));
/// ```
pub fn check(root: &Path) -> EconomicsCheckReport {
    let summary = super::summarize(root, &Default::default());
    let (scanned, mut findings) = scan_prose(root);
    let (claims_checked, more) = check_claims(root, &summary);
    findings.extend(more);
    EconomicsCheckReport {
        ok: findings.is_empty(),
        scanned,
        claims_checked,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shapes_a_savings_claim_takes_are_all_caught() {
        for line in [
            "Majordomus saves 50% of tokens",
            "cuts token spend by 44.2 %",
            "uses 3x fewer tokens",
            "3 × cheaper per task",
            "reduces context by 60 percent",
            "tokens: 2 times fewer",
            "Majordomus reduces the amount of context loaded into each coding session by 50%.",
            "| context reduction | 61.4% |",
            "total-token use falls by 30%",
            "a token-saving of 30%",
            "cuts context by 60 per cent",
            "token usage fell 12 percentage points",
            "token usage fell 12pp",
            "token usage fell 12 pp.",
            "saves 50％ of tokens",
            "saves **50%** of tokens",
            "saves __50%__ of tokens",
            "saves 50\u{a0}% of tokens",
            "halves the token bill",
            "context is cut to a third",
            "uses half the tokens, and fewer calls",
            "twice as many tokens are saved",
            "two thirds of the tokens are saved",
            "Majordomus cuts tokens, e.g. by 40% on bug fixes.",
            "the prompt's size falls 30%",
            "cost\u{2014}40% lower",
        ] {
            assert!(unsupported_quantity(line).is_some(), "missed: {line}");
        }
    }

    #[test]
    fn a_savings_claim_needs_a_word_of_each_list_unless_one_word_says_both() {
        assert!(saving_claim("Majordomus saves tokens"));
        assert!(saving_claim(
            "The compiler reduces the context a session loads"
        ));
        assert!(
            saving_claim("Sessions are cheaper"),
            "cheaper is about cost already"
        );
        assert!(saving_claim("The savings are real"));
        assert!(
            !saving_claim("Majordomus saves every decision it records"),
            "saves alone says nothing about tokens"
        );
        assert!(
            !saving_claim("Context is counted in tokens"),
            "nothing goes down"
        );
    }

    #[test]
    fn ordinary_numbers_are_not_claims() {
        for line in [
            "The briefing is capped at 60 lines of context.",
            "coverage must stay above 80% for the crate",
            "a 5x5 grid of tokens",
            "context/v1 documents",
            "I0305 plans token telemetry",
            "The hex digest 3x7f of the token is stable.",
            "there is no `0.x` hatch at all, and breaking changes cost a major",
            "phase       648 ms     1 x  validate:context",
            "The operator asked for coverage near 100 percent. It costs something.",
            "The other half of the context is rules.",
            "The token is read twice.",
            "a_50% token identifier",
            "12 ppm of tokens",
            "{%- if s.segments | length > 0 %} token {% endif %}",
            "It runs twice, without Majordomus and with it, and the provider's usage is recorded; savings are refused.",
            "The test reads both halves of the prompt and fails when fewer are declared.",
            "The third session reduces nothing about how tokens are counted.",
            "Tokens are counted, etc. Coverage is 80%.",
        ] {
            assert!(unsupported_quantity(line).is_none(), "false positive: {line}");
        }
    }

    #[test]
    fn a_sentence_wrapped_over_lines_is_one_sentence_reported_where_its_quantity_starts() {
        let found = prose_quantities(
            "a.md",
            "Majordomus cuts the token\nbill by 40% on each task.\n",
        );
        assert_eq!(
            found,
            vec![ProseQuantity {
                line: 2,
                quantity: "40%".into(),
                code: false
            }]
        );
        let found = prose_quantities(
            "a.md",
            "It uses 40% fewer\nof the tokens a session would.\n",
        );
        assert_eq!(found[0].line, 1, "the quantity starts on the first line");
        let found = prose_quantities("a.md", "Coverage is 80%.\nTokens are counted.\n");
        assert!(found.is_empty(), "two sentences stay two: {found:?}");
        let found = prose_quantities("a.md", "Tokens are counted\n\n40% of runs are slow.\n");
        assert!(
            found.is_empty(),
            "a blank line ends the paragraph: {found:?}"
        );
    }

    #[test]
    fn blocks_do_not_run_into_the_prose_around_them() {
        let text = "Tokens are counted\n```\n40% done\n```\n# Tokens\n40% of runs\n- tokens\n- 40% of runs\n";
        assert!(
            prose_quantities("a.md", text).is_empty(),
            "{:?}",
            prose_quantities("a.md", text)
        );
        let text = "Intro\n```sh\n# tokens\n# 40% fewer\n```\n";
        assert_eq!(
            prose_quantities("a.md", text),
            vec![ProseQuantity {
                line: 4,
                quantity: "40%".into(),
                code: true
            }],
            "a fenced block is a paragraph of its own"
        );
        let toml = "# Tokens are what\n# a session spends: 40% fewer.\n[hero]\ntitle = \"tokens\"\nlead = \"40% faster\"\n";
        let found = prose_quantities("site/data/x.toml", toml);
        assert_eq!(found.len(), 1, "comments join, keys do not: {found:?}");
        assert_eq!(found[0].line, 2);
        let front = "---\ntitle: tokens\nsummary: 40% faster\n---\nBody.\n";
        assert!(
            prose_quantities("a.md", front).is_empty(),
            "each front-matter key is its own"
        );
    }

    #[test]
    fn a_generated_file_is_known_by_provenance_not_by_its_prose() {
        let manifest: BTreeSet<String> = ["AGENTS.md".to_string()].into();
        assert!(is_generated("AGENTS.md", "hand-written?", &manifest));
        assert!(is_generated("site/content/x.md", "", &manifest));
        assert!(is_generated(
            "docs/generated/x.md",
            "<!-- GENERATED FILE — DO NOT EDIT\n",
            &manifest
        ));
        assert!(is_generated("x.rs", "// GENERATED FILE\n", &manifest));
        assert!(
            !is_generated("docs/generated/x.md", "# Report\n", &manifest),
            "the directory alone says nothing"
        );
        assert!(
            !is_generated(
                "docs/NOTES.md",
                "# Notes\n\nThis was generated by hand.\n",
                &manifest
            ),
            "a prose line is not a stamp"
        );
        assert!(
            !is_generated(
                "docs/NOTES.md",
                "# Notes\n<!-- generated by x -->\n",
                &manifest
            ),
            "only the first line"
        );
    }

    /// A methodology with no evidence recorded under it, binding three claims: one the claims
    /// matrix guarantees, one it plans, and one it does not carry at all.
    const METHODOLOGY: &str = "\
schema: economics-methodology/v1
version: 1
title: fixture
question: how many tokens
unit: tokens per task
primary_metric: effective_token_reduction
classes: []
variants: []
success: []
pairing:
  key: [suite, task, repetition]
  comparable: []
  valid: both runs succeeded
statistics:
  per_pair: reduction
  location: median
  interval: bootstrap
  confidence_bp: 9500
  resamples: 100
  seed: 1
  min_pairs_for_interval: 5
publication:
  min_valid_pairs: 30
  min_categories: 4
  min_pairs_per_category: 5
  min_repetitions: 3
  min_valid_pair_rate_bp: 8000
  max_interval_width_bp: 2000
  require_current: true
outliers:
  rule: no run is excluded
claims:
  - claim: guaranteed-early
    metric: effective_token_reduction
    requires: verified
  - claim: still-planned
    metric: effective_token_reduction
    requires: verified
  - claim: never-written
    metric: context_reduction_ratio
    requires: measured
";

    fn git(root: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn repository(claims: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let economics = dir.path().join(crate::economics::DIR);
        std::fs::create_dir_all(&economics).unwrap();
        std::fs::write(economics.join("methodology.yaml"), METHODOLOGY).unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/CLAIMS.yaml"), claims).unwrap();
        git(dir.path(), &["init", "-q"]);
        dir
    }

    #[test]
    fn a_guaranteed_claim_without_standing_evidence_is_refused() {
        let dir = repository(
            "claims:\n  - id: guaranteed-early\n    status: guaranteed\n  \
             - id: still-planned\n    status: planned\n",
        );
        let summary = crate::economics::summarize(dir.path(), &Default::default());
        assert!(
            summary.present,
            "the fixture must load: {:?}",
            summary.diagnostics
        );
        let (checked, findings) = check_claims(dir.path(), &summary);
        assert_eq!(
            checked, 3,
            "every binding is counted, whatever its claim's status"
        );
        let details: Vec<&str> = findings.iter().map(|f| f.detail.as_str()).collect();
        assert_eq!(findings.len(), 2, "{details:?}");
        assert!(findings.iter().all(|f| f.kind == "claim_unsupported"));
        assert!(details
            .iter()
            .any(|d| d.starts_with("claim guaranteed-early is guaranteed")));
        assert_eq!(
            findings
                .iter()
                .find(|f| f.detail.contains("guaranteed-early"))
                .unwrap()
                .line,
            Some(2),
            "the claim's own line"
        );
        assert!(details
            .iter()
            .any(|d| d.contains("claim never-written") && d.contains("has no such claim")));
        assert!(
            !details.iter().any(|d| d.contains("still-planned")),
            "a planned claim promises nothing and is left alone"
        );
    }

    #[test]
    fn a_verified_metric_on_stale_evidence_does_not_hold_a_guarantee() {
        let dir = repository(
            "claims:\n  - id: guaranteed-early\n    status: guaranteed\n  \
             - id: still-planned\n    status: planned\n  - id: never-written\n    status: planned\n",
        );
        let suite = [
            "schema: economics-suite/v1",
            "id: pilot",
            "version: 1",
            "kind: live",
            "title: Pilot",
            "control: baseline",
            "treatment: majordomus",
            "freshness_inputs: [methodology.yaml]",
            "",
        ]
        .join("\n");
        let suites = dir.path().join(crate::economics::DIR).join("suites");
        std::fs::create_dir_all(&suites).unwrap();
        std::fs::write(suites.join("pilot.yaml"), suite).unwrap();
        let mut summary = crate::economics::summarize(dir.path(), &Default::default());
        assert!(summary.present, "{:?}", summary.diagnostics);
        // what a publishable verdict over evidence that has since gone stale would look like
        summary.verdict.publishable = true;
        for m in &mut summary.metrics {
            if m.id == crate::economics::EFFECTIVE_TOKEN_REDUCTION {
                m.status = EconomicsMetricStatus::Verified;
            }
        }
        let pilot = summary.suites.iter_mut().find(|s| s.id == "pilot").unwrap();
        pilot.freshness = EconomicsFreshness::Stale;
        let (_, findings) = check_claims(dir.path(), &summary);
        let refused = findings
            .iter()
            .find(|f| f.detail.contains("guaranteed-early"));
        assert!(
            refused.is_some_and(|f| f.detail.contains("(evidence not current: pilot stale)")),
            "{findings:?}"
        );
        summary
            .suites
            .iter_mut()
            .find(|s| s.id == "pilot")
            .unwrap()
            .freshness = EconomicsFreshness::Current;
        let (_, findings) = check_claims(dir.path(), &summary);
        assert!(
            findings.is_empty(),
            "verified, publishable and current stands: {findings:?}"
        );
    }

    #[test]
    fn an_unbound_or_duplicated_savings_claim_is_refused() {
        let dir = repository(
            "claims:\n  - id: guaranteed-early\n    status: planned\n  - id: still-planned\n    status: planned\n  \
             - id: never-written\n    status: planned\n  \
             - id: cheaper-sessions\n    claim: Sessions spend fewer tokens with the compiler\n    status: guaranteed\n  \
             - id: counted\n    claim: Context is counted in tokens\n    status: guaranteed\n  \
             - id: planned-saving\n    claim: Majordomus will reduce token spend\n    status: planned\n  \
             - id: counted\n    claim: again\n    status: planned\n",
        );
        let summary = crate::economics::summarize(dir.path(), &Default::default());
        let (_, findings) = check_claims(dir.path(), &summary);
        let kinds: Vec<(&str, Option<usize>)> =
            findings.iter().map(|f| (f.kind.as_str(), f.line)).collect();
        assert_eq!(
            kinds,
            [("claim_duplicate", Some(17)), ("claim_unbound", Some(8))],
            "{findings:?}"
        );
        assert!(findings[1]
            .detail
            .starts_with("claim cheaper-sessions is guaranteed"));
    }

    #[test]
    fn a_finding_names_the_line_of_the_claim_not_of_an_id_nested_in_it() {
        let dir = repository(
            "claims:\n  - id: guaranteed-early\n    status: planned\n    evidence:\n      - id: cheap\n  \
             - id: still-planned\n    status: planned\n  - id: never-written\n    status: planned\n  \
             - id: cheap\n    claim: Sessions spend fewer tokens\n    status: guaranteed\n",
        );
        let summary = crate::economics::summarize(dir.path(), &Default::default());
        let (_, findings) = check_claims(dir.path(), &summary);
        let kinds: Vec<(&str, Option<usize>)> =
            findings.iter().map(|f| (f.kind.as_str(), f.line)).collect();
        assert_eq!(kinds, [("claim_unbound", Some(10))], "{findings:?}");
    }

    #[test]
    fn declarations_that_cannot_be_read_fail_closed() {
        let dir = repository("claims: []\n");
        std::fs::write(
            dir.path()
                .join(crate::economics::DIR)
                .join("methodology.yaml"),
            "schema: x\n",
        )
        .unwrap();
        let summary = crate::economics::summarize(dir.path(), &Default::default());
        let (checked, findings) = check_claims(dir.path(), &summary);
        assert_eq!(checked, 0);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].kind, "declarations_unreadable");
        assert_eq!(
            findings[0].path,
            format!("{}/methodology.yaml", crate::economics::DIR)
        );

        std::fs::write(dir.path().join("docs/CLAIMS.yaml"), "claims:\n\tbad\n").unwrap();
        let (_, findings) = check_claims(dir.path(), &summary);
        assert!(
            findings.iter().any(|f| f.kind == "claims_unreadable"),
            "{findings:?}"
        );
    }

    #[test]
    fn check_joins_every_check_and_is_ok_only_without_findings() {
        let dir = repository(
            "claims:\n  - id: guaranteed-early\n    status: planned\n  \
             - id: still-planned\n    status: planned\n",
        );
        let report = check(dir.path());
        assert_eq!((report.scanned, report.claims_checked), (1, 3));
        assert_eq!(
            report.findings.len(),
            1,
            "only the unwritten claim: {:?}",
            report.findings
        );
        assert!(!report.ok);
    }

    #[test]
    fn a_repository_git_cannot_list_is_not_a_clean_one() {
        let dir = tempfile::tempdir().unwrap();
        let (scanned, findings) = scan_prose(dir.path());
        assert_eq!(scanned, 0);
        assert_eq!(findings.len(), 1);
        assert_eq!(
            (findings[0].kind.as_str(), findings[0].path.as_str()),
            ("scan_failed", ".")
        );
    }
}

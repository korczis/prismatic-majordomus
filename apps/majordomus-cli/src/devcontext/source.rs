//! The source ranges a request's paths reach: where each Rust definition under them lies,
//! offered to the budget as a range rather than as a whole file.
//!
//! # Why a range, and why here
//!
//! A worker who is told "the code under `apps/x/src/devcontext`" has been told nothing it
//! can afford to read: that is five files and six thousand lines. What it can afford is the
//! outline — `fn compile`, lines 762–936 of `mod.rs` — and then the one range it needs. So a
//! request whose effective paths reach Rust source is answered with one candidate per
//! definition, each carrying the path, the line span, the symbol, how it was found and what
//! reading it costs, and the budget spends those candidates in the same total order as
//! everything else. The whole-file cost and what the selected ranges cost instead are
//! reported beside them ([`super::SourceCoverage`]), so the saving is a measured number in
//! the answer rather than a claim about it.
//!
//! # What it reads, and what it refuses to
//!
//! [`super::select`] reads no file and walks no tree; this module is the one place the
//! compiler reads source, and it is bounded on every axis:
//!
//! - only files git tracks under a requested path, asked for as a literal pathspec so that a
//!   path is never interpreted as pathspec magic;
//! - only files the repository scope admits ([`crate::scope::Scope::classify`]: undeclared,
//!   secret, binary and oversized files are out before anything is read);
//! - never a symbolic link, and never a file whose real path leaves the repository root;
//! - at most [`MAX_SOURCE_FILES`] files, [`MAX_SOURCE_BYTES`] bytes each and
//!   [`MAX_RANGES_PER_FILE`] ranges each — whatever is cut is counted in the answer.
//!
//! Nothing is built and nothing of the analysed project runs: the parser is `syn` over the
//! bytes read, with no macro expansion, no `cfg` evaluation and no name resolution. A range
//! is therefore a *syntactic definition*, observed: it says where an item is written, never
//! what calls it. A file `syn` cannot parse, and every language it does not cover, is
//! reported as such rather than answered with an empty outline that would read as complete.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use syn::spanned::Spanned;

use super::model::{tokens_for, Discovery, Selector, SourceFile, Tier, UnreadSource};
use super::select::{Candidate, Selection};
use crate::capability::handler::Context;
use crate::scope::Verdict;

/// The most files one request reads for ranges. A directory with more is read in the
/// index's byte order up to the bound, and the rest are counted as `truncated`.
pub const MAX_SOURCE_FILES: usize = 128;

/// The largest file read for ranges. A larger one is reported, not parsed: the parser has a
/// budget like everything else, and the repository scope may already set a lower one.
pub const MAX_SOURCE_BYTES: u64 = 1 << 20;

/// The most ranges one file contributes; the rest of its definitions are counted.
pub const MAX_RANGES_PER_FILE: usize = 2_000;

/// The kind every range candidate carries.
pub const KIND: &str = "source-range";

/// The scheme of a range's identifier: `source:<path>#L<first>-L<last>`.
pub const SCHEME: &str = "source:";

/// What `syn` reads, said once so every range and the coverage name the same parser.
pub const PARSER: &str =
    "syn 2 over the bytes read: Rust syntax only; macros not expanded, cfg not evaluated, names not resolved";

/// The relevance of a range reached by a declared path alone: the same weight a whole
/// implementation object under a declared path gets, so a range is never preferred to it.
const RELEVANCE: f64 = 0.8;

/// How much an intent match may lift a range above its siblings: enough to order the
/// ranges of one file, never enough to reach a seed.
const INTENT_LIFT: f64 = 0.1;

/// What the source reading produced: the files outlined and the ones that were not.
#[derive(Debug, Default)]
pub struct Outlined {
    /// The Rust files read and outlined, in path order.
    pub files: Vec<SourceFile>,
    /// What was reached and not outlined, with the reason, in path order.
    pub unread: Vec<UnreadSource>,
    /// Tracked files under the paths that no parser here covers, counted by extension.
    pub unparsed: BTreeMap<String, usize>,
    /// Files past [`MAX_SOURCE_FILES`], counted and not read.
    pub truncated: usize,
}

/// One definition `syn` found: what it is, what it is called, and where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// The noun: `fn`, `struct`, `impl fn`, ...
    pub item: &'static str,
    /// The name within the file, qualified by any inline module and `impl` it sits in.
    pub symbol: String,
    /// The first line, 1-based, including the item's outer attributes and doc comments.
    pub first: usize,
    /// The last line, 1-based, inclusive.
    pub last: usize,
}

/// The definitions of one Rust file, in the order they are written.
///
/// An `impl` block and an inline `mod` are decomposed into the definitions inside them, so
/// the ranges of one file never overlap and their costs can be added up; every other item
/// is one range, a `trait` included.
///
/// ```
/// use majordomus_cli::devcontext::source::outline;
///
/// let text = "/// A thing.\npub struct T;\n\nimpl T {\n    fn go(&self) {}\n}\n\nmod inner {\n    pub fn deep() {}\n}\n";
/// let defs = outline(text).unwrap();
/// let named: Vec<(&str, &str, usize, usize)> =
///     defs.iter().map(|d| (d.item, d.symbol.as_str(), d.first, d.last)).collect();
/// assert_eq!(
///     named,
///     [("struct", "T", 1, 2), ("impl fn", "T::go", 5, 5), ("fn", "inner::deep", 9, 9)],
/// );
///
/// // a file that is not Rust syntax is an error, never an empty outline
/// assert!(outline("fn (").is_err());
/// ```
pub fn outline(text: &str) -> Result<Vec<Definition>, String> {
    let file = syn::parse_file(text).map_err(|e| {
        let at = e.span().start();
        format!("{e} (line {}, column {})", at.line, at.column + 1)
    })?;
    let mut out = Vec::new();
    definitions(&file.items, "", &mut out);
    Ok(out)
}

fn lines_of(span: proc_macro2::Span, attrs: &[syn::Attribute]) -> (usize, usize) {
    let first = attrs
        .iter()
        .filter(|a| matches!(a.style, syn::AttrStyle::Outer))
        .map(|a| a.span().start().line)
        .chain(std::iter::once(span.start().line))
        .min()
        .unwrap_or(1);
    (first, span.end().line.max(first))
}

fn qualified(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "_".into()),
        syn::Type::Reference(r) => type_name(&r.elem),
        _ => "_".into(),
    }
}

fn definitions(items: &[syn::Item], prefix: &str, out: &mut Vec<Definition>) {
    for item in items {
        let mut push = |noun: &'static str, name: String, attrs: &[syn::Attribute]| {
            let (first, last) = lines_of(item.span(), attrs);
            out.push(Definition {
                item: noun,
                symbol: qualified(prefix, &name),
                first,
                last,
            });
        };
        match item {
            syn::Item::Fn(f) => push("fn", f.sig.ident.to_string(), &f.attrs),
            syn::Item::Struct(s) => push("struct", s.ident.to_string(), &s.attrs),
            syn::Item::Enum(e) => push("enum", e.ident.to_string(), &e.attrs),
            syn::Item::Union(u) => push("union", u.ident.to_string(), &u.attrs),
            syn::Item::Trait(t) => push("trait", t.ident.to_string(), &t.attrs),
            syn::Item::Const(c) => push("const", c.ident.to_string(), &c.attrs),
            syn::Item::Static(s) => push("static", s.ident.to_string(), &s.attrs),
            syn::Item::Type(t) => push("type", t.ident.to_string(), &t.attrs),
            syn::Item::Macro(m) => {
                // a `macro_rules!` definition has a name; an invocation at item position
                // (`thread_local!`, a capability declaration) is named by its macro, and
                // what it expands to is not seen
                let name = match &m.ident {
                    Some(id) => id.to_string(),
                    None => m
                        .mac
                        .path
                        .segments
                        .last()
                        .map(|s| format!("{}!", s.ident))
                        .unwrap_or_else(|| "macro!".into()),
                };
                let noun = if m.ident.is_some() {
                    "macro_rules"
                } else {
                    "macro call"
                };
                push(noun, name, &m.attrs);
            }
            syn::Item::Mod(m) => match &m.content {
                Some((_, inner)) => {
                    definitions(inner, &qualified(prefix, &m.ident.to_string()), out)
                }
                // `mod x;` names another file, which is outlined when it is reached itself
                None => push("mod", m.ident.to_string(), &m.attrs),
            },
            syn::Item::Impl(i) => {
                let target = type_name(&i.self_ty);
                let owner = match &i.trait_ {
                    Some((_, path, _)) => {
                        let tr = path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .unwrap_or_else(|| "_".into());
                        format!("<{target} as {tr}>")
                    }
                    None => target,
                };
                let owner = qualified(prefix, &owner);
                for it in &i.items {
                    let (noun, name, attrs): (&'static str, String, &[syn::Attribute]) = match it {
                        syn::ImplItem::Fn(f) => ("impl fn", f.sig.ident.to_string(), &f.attrs),
                        syn::ImplItem::Const(c) => ("impl const", c.ident.to_string(), &c.attrs),
                        syn::ImplItem::Type(t) => ("impl type", t.ident.to_string(), &t.attrs),
                        _ => continue,
                    };
                    let (first, last) = lines_of(it.span(), attrs);
                    out.push(Definition {
                        item: noun,
                        symbol: format!("{owner}::{name}"),
                        first,
                        last,
                    });
                }
            }
            // `use`, `extern crate`, foreign blocks and verbatim tokens define nothing a
            // reader looks up by name; they stay in the file and out of the outline
            _ => {}
        }
    }
}

/// The words of an identifier, lower-cased: `tokens_for` is `tokens`, `for`; `TierSpend`
/// is `tier`, `spend`.
fn words(symbol: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for part in symbol.split(|c: char| !c.is_alphanumeric()) {
        let mut word = String::new();
        let mut prev_lower = false;
        for ch in part.chars() {
            if ch.is_uppercase() && prev_lower && !word.is_empty() {
                out.insert(std::mem::take(&mut word));
            }
            prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
            word.extend(ch.to_lowercase());
        }
        if !word.is_empty() {
            out.insert(word);
        }
    }
    out
}

/// The byte offset at which each line starts, plus the length, so a span's cost is a
/// subtraction rather than a second pass.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    if *starts.last().unwrap_or(&0) != text.len() {
        starts.push(text.len());
    }
    starts
}

fn bytes_between(starts: &[usize], first: usize, last: usize) -> u64 {
    let from = starts.get(first.saturating_sub(1)).copied().unwrap_or(0);
    let to = starts
        .get(last)
        .copied()
        .unwrap_or_else(|| *starts.last().unwrap_or(&0));
    to.saturating_sub(from) as u64
}

/// Read one tracked, in-scope file through every guard, or say which guard refused it.
fn read_guarded(ctx: &Context, root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let judged = ctx.index.scoped.scope.classify(root, rel);
    if judged.verdict == Verdict::Out {
        return Err(format!(
            "out of the repository scope ({})",
            judged.reason.map(|r| r.as_str()).unwrap_or("out")
        ));
    }
    let abs = root.join(rel);
    let meta = std::fs::symlink_metadata(&abs).map_err(|e| format!("cannot stat: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("a symbolic link; not followed".into());
    }
    if !meta.is_file() {
        return Err("not a regular file in the work tree".into());
    }
    if meta.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "{} bytes, over the {MAX_SOURCE_BYTES}-byte bound for one file",
            meta.len()
        ));
    }
    // a parent directory replaced by a link would pass the checks above: the real path is
    // what is opened, so the real path is what must stay inside the root
    let real_root = root
        .canonicalize()
        .map_err(|e| format!("cannot resolve the repository root: {e}"))?;
    let real = abs
        .canonicalize()
        .map_err(|e| format!("cannot resolve: {e}"))?;
    if !real.starts_with(&real_root) {
        return Err("its real path leaves the repository; not read".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&real)
        .and_then(|f| f.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("cannot read: {e}"))?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err("grew past the one-file bound while it was read".into());
    }
    Ok(bytes)
}

/// Offer every definition of the Rust files under `paths` to the selection, and report
/// what was outlined and what was not.
///
/// `paths` are the request's effective paths, already normalised (repository-relative, no
/// `..`). `terms` are the intent's words; a definition whose name holds one of them is
/// recorded as an intent match as well, which orders it before its siblings and never
/// above a declared object.
pub fn offer(
    ctx: &Context,
    sel: &mut Selection,
    paths: &BTreeSet<String>,
    terms: &BTreeSet<String>,
) -> Outlined {
    let mut out = Outlined::default();
    if paths.is_empty() {
        return out;
    }
    let root = Path::new(&ctx.index.repository.root);
    let specs: Vec<String> = paths.iter().map(|p| format!(":(literal){p}")).collect();
    let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
    let tracked = match crate::git::ls_files_any(root, &specs) {
        Ok(files) => files,
        Err(e) => {
            out.unread.push(UnreadSource {
                path: paths.iter().cloned().collect::<Vec<_>>().join(", "),
                reason: format!("the tracked files could not be listed: {e}"),
            });
            return out;
        }
    };
    let mut rust = Vec::new();
    for rel in tracked {
        if rel.ends_with(".rs") {
            rust.push(rel);
        } else {
            let ext = rel
                .rsplit_once('/')
                .map_or(rel.as_str(), |(_, name)| name)
                .rsplit_once('.')
                .map(|(_, e)| format!(".{e}"))
                .unwrap_or_else(|| "(none)".into());
            *out.unparsed.entry(ext).or_default() += 1;
        }
    }
    rust.sort();
    if rust.len() > MAX_SOURCE_FILES {
        out.truncated = rust.len() - MAX_SOURCE_FILES;
        rust.truncate(MAX_SOURCE_FILES);
    }

    for rel in rust {
        let bytes = match read_guarded(ctx, root, &rel) {
            Ok(b) => b,
            Err(reason) => {
                out.unread.push(UnreadSource { path: rel, reason });
                continue;
            }
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            out.unread.push(UnreadSource {
                path: rel,
                reason: "not UTF-8 text".into(),
            });
            continue;
        };
        let defs = match outline(text) {
            Ok(d) => d,
            Err(e) => {
                out.unread.push(UnreadSource {
                    path: rel,
                    reason: format!("syn cannot parse it: {e}"),
                });
                continue;
            }
        };
        let digest = crate::evidence::digest_of(&bytes);
        let starts = line_starts(text);
        let via = paths
            .iter()
            .filter(|p| rel == **p || rel.starts_with(&format!("{p}/")))
            .max_by_key(|p| p.len())
            .cloned()
            .unwrap_or_else(|| rel.clone());
        let cut = defs.len().saturating_sub(MAX_RANGES_PER_FILE);
        for d in defs.iter().take(MAX_RANGES_PER_FILE) {
            let range_bytes = bytes_between(&starts, d.first, d.last);
            let uri = format!("{SCHEME}{rel}#L{}-L{}", d.first, d.last);
            let mut discovered_by = vec![Discovery {
                selector: Selector::ScopePath,
                reason: format!(
                    "`{} {}` is written at lines {}–{} of `{rel}`, under `{via}`, which the work declares; the span is syntax, read by syn",
                    d.item, d.symbol, d.first, d.last
                ),
                via: Some(via.clone()),
                edge: None,
                depth: 1,
                confidence: 1.0,
                weight: RELEVANCE,
            }];
            let mut relevance = RELEVANCE;
            if !terms.is_empty() {
                let named = words(&d.symbol);
                let hits: Vec<&str> = terms
                    .iter()
                    .filter(|t| named.contains(*t))
                    .map(String::as_str)
                    .collect();
                if !hits.is_empty() {
                    let confidence = (hits.len() as f64 / terms.len() as f64).min(1.0);
                    relevance += INTENT_LIFT * confidence;
                    discovered_by.push(Discovery {
                        selector: Selector::IntentMatch,
                        reason: format!(
                            "the intent's term(s) {} are words of the name `{}`",
                            hits.join(", "),
                            d.symbol
                        ),
                        via: None,
                        edge: None,
                        depth: 1,
                        confidence,
                        weight: INTENT_LIFT * confidence,
                    });
                }
            }
            let mut facts = BTreeMap::new();
            facts.insert("bytes".into(), range_bytes.to_string());
            facts.insert("lines".into(), format!("{}-{}", d.first, d.last));
            facts.insert("symbol".into(), d.symbol.clone());
            facts.insert("item".into(), d.item.into());
            facts.insert("file_digest".into(), digest.clone());
            facts.insert("parser".into(), PARSER.into());
            sel.candidates.insert(
                uri.clone(),
                Candidate {
                    uri,
                    kind: KIND.into(),
                    title: Some(format!("{} {}", d.item, d.symbol)),
                    tier: Tier::Source,
                    relevance,
                    depth: 1,
                    discovered_by,
                    object: None,
                    path: Some(rel.clone()),
                    status: None,
                    facts,
                },
            );
        }
        out.files.push(SourceFile {
            path: rel,
            digest,
            bytes: bytes.len() as u64,
            whole_tokens: tokens_for(bytes.len() as u64),
            definitions: defs.len(),
            truncated: cut,
            selected: 0,
            selected_tokens: 0,
            saved_tokens: 0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_definition_spans_its_doc_comment_and_its_body() {
        let text = "//! Module.\n\n/// Says hello.\n///\n/// Twice.\n#[inline]\npub fn hello() {\n    println!(\"hi\");\n}\n";
        let defs = outline(text).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!((defs[0].first, defs[0].last), (3, 9));
        let starts = line_starts(text);
        let cost = bytes_between(&starts, 3, 9) as usize;
        assert_eq!(
            &text[text.len() - cost..],
            &text[text.find("/// Says").unwrap()..]
        );
    }

    #[test]
    fn homonyms_in_different_owners_stay_apart() {
        let text = "struct A;\nstruct B;\nimpl A { fn new() -> Self { A } }\nimpl B { fn new() -> Self { B } }\nimpl Default for A { fn default() -> Self { A } }\n";
        let symbols: Vec<String> = outline(text)
            .unwrap()
            .into_iter()
            .map(|d| d.symbol)
            .collect();
        assert_eq!(
            symbols,
            ["A", "B", "A::new", "B::new", "<A as Default>::default"]
        );
    }

    #[test]
    fn a_macro_call_is_named_by_its_macro_and_not_expanded() {
        let text = "macro_rules! twice { () => {} }\nthread_local! { static X: u8 = 0; }\n";
        let defs = outline(text).unwrap();
        let named: Vec<(&str, &str)> = defs.iter().map(|d| (d.item, d.symbol.as_str())).collect();
        assert_eq!(
            named,
            [("macro_rules", "twice"), ("macro call", "thread_local!")]
        );
    }

    #[test]
    fn the_words_of_a_name_split_on_case_and_underscores() {
        let w = words("<TierSpend as Ordered>::order_key");
        for expected in ["tier", "spend", "ordered", "order", "key"] {
            assert!(w.contains(expected), "{expected} missing from {w:?}");
        }
    }

    #[test]
    fn a_last_line_without_a_newline_is_still_counted() {
        let text = "fn a() {}\nfn b() {}";
        let starts = line_starts(text);
        assert_eq!(bytes_between(&starts, 1, 1), 10);
        assert_eq!(bytes_between(&starts, 2, 2), 9);
        assert_eq!(bytes_between(&starts, 1, 2), text.len() as u64);
    }
}

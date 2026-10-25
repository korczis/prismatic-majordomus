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
//!
//! # A file, end to end
//!
//! A file is outlined into its definitions; the compiler names each one by a range of that
//! file under [`SCHEME`], and the budget spends them as [`KIND`] entries.
//!
//! ```
//! use majordomus_cli::devcontext::source::{outline, KIND, SCHEME};
//!
//! let text = "pub struct Budget;\n\nimpl Budget {\n    pub fn spend(&self) {}\n}\n";
//! let defs = outline(text).unwrap();
//! let uris: Vec<String> = defs
//!     .iter()
//!     .map(|d| format!("{SCHEME}src/budget.rs#L{}-L{}", d.first, d.last))
//!     .collect();
//! assert_eq!(uris, ["source:src/budget.rs#L1-L1", "source:src/budget.rs#L4-L4"]);
//! assert_eq!(KIND, "source-range");
//!
//! // a definition never spans another: the impl block is its methods, not a range of its own
//! assert_eq!(defs[1].symbol, "Budget::spend");
//! ```

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

/// The relevance of a range reached by a declared path alone: below the 0.8 a whole
/// implementation object under a declared path gets, so a range is never preferred to it.
const RELEVANCE: f64 = 0.7;

/// How much an intent match may lift a range above its siblings: enough to order the
/// ranges of one file, never enough to reach a whole object (0.8), let alone a seed.
const INTENT_LIFT: f64 = 0.09;

/// What the source reading produced: the files outlined and the ones that were not.
#[derive(Debug, Default)]
pub(crate) struct Outlined {
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
///
/// ```
/// use majordomus_cli::devcontext::source::{outline, Definition};
///
/// let d: Definition = outline("/// Doc.\nfn go() {}\n").unwrap().remove(0);
/// assert_eq!((d.item, d.symbol.as_str(), d.first, d.last), ("fn", "go", 1, 2));
/// ```
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
/// the ranges of one file do not nest; every other item is one range, a `trait` included.
/// Two definitions written on one line share their span, and the compiler offers that span
/// once, naming both; adjacent items may share a boundary line, so the costs of a file's
/// ranges add up to about, not exactly, the file.
///
/// The parser's span table is released before this returns: the spans are turned into line
/// numbers here, and a long-lived server that parses source on every request must not keep
/// every file it ever read. Spans of other parses held on this thread across the call are
/// invalidated with it.
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
    drop(file);
    proc_macro2::extra::invalidate_current_thread_spans();
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
fn read_guarded(
    ctx: &Context,
    root: &Path,
    real_root: &Path,
    rel: &str,
) -> Result<Vec<u8>, String> {
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
    // what is opened, so the real path is what must stay inside the root; one that cannot be
    // resolved is not known to stay inside it, and is refused the same way
    let real = match abs.canonicalize() {
        Ok(real) if real.starts_with(real_root) => real,
        _ => return Err("its real path leaves the repository; not read".into()),
    };
    // opened without following a final link, and judged by the handle that was opened
    // rather than by the path, so a file swapped between the checks above and the read is
    // refused instead of read
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(&real)
        .map_err(|e| format!("cannot open without following a link: {e}"))?;
    let is_file = file.metadata().map(|m| m.is_file());
    read_opened(file, is_file)
}

/// What [`read_guarded`] reads from the handle it opened, judged by that handle: refused when
/// it is not a regular file, and when the file grew past the bound while it was read. A file
/// swapped or grown after the checks on its path is what reaches these refusals.
fn read_opened(file: impl Read, is_file: std::io::Result<bool>) -> Result<Vec<u8>, String> {
    if !is_file.map_err(|e| format!("cannot stat: {e}"))? {
        return Err("not a regular file when it was opened".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
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
pub(crate) fn offer(
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
    crate::order::canonical_strings(&mut rust);
    let real_root = match root.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            out.unread.push(UnreadSource {
                path: ".".into(),
                reason: format!("the repository root cannot be resolved: {e}"),
            });
            return out;
        }
    };
    if rust.len() > MAX_SOURCE_FILES {
        out.truncated = rust.len() - MAX_SOURCE_FILES;
        rust.truncate(MAX_SOURCE_FILES);
    }

    for rel in rust {
        let bytes = match read_guarded(ctx, root, &real_root, &rel) {
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
        // definitions written on the same lines are one span: offered once, naming each,
        // so no definition is lost to another that shares its identifier
        let mut spans: BTreeMap<(usize, usize), Vec<&Definition>> = BTreeMap::new();
        for d in defs.iter().take(MAX_RANGES_PER_FILE) {
            spans.entry((d.first, d.last)).or_default().push(d);
        }
        for ((first, last), named) in &spans {
            let (first, last) = (*first, *last);
            let range_bytes = bytes_between(&starts, first, last);
            let uri = format!("{SCHEME}{rel}#L{first}-L{last}");
            let symbol = named
                .iter()
                .map(|d| d.symbol.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let item = named
                .iter()
                .map(|d| d.item)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let title = named
                .iter()
                .map(|d| format!("{} {}", d.item, d.symbol))
                .collect::<Vec<_>>()
                .join(", ");
            let mut discovered_by = vec![Discovery {
                selector: Selector::ScopePath,
                reason: format!(
                    "`{title}` is written at lines {first}–{last} of `{rel}`, under `{via}`, which the work declares; the span is syntax, read by syn"
                ),
                via: Some(via.clone()),
                edge: None,
                depth: 1,
                confidence: 1.0,
                weight: RELEVANCE,
            }];
            let mut relevance = RELEVANCE;
            if !terms.is_empty() {
                let words_of: BTreeSet<String> =
                    named.iter().flat_map(|d| words(&d.symbol)).collect();
                let hits: Vec<&str> = terms
                    .iter()
                    .filter(|t| words_of.contains(*t))
                    .map(String::as_str)
                    .collect();
                if !hits.is_empty() {
                    let confidence = (hits.len() as f64 / terms.len() as f64).min(1.0);
                    relevance += INTENT_LIFT * confidence;
                    discovered_by.push(Discovery {
                        selector: Selector::IntentMatch,
                        reason: format!(
                            "the intent's term(s) {} are words of the name `{symbol}`",
                            hits.join(", ")
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
            facts.insert("lines".into(), format!("{first}-{last}"));
            facts.insert("symbol".into(), symbol);
            facts.insert("item".into(), item);
            facts.insert("file_digest".into(), digest.clone());
            facts.insert("parser".into(), PARSER.into());
            sel.candidates.insert(
                uri.clone(),
                Candidate {
                    uri,
                    kind: KIND.into(),
                    title: Some(title),
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
            ranges: spans.len(),
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
    fn definitions_on_one_line_share_one_span_and_each_is_kept() {
        let defs = outline("struct A; struct B;\nimpl A { fn a() {} fn b() {} }\n").unwrap();
        let spans: BTreeSet<(usize, usize)> = defs.iter().map(|d| (d.first, d.last)).collect();
        assert_eq!(defs.len(), 4, "every definition is found: {defs:?}");
        assert_eq!(spans.len(), 2, "two lines, two spans: {spans:?}");
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

    #[test]
    fn a_range_past_the_last_line_ends_where_the_text_ends() {
        let text = "fn a() {}\nfn b() {}\n";
        let starts = line_starts(text);
        assert_eq!(bytes_between(&starts, 2, 99), 10);
        assert_eq!(bytes_between(&starts, 1, 99), text.len() as u64);
    }

    #[test]
    fn every_kind_of_named_item_is_a_range_and_an_impl_macro_is_not() {
        let text = "\
enum E { A }
union U { a: u8 }
trait Tr {}
const C: u8 = 0;
static S: u8 = 0;
type Alias = u8;
mod elsewhere;
impl Tr for &E {
    fn by_ref(&self) {}
}
impl Tr for [u8] {
    const LEN: usize = 0;
    type Out = u8;
    inner!();
}
";
        let named: Vec<(&str, String)> = outline(text)
            .unwrap()
            .into_iter()
            .map(|d| (d.item, d.symbol))
            .collect();
        let want: Vec<(&str, String)> = [
            ("enum", "E"),
            ("union", "U"),
            ("trait", "Tr"),
            ("const", "C"),
            ("static", "S"),
            ("type", "Alias"),
            ("mod", "elsewhere"),
            // a reference is named by what it refers to
            ("impl fn", "<E as Tr>::by_ref"),
            // a type without a path is named by the placeholder
            ("impl const", "<_ as Tr>::LEN"),
            ("impl type", "<_ as Tr>::Out"),
        ]
        .into_iter()
        .map(|(i, s)| (i, s.to_string()))
        .collect();
        // the macro call inside the impl block is skipped, not named
        assert_eq!(named, want);
    }

    #[test]
    fn a_path_with_no_segment_is_named_by_a_placeholder() {
        let mut ty: syn::Type = syn::parse_str("Budget").unwrap();
        if let syn::Type::Path(p) = &mut ty {
            p.path.segments.clear();
        }
        assert_eq!(type_name(&ty), "_");

        let mut call: syn::Item = syn::parse_str("declare!();").unwrap();
        if let syn::Item::Macro(m) = &mut call {
            m.mac.path.segments.clear();
        }
        let mut imp: syn::Item = syn::parse_str("impl Shown for Budget { fn go() {} }").unwrap();
        if let syn::Item::Impl(i) = &mut imp {
            if let Some((_, path, _)) = &mut i.trait_ {
                path.segments.clear();
            }
        }
        let mut out = Vec::new();
        definitions(&[call, imp], "", &mut out);
        proc_macro2::extra::invalidate_current_thread_spans();
        let named: Vec<(&str, &str)> = out.iter().map(|d| (d.item, d.symbol.as_str())).collect();
        assert_eq!(
            named,
            [("macro call", "macro!"), ("impl fn", "<Budget as _>::go")]
        );
    }

    use crate::devcontext::fixture::Planned;

    const SCOPE: &str = "version: 1\nin:\n  - .ai/manifest.yaml\n  - .ai/README.md\n  - .ai/repo/**\n  - lib/**\n  - test/**\n  - src/**\nout:\n  paths: []\n  binary: false\n  max_bytes: 8388608\n";

    fn git(root: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .status()
            .expect("git runs");
        assert!(ok.success(), "git {args:?}");
    }

    /// A repository whose scope admits `src/**`, judges no content as binary and admits
    /// up to 8 MiB, so that the guards of [`read_guarded`] are the ones that refuse;
    /// `files` are written and tracked.
    fn tracked(files: &[(&str, &[u8])]) -> Planned {
        let repo = Planned::build().expect("a repository");
        repo.write(".ai/repo/scope.yaml", SCOPE).unwrap();
        for (rel, bytes) in files {
            let p = repo.root().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        git(repo.root(), &["init", "-q"]);
        git(repo.root(), &["add", "-A"]);
        repo
    }

    fn selection() -> Selection {
        Selection {
            candidates: BTreeMap::new(),
            unresolved: Vec::new(),
            diagnostics: Vec::new(),
            paths: BTreeSet::new(),
        }
    }

    fn paths(of: &[&str]) -> BTreeSet<String> {
        of.iter().map(|p| p.to_string()).collect()
    }

    fn reason<'a>(out: &'a Outlined, path: &str) -> &'a str {
        out.unread
            .iter()
            .find(|u| u.path == path)
            .map(|u| u.reason.as_str())
            .unwrap_or_else(|| panic!("{path} is not unread: {:?}", out.unread))
    }

    #[test]
    fn a_request_with_no_paths_reads_nothing() {
        let repo = tracked(&[("src/a.rs", b"fn a() {}\n")]);
        let ctx = repo.context().expect("a context");
        let mut sel = selection();
        let out = offer(&ctx, &mut sel, &BTreeSet::new(), &BTreeSet::new());
        assert!(out.files.is_empty() && out.unread.is_empty() && out.unparsed.is_empty());
        assert_eq!(out.truncated, 0);
        assert!(sel.candidates.is_empty());
    }

    /// The handle a file was opened as is what decides: one that cannot be judged, is no
    /// regular file, cannot be read, or grew past the bound is refused by name.
    #[test]
    fn an_opened_file_is_judged_by_its_handle() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("the disk went away"))
            }
        }
        let stat = read_opened(&b"x"[..], Err(std::io::Error::other("gone"))).unwrap_err();
        assert!(stat.starts_with("cannot stat: "), "{stat}");
        assert_eq!(
            read_opened(&b"x"[..], Ok(false)).unwrap_err(),
            "not a regular file when it was opened"
        );
        let read = read_opened(Broken, Ok(true)).unwrap_err();
        assert!(read.starts_with("cannot read: "), "{read}");
        let grown = std::io::repeat(b'x').take(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            read_opened(grown, Ok(true)).unwrap_err(),
            "grew past the one-file bound while it was read"
        );
        assert_eq!(
            read_opened(&b"fn a() {}"[..], Ok(true)).unwrap(),
            b"fn a() {}"
        );
    }

    #[test]
    fn every_guard_names_the_file_it_refused() {
        let huge = format!("// {}\n", "x".repeat(MAX_SOURCE_BYTES as usize));
        let repo = tracked(&[
            ("src/ok.rs", b"fn fine() {}\n"),
            ("src/gone.rs", b"fn gone() {}\n"),
            ("src/dir.rs", b"fn dir() {}\n"),
            ("src/huge.rs", huge.as_bytes()),
            ("src/linked/a.rs", b"fn linked() {}\n"),
            ("src/latin1.rs", b"// caf\xe9\nfn latin() {}\n"),
            ("src/locked.rs", b"fn locked() {}\n"),
            ("src/LICENSE", b"text\n"),
        ]);
        let root = repo.root();
        // tracked, then gone from the work tree
        std::fs::remove_file(root.join("src/gone.rs")).unwrap();
        // tracked as a file, a directory in the work tree
        std::fs::remove_file(root.join("src/dir.rs")).unwrap();
        std::fs::create_dir(root.join("src/dir.rs")).unwrap();
        // a parent directory replaced by a link that leaves the repository
        let outside = root.with_extension("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("a.rs"), "fn canary_outside() {}\n").unwrap();
        std::fs::remove_dir_all(root.join("src/linked")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("src/linked")).unwrap();
        // readable by nobody: every check by path passes, and the open is refused
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                root.join("src/locked.rs"),
                std::fs::Permissions::from_mode(0o000),
            )
            .unwrap();
        }

        let ctx = repo.context().expect("a context");
        let mut sel = selection();
        let out = offer(&ctx, &mut sel, &paths(&["src"]), &BTreeSet::new());
        let _ = std::fs::remove_dir_all(&outside);

        assert!(
            reason(&out, "src/gone.rs").starts_with("cannot stat: "),
            "{out:?}"
        );
        assert_eq!(
            reason(&out, "src/dir.rs"),
            "not a regular file in the work tree"
        );
        assert_eq!(
            reason(&out, "src/huge.rs"),
            format!(
                "{} bytes, over the {MAX_SOURCE_BYTES}-byte bound for one file",
                huge.len()
            )
        );
        #[cfg(unix)]
        assert_eq!(
            reason(&out, "src/linked/a.rs"),
            "its real path leaves the repository; not read"
        );
        #[cfg(unix)]
        assert!(
            reason(&out, "src/locked.rs").starts_with("cannot open without following a link: "),
            "{out:?}"
        );
        assert_eq!(reason(&out, "src/latin1.rs"), "not UTF-8 text");
        // a file with no extension is counted under a name that says so
        assert_eq!(out.unparsed.get("(none)"), Some(&1), "{:?}", out.unparsed);
        // the one file every guard admits is the one outlined, and nothing outside leaks
        let read: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(read, ["src/ok.rs"]);
        assert!(sel.candidates.values().all(|c| !c
            .title
            .as_deref()
            .unwrap_or("")
            .contains("canary")));
    }

    #[test]
    fn files_past_the_request_bound_are_counted_and_not_read() {
        let names: Vec<String> = (0..MAX_SOURCE_FILES + 2)
            .map(|i| format!("src/many/f{i:03}.rs"))
            .collect();
        let files: Vec<(&str, &[u8])> = names
            .iter()
            .map(|n| (n.as_str(), &b"fn f() {}\n"[..]))
            .collect();
        let repo = tracked(&files);
        let ctx = repo.context().expect("a context");
        let mut sel = selection();
        let out = offer(&ctx, &mut sel, &paths(&["src/many"]), &BTreeSet::new());
        assert_eq!(out.truncated, 2);
        assert_eq!(out.files.len(), MAX_SOURCE_FILES);
        assert!(out.unread.is_empty(), "{:?}", out.unread);
    }

    #[test]
    fn a_file_no_path_names_exactly_is_reached_through_itself() {
        let repo = tracked(&[("src/a.rs", b"fn a() {}\n")]);
        let ctx = repo.context().expect("a context");
        let mut sel = selection();
        // git reads `src/` as the directory; neither `src/a.rs == src/` nor the prefix
        // `src//` holds, so the way in is the file itself
        let out = offer(&ctx, &mut sel, &paths(&["src/"]), &BTreeSet::new());
        assert_eq!(out.files.len(), 1, "{out:?}");
        let c = sel
            .candidates
            .get("source:src/a.rs#L1-L1")
            .expect("the range");
        assert_eq!(c.discovered_by[0].via.as_deref(), Some("src/a.rs"));
    }

    #[test]
    fn a_root_that_cannot_be_resolved_is_reported_and_nothing_is_read() {
        let repo = tracked(&[("src/a.rs", b"fn a() {}\n")]);
        let mut index = repo.index().expect("an index");
        // git reads an empty `-C` as the working directory, which is this crate's checkout,
        // so the listing succeeds; the empty path itself resolves to nothing
        index.repository.root = String::new();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .with_index(&index)
            .build()
            .expect("a registry");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let mut sel = selection();
        let out = offer(
            &ctx,
            &mut sel,
            &paths(&["src/devcontext/source.rs"]),
            &BTreeSet::new(),
        );
        assert_eq!(out.unread.len(), 1, "{:?}", out.unread);
        assert_eq!(out.unread[0].path, ".");
        assert!(
            out.unread[0]
                .reason
                .starts_with("the repository root cannot be resolved: "),
            "{:?}",
            out.unread
        );
        assert!(out.files.is_empty());
        assert!(sel.candidates.is_empty());
    }
}

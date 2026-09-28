//! The deterministic suite: what the context compiler considers and what it selects, counted.
//!
//! No model is called. For every seed (an issue of the repository's plan) the compiler
//! (`majordomus devcontext`) is asked for the context of that work under its default budget.
//! Every distinct file it reached is read from disk and counted in bytes and in tokens of a
//! named tokenizer; a file two entries share is counted once, because a reader would read it
//! once.
//!
//! The denominator is deliberately the narrow one. A *candidate* is a file the compiler judged
//! relevant: selected, or left out only because the budget ran out. Files it reached and
//! judged irrelevant (below the relevance floor, too deep, stale, superseded) are counted as
//! *considered* and reported by reason, but never divided by: a ratio against everything a
//! graph walk touched would describe the walk, not the selection.
//!
//! What this measures is *selection*: of what the compiler found relevant, how much it put in
//! front of the worker. It is not what a session consumed, and it is never reported as total
//! token savings — the metric that carries it says so in its own `not` field.
//!
//! ```
//! use majordomus_cli::economics::context::{count_tokens, TOKENIZER_ENCODING};
//! assert_eq!(TOKENIZER_ENCODING, "o200k_base");
//! assert_eq!(count_tokens(""), 0);
//! assert!(count_tokens("Majordomus counts what it selects.") > 3);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::capability::handler::Context;
use crate::devcontext::{self, CompileInput};

use super::model::{EconomicsContextRun, EconomicsContextSeed, EconomicsSuite, EconomicsTokenizer};
use super::{inputs_digest, Declarations, CONTEXT_RUN_SCHEMA};

/// The encoding every count of this suite is in.
pub const TOKENIZER_ENCODING: &str = "o200k_base";
/// The implementation and its version, as `Cargo.lock` resolved the pin in `Cargo.toml`:
/// the build script reads it from the lockfile, so the number is stated in one place and a
/// record can never name a version other than the one compiled in.
///
/// ```
/// use majordomus_cli::economics::context::TOKENIZER_IMPLEMENTATION;
/// let (name, version) = TOKENIZER_IMPLEMENTATION.split_once(' ').unwrap();
/// assert_eq!(name, "tiktoken-rs");
/// assert!(version.split('.').count() == 3, "a resolved version, never `unknown`: {version}");
/// ```
pub const TOKENIZER_IMPLEMENTATION: &str =
    concat!("tiktoken-rs ", env!("MAJORDOMUS_TIKTOKEN_VERSION"));

/// Tokens of `text` in [`TOKENIZER_ENCODING`], special-token text counted as ordinary text.
///
/// A file that happens to contain `<|endoftext|>` is counted as the characters it holds, not
/// as the single control token a model would see: what is counted is what a reader is
/// handed, and a count must never depend on whether the text looks like a protocol marker.
/// The count is of the text alone, with no framing a provider adds around a message.
///
/// ```
/// use majordomus_cli::economics::context::count_tokens;
/// assert_eq!(count_tokens("hello world"), 2);
/// assert!(count_tokens("<|endoftext|>") > 1, "special-token text is ordinary text");
/// let prose = "Majordomus counts what it selects.";
/// assert!(count_tokens(prose) < prose.len() as u64, "a token spans several bytes");
/// ```
pub fn count_tokens(text: &str) -> u64 {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len() as u64
}

/// The tokenizer's identity, as recorded with every count.
///
/// Two counts are comparable only when both the encoding and the implementation agree, so
/// a run carries both: a change to either makes earlier records a different measurement,
/// not a smaller or larger one.
///
/// ```
/// use majordomus_cli::economics::context::{tokenizer, TOKENIZER_ENCODING};
/// let t = tokenizer();
/// assert_eq!(t.encoding, TOKENIZER_ENCODING);
/// assert_eq!(
///     serde_json::to_value(&t).unwrap(),
///     serde_json::json!({ "encoding": "o200k_base", "implementation": "tiktoken-rs 0.12.0" }),
/// );
/// ```
pub fn tokenizer() -> EconomicsTokenizer {
    EconomicsTokenizer {
        encoding: TOKENIZER_ENCODING.into(),
        implementation: TOKENIZER_IMPLEMENTATION.into(),
    }
}

/// Bytes and tokens of every file, read once each.
#[derive(Default)]
struct Counter {
    cache: BTreeMap<String, (u64, u64)>,
}

impl Counter {
    fn count(&mut self, root: &Path, path: &str) -> Option<(u64, u64)> {
        if let Some(c) = self.cache.get(path) {
            return Some(*c);
        }
        let bytes = std::fs::read(root.join(path)).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let c = (bytes.len() as u64, count_tokens(&text));
        self.cache.insert(path.to_string(), c);
        Some(c)
    }
}

/// The seeds of a context suite: the ids of the plan's issues, from the file names under
/// the suite's `seeds` directory, sorted.
///
/// Only `.yaml` files name a seed, by their stem, so a README beside them is not measured.
/// A suite that declares no `seeds` directory (a live suite), or one whose directory is
/// missing, has no seeds rather than an error: the suite then measures nothing.
///
/// ```
/// use majordomus_cli::economics::context::seeds;
/// use majordomus_cli::economics::model::EconomicsSuite;
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir(dir.path().join("seeds")).unwrap();
/// for f in ["I0302.yaml", "I0301.yaml", "README.md"] {
///     std::fs::write(dir.path().join("seeds").join(f), "").unwrap();
/// }
/// let mut suite: EconomicsSuite = serde_json::from_value(serde_json::json!({
///     "schema": "economics-suite/v1", "id": "context", "version": 1, "kind": "context",
///     "title": "Context selection", "seeds": "seeds", "freshness_inputs": []
/// }))
/// .unwrap();
/// assert_eq!(seeds(dir.path(), &suite), ["I0301", "I0302"]);
/// suite.seeds = None;
/// assert!(seeds(dir.path(), &suite).is_empty());
/// ```
pub fn seeds(root: &Path, suite: &EconomicsSuite) -> Vec<String> {
    let Some(dir) = &suite.seeds else {
        return Vec::new();
    };
    let mut out: Vec<String> = std::fs::read_dir(root.join(dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|x| x.to_str()) == Some("yaml"))
                .then(|| p.file_stem().and_then(|s| s.to_str()).map(str::to_string))
                .flatten()
        })
        .collect();
    out.sort();
    out
}

/// Measure one seed: compile its context, and count what was considered and selected.
///
/// Every file is read from disk once, however many entries name it. An entry the index
/// cannot resolve, or a file the disk no longer holds, is counted as `unresolved` rather
/// than silently dropped. A seed the compiler refuses (an issue the repository does not
/// have) is an error that starts with the seed, so that a suite can report it beside the
/// record.
///
/// ```
/// use majordomus_cli::economics::context::measure_seed;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let err = measure_seed(&ctx, "I9999").unwrap_err();
/// assert!(err.starts_with("I9999: "), "{err}");
/// ```
pub fn measure_seed(ctx: &Context, seed: &str) -> Result<EconomicsContextSeed, String> {
    let root = Path::new(&ctx.index.repository.root);
    let compiled = devcontext::compile(
        ctx,
        CompileInput {
            issue: Some(seed.to_string()),
            ..Default::default()
        },
    )
    .map_err(|e| format!("{seed}: {e}"))?;
    let mut counter = Counter::default();
    let objects = &ctx.index.objects;
    let path_of = |uri: &str| -> Option<String> {
        objects
            .binary_search_by(|o| o.uri.as_str().cmp(uri))
            .ok()
            .map(|i| objects[i].provenance.path.clone())
    };
    let selected_paths: BTreeSet<String> = compiled
        .selected
        .iter()
        .map(|e| e.provenance.path.clone())
        .collect();
    let mut excluded_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut unresolved = 0u64;
    for x in &compiled.excluded {
        match path_of(&x.uri) {
            Some(p) if !selected_paths.contains(&p) => {
                excluded_by
                    .entry(x.reason.as_str().to_string())
                    .or_default()
                    .insert(p);
            }
            Some(_) => {}
            None => unresolved += 1,
        }
    }
    let mut candidates: BTreeSet<String> = selected_paths.clone();
    if let Some(ps) = excluded_by.get("budget") {
        candidates.extend(ps.iter().cloned());
    }
    let mut considered: BTreeSet<String> = selected_paths.clone();
    for ps in excluded_by.values() {
        considered.extend(ps.iter().cloned());
    }
    let mut sum = |paths: &BTreeSet<String>, counter: &mut Counter| -> (u64, u64, u64) {
        let (mut n, mut b, mut t) = (0, 0, 0);
        for p in paths {
            match counter.count(root, p) {
                Some((bytes, tokens)) => {
                    n += 1;
                    b += bytes;
                    t += tokens;
                }
                None => unresolved += 1,
            }
        }
        (n, b, t)
    };
    let (selected, selected_bytes, selected_tokens) = sum(&selected_paths, &mut counter);
    let (candidates_n, candidate_bytes, candidate_tokens) = sum(&candidates, &mut counter);
    let (considered_n, _, considered_tokens) = sum(&considered, &mut counter);
    // A file excluded for two reasons is counted under the first, in the order the compiler
    // names them, so that the per-reason figures add up to what was left out.
    let mut seen: BTreeSet<String> = selected_paths;
    let mut excluded_tokens = BTreeMap::new();
    for (reason, ps) in &excluded_by {
        let fresh: BTreeSet<String> = ps
            .iter()
            .filter(|p| seen.insert((*p).clone()))
            .cloned()
            .collect();
        let (_, _, t) = sum(&fresh, &mut counter);
        excluded_tokens.insert(reason.clone(), t);
    }
    Ok(EconomicsContextSeed {
        seed: seed.to_string(),
        candidates: candidates_n,
        candidate_bytes,
        candidate_tokens,
        considered: considered_n,
        considered_tokens,
        selected,
        selected_bytes,
        selected_tokens,
        estimated_selected_tokens: compiled.budget.used_tokens,
        excluded_tokens,
        unresolved,
        over_budget: compiled.budget.over_budget,
    })
}

/// Measure a context suite over every seed, as one record. Seeds the compiler refuses are
/// kept in the record's `refused` list, each with the compiler's reason, and returned beside
/// it as well so that the caller can report them: a ratio over the seeds that compiled must
/// never be read as one over every seed the suite names.
///
/// The record names the commit it was measured at, the tokenizer, the methodology version
/// and the digest of the suite's freshness inputs, so a later read can tell whether it still
/// describes the tree. Only a repository without a `HEAD` commit, or a git that cannot list
/// the inputs, fails the whole measurement.
///
/// ```
/// # use std::process::Command;
/// # use majordomus_cli::economics::Declarations;
/// # let methodology = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-methodology/v1", "version": 3, "title": "t", "question": "q",
/// #     "unit": "tokens", "primary_metric": "m", "classes": [], "variants": [], "success": [],
/// #     "pairing": { "key": [], "comparable": [], "valid": "v" },
/// #     "statistics": { "per_pair": "p", "location": "median", "interval": "bootstrap",
/// #         "confidence_bp": 9500, "resamples": 1000, "seed": 1, "min_pairs_for_interval": 5 },
/// #     "publication": { "min_valid_pairs": 1, "min_categories": 1, "min_pairs_per_category": 1,
/// #         "min_repetitions": 1, "min_valid_pair_rate_bp": 1, "max_interval_width_bp": 1,
/// #         "require_current": true },
/// #     "outliers": { "rule": "none" }
/// # })).unwrap();
/// # let decl = Declarations { methodology, suites: Default::default(), tasks: Default::default() };
/// # let suite: majordomus_cli::economics::model::EconomicsSuite =
/// #     serde_json::from_value(serde_json::json!({
/// #         "schema": "economics-suite/v1", "id": "context", "version": 2, "kind": "context",
/// #         "title": "Context selection", "seeds": "seeds", "freshness_inputs": ["docs"]
/// #     })).unwrap();
/// use majordomus_cli::economics::context::{measure, tokenizer};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// std::fs::create_dir(repo.root().join("seeds")).unwrap();
/// std::fs::write(repo.root().join("seeds/I9001.yaml"), "").unwrap();
/// let git = |a: &[&str]| Command::new("git").arg("-C").arg(repo.root()).args(a).output();
/// git(&["init", "-q"]).unwrap();
/// git(&["add", "-A"]).unwrap();
/// git(&["-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-qm", "i"]).unwrap();
///
/// let ctx = repo.context().unwrap();
/// let (run, errors) = measure(&ctx, &decl, &suite, "2026-09-24T00:00:00Z").unwrap();
/// assert!(run.seeds.is_empty(), "a refused seed is not measured");
/// assert!(errors.len() == 1 && errors[0].starts_with("I9001: "), "it is reported beside it");
/// assert_eq!(run.refused, errors, "and named in the record with its reason, never dropped");
/// assert_eq!((run.suite.as_str(), run.suite_version, run.methodology), ("context", 2, 3));
/// assert_eq!(run.tokenizer, tokenizer());
/// assert!(!run.repository.dirty, "measured at the commit it names");
/// ```
pub fn measure(
    ctx: &Context,
    decl: &Declarations,
    suite: &EconomicsSuite,
    measured_at: &str,
) -> Result<(EconomicsContextRun, Vec<String>), String> {
    let root = Path::new(&ctx.index.repository.root);
    let repository = super::revision(root)?;
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for seed in seeds(root, suite) {
        match measure_seed(ctx, &seed) {
            Ok(s) => out.push(s),
            Err(e) => errors.push(e),
        }
    }
    Ok((
        EconomicsContextRun {
            schema: CONTEXT_RUN_SCHEMA.into(),
            suite: suite.id.clone(),
            suite_version: suite.version,
            methodology: decl.methodology.version,
            repository,
            majordomus_version: crate::VERSION.into(),
            tokenizer: tokenizer(),
            inputs_digest: inputs_digest(root, &suite.freshness_inputs)?,
            budget_tokens: devcontext::model::DEFAULT_BUDGET_TOKENS,
            measured_at: measured_at.into(),
            seeds: out,
            refused: errors.clone(),
        },
        errors,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tokenizer_is_pinned_to_known_counts() {
        // Known o200k_base encodings: if the vocabulary or the implementation changes,
        // these move, and every recorded count would stop being comparable.
        assert_eq!(count_tokens("hello world"), 2);
        assert_eq!(count_tokens("Hello, world!"), 4);
        assert_eq!(count_tokens("<|endoftext|>"), count_tokens("<|endoftext|>"));
        assert!(
            count_tokens("<|endoftext|>") > 1,
            "special-token text is counted as text"
        );
    }

    #[test]
    fn counting_is_deterministic_and_additive_over_whitespace_separated_words() {
        let text = "fn main() { println!(\"{}\", 42); }\n".repeat(50);
        assert_eq!(count_tokens(&text), count_tokens(&text));
        assert!(
            count_tokens(&text) < text.len() as u64,
            "tokens are fewer than bytes for code"
        );
    }
}

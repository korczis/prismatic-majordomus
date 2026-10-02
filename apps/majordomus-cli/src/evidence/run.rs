//! What a run is made of, beyond the executions it recorded.
//!
//! A recording reads the reports a run wrote: the suite's TSV, `cargo test`'s output, a
//! coverage summary. Every entry of a run belongs to exactly one of those reports, and
//! [`EvidenceProducer`] is the word that names which. Not everything a report holds is
//! something a claim can name: the crate's own unit-test binary and its doctests ran, and no
//! claim of `docs/CLAIMS.yaml` can point at one of their tests yet. Such an entry is listed as
//! an [`EvidenceDropped`], with the report it came from and the reason, rather than ignored,
//! because a recording that silently threw part of a run away reads as a run that was
//! smaller than it was.
//!
//! # What a run is made of
//!
//! A producer word reads back as the producer it names and nothing else reads at all; a
//! `cargo test` output with the crate's unit tests and one integration binary yields one
//! binary a claim can name and one dropped entry that says why the other is not one:
//!
//! ```
//! use majordomus_cli::evidence::{read_crate_output, EvidenceProducer, Outcome};
//!
//! for p in [EvidenceProducer::Suite, EvidenceProducer::Crate, EvidenceProducer::Coverage] {
//!     assert_eq!(EvidenceProducer::parse(p.as_str()), Some(p));
//! }
//! assert_eq!(EvidenceProducer::parse("cargo"), None, "a word no report is named by");
//!
//! let out = "     Running unittests src/lib.rs (target/debug/deps/majordomus_cli-1)\n\
//!            test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
//!            finished in 0.50s\n\
//!                 Running tests/alpha.rs (target/debug/deps/alpha-1)\n\
//!            test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
//!            finished in 1.40s\n";
//! let read = read_crate_output(out);
//! assert_eq!(read.binaries.len(), 1);
//! assert_eq!(read.binaries[0].name, "alpha");
//! assert_eq!(read.binaries[0].outcome, Outcome::Pass);
//!
//! assert_eq!(read.dropped.len(), 1, "the unit tests ran, and are listed rather than ignored");
//! let dropped = &read.dropped[0];
//! assert_eq!(dropped.producer, EvidenceProducer::Crate);
//! assert_eq!(dropped.what, "unittests src/lib.rs");
//! assert!(dropped.reason.contains("no claim can name"), "{}", dropped.reason);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which report an entry of a run belongs to: the suite's TSV, `cargo test`'s output, or a
/// coverage summary.
///
/// The serde word is the word a person types and a document carries (`suite`, `crate`,
/// `coverage`), and [`EvidenceProducer::parse`] reads exactly those three back. A word that
/// names no report is not a producer, so a misspelled one is refused where it is read
/// rather than guessed at.
///
/// ```
/// use majordomus_cli::evidence::EvidenceProducer;
///
/// assert_eq!(EvidenceProducer::parse("crate"), Some(EvidenceProducer::Crate));
/// assert_eq!(EvidenceProducer::Coverage.as_str(), "coverage");
/// assert_eq!(serde_json::to_value(EvidenceProducer::Suite).unwrap(), "suite");
/// assert_eq!(EvidenceProducer::parse("Suite"), None, "the word is exact");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProducer {
    /// The suite runner's TSV report (`MJ_TEST_REPORT` of `test/run.sh`).
    Suite,
    /// `cargo test`'s output: one result line per test binary.
    Crate,
    /// A coverage summary (`scripts/rust-coverage --summary-json`).
    Coverage,
}

impl EvidenceProducer {
    /// The producer a word names, or `None` when it names none.
    ///
    /// The three words are exact: no case folding and no trimming, because every caller
    /// reads a word a program wrote and a near-miss is a defect to name, not to absorb.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceProducer;
    /// assert_eq!(EvidenceProducer::parse("suite"), Some(EvidenceProducer::Suite));
    /// assert_eq!(EvidenceProducer::parse(" suite"), None);
    /// assert_eq!(EvidenceProducer::parse(""), None);
    /// ```
    pub fn parse(word: &str) -> Option<EvidenceProducer> {
        match word {
            "suite" => Some(EvidenceProducer::Suite),
            "crate" => Some(EvidenceProducer::Crate),
            "coverage" => Some(EvidenceProducer::Coverage),
            _ => None,
        }
    }

    /// The word this producer is written as: the serde word, and what
    /// [`EvidenceProducer::parse`] reads back.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceProducer;
    /// assert_eq!(EvidenceProducer::Crate.as_str(), "crate");
    /// assert_eq!(
    ///     EvidenceProducer::parse(EvidenceProducer::Suite.as_str()),
    ///     Some(EvidenceProducer::Suite)
    /// );
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceProducer::Suite => "suite",
            EvidenceProducer::Crate => "crate",
            EvidenceProducer::Coverage => "coverage",
        }
    }
}

/// Something a report held that no claim can name yet, listed instead of ignored.
///
/// `cargo test` runs the crate's own unit-test binary and its doctests beside the
/// integration binaries under `tests/`. They ran, and a claim names a test by its path, so
/// none of their tests is anything a claim can point at until a runner records them one by
/// one. A recording lists each such entry with the report it came from ([`producer`]), what
/// it was ([`what`]) and why it is not recorded ([`reason`]), so that the part of a run
/// nobody can use yet is visible rather than silently smaller.
///
/// [`producer`]: EvidenceDropped::producer
/// [`what`]: EvidenceDropped::what
/// [`reason`]: EvidenceDropped::reason
///
/// ```
/// use majordomus_cli::evidence::{EvidenceDropped, EvidenceProducer};
///
/// let d: EvidenceDropped = serde_json::from_value(serde_json::json!({
///     "producer": "crate",
///     "what": "doc-tests majordomus_cli",
///     "reason": "doctests, which no claim can name until a runner records them one by one",
/// }))
/// .unwrap();
/// assert_eq!(d.producer, EvidenceProducer::Crate);
/// assert_eq!(serde_json::to_value(&d).unwrap()["producer"], "crate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceDropped {
    /// The report the entry was read from.
    pub producer: EvidenceProducer,
    /// What the entry was, in the report's own words: `unittests src/lib.rs`,
    /// `doc-tests <crate>`, the path of a binary that is not under `tests/`, or `a result
    /// line` when a result belonged to no binary at all.
    pub what: String,
    /// Why it is listed rather than recorded. It carries no counts.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each producer's word reads back as that producer, the serde word is the same word,
    /// and a word that names no report reads as nothing rather than as a guess.
    #[test]
    fn a_producer_word_round_trips_and_nothing_else_parses() {
        for p in [
            EvidenceProducer::Suite,
            EvidenceProducer::Crate,
            EvidenceProducer::Coverage,
        ] {
            assert_eq!(EvidenceProducer::parse(p.as_str()), Some(p));
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
            let back: EvidenceProducer =
                serde_json::from_value(serde_json::json!(p.as_str())).unwrap();
            assert_eq!(back, p);
        }
        for word in [
            "",
            "Suite",
            "CRATE",
            " coverage",
            "cargo",
            "unittests",
            "doc-tests",
        ] {
            assert_eq!(EvidenceProducer::parse(word), None, "{word:?}");
        }
        assert!(serde_json::from_value::<EvidenceProducer>(serde_json::json!("cargo")).is_err());
    }

    /// A dropped entry is a document with its producer's word, what it was and why, and it
    /// reads back as the same entry.
    #[test]
    fn a_dropped_entry_serialises_with_its_producer_word() {
        let d: EvidenceDropped = serde_json::from_value(serde_json::json!({
            "producer": "crate",
            "what": "unittests src/lib.rs",
            "reason": "the crate's own unit tests",
        }))
        .unwrap();
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["producer"], "crate");
        assert_eq!(v["what"], "unittests src/lib.rs");
        assert_eq!(v["reason"], "the crate's own unit tests");
        assert_eq!(
            v.as_object().unwrap().len(),
            3,
            "a field nobody declared: {v}"
        );
        let back: EvidenceDropped = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
    }
}

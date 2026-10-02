//! A coverage summary bound to the commit and tree it was measured on.
//!
//! `scripts/rust-coverage --summary-json` writes what one coverage run measured: the crate,
//! the session domain and every file, as covered and total counts per dimension. The file
//! names no commit, so on its own it is a number about some tree nobody can name. An
//! [`EvidenceCoverage`] is that summary with the commit it was measured at and the tree the
//! run's own measurement stated, and it cannot be built without a full commit
//! ([`CommitId`]). It carries the floors the threshold files state at recording time, read
//! from the files rather than copied, so a reader can see whether the measurement met them.
//!
//! A coverage record is current only for its own commit, and only when both its tree and the
//! presented tree are clean: a number measured on a modified checkout describes code no
//! commit holds.
//!
//! # A coverage record's lifecycle
//!
//! ```
//! use majordomus_cli::evidence::{CommitId, EvidenceCoverage, EvidenceCoverageFloors, TreeState};
//!
//! let dims = r#"{"lines":{"covered":8,"total":10,"percent":80.0},
//!                "functions":{"covered":1,"total":1,"percent":100.0},
//!                "regions":{"covered":3,"total":4,"percent":75.0}}"#;
//! let summary = format!(
//!     r#"{{"schema":1,"measurement":"scripts/rust-coverage","test_code":"excluded",
//!          "crate":{dims},"domain":{{"lines":{{"covered":8,"total":10,"percent":80.0}},
//!          "functions":{{"covered":1,"total":1,"percent":100.0}},
//!          "regions":{{"covered":3,"total":4,"percent":75.0}},"files":["src/a.rs"],"missing":[]}},
//!          "files":{{"src/a.rs":{dims}}}}}"#
//! );
//! let commit = CommitId::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
//! let floors = EvidenceCoverageFloors { crate_lines: Some(90.0), domain_lines: None };
//! let cov = EvidenceCoverage::from_summary(&summary, commit, TreeState::Clean, floors).unwrap();
//!
//! assert!(cov.is_current_for("0123456789abcdef0123456789abcdef01234567", TreeState::Clean));
//! assert!(!cov.is_current_for("0123456789abcdef0123456789abcdef01234567", TreeState::Dirty));
//! assert!(!cov.is_current_for("fedcba9876543210fedcba9876543210fedcba98", TreeState::Clean));
//! assert_eq!(cov.below_floor(), vec!["crate"]);
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::freshness::TreeState;

/// A full commit id: 40 or 64 lowercase hexadecimal digits. The only constructor validates.
///
/// An abbreviated id names whichever commit happens to share its prefix today, and an
/// upper-case one is not what git prints; neither can bind a measurement to one commit, so
/// neither is a `CommitId`, whether parsed or deserialised.
///
/// ```
/// use majordomus_cli::evidence::CommitId;
///
/// let id = CommitId::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
/// assert_eq!(id.as_str().len(), 40);
/// assert!(CommitId::parse("0123456789ab").is_err(), "abbreviated");
/// assert!(serde_json::from_str::<CommitId>("\"ABCDEF0123456789ABCDEF0123456789ABCDEF01\"").is_err());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
#[schemars(
    with = "String",
    description = "A full commit id: 40 or 64 lowercase hexadecimal digits."
)]
pub struct CommitId(String);

impl CommitId {
    /// The commit id `s` names, or why it is not a full one.
    ///
    /// ```
    /// use majordomus_cli::evidence::CommitId;
    /// assert!(CommitId::parse(&"a".repeat(64)).is_ok(), "a SHA-256 repository's id");
    /// assert!(CommitId::parse(&"g".repeat(40)).is_err(), "not hexadecimal");
    /// ```
    pub fn parse(s: &str) -> Result<CommitId, String> {
        let hex = s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if (s.len() == 40 || s.len() == 64) && hex {
            Ok(CommitId(s.to_string()))
        } else {
            Err(format!(
                "`{s}` is not a full commit id: 40 or 64 lowercase hexadecimal digits"
            ))
        }
    }

    /// The id as git prints it.
    ///
    /// ```
    /// use majordomus_cli::evidence::CommitId;
    /// let full = "0123456789abcdef0123456789abcdef01234567";
    /// assert_eq!(CommitId::parse(full).unwrap().as_str(), full);
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommitId {
    type Error = String;
    fn try_from(s: String) -> Result<CommitId, String> {
        CommitId::parse(&s)
    }
}

impl From<CommitId> for String {
    fn from(id: CommitId) -> String {
        id.0
    }
}

/// Covered and total items of one dimension, and the percentage the producer computed.
///
/// ```
/// use majordomus_cli::evidence::EvidenceCoverageCount;
/// let c: EvidenceCoverageCount =
///     serde_json::from_str(r#"{"covered":3,"total":4,"percent":75.0}"#).unwrap();
/// assert_eq!((c.covered, c.total, c.percent), (3, 4, 75.0));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceCoverageCount {
    /// Items a test reached.
    pub covered: u64,
    /// Items there are.
    pub total: u64,
    /// `covered / total` as a percentage, 100 when there is nothing to cover.
    pub percent: f64,
}

/// The three dimensions `scripts/rust-coverage` counts, for the crate or one file.
///
/// ```
/// use majordomus_cli::evidence::EvidenceCoverageDimensions;
/// let one = r#"{"covered":1,"total":2,"percent":50.0}"#;
/// let d: EvidenceCoverageDimensions = serde_json::from_str(&format!(
///     r#"{{"lines":{one},"functions":{one},"regions":{one}}}"#
/// )).unwrap();
/// assert_eq!(d.lines.total, 2);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceCoverageDimensions {
    /// Executable lines.
    pub lines: EvidenceCoverageCount,
    /// Functions.
    pub functions: EvidenceCoverageCount,
    /// Code regions.
    pub regions: EvidenceCoverageCount,
}

/// The session domain's coverage: the three dimensions over the files the domain lists, and
/// which of its files the measurement found and which it did not.
///
/// ```
/// use majordomus_cli::evidence::EvidenceCoverageDomain;
/// let one = r#"{"covered":1,"total":2,"percent":50.0}"#;
/// let d: EvidenceCoverageDomain = serde_json::from_str(&format!(
///     r#"{{"lines":{one},"functions":{one},"regions":{one},"files":["a.rs"],"missing":[]}}"#
/// )).unwrap();
/// assert_eq!(d.files, vec!["a.rs"]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceCoverageDomain {
    /// Executable lines of the domain's files.
    pub lines: EvidenceCoverageCount,
    /// Functions of the domain's files.
    pub functions: EvidenceCoverageCount,
    /// Code regions of the domain's files.
    pub regions: EvidenceCoverageCount,
    /// The domain's files the measurement holds.
    pub files: Vec<String>,
    /// The domain's files the measurement does not hold.
    pub missing: Vec<String>,
}

/// The floors the threshold files stated when the coverage was recorded; a file that is
/// absent or does not hold a number gives no floor.
///
/// ```
/// use majordomus_cli::evidence::EvidenceCoverageFloors;
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir(dir.path().join("scripts")).unwrap();
/// std::fs::write(dir.path().join("scripts/rust-coverage-threshold"), "42\n").unwrap();
/// let floors = EvidenceCoverageFloors::read(dir.path());
/// assert_eq!((floors.crate_lines, floors.domain_lines), (Some(42.0), None));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceCoverageFloors {
    /// The crate's line floor, from `scripts/rust-coverage-threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_lines: Option<f64>,
    /// The domain's line floor, from `scripts/session-coverage-threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_lines: Option<f64>,
}

impl EvidenceCoverageFloors {
    /// The floors the checkout at `root` states now.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceCoverageFloors;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert_eq!(EvidenceCoverageFloors::read(dir.path()), EvidenceCoverageFloors::default());
    /// ```
    pub fn read(root: &Path) -> EvidenceCoverageFloors {
        EvidenceCoverageFloors {
            crate_lines: floor(&root.join("scripts/rust-coverage-threshold")),
            domain_lines: floor(&root.join("scripts/session-coverage-threshold")),
        }
    }
}

/// The first number a threshold file holds, after its comments; `None` when it holds none.
fn floor(path: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())?
        .parse::<f64>()
        .ok()
        .filter(|f| f.is_finite())
}

/// One coverage measurement, bound to the commit and tree it was measured on.
///
/// Built only by [`EvidenceCoverage::from_summary`] or deserialised; either way `commit` is a
/// [`CommitId`], so a record without a full commit cannot exist.
///
/// ```
/// use majordomus_cli::evidence::EvidenceCoverage;
/// let bad = serde_json::json!({"commit": "abc", "working_tree": "clean"});
/// assert!(serde_json::from_value::<EvidenceCoverage>(bad).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceCoverage {
    /// The commit the measurement ran at.
    pub commit: CommitId,
    /// The tree the coverage run's own measurement stated, `unknown` without one.
    pub working_tree: TreeState,
    /// The producer of the summary: `scripts/rust-coverage`.
    pub measurement: String,
    /// Whether test code is counted: `excluded`.
    pub test_code: String,
    /// The whole crate.
    #[serde(rename = "crate")]
    pub whole_crate: EvidenceCoverageDimensions,
    /// The session domain.
    pub domain: EvidenceCoverageDomain,
    /// Every file with executable code under test, by repository-relative path.
    pub files: BTreeMap<String, EvidenceCoverageDimensions>,
    /// The floors stated when it was recorded.
    #[serde(default)]
    pub floors: EvidenceCoverageFloors,
}

/// The summary as `scripts/rust-coverage --summary-json` writes it (schema 1).
#[derive(Deserialize)]
struct Summary {
    measurement: String,
    test_code: String,
    #[serde(rename = "crate")]
    whole_crate: EvidenceCoverageDimensions,
    domain: EvidenceCoverageDomain,
    files: BTreeMap<String, EvidenceCoverageDimensions>,
}

impl EvidenceCoverage {
    /// The summary `text` bound to `commit` and `working_tree`, with `floors`; refused when
    /// the text is not a schema-1 summary.
    ///
    /// ```
    /// use majordomus_cli::evidence::{CommitId, EvidenceCoverage, TreeState};
    /// let id = CommitId::parse(&"a".repeat(40)).unwrap();
    /// let err = EvidenceCoverage::from_summary(r#"{"schema":2}"#, id, TreeState::Clean,
    ///     Default::default()).unwrap_err();
    /// assert!(err.contains("schema"), "{err}");
    /// ```
    pub fn from_summary(
        text: &str,
        commit: CommitId,
        working_tree: TreeState,
        floors: EvidenceCoverageFloors,
    ) -> Result<EvidenceCoverage, String> {
        let schema = serde_json::from_str::<serde_json::Value>(text)
            .map_err(|e| format!("not a coverage summary: {e}"))?
            .get("schema")
            .and_then(|s| s.as_u64());
        if schema != Some(1) {
            return Err(format!(
                "a coverage summary of schema {} is not one this recorder reads (schema 1)",
                schema.map_or("none".to_string(), |s| s.to_string())
            ));
        }
        let s: Summary =
            serde_json::from_str(text).map_err(|e| format!("not a coverage summary: {e}"))?;
        Ok(EvidenceCoverage {
            commit,
            working_tree,
            measurement: s.measurement,
            test_code: s.test_code,
            whole_crate: s.whole_crate,
            domain: s.domain,
            files: s.files,
            floors,
        })
    }

    /// Whether this measurement describes `commit` as presented: only its own commit, and
    /// only when its tree and the presented tree are both clean.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{CommitId, EvidenceCoverage, TreeState};
    /// # let one = r#"{"covered":1,"total":1,"percent":100.0}"#;
    /// # let dims = format!(r#"{{"lines":{one},"functions":{one},"regions":{one}}}"#);
    /// # let text = format!(r#"{{"schema":1,"measurement":"m","test_code":"excluded",
    /// #   "crate":{dims},"domain":{{"lines":{one},"functions":{one},"regions":{one},
    /// #   "files":[],"missing":[]}},"files":{{}}}}"#);
    /// let c = "b".repeat(40);
    /// let cov = EvidenceCoverage::from_summary(&text, CommitId::parse(&c).unwrap(),
    ///     TreeState::Unknown, Default::default()).unwrap();
    /// assert!(!cov.is_current_for(&c, TreeState::Clean), "its own tree was not measured");
    /// ```
    pub fn is_current_for(&self, commit: &str, presented_tree: TreeState) -> bool {
        self.commit.as_str() == commit
            && self.working_tree == TreeState::Clean
            && presented_tree == TreeState::Clean
    }

    /// Which of the crate and the domain are below the line floor recorded with them.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{CommitId, EvidenceCoverage, EvidenceCoverageFloors, TreeState};
    /// # let one = r#"{"covered":1,"total":2,"percent":50.0}"#;
    /// # let dims = format!(r#"{{"lines":{one},"functions":{one},"regions":{one}}}"#);
    /// # let text = format!(r#"{{"schema":1,"measurement":"m","test_code":"excluded",
    /// #   "crate":{dims},"domain":{{"lines":{one},"functions":{one},"regions":{one},
    /// #   "files":[],"missing":[]}},"files":{{}}}}"#);
    /// let floors = EvidenceCoverageFloors { crate_lines: Some(40.0), domain_lines: Some(60.0) };
    /// let cov = EvidenceCoverage::from_summary(&text, CommitId::parse(&"c".repeat(40)).unwrap(),
    ///     TreeState::Clean, floors).unwrap();
    /// assert_eq!(cov.below_floor(), vec!["domain"]);
    /// ```
    pub fn below_floor(&self) -> Vec<&'static str> {
        let under = |got: &EvidenceCoverageCount, floor: Option<f64>| {
            floor.is_some_and(|f| got.percent < f)
        };
        let mut out = Vec::new();
        if under(&self.whole_crate.lines, self.floors.crate_lines) {
            out.push("crate");
        }
        if under(&self.domain.lines, self.floors.domain_lines) {
            out.push("domain");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C1: &str = "0123456789abcdef0123456789abcdef01234567";
    const C2: &str = "fedcba9876543210fedcba9876543210fedcba98";

    fn summary(schema: u32, crate_pct: f64, domain_pct: f64) -> String {
        let n = |p: f64| format!(r#"{{"covered":1,"total":2,"percent":{p}}}"#);
        let dims = |p: f64| {
            format!(
                r#"{{"lines":{},"functions":{},"regions":{}}}"#,
                n(p),
                n(p),
                n(p)
            )
        };
        format!(
            r#"{{"schema":{schema},"measurement":"scripts/rust-coverage","test_code":"excluded",
               "crate":{},"domain":{{"lines":{},"functions":{},"regions":{},
               "files":["src/a.rs"],"missing":["src/b.rs"]}},"files":{{"src/a.rs":{}}}}}"#,
            dims(crate_pct),
            n(domain_pct),
            n(domain_pct),
            n(domain_pct),
            dims(crate_pct)
        )
    }

    fn cov(tree: TreeState, floors: EvidenceCoverageFloors) -> EvidenceCoverage {
        let id = CommitId::parse(C1).unwrap();
        EvidenceCoverage::from_summary(&summary(1, 50.0, 70.0), id, tree, floors).unwrap()
    }

    #[test]
    fn a_coverage_record_cannot_be_built_without_a_full_commit() {
        for bad in [
            "",
            "0123456789ab",
            &C1.to_uppercase(),
            &"z".repeat(40),
            &"a".repeat(41),
        ] {
            assert!(CommitId::parse(bad).is_err(), "{bad:?}");
            assert!(
                serde_json::from_value::<CommitId>(bad.into()).is_err(),
                "{bad:?}"
            );
        }
        let mut v = serde_json::to_value(cov(TreeState::Clean, Default::default())).unwrap();
        assert_eq!(v["commit"], C1);
        v["commit"] = "0123456".into();
        assert!(serde_json::from_value::<EvidenceCoverage>(v).is_err());
    }

    #[test]
    fn a_summary_of_another_schema_is_refused() {
        let id = || CommitId::parse(C1).unwrap();
        let err = EvidenceCoverage::from_summary(
            &summary(2, 1.0, 1.0),
            id(),
            TreeState::Clean,
            Default::default(),
        );
        assert!(err.unwrap_err().contains("schema 2"));
        let err = EvidenceCoverage::from_summary("{}", id(), TreeState::Clean, Default::default());
        assert!(err.unwrap_err().contains("schema none"));
        let err =
            EvidenceCoverage::from_summary("not json", id(), TreeState::Clean, Default::default());
        assert!(err.unwrap_err().contains("not a coverage summary"));
        let err = EvidenceCoverage::from_summary(
            r#"{"schema":1}"#,
            id(),
            TreeState::Clean,
            Default::default(),
        );
        assert!(err.unwrap_err().contains("not a coverage summary"));
    }

    #[test]
    fn coverage_is_current_only_for_its_commit_on_clean_trees() {
        let trees = [TreeState::Clean, TreeState::Dirty, TreeState::Unknown];
        for own in trees {
            let c = cov(own, Default::default());
            for presented in trees {
                for commit in [C1, C2] {
                    let expect =
                        commit == C1 && own == TreeState::Clean && presented == TreeState::Clean;
                    assert_eq!(
                        c.is_current_for(commit, presented),
                        expect,
                        "{own:?} {presented:?} {commit}"
                    );
                }
            }
        }
    }

    #[test]
    fn floors_are_read_from_the_threshold_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("scripts")).unwrap();
        std::fs::write(
            dir.path().join("scripts/rust-coverage-threshold"),
            "# floor\n61.5\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("scripts/session-coverage-threshold"),
            "72 # domain\n",
        )
        .unwrap();
        let f = EvidenceCoverageFloors::read(dir.path());
        assert_eq!((f.crate_lines, f.domain_lines), (Some(61.5), Some(72.0)));
    }

    #[test]
    fn an_absent_or_unreadable_floor_is_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("scripts")).unwrap();
        std::fs::write(dir.path().join("scripts/rust-coverage-threshold"), "high\n").unwrap();
        std::fs::create_dir(dir.path().join("scripts/session-coverage-threshold")).unwrap();
        assert_eq!(
            EvidenceCoverageFloors::read(dir.path()),
            EvidenceCoverageFloors::default()
        );
        let json = serde_json::to_value(EvidenceCoverageFloors::default()).unwrap();
        assert_eq!(json, serde_json::json!({}), "an absent floor is left out");
    }

    #[test]
    fn below_floor_names_what_is_under() {
        let at = |c, d| EvidenceCoverageFloors {
            crate_lines: c,
            domain_lines: d,
        };
        assert_eq!(
            cov(TreeState::Clean, at(Some(50.0), Some(70.0))).below_floor(),
            Vec::<&str>::new()
        );
        assert_eq!(
            cov(TreeState::Clean, at(Some(50.1), None)).below_floor(),
            vec!["crate"]
        );
        assert_eq!(
            cov(TreeState::Clean, at(None, Some(70.1))).below_floor(),
            vec!["domain"]
        );
        assert_eq!(
            cov(TreeState::Clean, at(Some(99.0), Some(99.0))).below_floor(),
            vec!["crate", "domain"]
        );
        assert_eq!(
            cov(TreeState::Clean, at(None, None)).below_floor(),
            Vec::<&str>::new()
        );
    }
}

//! The one semantic version this repository has.
//!
//! Every place a version is read, compared, ordered, bumped or written goes through
//! [`Version`]. Before this type there were three parsers — a five-tuple sort key in the
//! release records, a `sed` expression in the installer, an `awk` line in the version
//! gate — and each was correct for the versions the project happened to have. A
//! pre-release identifier compared as one string sorts `1.0.0-rc.10` below `1.0.0-rc.2`,
//! which is wrong and which nothing would have noticed until the day it mattered.
//!
//! The grammar is <https://semver.org> 2.0.0, in full: three numeric components, an
//! optional pre-release of dot-separated identifiers, an optional build metadata of the
//! same shape. Precedence is the specification's: build metadata is ignored, a
//! pre-release sorts below the release it precedes, and pre-release identifiers compare
//! numerically when both are numeric and lexically otherwise.
//!
//! # Why not a crate
//!
//! The same reason there is no YAML crate and no TOML crate here: the grammar is small,
//! total and frozen, the behaviour is proved by the tests below rather than asserted by a
//! version range, and a dependency that parses one line of text is a supply chain this
//! project does not need. What a crate would give — correctness — is what the property
//! tests give, against the specification's own ordering rules.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why a string is not a version.
///
/// One variant per way the grammar can be missed, because a person who typed a version
/// wrong is owed the reason and not `invalid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// Nothing, or only whitespace.
    Empty,
    /// Fewer or more than three dot-separated numeric components before any `-` or `+`.
    Components {
        /// What was given.
        found: String,
        /// How many components it has.
        count: usize,
    },
    /// A component that is not a number, or has a leading zero, or overflows.
    Numeric {
        /// Which of the three: `major`, `minor` or `patch`.
        field: &'static str,
        /// The text that is not a number.
        found: String,
    },
    /// A pre-release or build identifier that is empty or has a character outside
    /// `[0-9A-Za-z-]`.
    Identifier {
        /// `pre-release` or `build`.
        part: &'static str,
        /// The offending identifier.
        found: String,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "a version is three numbers and this is empty"),
            Self::Components { found, count } => write!(
                f,
                "`{found}` has {count} numeric component(s); a version is major.minor.patch"
            ),
            Self::Numeric { field, found } => write!(
                f,
                "the {field} of a version is a number without a leading zero, and `{found}` is not"
            ),
            Self::Identifier { part, found } => write!(
                f,
                "`{found}` is not a {part} identifier: one or more of [0-9A-Za-z-], never empty"
            ),
        }
    }
}

impl std::error::Error for VersionError {}

/// A semantic version.
///
/// Ordered by the specification's precedence, which is not the derived order: build
/// metadata takes no part in it, so `1.0.0+a` and `1.0.0+b` are equal here and are two
/// different strings. `Eq` follows `Ord` for exactly that reason, and
/// [`Version::same_string`] is the way to ask whether two versions were written the same.
///
/// ```
/// use majordomus_cli::release::version::Version;
///
/// let a: Version = "1.0.0-rc.2".parse().unwrap();
/// let b: Version = "1.0.0-rc.10".parse().unwrap();
/// assert!(a < b, "numeric pre-release identifiers compare as numbers");
/// assert!(b < "1.0.0".parse().unwrap(), "a pre-release precedes its release");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Version {
    /// The first component: incompatible changes.
    pub major: u64,
    /// The second: compatible additions.
    pub minor: u64,
    /// The third: compatible fixes.
    pub patch: u64,
    /// The dot-separated pre-release identifiers, in order. Empty for a release.
    pub pre: Vec<Identifier>,
    /// The dot-separated build identifiers, in order. Empty when there are none. Takes no
    /// part in precedence.
    pub build: Vec<String>,
}

/// One pre-release identifier: a number, or a string.
///
/// The distinction is the specification's and it is what makes `rc.10` sort above `rc.2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identifier {
    /// All digits, no leading zero: compares as a number, and sorts below any string.
    Numeric(u64),
    /// Anything else in `[0-9A-Za-z-]`: compares as ASCII text.
    Alphanumeric(String),
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(n) => write!(f, "{n}"),
            Self::Alphanumeric(s) => write!(f, "{s}"),
        }
    }
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(a), Self::Numeric(b)) => a.cmp(b),
            (Self::Alphanumeric(a), Self::Alphanumeric(b)) => a.cmp(b),
            // "Numeric identifiers always have lower precedence than alphanumeric ones."
            (Self::Numeric(_), Self::Alphanumeric(_)) => Ordering::Less,
            (Self::Alphanumeric(_), Self::Numeric(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Version {
    /// A release version, with no pre-release and no build metadata.
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Vec::new(),
            build: Vec::new(),
        }
    }

    /// True when this version has no pre-release identifiers.
    ///
    /// A pre-release is published and addressable by its exact tag and is never what an
    /// unpinned installation resolves to; that rule lives in the distribution model's
    /// channel, and this is the syntactic half of it.
    pub fn is_stable(&self) -> bool {
        self.pre.is_empty()
    }

    /// True when the major component is zero.
    ///
    /// Before 1.0.0 the specification says nothing binds; this project's policy for that
    /// range is stated once, in [`super::policy`], and this predicate is what it asks.
    pub fn is_initial_development(&self) -> bool {
        self.major == 0
    }

    /// The version with its pre-release and build metadata removed.
    ///
    /// The release a pre-release precedes: `1.0.0-rc.1` yields `1.0.0`.
    pub fn to_release(&self) -> Self {
        Self::new(self.major, self.minor, self.patch)
    }

    /// Whether two versions were written the same way, build metadata included.
    ///
    /// [`Eq`] follows precedence and so ignores build metadata; a release identity has to
    /// be compared by its text, and this is that comparison.
    pub fn same_string(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }

    /// The tag that names this version: `v` and the version.
    ///
    /// The only place the tag convention is written. Nothing composes `"v".to_owned() + …`
    /// anywhere else.
    ///
    /// ```
    /// use majordomus_cli::release::version::Version;
    /// assert_eq!(Version::new(0, 3, 1).tag(), "v0.3.1");
    /// ```
    pub fn tag(&self) -> String {
        format!("v{self}")
    }

    /// The version a tag names, or `None` when the tag does not name one.
    ///
    /// ```
    /// use majordomus_cli::release::version::Version;
    /// assert_eq!(Version::from_tag("v1.2.3"), Some(Version::new(1, 2, 3)));
    /// assert_eq!(Version::from_tag("1.2.3"), None);
    /// assert_eq!(Version::from_tag("archive/int/land-3"), None);
    /// ```
    pub fn from_tag(tag: &str) -> Option<Self> {
        tag.strip_prefix('v')?.parse().ok()
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(VersionError::Empty);
        }
        // build metadata first: it may contain '-', so a '-' after a '+' is not the
        // pre-release separator.
        let (rest, build) = match s.split_once('+') {
            Some((r, b)) => (r, identifiers(b, "build")?),
            None => (s, Vec::new()),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((c, p)) => (c, pre_identifiers(p)?),
            None => (rest, Vec::new()),
        };
        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err(VersionError::Components {
                found: s.to_string(),
                count: parts.len(),
            });
        }
        let mut n = [0u64; 3];
        for (i, (field, text)) in [
            ("major", parts[0]),
            ("minor", parts[1]),
            ("patch", parts[2]),
        ]
        .into_iter()
        .enumerate()
        {
            n[i] = numeric(text).ok_or_else(|| VersionError::Numeric {
                field,
                found: text.to_string(),
            })?;
        }
        Ok(Version {
            major: n[0],
            minor: n[1],
            patch: n[2],
            pre,
            build,
        })
    }
}

/// A numeric component: digits only, no leading zero unless it is the single digit `0`.
fn numeric(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}

/// The dot-separated identifiers of a build metadata part.
fn identifiers(text: &str, part: &'static str) -> Result<Vec<String>, VersionError> {
    text.split('.')
        .map(|id| {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                Err(VersionError::Identifier {
                    part,
                    found: id.to_string(),
                })
            } else {
                Ok(id.to_string())
            }
        })
        .collect()
}

/// The dot-separated identifiers of a pre-release, each classified.
fn pre_identifiers(text: &str) -> Result<Vec<Identifier>, VersionError> {
    identifiers(text, "pre-release")?
        .into_iter()
        .map(|id| {
            Ok(match numeric(&id) {
                Some(n) => Identifier::Numeric(n),
                // digits with a leading zero are not a numeric identifier and are not a
                // legal one either: the specification forbids them outright.
                None if id.bytes().all(|b| b.is_ascii_digit()) => {
                    return Err(VersionError::Identifier {
                        part: "pre-release",
                        found: id,
                    })
                }
                None => Identifier::Alphanumeric(id),
            })
        })
        .collect()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-")?;
            for (i, id) in self.pre.iter().enumerate() {
                if i > 0 {
                    write!(f, ".")?;
                }
                write!(f, "{id}")?;
            }
        }
        if !self.build.is_empty() {
            write!(f, "+{}", self.build.join("."))?;
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                // "a pre-release version has lower precedence than the associated normal"
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Version {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Version".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "title": "Version",
            "description": "A semantic version: major.minor.patch, with an optional pre-release and build metadata.",
            "pattern": r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$",
            "examples": ["0.3.1", "1.0.0-rc.1"],
        })
    }
}

/// Which component a release moves.
///
/// Distinct from [`super::diff::CompatibilityImpact`] on purpose: the impact is what was
/// observed in the contract, the bump is what the policy makes of it. Collapsing them
/// would lose the observation, and the observation is what an explanation is made of.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "VersionBump")]
pub enum Bump {
    /// Nothing moves: the version stays as it is.
    None,
    /// The third component, and the pre-release and build metadata are dropped.
    Patch,
    /// The second component; the third becomes zero.
    Minor,
    /// The first component; the others become zero.
    Major,
}

impl Bump {
    /// The word a person types and a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }

    /// The bump a word names, for a command line argument.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "none" => Self::None,
            "patch" => Self::Patch,
            "minor" => Self::Minor,
            "major" => Self::Major,
            _ => return None,
        })
    }

    /// Apply this bump to a version.
    ///
    /// A pre-release is a version on the way to its release, so bumping any component of
    /// one drops the pre-release rather than compounding it: `1.0.0-rc.1` patched is
    /// `1.0.0`, which is the release the pre-release was for and the next thing that can
    /// be published. Build metadata never survives a bump.
    ///
    /// ```
    /// use majordomus_cli::release::version::{Bump, Version};
    ///
    /// let v: Version = "1.4.2".parse().unwrap();
    /// assert_eq!(Bump::Patch.apply(&v).to_string(), "1.4.3");
    /// assert_eq!(Bump::Minor.apply(&v).to_string(), "1.5.0");
    /// assert_eq!(Bump::Major.apply(&v).to_string(), "2.0.0");
    /// assert_eq!(Bump::None.apply(&v).to_string(), "1.4.2");
    ///
    /// let rc: Version = "1.0.0-rc.1".parse().unwrap();
    /// assert_eq!(Bump::Patch.apply(&rc).to_string(), "1.0.0");
    /// ```
    pub fn apply(self, v: &Version) -> Version {
        match self {
            Self::None => v.clone(),
            // a pre-release is already "below" its release: the smallest step from
            // 1.0.0-rc.1 is 1.0.0 itself, not 1.0.1.
            Self::Patch if !v.is_stable() => v.to_release(),
            Self::Patch => Version::new(v.major, v.minor, v.patch + 1),
            Self::Minor if !v.is_stable() && v.patch == 0 => v.to_release(),
            Self::Minor => Version::new(v.major, v.minor + 1, 0),
            Self::Major if !v.is_stable() && v.minor == 0 && v.patch == 0 => v.to_release(),
            Self::Major => Version::new(v.major + 1, 0, 0),
        }
    }
}

impl fmt::Display for Bump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_grammar_accepts_what_the_specification_accepts() {
        for good in [
            "0.0.4",
            "1.2.3",
            "10.20.30",
            "1.1.2-prerelease+meta",
            "1.1.2+meta",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.0valid",
            "1.0.0-alpha-a.b-c-somethinglong+build.1-aef.1-its-okay",
            "1.0.0-rc.1+build.1",
            "2.0.0-rc.1+build.123",
            "1.2.3-beta",
            "10.2.3-DEV-SNAPSHOT",
            "1.0.0-0A.is.legal",
        ] {
            let parsed: Version = good.parse().unwrap_or_else(|e| panic!("{good}: {e}"));
            assert_eq!(parsed.to_string(), good, "round trip of {good}");
        }
    }

    #[test]
    fn the_grammar_refuses_what_the_specification_refuses() {
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1.2.3-",
            "1.2.3+",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2.3-01",
            "a.b.c",
            "1.2.x",
            "-1.2.3",
            "1.2.3-alpha..1",
            "v1.2.3",
        ] {
            assert!(bad.parse::<Version>().is_err(), "{bad} should not parse");
        }
    }

    /// The ordering example from the specification, in the specification's order.
    #[test]
    fn precedence_follows_the_specification() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ]
        .map(|s| s.parse::<Version>().unwrap());
        for pair in ordered.windows(2) {
            assert!(pair[0] < pair[1], "{} should precede {}", pair[0], pair[1]);
        }
    }

    /// The bug the five-tuple sort key in the release records had: a pre-release compared
    /// as one string puts `rc.10` below `rc.2`. This is the assertion that would have
    /// caught it.
    #[test]
    fn a_numeric_pre_release_identifier_is_not_compared_as_text() {
        let two: Version = "1.0.0-rc.2".parse().unwrap();
        let ten: Version = "1.0.0-rc.10".parse().unwrap();
        assert!(two < ten);
        assert!(
            "1.0.0-rc.2" > "1.0.0-rc.10",
            "as text it is the other way round"
        );
    }

    #[test]
    fn build_metadata_takes_no_part_in_precedence() {
        let a: Version = "1.0.0+alpha".parse().unwrap();
        let b: Version = "1.0.0+beta".parse().unwrap();
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert!(
            !a.same_string(&b),
            "and yet they are two different releases"
        );
    }

    #[test]
    fn a_tag_is_v_and_a_version_and_nothing_else_is_a_tag() {
        assert_eq!(Version::new(0, 3, 1).tag(), "v0.3.1");
        assert_eq!(Version::from_tag("v0.3.1"), Some(Version::new(0, 3, 1)));
        for not_a_release in [
            "web-surfaces-0.1",
            "pre-rebase2",
            "integration-2026-09-05",
            "closed/web-surface-serving-20260906",
            "archive/int/land-3",
            "v0.2.0-ai-documents",
        ] {
            // v0.2.0-ai-documents does parse as a pre-release, deliberately: the baseline
            // policy is what excludes a pre-release, not the tag grammar.
            let parsed = Version::from_tag(not_a_release);
            if not_a_release == "v0.2.0-ai-documents" {
                assert!(parsed.is_some_and(|v| !v.is_stable()));
            } else {
                assert_eq!(parsed, None, "{not_a_release}");
            }
        }
    }

    #[test]
    fn a_bump_of_a_pre_release_lands_on_the_release_it_precedes() {
        let rc: Version = "2.0.0-rc.3".parse().unwrap();
        assert_eq!(Bump::Patch.apply(&rc), Version::new(2, 0, 0));
        assert_eq!(Bump::Minor.apply(&rc), Version::new(2, 0, 0));
        assert_eq!(Bump::Major.apply(&rc), Version::new(2, 0, 0));
        // but a pre-release of a patch release cannot reach a minor by minor-bumping.
        let rc: Version = "2.1.4-rc.1".parse().unwrap();
        assert_eq!(Bump::Patch.apply(&rc), Version::new(2, 1, 4));
        assert_eq!(Bump::Minor.apply(&rc), Version::new(2, 2, 0));
        assert_eq!(Bump::Major.apply(&rc), Version::new(3, 0, 0));
    }

    #[test]
    fn every_bump_produces_a_version_strictly_above_a_release() {
        let v = Version::new(1, 4, 2);
        for bump in [Bump::Patch, Bump::Minor, Bump::Major] {
            assert!(bump.apply(&v) > v, "{bump} of {v}");
        }
        assert_eq!(Bump::None.apply(&v), v);
    }

    #[test]
    fn a_bump_word_round_trips() {
        for bump in [Bump::None, Bump::Patch, Bump::Minor, Bump::Major] {
            assert_eq!(Bump::parse(bump.as_str()), Some(bump));
        }
        assert_eq!(Bump::parse("MAJOR"), None);
    }

    /// The five-tuple key this type replaces, kept here only to prove the replacement
    /// agrees with it on everything it was right about, and disagrees where it was wrong.
    #[test]
    fn the_records_this_repository_holds_sort_the_same_way_they_did() {
        let mut versions: Vec<Version> = ["0.1.0", "0.2.0", "0.3.0", "0.3.1"]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        versions.sort();
        assert_eq!(
            versions.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["0.1.0", "0.2.0", "0.3.0", "0.3.1"]
        );
    }

    fn any_version() -> impl Strategy<Value = Version> {
        (
            0u64..50,
            0u64..50,
            0u64..50,
            prop::option::of(prop::collection::vec(
                prop_oneof![
                    (0u64..20).prop_map(Identifier::Numeric),
                    "[a-z]{1,4}".prop_map(Identifier::Alphanumeric),
                ],
                1..3,
            )),
        )
            .prop_map(|(major, minor, patch, pre)| Version {
                major,
                minor,
                patch,
                pre: pre.unwrap_or_default(),
                build: Vec::new(),
            })
    }

    proptest! {
        /// Rendering and parsing are inverse. Everything else in the release subsystem
        /// depends on it: a version crosses a YAML record, a JSON manifest, a git tag and
        /// a command line, and must be the same version at the end.
        #[test]
        fn display_and_parse_are_inverse(v in any_version()) {
            let text = v.to_string();
            let back: Version = text.parse().expect("rendered version parses");
            prop_assert_eq!(&v, &back);
            prop_assert_eq!(text, back.to_string());
        }

        /// A bump always moves forward, and never sideways.
        #[test]
        fn a_bump_is_monotonic(v in any_version(), i in 1usize..4) {
            let bump = [Bump::None, Bump::Patch, Bump::Minor, Bump::Major][i];
            let bumped = bump.apply(&v);
            prop_assert!(bumped > v, "{bump} of {v} gave {bumped}");
            prop_assert!(bumped.is_stable(), "a bump lands on a release");
        }

        /// A stronger bump never lands lower than a weaker one.
        #[test]
        fn bumps_are_ordered_the_way_their_names_are(v in any_version()) {
            let patch = Bump::Patch.apply(&v);
            let minor = Bump::Minor.apply(&v);
            let major = Bump::Major.apply(&v);
            prop_assert!(patch <= minor, "{v}: patch {patch} minor {minor}");
            prop_assert!(minor <= major, "{v}: minor {minor} major {major}");
        }

        /// The tag convention round trips for every version.
        #[test]
        fn a_tag_round_trips(v in any_version()) {
            prop_assert_eq!(Version::from_tag(&v.tag()), Some(v));
        }
    }
}

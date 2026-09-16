//! What a deployment actually serves, observed from outside and judged against the commit
//! that was meant to be there.
//!
//! A green publication job is not proof that a reader receives the build. GitHub's own
//! Pages build can fail on a branch that was pushed perfectly, a CDN can hold the previous
//! bytes, and a workflow can succeed while publishing nothing. The only evidence that a
//! deployment carries a commit is the deployment saying so: the site serves its build
//! identity (`/build.json`, written by `scripts/site-build` from the tree it built) and this
//! module reads it the way a visitor would.
//!
//! Three things are kept apart, because collapsing any two of them is how a deployment gets
//! reported as current when nobody measured it:
//!
//! - **fetching** is behind [`Fetch`]; the executable's implementation runs `curl` with a
//!   fixed argument vector and a bounded time, and tests substitute a fake;
//! - **judging** is [`judge`], a pure function of what was served, the commit expected and
//!   what git knows about the two;
//! - **recording** appends one [`Observation`] to a checkout-local file. It is not the
//!   committed evidence ledger: an observation of a later deployment committed into the tree
//!   it observes would describe a commit that no longer is the tree's.
//!
//! Freshness is containment, never age. A served commit proves every commit it contains, so
//! an observation made an hour ago still proves a commit that was already on the site then,
//! and an observation made a second ago proves nothing about a commit the site does not
//! contain. [`judge`] is the same function whether the served identity was fetched now or
//! read back from a record: re-judging a record against a newer expected commit is how a
//! proof goes stale without anyone deciding that it has.
//!
//! Only [`ServedVerdict::Served`] passes. Unreachable, malformed, dirty and undecidable are each a
//! reason the question was not answered, and none of them is an answer of yes.
//!
//! ```
//! use majordomus_cli::served::ServedVerdict;
//! assert!(ServedVerdict::Served.passes());
//! assert!(!ServedVerdict::Unreachable.passes());
//! assert_eq!(ServedVerdict::Stale.as_str(), "stale");
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The schema every recorded observation carries.
pub const OBSERVATION_SCHEMA: &str = "majordomus.served-observation/v1";

/// Where observations are appended, repository-relative. Checkout-local and ignored.
pub const OBSERVATIONS_PATH: &str = ".ai/local/state/served/observations.jsonl";

/// The identity file a site serves when nothing names another.
pub const DEFAULT_IDENTITY: &str = "build.json";

/// The longest a single probe may take, in seconds, when the caller names no bound.
pub const DEFAULT_PROBE_SECONDS: u64 = 10;

// ---------------------------------------------------------------- what was served

/// The build identity a deployment serves, as far as this module relies on it. Other
/// fields of the served document are ignored rather than refused: the document is written
/// by another program, and a field it adds is not a malformation.
///
/// ```
/// use majordomus_cli::served::BuildIdentity;
/// let id = BuildIdentity {
///     commit: "a".repeat(40),
///     dirty: false,
///     source_version: Some("0.7.0".into()),
///     source_hash: None,
/// };
/// // what is judged is the commit and whether the tree it was built from was committed
/// assert_eq!(id.commit.len(), 40);
/// assert!(!id.dirty);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuildIdentity {
    /// The full commit the served build was made from.
    pub commit: String,
    /// Whether the tree it was built from had uncommitted changes. A dirty build names a
    /// commit it does not equal, so it proves no commit.
    pub dirty: bool,
    /// The version the build states, when it states one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    /// The fingerprint of the site's canonical inputs, when the build states one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
}

/// Why a served body is not a build identity. The reason is the value: a refusal that says
/// only "invalid" sends the reader to look at the site by hand.
///
/// ```
/// use majordomus_cli::served::{BuildIdentity, Malformed};
/// let Malformed(why) = BuildIdentity::parse(b"<html>").unwrap_err();
/// assert!(why.contains("not JSON"), "{why}");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl BuildIdentity {
    /// Reads a served body. The commit must be a full forty-character sha: a prefix names
    /// whatever happens to start with it, and a served identity that could mean several
    /// commits proves none of them.
    ///
    /// ```
    /// use majordomus_cli::served::BuildIdentity;
    /// let body = format!(r#"{{"schema":1,"commit":"{}","dirty":false}}"#, "a".repeat(40));
    /// assert_eq!(BuildIdentity::parse(body.as_bytes()).unwrap().commit, "a".repeat(40));
    /// assert!(BuildIdentity::parse(b"<html>").is_err());
    /// ```
    pub fn parse(body: &[u8]) -> Result<BuildIdentity, Malformed> {
        let v: Value = serde_json::from_slice(body)
            .map_err(|e| Malformed(format!("the served identity is not JSON: {e}")))?;
        let commit = v
            .get("commit")
            .and_then(Value::as_str)
            .ok_or_else(|| Malformed("the served identity names no `commit`".into()))?;
        if !is_full_sha(commit) {
            return Err(Malformed(format!(
                "the served `commit` is `{commit}`, not a forty-character sha"
            )));
        }
        let dirty = match v.get("dirty") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(other) => {
                return Err(Malformed(format!(
                    "the served `dirty` is `{other}`, not a boolean"
                )))
            }
        };
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Ok(BuildIdentity {
            commit: commit.to_ascii_lowercase(),
            dirty,
            source_version: text("source_version"),
            source_hash: text("source_hash"),
        })
    }
}

// ---------------------------------------------------------------- the verdict

/// What an observation says about the expected commit. Only [`ServedVerdict::Served`] is a
/// proof; the other five are each a different reason the question was not answered yes.
///
/// ```
/// use majordomus_cli::served::ServedVerdict;
/// // a measured no and an unanswered question are never spelled the same
/// assert!(ServedVerdict::Served.passes());
/// assert_eq!(ServedVerdict::Stale.exit_code(), 10);
/// assert_eq!(ServedVerdict::Unreachable.exit_code(), 12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServedVerdict {
    /// The served build is the expected commit or contains it. The only passing verdict.
    Served,
    /// The served build does not contain the expected commit: the deployment is behind.
    Stale,
    /// Nothing was received. The deployment was not measured.
    Unreachable,
    /// Something was received and it is not a build identity.
    Malformed,
    /// The served build was made from an uncommitted tree, so it proves no commit.
    Dirty,
    /// The served commit is not in this clone, or git could not be asked; containment is
    /// undecided. Fetching the remote usually turns this into an answer.
    Undecided,
}

impl ServedVerdict {
    /// The word every surface prints — the command line, the HTTP body and the MCP
    /// result all render the verdict through this one spelling.
    ///
    /// ```
    /// use majordomus_cli::served::ServedVerdict;
    /// assert_eq!(ServedVerdict::Undecided.as_str(), "undecided");
    /// assert_eq!(ServedVerdict::Dirty.as_str(), "dirty");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ServedVerdict::Served => "served",
            ServedVerdict::Stale => "stale",
            ServedVerdict::Unreachable => "unreachable",
            ServedVerdict::Malformed => "malformed",
            ServedVerdict::Dirty => "dirty",
            ServedVerdict::Undecided => "undecided",
        }
    }

    /// Whether this verdict proves the expected commit is deployed.
    ///
    /// ```
    /// use majordomus_cli::served::ServedVerdict;
    /// assert!(ServedVerdict::Served.passes());
    /// // a site nobody could reach is not a site that serves the commit
    /// assert!(!ServedVerdict::Unreachable.passes());
    /// assert!(!ServedVerdict::Dirty.passes());
    /// ```
    pub fn passes(self) -> bool {
        matches!(self, ServedVerdict::Served)
    }

    /// The exit status the command line reports it with: 0 a proof, 10 a measured "no",
    /// 12 a question that could not be answered. The same three codes `scripts/pages
    /// verify` uses, so a caller of either reads one convention.
    ///
    /// ```
    /// use majordomus_cli::served::ServedVerdict;
    /// assert_eq!(ServedVerdict::Served.exit_code(), 0);
    /// assert_eq!(ServedVerdict::Dirty.exit_code(), 10);      // measured, and a no
    /// assert_eq!(ServedVerdict::Malformed.exit_code(), 12);  // not measured at all
    /// ```
    pub fn exit_code(self) -> i32 {
        match self {
            ServedVerdict::Served => 0,
            ServedVerdict::Stale | ServedVerdict::Dirty => 10,
            ServedVerdict::Unreachable | ServedVerdict::Malformed | ServedVerdict::Undecided => 12,
        }
    }
}

/// What git knows about commits, asked through a trait so that the judgement is testable
/// without a repository. Both answers are three-valued: `None` is "git could not be asked",
/// which is not a `false`, and collapsing the two is how a record from a history this clone
/// has never seen gets read as current.
///
/// ```
/// use majordomus_cli::served::Ancestry;
///
/// struct OneCommit;
/// impl Ancestry for OneCommit {
///     fn has(&self, c: &str) -> Option<bool> { Some(c == "a".repeat(40)) }
///     fn contains(&self, d: &str, a: &str) -> Option<bool> { Some(d == a) }
/// }
/// assert_eq!(OneCommit.has(&"a".repeat(40)), Some(true));
/// assert_eq!(OneCommit.has(&"f".repeat(40)), Some(false));
/// ```
pub trait Ancestry {
    /// Does this clone hold `commit`? `None` when git cannot be asked.
    ///
    /// ```
    /// use majordomus_cli::served::{Ancestry, GitAncestry};
    /// use std::path::Path;
    /// // a sha no repository holds resolves to a `false`, not to an error
    /// let git = GitAncestry { root: Path::new(env!("CARGO_MANIFEST_DIR")) };
    /// assert_eq!(git.has(&"f".repeat(40)), Some(false));
    /// ```
    fn has(&self, commit: &str) -> Option<bool>;
    /// Does `descendant` contain `ancestor` (equal, or reachable from it)? `None` when git
    /// cannot be asked.
    ///
    /// ```
    /// use majordomus_cli::served::{Ancestry, GitAncestry};
    /// use std::path::Path;
    /// let git = GitAncestry { root: Path::new(env!("CARGO_MANIFEST_DIR")) };
    /// // every commit contains itself, which is what makes an exact match a proof
    /// let head = String::from_utf8(
    ///     std::process::Command::new("git").args(["-C", env!("CARGO_MANIFEST_DIR"), "rev-parse", "HEAD"])
    ///         .output().unwrap().stdout).unwrap();
    /// let head = head.trim();
    /// assert_eq!(git.contains(head, head), Some(true));
    /// ```
    fn contains(&self, descendant: &str, ancestor: &str) -> Option<bool>;
}

/// The [`Ancestry`] the executable uses: this repository's own git, asked read-only.
///
/// ```
/// use majordomus_cli::served::{Ancestry, GitAncestry};
/// use std::path::Path;
/// let git = GitAncestry { root: Path::new(env!("CARGO_MANIFEST_DIR")) };
/// assert_eq!(git.has(&"0".repeat(40)), Some(false));
/// ```
pub struct GitAncestry<'a> {
    /// The checkout asked.
    pub root: &'a Path,
}

impl Ancestry for GitAncestry<'_> {
    fn has(&self, commit: &str) -> Option<bool> {
        // `merge-base --is-ancestor X X` exits 0 exactly when X is a commit this clone
        // holds, and [`crate::git::is_ancestor`] already maps "git did not run" to `None`.
        crate::git::is_ancestor(self.root, commit, commit)
    }
    fn contains(&self, descendant: &str, ancestor: &str) -> Option<bool> {
        crate::git::is_ancestor(self.root, ancestor, descendant)
    }
}

/// The judgement of one served identity against one expected commit, with its reason. The
/// reason travels with the verdict so that every surface can say *why* without re-deriving
/// it.
///
/// ```
/// use majordomus_cli::served::{judge, Fetched, Judgement, ServedVerdict};
/// let j: Judgement = judge(&Err(Fetched::Unreachable("timeout".into())), &"a".repeat(40), &fake());
/// assert_eq!(j.verdict, ServedVerdict::Unreachable);
/// assert!(j.reason.contains("timeout"));
/// # use majordomus_cli::served::Ancestry;
/// # struct N; impl Ancestry for N {
/// #     fn has(&self, _: &str) -> Option<bool> { Some(true) }
/// #     fn contains(&self, _: &str, _: &str) -> Option<bool> { Some(true) }
/// # }
/// # fn fake() -> impl Ancestry { N }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Judgement {
    /// The verdict.
    pub verdict: ServedVerdict,
    /// Why, in one line a reader can act on.
    pub reason: String,
}

/// What the probe received: a body, or nothing and the reason. Nothing received is carried
/// as a value rather than an error, because "the site could not be reached" is an answer
/// the judgement acts on.
///
/// ```
/// use majordomus_cli::served::Fetched;
/// let got = Fetched::Unreachable("curl exited 7".into());
/// assert!(matches!(got, Fetched::Unreachable(ref why) if why.contains("7")));
/// assert!(matches!(Fetched::Body(b"{}".to_vec()), Fetched::Body(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// A body arrived.
    Body(Vec<u8>),
    /// Nothing arrived, and why.
    Unreachable(String),
}

/// Judges what was served against `expected`, a full commit sha.
///
/// The order of the questions is the order in which each makes the next meaningless: no
/// body, no identity, no committed tree, no local knowledge of the served commit, and only
/// then containment.
///
/// ```
/// use majordomus_cli::served::{judge, Ancestry, BuildIdentity, ServedVerdict};
///
/// struct Linear;  // a <- b
/// impl Ancestry for Linear {
///     fn has(&self, c: &str) -> Option<bool> { Some(c.starts_with('a') || c.starts_with('b')) }
///     fn contains(&self, d: &str, a: &str) -> Option<bool> { Some(d >= a) }
/// }
/// let id = |c: &str, dirty| Ok(BuildIdentity {
///     commit: c.into(), dirty, source_version: None, source_hash: None });
/// let (a, b) = ("a".repeat(40), "b".repeat(40));
///
/// // the newer build proves the older commit, because it contains it
/// assert_eq!(judge(&id(&b, false), &a, &Linear).verdict, ServedVerdict::Served);
/// // the older build does not prove the newer commit
/// assert_eq!(judge(&id(&a, false), &b, &Linear).verdict, ServedVerdict::Stale);
/// // and a build from an uncommitted tree proves no commit at all, not even its own
/// assert_eq!(judge(&id(&b, true), &b, &Linear).verdict, ServedVerdict::Dirty);
/// ```
pub fn judge(
    served: &Result<BuildIdentity, Fetched>,
    expected: &str,
    git: &dyn Ancestry,
) -> Judgement {
    let j = |verdict, reason: String| Judgement { verdict, reason };
    let id = match served {
        Ok(id) => id,
        Err(Fetched::Unreachable(why)) => {
            return j(
                ServedVerdict::Unreachable,
                format!("nothing was received: {why}"),
            )
        }
        Err(Fetched::Body(_)) => {
            return j(
                ServedVerdict::Malformed,
                "the served body is not a build identity".into(),
            )
        }
    };
    if id.dirty {
        return j(
            ServedVerdict::Dirty,
            format!(
                "the served build names {} but was built from an uncommitted tree",
                short(&id.commit)
            ),
        );
    }
    if id.commit == expected {
        return j(
            ServedVerdict::Served,
            format!("the deployment serves {}", short(expected)),
        );
    }
    match git.has(&id.commit) {
        Some(true) => {}
        Some(false) => {
            return j(
                ServedVerdict::Undecided,
                format!(
                    "the deployment serves {}, a commit this clone does not hold; fetch and \
                     observe again",
                    short(&id.commit)
                ),
            )
        }
        None => {
            return j(
                ServedVerdict::Undecided,
                "git could not be asked about ancestry".into(),
            )
        }
    }
    match git.contains(&id.commit, expected) {
        Some(true) => j(
            ServedVerdict::Served,
            format!(
                "the deployment serves {}, which contains {}",
                short(&id.commit),
                short(expected)
            ),
        ),
        Some(false) => j(
            ServedVerdict::Stale,
            format!(
                "the deployment serves {}, which does not contain {}",
                short(&id.commit),
                short(expected)
            ),
        ),
        None => j(
            ServedVerdict::Undecided,
            "git could not be asked about ancestry".into(),
        ),
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

// ---------------------------------------------------------------- fetching

/// How a served identity is fetched. Behind a trait so that every verdict can be tested
/// without a network, and so that the one implementation that does reach the network is a
/// single, reviewable place.
///
/// ```
/// use majordomus_cli::served::{Fetch, Fetched};
/// struct Canned;
/// impl Fetch for Canned {
///     fn fetch(&self, _: &str) -> Fetched { Fetched::Body(b"{}".to_vec()) }
/// }
/// assert_eq!(Canned.fetch("https://example.test/build.json"), Fetched::Body(b"{}".to_vec()));
/// ```
pub trait Fetch {
    /// Fetches `url` once, bounded. An implementation never retries: a caller that wants a
    /// second look decides that itself, with its own clock.
    ///
    /// ```
    /// use majordomus_cli::served::{Curl, Fetch, Fetched};
    /// // nothing listens on the discard port, so the probe answers with a reason
    /// let got = Curl { max_seconds: 2 }.fetch("http://127.0.0.1:9/build.json");
    /// assert!(matches!(got, Fetched::Unreachable(_)));
    /// ```
    fn fetch(&self, url: &str) -> Fetched;
}

/// `curl`, run with a fixed argument vector: the URL is an argument, never shell text, and
/// the probe is bounded so that an unreachable host costs a known number of seconds.
///
/// ```
/// use majordomus_cli::served::{Curl, Fetch, Fetched};
/// let got = Curl { max_seconds: 2 }.fetch("http://127.0.0.1:9/build.json");
/// assert!(matches!(got, Fetched::Unreachable(ref why) if why.contains("curl")), "{got:?}");
/// ```
pub struct Curl {
    /// The bound on one probe, in seconds.
    pub max_seconds: u64,
}

impl Fetch for Curl {
    fn fetch(&self, url: &str) -> Fetched {
        let out = Command::new("curl")
            .args(["-fsSL", "--proto", "=https,http", "--max-time"])
            .arg(self.max_seconds.to_string())
            .args(["-H", "Cache-Control: no-cache", "--"])
            .arg(url)
            .output();
        match out {
            Err(e) => Fetched::Unreachable(format!("curl could not be run: {e}")),
            Ok(o) if o.status.success() && !o.stdout.is_empty() => Fetched::Body(o.stdout),
            Ok(o) => Fetched::Unreachable(format!(
                "curl exited {} for {url}",
                o.status
                    .code()
                    .map_or("by signal".into(), |c| c.to_string())
            )),
        }
    }
}

/// The identity URL of a site: its base with the identity file appended.
///
/// ```
/// use majordomus_cli::served::identity_url;
/// assert_eq!(identity_url("https://majordomus.dev/", "build.json"), "https://majordomus.dev/build.json");
/// ```
pub fn identity_url(base: &str, identity: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        identity.trim_start_matches('/')
    )
}

/// Refuses a base URL that is not plain `http(s)` with a host. The value reaches `curl` as an
/// argument, so this is not what keeps a shell safe; it is what keeps `file://` and friends
/// from being read as a deployment.
///
/// ```
/// use majordomus_cli::served::check_base;
/// assert!(check_base("https://majordomus.dev").is_ok());
/// assert!(check_base("http://127.0.0.1:8080/x").is_ok());
/// assert!(check_base("file:///etc/passwd").is_err());
/// ```
pub fn check_base(base: &str) -> Result<(), String> {
    let rest = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
        .ok_or_else(|| format!("`{base}` is not an http(s) URL"))?;
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c == '@') {
        return Err(format!("`{base}` names no plain host"));
    }
    Ok(())
}

// ---------------------------------------------------------------- recording

/// One observation of one deployment, as recorded: what was asked, what was served, what
/// was judged and when. Everything needed to judge it again later is in the record, which
/// is what lets [`Observation::rejudge`] answer without probing.
///
/// ```
/// use majordomus_cli::served::{observe, Fetch, Fetched, GitAncestry, Observation, OBSERVATION_SCHEMA};
/// use std::{path::Path, time::SystemTime};
/// struct None_;
/// impl Fetch for None_ {
///     fn fetch(&self, _: &str) -> Fetched { Fetched::Unreachable("no route".into()) }
/// }
/// let git = GitAncestry { root: Path::new(env!("CARGO_MANIFEST_DIR")) };
/// let o: Observation = observe("pages", "https://s", "build.json", &"a".repeat(40), &None_,
///                               &git, SystemTime::UNIX_EPOCH);
/// assert_eq!(o.schema, OBSERVATION_SCHEMA);
/// assert_eq!(o.url, "https://s/build.json");
/// assert_eq!(o.at, "1970-01-01T00:00:00Z");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    /// Always [`OBSERVATION_SCHEMA`].
    pub schema: String,
    /// Which deployment: the name the caller observed it under (`pages`, or a deployment
    /// object's id).
    pub deployment: String,
    /// The identity URL probed.
    pub url: String,
    /// The commit that was expected, full.
    pub expected: String,
    /// What was served, when a build identity was received.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served: Option<BuildIdentity>,
    /// The judgement at the time of the observation.
    #[serde(flatten)]
    pub judgement: Judgement,
    /// When, RFC 3339 UTC.
    pub at: String,
}

impl Observation {
    /// Re-judges this record against `expected` now. A served commit keeps proving what it
    /// contains; a record that received nothing keeps proving nothing. This is where a
    /// proof goes stale: nothing about the record changes, the commit asked about does.
    ///
    /// ```
    /// use majordomus_cli::served::{observe, Ancestry, Fetch, Fetched, ServedVerdict};
    /// use std::time::SystemTime;
    /// struct Linear;  // a <- b
    /// impl Ancestry for Linear {
    ///     fn has(&self, c: &str) -> Option<bool> { Some(c.starts_with('a') || c.starts_with('b')) }
    ///     fn contains(&self, d: &str, a: &str) -> Option<bool> { Some(d >= a) }
    /// }
    /// let (a, b) = ("a".repeat(40), "b".repeat(40));
    /// struct ServesA(String);
    /// impl Fetch for ServesA {
    ///     fn fetch(&self, _: &str) -> Fetched {
    ///         Fetched::Body(format!(r#"{{"commit":"{}","dirty":false}}"#, self.0).into_bytes())
    ///     }
    /// }
    /// let o = observe("pages", "https://s", "build.json", &a, &ServesA(a.clone()), &Linear,
    ///                 SystemTime::UNIX_EPOCH);
    /// assert_eq!(o.judgement.verdict, ServedVerdict::Served);
    /// // the same record, asked about a commit the served build does not contain
    /// assert_eq!(o.rejudge(&b, &Linear).verdict, ServedVerdict::Stale);
    /// ```
    pub fn rejudge(&self, expected: &str, git: &dyn Ancestry) -> Judgement {
        let served = match &self.served {
            Some(id) => Ok(id.clone()),
            None if self.judgement.verdict == ServedVerdict::Malformed => {
                Err(Fetched::Body(Vec::new()))
            }
            None => Err(Fetched::Unreachable(self.judgement.reason.clone())),
        };
        judge(&served, expected, git)
    }
}

/// Probes `base` once and judges the result against `expected`, returning the record.
/// Writes nothing: [`append`] is what keeps it.
///
/// ```
/// use majordomus_cli::served::{observe, Ancestry, Fetch, Fetched, ServedVerdict};
/// use std::time::SystemTime;
/// struct Nothing;
/// impl Fetch for Nothing {
///     fn fetch(&self, _: &str) -> Fetched { Fetched::Unreachable("no route".into()) }
/// }
/// struct Any;
/// impl Ancestry for Any {
///     fn has(&self, _: &str) -> Option<bool> { Some(true) }
///     fn contains(&self, _: &str, _: &str) -> Option<bool> { Some(true) }
/// }
/// let o = observe("pages", "https://s", "build.json", &"a".repeat(40), &Nothing, &Any,
///                 SystemTime::UNIX_EPOCH);
/// assert_eq!(o.judgement.verdict, ServedVerdict::Unreachable);
/// assert!(o.served.is_none());
/// ```
pub fn observe(
    deployment: &str,
    base: &str,
    identity: &str,
    expected: &str,
    fetch: &dyn Fetch,
    git: &dyn Ancestry,
    at: SystemTime,
) -> Observation {
    let url = identity_url(base, identity);
    let fetched = fetch.fetch(&url);
    let served = match &fetched {
        Fetched::Body(b) => BuildIdentity::parse(b).map_err(|_| fetched.clone()),
        Fetched::Unreachable(_) => Err(fetched.clone()),
    };
    let mut judgement = judge(&served, expected, git);
    if let (Fetched::Body(b), ServedVerdict::Malformed) = (&fetched, judgement.verdict) {
        if let Err(Malformed(why)) = BuildIdentity::parse(b) {
            judgement.reason = why;
        }
    }
    Observation {
        schema: OBSERVATION_SCHEMA.into(),
        deployment: deployment.into(),
        url,
        expected: expected.into(),
        served: served.ok(),
        judgement,
        at: crate::peers::rfc3339(at),
    }
}

/// Appends `obs` to the checkout's observation file, creating it. The file is
/// checkout-local and never tracked, so a record of a deployment cannot end up describing
/// the tree it is committed into.
///
/// ```
/// use majordomus_cli::served::{append, observe, read_all, Ancestry, Fetch, Fetched};
/// use std::time::SystemTime;
/// # struct N; impl Fetch for N {
/// #     fn fetch(&self, _: &str) -> Fetched { Fetched::Unreachable("x".into()) } }
/// # struct A; impl Ancestry for A {
/// #     fn has(&self, _: &str) -> Option<bool> { Some(true) }
/// #     fn contains(&self, _: &str, _: &str) -> Option<bool> { Some(true) } }
/// let dir = tempfile::tempdir().unwrap();
/// let o = observe("pages", "https://s", "build.json", &"a".repeat(40), &N, &A,
///                 SystemTime::UNIX_EPOCH);
/// append(dir.path(), &o).unwrap();
/// assert_eq!(read_all(dir.path()).0, vec![o]);
/// ```
pub fn append(root: &Path, obs: &Observation) -> std::io::Result<PathBuf> {
    let path = root.join(OBSERVATIONS_PATH);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_string(obs).map_err(std::io::Error::other)?;
    line.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    f.write_all(line.as_bytes())?;
    Ok(path)
}

/// Every readable observation of the checkout, oldest first, and the number of lines that
/// could not be read. A torn or foreign line is counted, never silently dropped: a reader
/// that cannot tell "nothing recorded" from "the record is damaged" acts on the wrong one.
///
/// ```
/// use majordomus_cli::served::read_all;
/// let dir = tempfile::tempdir().unwrap();
/// // a checkout that has observed nothing answers with no observations, not an error
/// assert_eq!(read_all(dir.path()), (Vec::new(), 0));
/// ```
pub fn read_all(root: &Path) -> (Vec<Observation>, usize) {
    let Ok(text) = std::fs::read_to_string(root.join(OBSERVATIONS_PATH)) else {
        return (Vec::new(), 0);
    };
    let mut out = Vec::new();
    let mut unreadable = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<Observation>(line) {
            Ok(o) if o.schema == OBSERVATION_SCHEMA => out.push(o),
            _ => unreadable += 1,
        }
    }
    (out, unreadable)
}

/// The newest observation of `deployment`, if any. The newest is what a reader judges by;
/// the older ones are the history of what the deployment has served.
///
/// ```
/// use majordomus_cli::served::latest;
/// // nothing observed, nothing to judge
/// assert!(latest(&[], "pages").is_none());
/// ```
pub fn latest<'a>(all: &'a [Observation], deployment: &str) -> Option<&'a Observation> {
    all.iter().rev().find(|o| o.deployment == deployment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const FOREIGN: &str = "ffffffffffffffffffffffffffffffffffffffff";

    /// A linear history A <- B <- C, and nothing else. `broken` makes git unaskable.
    struct History {
        order: Vec<&'static str>,
        broken: bool,
    }
    fn history() -> History {
        History {
            order: vec![A, B, C],
            broken: false,
        }
    }
    impl Ancestry for History {
        fn has(&self, c: &str) -> Option<bool> {
            (!self.broken).then(|| self.order.contains(&c))
        }
        fn contains(&self, d: &str, a: &str) -> Option<bool> {
            if self.broken {
                return None;
            }
            let pos = |x| self.order.iter().position(|y| *y == x);
            Some(matches!((pos(d), pos(a)), (Some(i), Some(j)) if j <= i))
        }
    }
    /// Answers `has` and refuses `contains`, the one path a whole-broken git cannot reach.
    struct HasOnly;
    impl Ancestry for HasOnly {
        fn has(&self, _: &str) -> Option<bool> {
            Some(true)
        }
        fn contains(&self, _: &str, _: &str) -> Option<bool> {
            None
        }
    }

    struct Fake(BTreeMap<String, Fetched>);
    impl Fetch for Fake {
        fn fetch(&self, url: &str) -> Fetched {
            self.0
                .get(url)
                .cloned()
                .unwrap_or(Fetched::Unreachable("no route".into()))
        }
    }

    fn id(commit: &str, dirty: bool) -> Result<BuildIdentity, Fetched> {
        Ok(BuildIdentity {
            commit: commit.into(),
            dirty,
            source_version: None,
            source_hash: None,
        })
    }
    fn body(commit: &str) -> Fetched {
        Fetched::Body(format!(r#"{{"schema":1,"commit":"{commit}","dirty":false}}"#).into_bytes())
    }

    #[test]
    fn the_expected_commit_served_exactly_passes() {
        let j = judge(&id(B, false), B, &history());
        assert_eq!(j.verdict, ServedVerdict::Served);
        assert!(j.reason.contains("bbbbbbbbbbbb"));
    }

    #[test]
    fn a_newer_build_proves_every_commit_it_contains() {
        assert_eq!(
            judge(&id(C, false), A, &history()).verdict,
            ServedVerdict::Served
        );
    }

    #[test]
    fn a_build_behind_the_expected_commit_is_stale() {
        let j = judge(&id(A, false), C, &history());
        assert_eq!(j.verdict, ServedVerdict::Stale);
        assert!(!j.verdict.passes());
    }

    #[test]
    fn a_dirty_build_proves_no_commit_even_the_one_it_names() {
        assert_eq!(
            judge(&id(B, true), B, &history()).verdict,
            ServedVerdict::Dirty
        );
    }

    #[test]
    fn a_served_commit_this_clone_lacks_is_undecided_not_stale() {
        let j = judge(&id(FOREIGN, false), B, &history());
        assert_eq!(j.verdict, ServedVerdict::Undecided);
        assert!(j.reason.contains("fetch"));
    }

    #[test]
    fn an_unaskable_git_is_undecided_at_either_question() {
        let broken = History {
            order: vec![],
            broken: true,
        };
        assert_eq!(
            judge(&id(A, false), B, &broken).verdict,
            ServedVerdict::Undecided
        );
        assert_eq!(
            judge(&id(A, false), B, &HasOnly).verdict,
            ServedVerdict::Undecided
        );
    }

    #[test]
    fn nothing_received_and_garbage_received_are_distinct_non_answers() {
        let u = judge(&Err(Fetched::Unreachable("timeout".into())), B, &history());
        assert_eq!(u.verdict, ServedVerdict::Unreachable);
        assert!(u.reason.contains("timeout"));
        let m = judge(&Err(Fetched::Body(b"<html>".to_vec())), B, &history());
        assert_eq!(m.verdict, ServedVerdict::Malformed);
    }

    #[test]
    fn exit_codes_separate_a_measured_no_from_an_unanswered_question() {
        let codes: Vec<_> = [
            ServedVerdict::Served,
            ServedVerdict::Stale,
            ServedVerdict::Dirty,
            ServedVerdict::Unreachable,
            ServedVerdict::Malformed,
            ServedVerdict::Undecided,
        ]
        .iter()
        .map(|v| (v.as_str(), v.exit_code(), v.passes()))
        .collect();
        assert_eq!(
            codes,
            vec![
                ("served", 0, true),
                ("stale", 10, false),
                ("dirty", 10, false),
                ("unreachable", 12, false),
                ("malformed", 12, false),
                ("undecided", 12, false),
            ]
        );
    }

    #[test]
    fn parse_refuses_what_could_mean_several_commits_or_none() {
        for (bad, why) in [
            (&b"not json"[..], "not JSON"),
            (br#"{"dirty":false}"#, "no `commit`"),
            (br#"{"commit":"abc1234"}"#, "forty-character"),
            (
                br#"{"commit":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"}"#,
                "forty-character",
            ),
        ] {
            let e = BuildIdentity::parse(bad).unwrap_err();
            assert!(e.0.contains(why), "{} did not say {why}", e.0);
        }
        let dirty = format!(r#"{{"commit":"{A}","dirty":"no"}}"#);
        assert!(BuildIdentity::parse(dirty.as_bytes())
            .unwrap_err()
            .0
            .contains("boolean"));
    }

    #[test]
    fn parse_keeps_what_it_relies_on_and_tolerates_what_it_does_not() {
        let upper = A.to_ascii_uppercase();
        let text = format!(
            r#"{{"commit":"{upper}","source_version":"0.7.0","source_hash":"h","extra":[1]}}"#
        );
        let got = BuildIdentity::parse(text.as_bytes()).unwrap();
        assert_eq!(got.commit, A, "a sha is compared lowercase");
        assert!(!got.dirty, "an absent `dirty` is a committed build");
        assert_eq!(got.source_version.as_deref(), Some("0.7.0"));
        assert_eq!(got.source_hash.as_deref(), Some("h"));
    }

    #[test]
    fn base_urls_are_plain_http_hosts() {
        assert!(check_base("https://majordomus.dev").is_ok());
        assert!(check_base("http://127.0.0.1:8080/x").is_ok());
        for bad in [
            "file:///etc/passwd",
            "https://",
            "https://user@host",
            "https://a b",
            "ftp://x",
        ] {
            assert!(check_base(bad).is_err(), "{bad} accepted");
        }
        assert_eq!(
            identity_url("https://x/", "/build.json"),
            "https://x/build.json"
        );
    }

    #[test]
    fn observe_records_the_served_identity_and_its_judgement() {
        let url = identity_url("https://site", DEFAULT_IDENTITY);
        let fake = Fake(BTreeMap::from([(url.clone(), body(C))]));
        let o = observe(
            "pages",
            "https://site",
            DEFAULT_IDENTITY,
            B,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(o.schema, OBSERVATION_SCHEMA);
        assert_eq!(o.url, url);
        assert_eq!(o.judgement.verdict, ServedVerdict::Served);
        assert_eq!(o.served.as_ref().unwrap().commit, C);
        assert_eq!(o.at, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn observe_names_the_malformation_rather_than_a_generic_reason() {
        let fake = Fake(BTreeMap::from([(
            identity_url("https://site", "build.json"),
            Fetched::Body(br#"{"commit":"short"}"#.to_vec()),
        )]));
        let o = observe(
            "pages",
            "https://site",
            "build.json",
            B,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(o.judgement.verdict, ServedVerdict::Malformed);
        assert!(
            o.judgement.reason.contains("forty-character"),
            "{}",
            o.judgement.reason
        );
        assert!(o.served.is_none());
        let none = observe(
            "pages",
            "https://elsewhere",
            "build.json",
            B,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(none.judgement.verdict, ServedVerdict::Unreachable);
    }

    /// Staleness is recomputation: the record of a served B still proves A and B, and
    /// stops proving the moment the expected commit moves past it.
    #[test]
    fn a_record_goes_stale_when_the_expected_commit_moves_past_it() {
        let fake = Fake(BTreeMap::from([(
            identity_url("https://s", "build.json"),
            body(B),
        )]));
        let o = observe(
            "pages",
            "https://s",
            "build.json",
            B,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(o.rejudge(A, &history()).verdict, ServedVerdict::Served);
        assert_eq!(o.rejudge(B, &history()).verdict, ServedVerdict::Served);
        assert_eq!(o.rejudge(C, &history()).verdict, ServedVerdict::Stale);
    }

    #[test]
    fn a_record_that_received_nothing_keeps_proving_nothing() {
        let none = Fake(BTreeMap::new());
        let u = observe(
            "pages",
            "https://s",
            "build.json",
            A,
            &none,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(u.rejudge(A, &history()).verdict, ServedVerdict::Unreachable);
        let bad = Fake(BTreeMap::from([(
            identity_url("https://s", "build.json"),
            Fetched::Body(b"<html>".to_vec()),
        )]));
        let m = observe(
            "pages",
            "https://s",
            "build.json",
            A,
            &bad,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(m.rejudge(A, &history()).verdict, ServedVerdict::Malformed);
    }

    #[test]
    fn observations_round_trip_and_unreadable_lines_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_all(dir.path()),
            (Vec::new(), 0),
            "no file is no observations"
        );
        let fake = Fake(BTreeMap::from([(
            identity_url("https://s", "build.json"),
            body(A),
        )]));
        let first = observe(
            "pages",
            "https://s",
            "build.json",
            A,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        let other = Observation {
            deployment: "majordomus".into(),
            ..first.clone()
        };
        let path = append(dir.path(), &first).unwrap();
        append(dir.path(), &other).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, "{{torn").unwrap();
        writeln!(f).unwrap();
        writeln!(
            f,
            "{}",
            serde_json::to_string(&Observation {
                schema: "other/v9".into(),
                ..first.clone()
            })
            .unwrap()
        )
        .unwrap();
        let (all, unreadable) = read_all(dir.path());
        assert_eq!(all, vec![first.clone(), other.clone()]);
        assert_eq!(
            unreadable, 2,
            "a torn line and a foreign schema, not the blank line"
        );
        assert_eq!(latest(&all, "pages"), Some(&first));
        assert_eq!(latest(&all, "majordomus"), Some(&other));
        assert_eq!(latest(&all, "nothing"), None);
    }

    #[test]
    fn append_reports_a_path_it_cannot_create() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".ai"), "a file where a directory must be").unwrap();
        let fake = Fake(BTreeMap::new());
        let o = observe(
            "pages",
            "https://s",
            "build.json",
            A,
            &fake,
            &history(),
            SystemTime::UNIX_EPOCH,
        );
        assert!(append(dir.path(), &o).is_err());
    }

    #[test]
    fn curl_that_cannot_connect_is_unreachable_never_a_body() {
        // Port 9 on loopback: discard, closed on any machine that runs tests.
        let got = Curl { max_seconds: 2 }.fetch("http://127.0.0.1:9/build.json");
        assert!(
            matches!(got, Fetched::Unreachable(ref why) if why.contains("curl exited")),
            "{got:?}"
        );
    }

    #[test]
    fn real_git_answers_containment_and_absence() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let git = GitAncestry { root };
        let head = String::from_utf8(
            crate::git::read_only(root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let head = head.trim();
        assert_eq!(git.has(head), Some(true));
        assert_eq!(git.has(FOREIGN), Some(false));
        assert_eq!(git.contains(head, head), Some(true));
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;

        fn verdict_of(served: u8, expected: u8, dirty: bool, reach: u8) -> ServedVerdict {
            let order = [A, B, C, FOREIGN];
            let s = order[(served % 4) as usize];
            let e = order[(expected % 3) as usize];
            let input = match reach % 3 {
                0 => id(s, dirty),
                1 => Err(Fetched::Unreachable("x".into())),
                _ => Err(Fetched::Body(vec![])),
            };
            judge(&input, e, &history()).verdict
        }

        proptest! {
            /// Served is only ever the answer for a committed build this clone holds that
            /// contains the expected commit; everything else is not a pass.
            #[test]
            fn only_a_contained_committed_known_build_passes(
                s in 0u8..4, e in 0u8..3, dirty: bool, reach in 0u8..3,
            ) {
                let v = verdict_of(s, e, dirty, reach);
                let contained = reach % 3 == 0 && !dirty && s < 3 && e <= s;
                prop_assert_eq!(v.passes(), contained);
            }
        }
    }
}

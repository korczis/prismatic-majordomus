//! The reports this repository generates as web surfaces: the test run, the benchmark run,
//! and the UI conformance audit that is a section of the first.
//!
//! A report is a *rendering* of evidence somebody else produced. The suite writes its
//! results, the benchmark run writes its own, and this module turns either into a directory
//! under the generated web root: an `index.html` a person reads, the machine-readable
//! results beside it, and the `surface.json` that declares what the directory is for. It
//! renders; it never runs anything and never decides what passed.
//!
//! The HTML is deliberately self-contained — its own style, no external asset, and every
//! link relative — so that the same directory works at `/tests`, at `/reports/tests` or
//! opened from disk. That is the whole of the "static-safe URL" problem, solved once by not
//! creating it.
//!
//! The lifecycle of a report is three writes into one directory and no registration
//! anywhere: declare the directory, write the rendering, write the evidence it was made
//! from. Discovery reads the declaration back, so nothing else in the repository has to be
//! told the surface exists.
//!
//! ```
//! use majordomus_cli::web::{discover, report};
//! let tmp = tempfile::tempdir().unwrap();
//! let dir = report::declare(tmp.path(), "example", "An example", "the example producer")
//!     .unwrap();
//! report::write(&dir, "index.html", "<h1>An example</h1>").unwrap();
//! report::write_json(&dir, "results.json", &serde_json::json!({ "cases": 1 })).unwrap();
//!
//! // nobody registered it, and it is a surface
//! let found = discover::generated(tmp.path()).unwrap();
//! assert_eq!(found.len(), 1);
//! assert_eq!(found[0].mount.as_str(), "/example");
//! assert_eq!(found[0].producer, "the example producer");
//! ```

pub(crate) mod benchmarks;
/// The page style every generated surface uses. It lives beside the model now that the
/// home page renders through it too; this re-export keeps the reports' own path to it.
pub use super::html;
pub mod tests;
pub mod ui;

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::discover::{Declaration, DECLARATION_FILE, DECLARATION_SCHEMA, GENERATED_ROOT};
use super::model::{Availability, Category, Visibility};
use crate::error::{Error, Result};

/// What every report writes beside its rendering: the declaration that makes the directory
/// a surface.
///
/// The producer states the intent — its id, where it belongs, what built it — and discovery
/// reads it back. Nothing else in the repository is told that the surface exists.
///
/// The mount is `/<id>` and is not a parameter: a report that could be mounted anywhere is
/// a report whose address two producers can disagree about, and the id is already the
/// selector. The revision is stamped from the repository, which is what later makes a stale
/// directory a finding rather than a surprise.
///
/// It overwrites: calling it again restates this producer's own intent, which is right for
/// the producer that owns the surface and wrong for anything else — see
/// [`declare_if_absent`].
///
/// ```
/// use majordomus_cli::web::report;
/// let tmp = tempfile::tempdir().unwrap();
/// let dir = report::declare(tmp.path(), "benchmarks", "Recorded runs", "bench report")
///     .unwrap();
/// assert!(dir.ends_with("target/web/benchmarks"));
/// let declaration = std::fs::read_to_string(dir.join("surface.json")).unwrap();
/// assert!(declaration.contains("\"mount\": \"/benchmarks\""), "{declaration}");
/// assert!(declaration.contains("web-surface/v1"));
/// ```
pub fn declare(root: &Path, id: &str, title: &str, producer: &str) -> Result<PathBuf> {
    let dir = directory(root, id);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    let declaration = Declaration {
        schema: DECLARATION_SCHEMA.into(),
        id: id.into(),
        mount: format!("/{id}"),
        title: Some(title.into()),
        index: Some("index.html".into()),
        producer: Some(producer.into()),
        availability: Some(Availability::Both),
        category: Some(Category::Report),
        visibility: Some(Visibility::Public),
        built_from: Origin::read(root).revision,
    };
    let path = dir.join(DECLARATION_FILE);
    let mut body =
        serde_json::to_string_pretty(&declaration).map_err(|e| Error::InvalidSurface {
            surface: id.into(),
            reason: format!("its declaration cannot be serialised: {e}"),
        })?;
    body.push('\n');
    std::fs::write(&path, body).map_err(|e| Error::io(path.display().to_string(), e))?;
    Ok(dir)
}

/// Declare a surface only when nothing has declared it yet.
///
/// A report that is a *section* of another's surface still needs that surface to exist —
/// an unreachable section is not a report — but it must never restate the enclosing
/// producer's identity over the top of it. So: create when absent, leave alone when there.
///
/// ```
/// use majordomus_cli::web::report;
/// let tmp = tempfile::tempdir().unwrap();
/// // the surface's owner declares it
/// report::declare(tmp.path(), "tests", "Test results", "bash test/run.sh").unwrap();
/// // a section of it asks for the same surface and does not take it over
/// let dir = report::declare_if_absent(tmp.path(), "tests", "UI", "scripts/ui audit").unwrap();
/// let declaration = std::fs::read_to_string(dir.join("surface.json")).unwrap();
/// assert!(declaration.contains("bash test/run.sh"), "{declaration}");
/// assert!(!declaration.contains("scripts/ui audit"));
///
/// // and where nobody has declared it, the section declares it rather than being
/// // unreachable
/// let fresh = tempfile::tempdir().unwrap();
/// let dir = report::declare_if_absent(fresh.path(), "tests", "UI", "scripts/ui audit").unwrap();
/// assert!(dir.join("surface.json").is_file());
/// ```
pub fn declare_if_absent(root: &Path, id: &str, title: &str, producer: &str) -> Result<PathBuf> {
    let dir = directory(root, id);
    if dir.join(DECLARATION_FILE).is_file() {
        return Ok(dir);
    }
    declare(root, id, title, producer)
}

/// Where a report's directory is, absolute.
///
/// One directory per surface under the generated root, named by the id and owned by its
/// producer alone: two generators never write into one tree, so a rebuild of one cannot
/// delete the other's output. Nothing here creates it.
///
/// ```
/// use majordomus_cli::web::report;
/// let root = std::path::Path::new("/repo");
/// assert_eq!(report::directory(root, "tests"), root.join("target/web/tests"));
/// // one directory per surface, so no two producers are handed the same tree
/// assert_ne!(report::directory(root, "tests"), report::directory(root, "benchmarks"));
/// ```
pub fn directory(root: &Path, id: &str) -> PathBuf {
    root.join(GENERATED_ROOT).join(id)
}

/// Write one file inside a report's directory, creating what it needs.
///
/// `name` may carry a subdirectory — a section of a report writes `ui/index.html` — and the
/// parents are created rather than being an error the caller has to pre-empt. The failure
/// it does report is the one that matters: an unwritable path, named, so a reader knows
/// which file the producer could not finish.
///
/// ```
/// use majordomus_cli::web::report;
/// let tmp = tempfile::tempdir().unwrap();
/// let dir = report::directory(tmp.path(), "tests");
/// report::write(&dir, "ui/index.html", "<h1>UI conformance</h1>").unwrap();
/// let written = std::fs::read_to_string(dir.join("ui/index.html")).unwrap();
/// assert!(written.contains("UI conformance"));
/// ```
pub fn write(dir: &Path, name: &str, body: &str) -> Result<()> {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent.display().to_string(), e))?;
    }
    std::fs::write(&path, body).map_err(|e| Error::io(path.display().to_string(), e))
}

/// Write a value as pretty JSON beside a rendering: the evidence the page was made from,
/// kept because a page nobody can check is not evidence.
///
/// Pretty and newline-terminated because it is committed to a repository and read in
/// diffs, not because a machine needs it that way.
///
/// ```
/// use majordomus_cli::web::report;
/// let tmp = tempfile::tempdir().unwrap();
/// let dir = report::directory(tmp.path(), "tests");
/// report::write_json(&dir, "results.json", &serde_json::json!({ "passed": 93 })).unwrap();
/// let body = std::fs::read_to_string(dir.join("results.json")).unwrap();
/// assert!(body.ends_with("\n"), "a committed file ends with a newline");
/// assert!(body.contains("\n  \"passed\": 93"), "{body}");
/// ```
pub fn write_json<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<()> {
    let mut body = serde_json::to_string_pretty(value).map_err(|e| Error::InvalidSurface {
        surface: name.into(),
        reason: format!("cannot be serialised: {e}"),
    })?;
    body.push('\n');
    write(dir, name, &body)
}

/// What a report says about where it came from, printed on every page.
///
/// Three facts and no more: which commit the evidence was produced from, when it was
/// rendered, and which executable rendered it. The revision is optional because a report
/// rendered outside a checkout is still a report, and the other two always exist, so a
/// footer can be written without asking whether it has anything to say.
///
/// ```
/// use majordomus_cli::web::report::Origin;
/// let anonymous = Origin {
///     revision: None,
///     rendered_at: "2026-09-10T12:00:00Z".into(),
///     version: "0.5.0".into(),
/// };
/// let json = serde_json::to_value(&anonymous).unwrap();
/// assert_eq!(json["version"], "0.5.0");
/// assert!(json["revision"].is_null(), "a report outside a checkout names no revision");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origin {
    /// The commit the evidence was produced from, when git could say.
    pub revision: Option<String>,
    /// When the report was rendered, in UTC.
    pub rendered_at: String,
    /// The executable that rendered it.
    pub version: String,
}

impl Origin {
    /// Read the origin from the repository, without failing when git is not there: a report
    /// rendered outside a checkout is still a report.
    ///
    /// The revision is asked of git and is `None` for anything that does not answer — no
    /// git, not a checkout, a repository with no commit. Nothing else about the origin
    /// depends on the environment, so a report is always renderable.
    ///
    /// ```no_run
    /// use majordomus_cli::web::report::Origin;
    /// // reads the checkout it is pointed at, so it is compiled here and not run
    /// let origin = Origin::read(std::path::Path::new("."));
    /// assert!(origin.rendered_at.ends_with('Z'), "the stamp is UTC");
    /// assert!(!origin.version.is_empty());
    /// ```
    pub fn read(root: &Path) -> Self {
        let revision = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Origin {
            revision,
            rendered_at: crate::web::report::now(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// The current time as `YYYY-MM-DDTHH:MM:SSZ`, without a date dependency.
fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Days since the Unix epoch to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_declaration_makes_a_directory_discoverable() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = declare(
            tmp.path(),
            "example",
            "Example report",
            "the example producer",
        )
        .unwrap();
        assert!(dir.join(DECLARATION_FILE).is_file());
        let found = crate::web::discover::generated(tmp.path()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "example");
        assert_eq!(found[0].mount.as_str(), "/example");
        assert_eq!(found[0].producer, "the example producer");
    }

    #[test]
    fn the_epoch_and_a_known_day_render_as_the_dates_they_are() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_608), (2026, 6, 4));
    }

    #[test]
    fn a_rendered_time_has_the_shape_every_report_prints() {
        let stamp = now();
        assert_eq!(stamp.len(), 20, "{stamp}");
        assert!(stamp.ends_with('Z'));
        assert!(stamp.contains('T'));
    }
}

//! The benchmark report: the run's own results, rendered.
//!
//! The evidence is a result document `majordomus bench` already writes — the accepted
//! baselines under the layer's benchmarks section have the same shape — and this module
//! renders it. It measures nothing: a figure on the page is a figure a run produced, which
//! is the whole of `project.performance-evidence` applied to the page as well as the prose.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{html, Origin};
use crate::error::{Error, Result};

/// A results document, read loosely: the fields this report needs, and nothing else, so a
/// document that grows a field does not stop rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// When the run finished, as the run recorded it.
    #[serde(default)]
    pub finished_at: Option<String>,
    /// The build profile the run measured.
    #[serde(default)]
    pub profile: Option<String>,
    /// One entry per measured target.
    #[serde(default)]
    pub results: Vec<Measurement>,
}

/// One measured target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// The target's key: what was measured, over which transport, with which case.
    pub key: String,
    /// Whether the run went through a cache.
    #[serde(default)]
    pub cache_mode: Option<String>,
    /// The distribution the run observed.
    pub stats: Stats,
}

/// The distribution of one target's samples, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// How many samples the figure rests on.
    pub samples: u64,
    /// The median.
    pub p50_us: f64,
    /// The 95th percentile.
    pub p95_us: f64,
    /// The 99th percentile.
    pub p99_us: f64,
    /// The slowest sample.
    pub max_us: f64,
}

/// Read a results document from disk.
pub fn read(path: &Path) -> Result<Document> {
    let text =
        std::fs::read_to_string(path).map_err(|e| Error::io(path.display().to_string(), e))?;
    serde_json::from_str(&text).map_err(|e| Error::InvalidSurface {
        surface: "benchmarks".into(),
        reason: format!(
            "{} does not parse as a results document: {e}",
            path.display()
        ),
    })
}

/// Render the document into its own directory under the generated web root, and declare it.
pub fn render(root: &Path, document: &Document, source: &str) -> Result<std::path::PathBuf> {
    let dir = super::declare(
        root,
        "benchmarks",
        "Benchmark results",
        "majordomus bench, rendered by majordomus web report benchmarks",
    )?;
    let origin = Origin::read(root);
    super::write_json(&dir, "results.json", document)?;

    let mut ordered = document.results.clone();
    ordered.sort_by(|a, b| {
        b.stats
            .p50_us
            .partial_cmp(&a.stats.p50_us)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    let rows: Vec<Vec<String>> = ordered
        .iter()
        .map(|m| {
            vec![
                format!("<span class=\"mono\">{}</span>", html::escape(&m.key)),
                html::escape(m.cache_mode.as_deref().unwrap_or("—")),
                format!("<span class=\"num\">{}</span>", m.stats.samples),
                micros(m.stats.p50_us),
                micros(m.stats.p95_us),
                micros(m.stats.p99_us),
                micros(m.stats.max_us),
            ]
        })
        .collect();

    let slowest = ordered
        .first()
        .map(|m| m.key.clone())
        .unwrap_or_else(|| "—".into());
    let summary = html::summary(&[
        ("targets", document.results.len().to_string()),
        (
            "profile",
            document.profile.clone().unwrap_or_else(|| "—".into()),
        ),
        (
            "finished",
            document.finished_at.clone().unwrap_or_else(|| "—".into()),
        ),
        ("slowest median", slowest),
    ]);
    let body = format!(
        "{summary}<h2>Every measured target</h2>{}{}",
        html::table(
            &["target", "cache", "samples", "p50 µs", "p95 µs", "p99 µs", "max µs"],
            &rows
        ),
        html::origin(&origin, &[("results.json", "results.json")])
    );
    let page = html::page(
        "Benchmark results",
        &format!(
            "Every externally callable operation, timed. Read from {}.",
            html::escape(source)
        ),
        &body,
    );
    super::write(&dir, "index.html", &page)?;
    Ok(dir)
}

/// Microseconds, at the precision a reader can act on.
fn micros(value: f64) -> String {
    format!("<span class=\"num\">{value:.1}</span>")
}

#[cfg(test)]
mod unit {
    use super::*;

    fn document() -> Document {
        Document {
            finished_at: Some("2026-09-06T00:00:00Z".into()),
            profile: Some("debug".into()),
            results: vec![
                Measurement {
                    key: "capabilities.describe|direct|repository-info".into(),
                    cache_mode: Some("uncached".into()),
                    stats: Stats {
                        samples: 200,
                        p50_us: 41.5,
                        p95_us: 76.5,
                        p99_us: 112.7,
                        max_us: 180.0,
                    },
                },
                Measurement {
                    key: "objects.search|http|one-word".into(),
                    cache_mode: None,
                    stats: Stats {
                        samples: 200,
                        p50_us: 900.0,
                        p95_us: 1200.0,
                        p99_us: 1500.0,
                        max_us: 2000.0,
                    },
                },
            ],
        }
    }

    #[test]
    fn a_real_baseline_document_parses() {
        let text = r#"{"schema":1,"finished_at":"x","profile":"debug","provenance":{"a":1},
            "results":[{"key":"k","kind":{"kind":"capability"},"cache_mode":"uncached",
            "stats":{"samples":200,"min_us":1.0,"p50_us":2.0,"p90_us":3.0,"p95_us":4.0,
                     "p99_us":5.0,"max_us":6.0,"mean_us":2.5}}]}"#;
        let doc: Document = serde_json::from_str(text).unwrap();
        assert_eq!(doc.results.len(), 1);
        assert_eq!(doc.results[0].stats.p95_us, 4.0);
    }

    #[test]
    fn the_report_declares_itself_and_orders_by_median() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = render(
            tmp.path(),
            &document(),
            ".ai/repo/benchmarks/rust/baseline.json",
        )
        .unwrap();
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let search = page.find("objects.search").unwrap();
        let describe = page.find("capabilities.describe").unwrap();
        assert!(search < describe, "the slowest target is listed first");
        let surfaces = crate::web::discover::generated(tmp.path()).unwrap();
        assert_eq!(surfaces[0].mount.as_str(), "/benchmarks");
    }

    #[test]
    fn a_document_that_is_not_one_is_refused_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("not-results.json");
        std::fs::write(&path, "{\"results\": 3}").unwrap();
        let err = read(&path).unwrap_err().to_string();
        assert!(err.contains("results document"), "{err}");
    }
}

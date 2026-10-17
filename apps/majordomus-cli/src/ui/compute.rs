//! Browser compute: the portable core run in this browser, and the server's verdict on what
//! it computed (ADR 0170, I2788). Read from `capabilities.describe` (what `compute.verify`
//! is and where it is served) and from the provenance the committed module carries.
//!
//! Nothing computes until the person asks. Then a same-origin Worker loads the committed
//! wasm32 module, computes the canonical digest of the JSON on the page, and the page asks
//! `compute.verify` to recompute it: the browser's answer is a claim, and the verdict shown is
//! the server's. Only the Worker script's response allows WebAssembly compilation
//! ([`worker_policy`]); this page keeps the Cockpit's policy.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{hero, kpi, panel, table, unanswered};

/// The route of the screen.
pub const ROUTE: &str = "/ui/compute";

/// The Worker that runs the module, under the UI's assets.
pub const WORKER: &str = "compute/worker.js";

/// The script that wires the page's form to the Worker and to `compute.verify`.
pub const PAGE: &str = "compute/page.js";

/// The committed wasm32 build of `apps/majordomus-core`.
pub const MODULE: &str = "compute/majordomus_core.wasm";

/// What the committed module was built from, written beside it when it is rebuilt.
pub const PROVENANCE: &str = "compute/majordomus_core.json";

/// The route the page asks to recompute a claim: `compute.verify`'s HTTP projection.
pub const VERIFY: &str = "/api/v1/compute/verify";

/// The JSON the form holds when the page opens: keys out of order, nested, with a string
/// that is not ASCII, so the canonical form differs from the text.
pub const SAMPLE: &str = r#"{"b": [3, 1.5, null], "a": {"z": true, "é": "ü"}}"#;

const VERIFIER: Source = Source::with("capabilities.describe", r#"{"id":"compute.verify"}"#);

/// The shared fixture every runtime of the portable core is held to (ADR 0170), compiled in
/// from the core so that the page runs the very rows the gates run and no copy exists (I2809).
pub const FIXTURE: &str =
    include_str!("../../../majordomus-core/fixtures/canonical-digest.v1.json");

/// The URLs the page hands to its script: each the digested URL of a file the UI serves.
#[derive(Debug, Clone, Default)]
pub struct Files {
    /// [`WORKER`]'s URL.
    pub worker: String,
    /// [`PAGE`]'s URL.
    pub page: String,
    /// [`MODULE`]'s URL.
    pub module: String,
}

/// The policy the Worker script is answered with: the Cockpit's sources, and WebAssembly
/// compilation, which no page is granted. A dedicated Worker from this origin is governed
/// by the policy of its own script's response, so this is the one place the module may be
/// compiled.
pub fn worker_policy() -> &'static str {
    "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'"
}

/// The Browser compute screen. `provenance` is the committed module's record, when the
/// distribution has one.
pub fn screen(ctx: &Context, files: &Files, provenance: Option<&Value>) -> View {
    let answers = ask_all(ctx, &[VERIFIER]);
    let verifier = &answers[&VERIFIER];

    let top = hero(
        "bolt",
        "violet",
        "Browser compute",
        "Whether this browser computes what the server computes.",
        "The portable core runs here as WebAssembly when you ask it to, and the server recomputes what it answered: the browser's result is a claim, and the verdict is the server's.",
        vec![
            kpi("Verifier", &verifier.read("/id")),
            kpi("Effect", &verifier.read("/execution/effect")),
            kpi("Route", &verifier.read("/exposure/http/path")),
        ],
    );

    let form = el("form")
        .class("mj-ui-compute-form")
        .attr("data-compute-form", "")
        .flag("hidden")
        .child(
            el("label")
                .attr("for", "mj-ui-compute-input")
                .text("JSON to compute the canonical digest of"),
        )
        .child(
            el("textarea")
                .class("mj-textarea")
                .attr("id", "mj-ui-compute-input")
                .attr("name", "input")
                .attr("rows", "5")
                .attr("spellcheck", "false")
                .text(SAMPLE),
        )
        .child(
            el("label")
                .class("mj-ui-compute-tamper")
                .child(el("input").attr("type", "checkbox").attr("name", "tamper"))
                .child(el("span").text(
                    " Claim a wrong digest (one digit changed), to see the server refuse it",
                )),
        )
        .child(
            el("button")
                .attr("type", "submit")
                .class("mj-btn mj-btn--primary")
                .text("Compute in this browser"),
        );
    let results = table(
        &["What", "Answer"],
        [
            ("browser", "Digest this browser computed"),
            ("millis", "Time in the module (ms)"),
            ("verdict", "The server's verdict"),
            ("recomputed", "Digest the server recomputed"),
            ("reason", "Why"),
        ]
        .into_iter()
        .map(|(key, label)| {
            el("tr")
                .child(el("th").attr("scope", "row").text(label))
                .child(
                    // a digest is one unbroken word: the kit's wrapping command text breaks it
                    el("td").class("mj-cmd--wrap").child(
                        el("output")
                            .class("mj-cmd-text mj-ui-mono")
                            .attr("data-compute-out", key)
                            .attr("aria-live", "polite")
                            .text("—"),
                    ),
                )
        })
        .collect(),
        "",
    );
    let compute = el("div")
        .class("mj-ui-compute")
        .attr("data-compute", "")
        .attr("data-compute-worker", files.worker.clone())
        .attr("data-compute-module", files.module.clone())
        .attr("data-compute-verify", VERIFY)
        .child(
            el("p")
                .attr("data-compute-needs-script", "")
                .text("Computing in the browser needs JavaScript; nothing on this page computes without it. The server's verification is POST ")
                .child(el("code").text(VERIFY))
                .child(el("span").text(".")),
        )
        .child(form)
        .child(results)
        .child(
            el("script")
                .attr("type", "module")
                .attr("src", files.page.clone()),
        );

    let module = provenance_table(provenance);
    let grid = el("div")
        .class("mj-ui-grid")
        .child(panel(
            "Compute in this browser",
            "Only when you ask; the module runs in a Worker of this origin and the server recomputes its answer.",
            unanswered(verifier).unwrap_or(compute),
            &[verifier],
        ))
        .child(panel(
            "The module",
            "The committed wasm32 build of the portable core, and the sources it was built from.",
            module,
            &[verifier],
        ))
        .child(super::view::wide(panel(
            "The shared fixture, in this browser",
            "Every row the gates hold each runtime to, computed here by the module and recomputed by the server: only when you ask.",
            fixture_rows(),
            &[verifier],
        )));

    View::new(
        "Browser compute",
        el("div")
            .class("mj-ui-stack")
            .attr("data-keeps-its-place", "")
            .child(top)
            .child(grid),
    )
}

/// The fixture's rows as a table the page's script fills: each case with the digest the fixture
/// records, each invalid row with the refusal it must meet. The inputs ride on the rows as
/// attributes, so the browser computes exactly the fixture's text.
fn fixture_rows() -> El {
    let fixture: Value = match serde_json::from_str(FIXTURE) {
        Ok(v) => v,
        Err(e) => {
            return super::view::nothing(&format!(
                "The fixture compiled into this executable does not parse: {e}"
            ))
        }
    };
    let out = |key: &str| {
        el("td").class("mj-cmd--wrap").child(
            el("output")
                .class("mj-cmd-text mj-ui-mono")
                .attr("data-fixture-out", key)
                .text("—"),
        )
    };
    let rows = |kind: &'static str| {
        fixture
            .get(if kind == "case" { "cases" } else { "invalid" })
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(move |row| {
                let id = row
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let input = row
                    .get("input")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let expected = match kind {
                    "case" => row
                        .get("digest")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    _ => String::new(),
                };
                el("tr")
                    .attr("data-fixture-row", id.clone())
                    .attr("data-fixture-kind", kind)
                    .attr("data-fixture-input", input)
                    .attr("data-fixture-digest", expected.clone())
                    .child(el("th").attr("scope", "row").child(el("code").text(id)))
                    .child(el("td").class("mj-cmd--wrap").child(
                        el("code").class("mj-cmd-text").text(match kind {
                            "case" => expected,
                            _ => "refused (not a canonical-digest input)".to_string(),
                        }),
                    ))
                    .child(out("browser"))
                    .child(out("agrees"))
                    .child(out("verdict"))
            })
            .collect::<Vec<_>>()
    };
    let mut all = rows("case");
    all.extend(rows("invalid"));
    el("div")
        .class("mj-ui-stack")
        .attr("data-compute-fixture", "")
        .child(
            el("p")
                .class("mj-ui-line")
                .child(
                    el("button")
                        .attr("type", "button")
                        .class("mj-btn mj-btn--primary")
                        .attr("data-fixture-run", "")
                        // enabled by page.js: nothing runs without it
                        .flag("disabled")
                        .text("Run every row in this browser"),
                )
                .child(
                    el("output")
                        .class("mj-ui-dim")
                        .attr("data-fixture-summary", "")
                        .attr("aria-live", "polite")
                        .text(" Not run."),
                ),
        )
        .child(table(
            &[
                "Row",
                "The fixture says",
                "This browser",
                "Agrees",
                "The server",
            ],
            all,
            "The fixture holds no row.",
        ))
}

/// The module's provenance as rows, or a line saying the distribution has none.
fn provenance_table(provenance: Option<&Value>) -> El {
    let Some(record) = provenance else {
        return super::view::nothing(
            "This distribution carries no record of the module's sources (share/ui/compute/majordomus_core.json).",
        );
    };
    let field = |key: &str| {
        record
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("—")
            .to_string()
    };
    table(
        &["Field", "Value"],
        [
            ("Module SHA-256", "module_sha256"),
            ("Sources SHA-256", "sources_sha256"),
            ("Toolchain", "toolchain"),
            ("Built on", "host"),
            ("Rebuilt by", "rebuild"),
        ]
        .into_iter()
        .map(|(label, key)| {
            el("tr")
                .child(el("th").attr("scope", "row").text(label))
                .child(
                    el("td")
                        .class("mj-cmd--wrap")
                        .child(el("code").class("mj-cmd-text").text(field(key))),
                )
        })
        .collect(),
        "",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_worker_may_compile_webassembly_and_never_eval() {
        let worker = worker_policy();
        assert!(
            worker.contains("script-src 'self' 'wasm-unsafe-eval'"),
            "{worker}"
        );
        assert!(
            worker.contains("connect-src 'self'"),
            "the module comes from this origin"
        );
        assert!(
            !worker.contains("'unsafe-eval'"),
            "WebAssembly only, never eval"
        );
        assert!(!worker.contains("http"), "no other origin");
        let page = crate::cockpit::csp();
        assert!(
            !page.contains("wasm-unsafe-eval"),
            "no page is granted it: {page}"
        );
    }
}

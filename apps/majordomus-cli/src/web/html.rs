//! The one page style every generated report uses.
//!
//! Self-contained by rule: the style is inline, no asset is fetched, and every link is
//! relative, so a report works at whatever mount it is given and from the filesystem. It is
//! mobile-first in the same sense the site is — one readable column at any width, tables
//! that scroll inside their own box rather than pushing the page sideways — because these
//! pages are this repository's own UI, not a third party's output.

/// Escape text for HTML: the only escaping a generated page needs, applied at every
/// interpolation rather than hoped for.
///
/// ```
/// use majordomus_cli::web::report::html::escape;
/// assert_eq!(escape("a<b & c\">"), "a&lt;b &amp; c&quot;&gt;");
/// ```
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The document around a report's body.
///
/// `subtitle` is the one line under the title; `body` is already-escaped HTML.
pub fn page(title: &str, subtitle: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --fg: #111; --bg: #fff; --muted: #555; --line: #d8d8d8;
         --ok: #0a7c3f; --bad: #b3261e; --soft: #f6f6f6; }}
@media (prefers-color-scheme: dark) {{
  :root {{ --fg: #e8e8e8; --bg: #14161a; --muted: #a6a6a6; --line: #2f3339;
           --ok: #4ade80; --bad: #ff6b6b; --soft: #1b1e24; }}
}}
* {{ box-sizing: border-box; }}
body {{ margin: 0; padding: 1.25rem 1rem 3rem; background: var(--bg); color: var(--fg);
        font: 15px/1.55 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }}
main {{ max-width: 62rem; margin: 0 auto; }}
h1 {{ font-size: 1.5rem; margin: 0 0 .25rem; }}
h2 {{ font-size: 1.1rem; margin: 2rem 0 .5rem; }}
p.lede {{ color: var(--muted); margin: 0 0 1.5rem; }}
dl.summary {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
              gap: .75rem; margin: 0 0 1.5rem; padding: 0; }}
dl.summary > div {{ border: 1px solid var(--line); border-radius: .5rem; padding: .75rem; background: var(--soft); }}
dl.summary dt {{ color: var(--muted); font-size: .8rem; margin: 0; }}
dl.summary dd {{ margin: .25rem 0 0; font-size: 1.35rem; font-variant-numeric: tabular-nums; }}
.scroll {{ overflow-x: auto; -webkit-overflow-scrolling: touch; border: 1px solid var(--line); border-radius: .5rem; }}
table {{ border-collapse: collapse; width: 100%; font-size: .9rem; }}
th, td {{ text-align: left; padding: .5rem .65rem; border-bottom: 1px solid var(--line); white-space: nowrap; }}
th {{ background: var(--soft); font-weight: 600; }}
td.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
tr:last-child td {{ border-bottom: 0; }}
.pass {{ color: var(--ok); font-weight: 600; }}
.fail {{ color: var(--bad); font-weight: 600; }}
code, .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .85em; }}
footer {{ margin-top: 2.5rem; color: var(--muted); font-size: .8rem; border-top: 1px solid var(--line); padding-top: .75rem; }}
a {{ color: inherit; text-decoration: underline; }}
</style>
</head>
<body>
<main>
<h1>{title}</h1>
<p class="lede">{subtitle}</p>
{body}
</main>
</body>
</html>
"#
    )
}

/// A table inside its own scrolling box: wide evidence never pushes the page sideways.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    // the box scrolls, so it is reachable from the keyboard: a scrollable region that only
    // a pointer can reach is the accessibility defect the site audit refuses (WCAG 2.1.1)
    let mut out = String::from("<div class=\"scroll\" tabindex=\"0\"><table><thead><tr>");
    for header in headers {
        out.push_str(&format!("<th>{}</th>", escape(header)));
    }
    out.push_str("</tr></thead><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str(&format!("<td>{cell}</td>"));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    out
}

/// The summary tiles at the top of a report.
pub fn summary(items: &[(&str, String)]) -> String {
    let mut out = String::from("<dl class=\"summary\">");
    for (label, value) in items {
        out.push_str(&format!(
            "<div><dt>{}</dt><dd>{}</dd></div>",
            escape(label),
            escape(value)
        ));
    }
    out.push_str("</dl>");
    out
}

/// The footer every report carries: where the evidence came from.
pub fn origin(origin: &super::report::Origin, evidence: &[(&str, &str)]) -> String {
    let mut out = String::from("<footer>");
    out.push_str(&format!(
        "rendered {} by majordomus {}",
        escape(&origin.rendered_at),
        escape(&origin.version)
    ));
    if let Some(revision) = &origin.revision {
        out.push_str(&format!(
            " at revision <span class=\"mono\">{}</span>",
            escape(revision)
        ));
    }
    if !evidence.is_empty() {
        out.push_str("<br>evidence: ");
        for (i, (label, href)) in evidence.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape(href),
                escape(label)
            ));
        }
    }
    out.push_str("</footer>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_self_contained_and_declares_a_viewport() {
        let html = page("Title", "Subtitle", "<p>body</p>");
        assert!(html.contains("name=\"viewport\""));
        assert!(!html.contains("<link"), "a report fetches no stylesheet");
        assert!(!html.contains("<script"), "a report runs no script");
        assert!(
            !html.contains("href=\"/"),
            "every link is relative to the mount"
        );
    }

    #[test]
    fn a_table_scrolls_inside_its_own_box() {
        let html = table(&["a"], &[vec!["1".into()]]);
        assert!(
            html.starts_with("<div class=\"scroll\" tabindex=\"0\">"),
            "{html}"
        );
    }

    #[test]
    fn text_from_evidence_cannot_close_a_tag() {
        let html = table(&["name"], &[vec![escape("</td><script>alert(1)</script>")]]);
        assert!(!html.contains("<script>"), "{html}");
    }
}

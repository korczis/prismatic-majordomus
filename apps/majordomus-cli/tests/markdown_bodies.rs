//! A Markdown body as a client is served it: the Cockpit's page for a document whose body
//! carries everything a body can get wrong — a script, a `javascript:` link, a link to a
//! neighbour and one to nothing, an aligned table, a diagram, a heading of its own
//! (project.markdown-is-rendered-once).

mod common;

use common::{Fixture, Served};

const BODY: &str = "\
# The first document

A **strong** claim with `code`, see [the second](SECOND.md#part), [nothing](ABSENT.md),
[the web](https://example.org/page) and [a trap](javascript:alert(1)).

<script>alert('run me')</script>

| source | before | after |
|:---|---:|:---:|
| doctor | 10 s | 1 s |

```mermaid
graph TD; a-->b
```

- [x] shipped
- [ ] owed
";

fn page(served: &Served, uri: &str) -> String {
    let target = format!(
        "/cockpit/object?uri={}",
        uri.replace(':', "%3A").replace('/', "%2F")
    );
    let (status, headers, body) = served.request("GET", &target, None);
    assert_eq!(status, 200, "{target}: {body}");
    let policy = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-security-policy"))
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    assert!(
        policy.contains("script-src 'self'")
            && !policy.contains("script-src 'self' 'unsafe-inline'"),
        "the page is served under a policy that runs no inline script: {policy}"
    );
    body
}

#[test]
fn a_documents_body_is_drawn_and_nothing_it_holds_runs() {
    let f = Fixture::new();
    f.write("docs/FIRST.md", BODY);
    f.write("docs/SECOND.md", "# The second document\n\nPlain.\n");
    f.commit("two documents");
    let served = Served::start(&f.root(), &[]);
    let html = page(&served, "majordomus://document/docs/FIRST.md");

    // drawn as the document it is, under the page's own headings
    for expected in [
        "<div class=\"mj-md\">",
        "<h3 class=\"mj-md-heading\">The first document</h3>",
        "<strong>strong</strong>",
        "<code class=\"mj-md-code\">code</code>",
        "<th scope=\"col\" class=\"mj-md-right\">before</th>",
        "<td class=\"mj-md-center\">1 s</td>",
        "<input type=\"checkbox\" aria-label=\"done\" disabled checked>",
        "data-mj-diagram=\"mermaid\"",
        "graph TD; a--&gt;b",
    ] {
        assert!(
            html.contains(expected),
            "{expected} is missing from the page"
        );
    }
    assert_eq!(html.matches("<h1").count(), 1, "the body adds no second h1");

    // a link leads to an object the index holds, to the web, or nowhere
    assert!(
        html.contains("href=\"/cockpit/object?uri=majordomus%3A%2F%2Fdocument%2Fdocs%2FSECOND.md\">the second</a>"),
        "the neighbour is not linked"
    );
    assert!(html.contains("<span class=\"mj-md-unlinked\" title=\"ABSENT.md\">nothing</span>"));
    assert!(
        html.contains("href=\"https://example.org/page\" rel=\"noopener noreferrer\">the web</a>")
    );
    assert!(
        html.contains("<span class=\"mj-md-unlinked\" title=\"javascript:alert(1)\">a trap</span>")
    );
    assert!(
        !html.contains("href=\"javascript:"),
        "a record chose a scheme and got an anchor"
    );

    // nothing the record holds is markup, and nothing brings an inline style
    assert!(
        !html.contains("<script>alert("),
        "the record's script is in the page as a script"
    );
    assert!(html.contains("&lt;script&gt;alert(&#39;run me&#39;)&lt;/script&gt;"));
    let body = html
        .split("<div class=\"mj-md\">")
        .nth(1)
        .unwrap_or_default();
    let body = body.split("</main>").next().unwrap_or(body);
    assert!(
        !body.contains(" style="),
        "the body carries an inline style"
    );

    // the neighbour's page answers, which is what made the link a link
    let second = page(&served, "majordomus://document/docs/SECOND.md");
    assert!(second.contains("<h3 class=\"mj-md-heading\">The second document</h3>"));
}

#[test]
fn an_object_that_is_not_markdown_is_still_the_text_it_is() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    // whichever policy object the index holds: its URI is the index's to say
    let (status, listed) = served.get("/api/v1/objects?kind=policy");
    assert_eq!(status, 200, "{listed}");
    let uri = listed["objects"][0]["uri"]
        .as_str()
        .expect("the fixture has a policy")
        .to_string();
    let html = page(&served, &uri);
    assert!(
        !html.contains("<div class=\"mj-md\">"),
        "a YAML object was drawn as Markdown"
    );
    assert!(
        html.contains("<pre class=\"mj-pre\""),
        "the policy's text is not shown"
    );
}

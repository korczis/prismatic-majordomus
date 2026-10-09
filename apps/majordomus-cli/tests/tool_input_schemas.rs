//! Every tool's input schema is an object at its root.
//!
//! The Model Context Protocol defines a tool's `inputSchema` as a JSON Schema whose root
//! `type` is `"object"`, and a client holds a server to it: Claude Code validates the whole
//! `tools/list` answer and, when one tool's root is anything else, offers none of the tools.
//! Two tools whose input type was a newtype rendered a bare `$ref` there, and for a month no
//! session of this repository could call a single tool — the peer board among them — while
//! every test of the projection passed, because none of them read the listing the way a
//! client does.
//!
//! Driven by the registry: a tool added tomorrow is held to this with no edit here.

mod common;

use common::Fixture;
use majordomus_cli::mcp::Surface;
use serde_json::Value;

/// Why a schema is not what a client accepts as a tool's input, or `None` when it is.
fn refusal(schema: &Value) -> Option<String> {
    let Some(root) = schema.as_object() else {
        return Some("the schema is not a JSON object".into());
    };
    match root.get("type") {
        Some(Value::String(t)) if t == "object" => None,
        Some(other) => Some(format!("the root `type` is {other}, not \"object\"")),
        None => Some(format!(
            "the root declares no `type`; it carries {:?}",
            root.keys().collect::<Vec<_>>()
        )),
    }
}

#[test]
fn every_tool_input_schema_is_an_object_at_its_root() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let surface = Surface::new(app.context.clone());

    let tools = surface.tools_json();
    assert!(!tools.is_empty(), "the surface projects no tool");
    let refused: Vec<String> = tools
        .iter()
        .filter_map(|tool| {
            let name = tool["name"].as_str().unwrap_or("<unnamed>");
            refusal(&tool["inputSchema"]).map(|why| format!("{name}: {why}"))
        })
        .collect();
    assert!(
        refused.is_empty(),
        "a client that validates tools/list drops every tool when one of these is listed:\n  {}",
        refused.join("\n  ")
    );
}

/// The two inputs that were a bare `$ref` keep the wire shape they had: the fields of the
/// type they wrap, at the root, with nothing nested under a wrapper's name.
#[test]
fn the_context_compiler_inputs_carry_their_fields_at_the_root() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let surface = Surface::new(app.context.clone());
    let tools = surface.tools_json();
    let schema_of = |name: &str| -> Value {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} is not projected"))["inputSchema"]
            .clone()
    };

    let compile = schema_of("majordomus_devcontext");
    assert_eq!(compile["title"], "DevContextInput", "{compile}");
    assert!(compile.get("$ref").is_none(), "{compile}");
    assert!(
        compile["properties"]
            .as_object()
            .is_some_and(|p| !p.is_empty()),
        "{compile}"
    );

    let explain = schema_of("majordomus_devcontext_explain");
    assert_eq!(explain["title"], "DevContextExplainInput", "{explain}");
    assert!(explain.get("$ref").is_none(), "{explain}");
    assert!(
        explain["properties"]["uri"].is_object(),
        "the identifier to explain is a root property: {explain}"
    );
}

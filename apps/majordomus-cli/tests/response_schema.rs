//! Every answer conforms to the schema its capability publishes.
//!
//! The schema is derived from the output type by `schemars` and published in three places:
//! as `outputSchema` on the MCP tool, as a component of the OpenAPI document, and in the
//! registry projections. Nothing until now read it back. A published schema that the
//! answer does not satisfy is worse than no schema: a client generates against it.
//!
//! `serde` and `schemars` describe the same type through two independent code paths, and
//! they can disagree — `skip_serializing_if` on a required field, a `rename` applied by one
//! attribute and not the other, `flatten`, or a hand-written `Serialize`. This is the check
//! that they did not.
//!
//! Driven by the registry: every executable with a case provider is covered, and its cases
//! come from its input type. Adding a capability adds coverage here with no edit.

mod common;

use common::Fixture;
use majordomus_cli::capability::{CaseContext, Context};
use serde_json::Value;

/// Every executable's cases, from its input type, against the fixture's index.
fn cases_of(ctx: &Context, id: &str) -> Vec<(String, Value)> {
    match ctx.registry.cases(id) {
        Some(provider) => provider(&CaseContext { index: &ctx.index })
            .into_iter()
            .map(|c| (c.name.to_string(), c.input))
            .collect(),
        None => Vec::new(),
    }
}

#[test]
fn every_answer_satisfies_the_schema_its_capability_publishes() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();

    let executables: Vec<String> = ctx
        .registry
        .iter()
        .filter(|c| ctx.registry.cases(c.id.as_str()).is_some())
        .map(|c| c.id.to_string())
        .collect();
    assert!(
        !executables.is_empty(),
        "the registry declares at least one executable with cases"
    );

    let mut checked = 0usize;
    let mut violations: Vec<String> = Vec::new();

    for id in &executables {
        let capability = ctx.registry.get(id).expect("the id came from the registry");
        let schema = capability.output.for_mcp();
        let validator = match jsonschema::validator_for(&schema) {
            Ok(v) => v,
            Err(e) => {
                violations.push(format!("{id}: its published output schema does not compile: {e}"));
                continue;
            }
        };

        for (case, input) in cases_of(&ctx, id) {
            let answer = match ctx.execute(id, input) {
                Ok(a) => a,
                // A case that cannot run in this fixture is the business of the tests that
                // own it; this one is about the shape of an answer that was produced.
                Err(_) => continue,
            };
            checked += 1;
            for error in validator.iter_errors(&answer) {
                violations.push(format!(
                    "{id}/{case}: the answer violates the schema this capability publishes \
                     at {}: {error}",
                    error.instance_path()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{} answer(s) do not satisfy their published schema:\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
    assert!(
        checked > 0,
        "no answer was produced, so nothing was actually checked"
    );
    eprintln!("{checked} answer(s) across {} executable(s)", executables.len());
}

/// The check above passes today, so this proves it is capable of failing. It takes a real
/// capability's published schema and an answer of the shape `serde` produces when it and
/// `schemars` disagree — a required member missing, and a member of the wrong type — and
/// asserts the validator reports both. A conformance test that cannot fail says nothing
/// about conformance.
#[test]
fn the_check_reports_an_answer_that_does_not_satisfy_its_schema() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();

    // A capability whose output is an object with required members, chosen from the
    // registry rather than named here, so this test does not rot when ids change.
    let (id, schema) = ctx
        .registry
        .iter()
        .filter(|c| ctx.registry.cases(c.id.as_str()).is_some())
        .map(|c| (c.id.to_string(), c.output.for_mcp()))
        .find(|(_, s)| {
            s.get("required")
                .and_then(Value::as_array)
                .is_some_and(|r| !r.is_empty())
        })
        .expect("some executable publishes an output schema with a required member");

    let validator = jsonschema::validator_for(&schema).expect("the published schema compiles");
    let required = schema["required"][0].as_str().unwrap().to_string();

    // the answer with that member removed
    let mut missing = ctx
        .execute(&id, serde_json::json!({}))
        .or_else(|_| {
            let (_, input) = cases_of(&ctx, &id).into_iter().next().expect("a case");
            ctx.execute(&id, input)
        })
        .expect("the capability answers");
    missing
        .as_object_mut()
        .expect("an object answer")
        .remove(&required);
    assert!(
        validator.iter_errors(&missing).next().is_some(),
        "{id}: dropping the required member `{required}` was not reported"
    );

    // and an answer that is not even the right kind of value
    assert!(
        validator
            .iter_errors(&Value::String("not an object".into()))
            .next()
            .is_some(),
        "{id}: a string where an object is declared was not reported"
    );
}

//! `MAJORDOMUS_CAPABILITY_CACHE=off` runs every capability without the executor's cache:
//! the switch `project.cache-is-invisible` asks for, so that a doubt about a kept answer
//! is settled by asking without it.
//!
//! One test, in a binary of its own, because it writes this process's environment: a test
//! beside it would read its cache switched off.

mod common;

use common::Fixture;
use majordomus_cli::capability::executor::{bypassed, SWITCH};
use serde_json::json;

#[test]
fn the_switch_runs_every_handler_and_keeps_nothing_and_the_answers_do_not_change() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let cached = ["capabilities.describe", "plan.status", "evidence.report"];
    let input = |id: &str| match id {
        "capabilities.describe" => json!({ "id": "plan.status" }),
        _ => json!({}),
    };

    // with the cache: the second call is answered from it
    let mut kept = Vec::new();
    for id in cached {
        let before = ctx.executor.handler_invocations();
        kept.push(ctx.execute(id, input(id)).expect("answers"));
        ctx.execute(id, input(id)).expect("answers");
        assert_eq!(
            ctx.executor.handler_invocations() - before,
            1,
            "{id} keeps its answer while the switch is not set"
        );
    }

    std::env::set_var(SWITCH, "off");
    assert!(bypassed());
    ctx.executor.clear();
    for (id, kept) in cached.into_iter().zip(&kept) {
        let before = ctx.executor.handler_invocations();
        let one = ctx.execute(id, input(id)).expect("answers");
        let two = ctx.execute(id, input(id)).expect("answers");
        assert_eq!(
            ctx.executor.handler_invocations() - before,
            2,
            "{id}: every call ran its handler"
        );
        assert_eq!(
            &one, kept,
            "{id}: the answer without the cache is the one it kept"
        );
        assert_eq!(one, two);
    }
    // a reading, with the switch set, is the handler's answer taken just now by the caller
    let before = ctx.executor.handler_invocations();
    let reading = ctx.read("plan.status", json!({}));
    assert_eq!(ctx.executor.handler_invocations() - before, 1);
    assert_eq!(
        reading.freshness,
        majordomus_cli::capability::readings::Freshness::Current
    );
    assert_eq!(reading.value.ok(), Some(kept[1].clone()));
    assert_eq!(ctx.executor.cached_entries(), 0, "nothing was kept");

    // any other value is not the switch
    std::env::set_var(SWITCH, "on");
    assert!(!bypassed());
    std::env::remove_var(SWITCH);
    assert!(!bypassed());
}

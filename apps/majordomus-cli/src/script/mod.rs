//! The engine a scripted capability runs in (ADR 0069).
//!
//! A capability may be scripted: declared in a file of the layer, with a Rhai script as its
//! handler. This module is the one place a Rhai [`Engine`] is built, so every script runs
//! under the same sandbox and nothing else in the crate configures an interpreter of its own.
//!
//! What the sandbox guarantees, each proved by a test below:
//!
//! - **no eval.** Rhai's own `eval` is disabled, so a script is compiled from its tracked file
//!   and never from a string assembled at run time (`project.no-network-no-eval`, and the
//!   exception ADR 0069 declares in its terms).
//! - **no hidden clock.** The crate is built with `no_time`: time reaches a script through its
//!   context, so the same script over the same inputs produces the same output.
//! - **bounded work.** Operations, call depth, expression depth and the sizes of strings, arrays
//!   and maps are limited, so a runaway script ends with a named error instead of a hang.
//! - **cancellation.** Every operation checks the execution's cancel flag, so a script stops
//!   when its execution is asked to stop, without a thread being killed.
//! - **deterministic maps.** A Rhai object map iterates in key order, whatever order its keys
//!   were inserted in.
//!
//! The engine registers no filesystem, process or network function. A script reaches the
//! repository only through capabilities, which a later change binds; `import` resolves no
//! module, and `print` and `debug` write nowhere until the caller routes them.

// The engine is wired to a capability by the next change of this milestone; until then only
// the tests below build one.
#![allow(dead_code)]

use rhai::{Dynamic, Engine};

/// How much one script execution may do before it is stopped.
///
/// The defaults are deliberately generous for repository automation — a script orchestrates
/// capabilities, it does not compute — and small enough that an accidental infinite loop ends
/// in well under a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    /// Rhai operations (roughly, evaluated expressions and statements).
    pub(crate) operations: u64,
    /// Nested function calls.
    pub(crate) call_levels: usize,
    /// Expression nesting at the top level of a script.
    pub(crate) expression_depth: usize,
    /// Expression nesting inside a function body.
    pub(crate) function_expression_depth: usize,
    /// Characters in one string.
    pub(crate) string_size: usize,
    /// Elements in one array.
    pub(crate) array_size: usize,
    /// Entries in one object map.
    pub(crate) map_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            operations: 1_000_000,
            call_levels: 64,
            expression_depth: 64,
            function_expression_depth: 32,
            string_size: 1 << 20,
            array_size: 100_000,
            map_size: 100_000,
        }
    }
}

/// The value a cancelled script's evaluation returns as its termination token.
const CANCELLED: &str = "cancelled";

/// Build the sandboxed engine every scripted capability runs in.
///
/// `cancelled` is asked on every operation: once it answers true, the script stops at that
/// operation with [`rhai::EvalAltResult::ErrorTerminated`]. An execution passes its progress
/// handle's flag; a compilation, which runs nothing, passes `|| false`.
///
/// `print` and `debug` write nowhere until the caller routes them: the shared server speaks
/// MCP on its standard output, and a script's line there would corrupt the protocol.
pub(crate) fn engine(
    limits: &Limits,
    cancelled: impl Fn() -> bool + Send + Sync + 'static,
) -> Engine {
    let mut engine = Engine::new();
    engine
        .set_max_operations(limits.operations)
        .set_max_call_levels(limits.call_levels)
        .set_max_expr_depths(limits.expression_depth, limits.function_expression_depth)
        .set_max_string_size(limits.string_size)
        .set_max_array_size(limits.array_size)
        .set_max_map_size(limits.map_size);
    // Compiled from a tracked file, never from a string: `eval` does not exist here.
    engine.disable_symbol("eval");
    // `import` would read a file from the filesystem at large; modules resolve nowhere.
    engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    engine.on_print(|_| {});
    engine.on_debug(|_, _, _| {});
    engine.on_progress(move |_| cancelled().then(|| Dynamic::from(CANCELLED)));
    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::EvalAltResult;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    fn sandbox() -> Engine {
        engine(&Limits::default(), || false)
    }

    #[test]
    fn a_map_iterates_in_key_order_whatever_order_it_was_built_in() {
        let e = sandbox();
        let keys: Vec<String> = e
            .eval::<rhai::Map>(r#"let m = #{}; m.zulu = 1; m.alpha = 2; m.mike = 3; m"#)
            .expect("a map")
            .keys()
            .map(|k| k.to_string())
            .collect();
        assert_eq!(keys, ["alpha", "mike", "zulu"]);
        // and a script sees the same order
        let listed = e
            .eval::<String>(
                r#"let m = #{ zulu: 1, alpha: 2 }; let s = ""; for k in m.keys() { s += k; } s"#,
            )
            .expect("keys in a script");
        assert_eq!(listed, "alphazulu");
    }

    #[test]
    fn eval_is_not_a_function_a_script_can_call() {
        let err = sandbox()
            .compile(r#"eval("40 + 2")"#)
            .expect_err("eval must not compile");
        assert!(err.to_string().contains("eval"), "{err}");
    }

    #[test]
    fn there_is_no_hidden_clock() {
        let err = sandbox()
            .eval::<Dynamic>("timestamp()")
            .expect_err("no_time removes the clock");
        assert!(
            matches!(*err, EvalAltResult::ErrorFunctionNotFound(..)),
            "{err}"
        );
    }

    #[test]
    fn a_runaway_script_is_stopped_by_its_operation_limit() {
        let limits = Limits {
            operations: 10_000,
            ..Limits::default()
        };
        let err = engine(&limits, || false)
            .eval::<Dynamic>("let x = 0; loop { x += 1; }")
            .expect_err("an endless loop must end");
        assert!(
            matches!(*err, EvalAltResult::ErrorTooManyOperations(..)),
            "{err}"
        );
    }

    #[test]
    fn a_cancelled_execution_stops_at_its_next_operation() {
        // the flag is raised part way through, as a cancel request arrives while it runs
        let cancel = Arc::new(AtomicBool::new(false));
        let raised = Arc::clone(&cancel);
        let mut e = engine(&Limits::default(), move || cancel.load(Ordering::SeqCst));
        e.register_fn("stop", move || raised.store(true, Ordering::SeqCst));
        let err = e
            .eval::<Dynamic>("let x = 0; loop { x += 1; if x == 100 { stop(); } }")
            .expect_err("a cancelled script must stop");
        match *err {
            EvalAltResult::ErrorTerminated(token, _) => {
                assert_eq!(token.into_string().unwrap_or_default(), CANCELLED)
            }
            other => panic!("expected termination, got {other}"),
        }
    }

    #[test]
    fn a_string_cannot_grow_past_its_limit() {
        let limits = Limits {
            string_size: 16,
            ..Limits::default()
        };
        let err = engine(&limits, || false)
            .eval::<Dynamic>(r#"let s = ""; for i in 0..100 { s += "x"; } s"#)
            .expect_err("the string limit must hold");
        assert!(
            matches!(*err, EvalAltResult::ErrorDataTooLarge(..)),
            "{err}"
        );
    }

    #[test]
    fn import_resolves_no_file() {
        let err = sandbox()
            .eval::<Dynamic>(r#"import "Cargo" as c; 1"#)
            .expect_err("no module resolves");
        assert!(
            matches!(*err, EvalAltResult::ErrorModuleNotFound(..)),
            "{err}"
        );
    }

    #[test]
    fn print_and_debug_write_nowhere_until_the_caller_routes_them() {
        // unrouted: the call succeeds and nothing reaches this process's standard output
        let quiet = sandbox()
            .eval::<i64>(r#"print("hidden"); debug("hidden"); 1"#)
            .expect("print is a no-op");
        assert_eq!(quiet, 1);
        // routed: the caller's sink receives the line
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&lines);
        let mut e = sandbox();
        e.on_print(move |l| sink.lock().unwrap().push(l.to_string()));
        let routed = e.eval::<i64>(r#"print("seen"); 1"#).expect("print runs");
        assert_eq!(routed, 1);
        assert_eq!(*lines.lock().unwrap(), ["seen"]);
    }

    #[test]
    fn the_engine_is_send_and_sync_for_the_shared_server() {
        fn needs<T: Send + Sync>() {}
        needs::<Engine>();
    }
}

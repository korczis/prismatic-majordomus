//! Benchmark identity is the capability's identity: every executable capability is a
//! benchmark target for each transport it is exposed on, and the only thing a declaration
//! may have to add is a representative input. That input comes from the input type
//! itself, through [`BenchmarkCases`], so that one typed case feeds the direct, MCP and
//! HTTP runners through the real serialisers, and a capability whose input type has no
//! cases does not compile.

use serde::Serialize;
use serde_json::Value;

use crate::index::Index;

/// One representative input, named so that results and baselines can refer to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedCase<T> {
    /// A short stable name: `default`, `kind-rule`, `first-object`.
    pub name: &'static str,
    /// The input.
    pub input: T,
}

impl<T> NamedCase<T> {
    /// A named case.
    pub fn new(name: &'static str, input: T) -> Self {
        NamedCase { name, input }
    }
}

/// What a case provider may look at: the repository's index, so that a case can name an
/// object that exists (`objects.get` needs a real URI) and skip itself when the
/// repository holds nothing it could ask for.
pub struct CaseContext<'a> {
    /// The index of the repository the benchmark runs against.
    pub index: &'a Index,
}

/// Representative inputs of a capability's input type. Implemented once per input type;
/// the `capability!` macro requires it, so a new capability without a benchmark case is a
/// compile error, not a coverage report.
///
/// ```
/// use majordomus_cli::capability::{BenchmarkCases, NamedCase, CaseContext};
/// #[derive(serde::Serialize)]
/// struct Input { limit: u64 }
/// impl BenchmarkCases for Input {
///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
///         vec![NamedCase::new("default", Input { limit: 10 })]
///     }
/// }
/// ```
pub trait BenchmarkCases: Sized + Serialize {
    /// The cases, in a stable order. An empty list means "nothing to benchmark in this
    /// repository", which the coverage check reports as missing.
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>>;

    /// The cases as JSON, the shape every runner serialises from.
    fn benchmark_cases_json(ctx: &CaseContext<'_>) -> Vec<NamedCase<Value>> {
        Self::benchmark_cases(ctx)
            .into_iter()
            .map(|c| NamedCase {
                name: c.name,
                input: serde_json::to_value(&c.input).unwrap_or(Value::Null),
            })
            .collect()
    }
}

/// The type-erased case provider an executable carries.
pub type CaseProvider = fn(&CaseContext<'_>) -> Vec<NamedCase<Value>>;

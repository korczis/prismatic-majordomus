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
///
/// ```
/// use majordomus_cli::capability::NamedCase;
/// // the name travels with the input, all the way into the row a baseline is compared
/// // against; two cases of one capability are told apart by it and by nothing else
/// let case = NamedCase::new("first-object", "majordomus://repository");
/// assert_eq!(case.name, "first-object");
/// assert_eq!(case.input, "majordomus://repository");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedCase<T> {
    /// A short stable name: `default`, `kind-rule`, `first-object`.
    pub name: &'static str,
    /// The input.
    pub input: T,
}

impl<T> NamedCase<T> {
    /// A case under a short, stable name.
    ///
    /// The name is `&'static str` rather than a `String` on purpose: it is written in the
    /// source beside the input it names and is never computed from the repository, because
    /// a baseline whose row names moved with the data could not be compared against the
    /// previous run.
    ///
    /// ```
    /// use majordomus_cli::capability::NamedCase;
    /// let case = NamedCase::new("default", 10u64);
    /// assert_eq!((case.name, case.input), ("default", 10));
    /// ```
    pub fn new(name: &'static str, input: T) -> Self {
        NamedCase { name, input }
    }
}

/// What a case provider may look at: the repository's index, so that a case can name an
/// object that exists (`objects.get` needs a real URI) and skip itself when the
/// repository holds nothing it could ask for.
///
/// Deliberately only the index. A case provider may look at what the repository holds and
/// may decline to produce a case; it cannot reach the network, the environment or the
/// clock, so the inputs of a benchmark run are a function of the repository state the run
/// reports having measured.
///
/// ```
/// use majordomus_cli::capability::CaseContext;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
/// let ctx = CaseContext { index: &index };
/// // this is what lets a case name an object that exists rather than a URI written into
/// // the declaration and true of one machine
/// let first = ctx.index.objects.first().expect("the synthetic layer holds objects");
/// assert!(first.uri.starts_with("majordomus://"));
/// ```
pub struct CaseContext<'a> {
    /// The index of the repository the benchmark runs against.
    pub index: &'a Index,
}

/// Representative inputs of a capability's input type. Implemented once per input type;
/// the `capability!` macro requires it, so a new capability without a benchmark case is a
/// compile error, not a coverage report.
///
/// ```
/// use majordomus_cli::capability::{BenchmarkCases, CaseContext, NamedCase};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// #[derive(serde::Serialize)]
/// struct Input { limit: u64 }
/// impl BenchmarkCases for Input {
///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
///         vec![NamedCase::new("default", Input { limit: 10 })]
///     }
/// }
///
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
/// let ctx = CaseContext { index: &index };
/// // one declaration, and the two readings every runner takes of it: the typed case and
/// // the JSON the transports serialise
/// assert_eq!(Input::benchmark_cases(&ctx)[0].name, "default");
/// assert_eq!(Input::benchmark_cases_json(&ctx)[0].input["limit"], 10);
/// ```
pub trait BenchmarkCases: Sized + Serialize {
    /// The cases, in a stable order. An empty list means "nothing to benchmark in this
    /// repository", which the coverage check reports as missing.
    ///
    /// Called with the repository being measured, so a case may name something that
    /// exists and a capability with nothing to ask about here may honestly return nothing.
    /// The order is the implementation's and must not depend on iteration order of a map
    /// or on the filesystem, because the sequence is what a baseline's rows line up with.
    ///
    /// ```
    /// use majordomus_cli::capability::{BenchmarkCases, CaseContext, NamedCase};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// #[derive(serde::Serialize)]
    /// struct Uri(String);
    /// impl BenchmarkCases for Uri {
    ///     fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
    ///         // name something the repository holds, or decline to have a case at all
    ///         ctx.index
    ///             .objects
    ///             .first()
    ///             .map(|o| vec![NamedCase::new("first-object", Uri(o.uri.clone()))])
    ///             .unwrap_or_default()
    ///     }
    /// }
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let index = repo.index().unwrap();
    /// let cases = Uri::benchmark_cases(&CaseContext { index: &index });
    /// assert_eq!(cases.len(), 1);
    /// assert_eq!(cases[0].input.0, index.objects[0].uri);
    /// ```
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>>;

    /// The cases as JSON, the shape every runner serialises from.
    ///
    /// Through the input type's own `Serialize`, so the direct, MCP and HTTP runners time
    /// the bytes a real caller would send rather than three hand-written approximations of
    /// them. A case whose serialisation fails becomes `null` instead of stopping the run:
    /// a benchmark that cannot build one input still has the others to report, and the
    /// null shows up as a failing sample rather than as a missing measurement.
    ///
    /// ```
    /// use majordomus_cli::capability::{BenchmarkCases, CaseContext, NamedCase};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// #[derive(serde::Serialize)]
    /// struct Input { limit: u64 }
    /// impl BenchmarkCases for Input {
    ///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
    ///         vec![NamedCase::new("default", Input { limit: 10 })]
    ///     }
    /// }
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let index = repo.index().unwrap();
    /// let json = Input::benchmark_cases_json(&CaseContext { index: &index });
    /// assert_eq!(json[0].name, "default", "the name survives the boundary");
    /// assert_eq!(json[0].input, serde_json::json!({ "limit": 10 }));
    /// ```
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

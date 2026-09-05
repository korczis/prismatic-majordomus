//! Modules compose capabilities; the application composes modules. A module is one Rust
//! module with a `module()` function built by [`crate::module!`], owning its id, its metadata
//! and the executables it ships; [`crate::compose_modules!`] names the modules the application
//! is made of, and that list of modules is the only composition a person writes at the
//! root. Adding a capability to an existing module touches that module alone.
//!
//! The macros build plain values. Nothing is registered behind the caller's back: the
//! result is handed to the registry builder explicitly in `app.rs`.

use super::handler::Executable;
use super::model::{ModuleId, Stability};

/// One module: its identity, what it is for, and the executables it composes. The module
/// stamps its id on every capability it carries, and the registry refuses a capability
/// whose id namespace is not its module.
pub struct ModuleDescriptor {
    /// The identity, `[a-z][a-z0-9_-]*`; the namespace of every capability in it.
    pub id: ModuleId,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    /// Where the module stands, as a whole.
    pub stability: Stability,
    /// The executables, each already stamped with the module id.
    pub capabilities: Vec<Executable>,
}

impl ModuleDescriptor {
    /// A module over its executables; stamps the module id on each of them.
    pub fn new(
        id: &str,
        title: &str,
        description: &str,
        stability: Stability,
        mut capabilities: Vec<Executable>,
    ) -> Self {
        let id = ModuleId::unchecked(id);
        for e in &mut capabilities {
            e.capability.module = id.clone();
        }
        ModuleDescriptor {
            id,
            title: title.into(),
            description: description.into(),
            stability,
            capabilities,
        }
    }
}

impl std::fmt::Debug for ModuleDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleDescriptor")
            .field("id", &self.id)
            .field("capabilities", &self.capabilities.len())
            .finish()
    }
}

/// Build a [`ModuleDescriptor`]: the module's identity and metadata once, and the
/// executables it composes, each a `capability!` value.
///
/// ```
/// use majordomus_cli::{capability, module};
/// use majordomus_cli::capability::{BenchmarkCases, CaseContext, Context, CapabilityError, Exposure, NamedCase, Stability};
/// #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
/// struct In {}
/// impl BenchmarkCases for In {
///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> { vec![NamedCase::new("default", In {})] }
/// }
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Out { ok: bool }
/// fn ping(_: &Context, _: In) -> Result<Out, CapabilityError> { Ok(Out { ok: true }) }
/// let m = module! {
///     id: "demo", title: "Demo", description: "A module of one.", stability: Stability::Experimental,
///     capabilities: [
///         capability! { id: "demo.ping", title: "Ping", description: "Answers.", input: In, output: Out,
///                       stability: Stability::Experimental, exposure: Exposure::default(), tags: [], handler: ping },
///     ],
/// };
/// assert_eq!(m.capabilities[0].capability.module.as_str(), "demo");
/// ```
#[macro_export]
macro_rules! module {
    (
        id: $id:expr,
        title: $title:expr,
        description: $description:expr,
        stability: $stability:expr,
        capabilities: [ $($cap:expr),* $(,)? ] $(,)?
    ) => {
        $crate::capability::ModuleDescriptor::new($id, $title, $description, $stability, vec![$($cap),*])
    };
}

/// Compose the application from its modules: each name is a Rust module with a
/// `module()` function. This list is the one composition a person maintains at the root;
/// a capability added inside a module needs no edit here.
///
/// ```
/// use majordomus_cli::compose_modules;
/// use majordomus_cli::capability::builtin;
/// let modules = compose_modules![builtin::repository, builtin::objects];
/// assert_eq!(modules.len(), 2);
/// assert_eq!(modules[0].id.as_str(), "repository");
/// ```
#[macro_export]
macro_rules! compose_modules {
    ( $( $($segment:ident)::+ ),* $(,)? ) => {
        vec![$( $($segment)::+::module() ),*]
    };
}

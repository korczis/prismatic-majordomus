//! The control plane: one graph of everything this repository can be asked to do, and the
//! projections that put it on every surface.
//!
//! The repository already refuses a second definition of a *capability*: it is declared
//! once and projected into MCP, HTTP, OpenAPI, the Cockpit and the generated
//! documentation. This module extends the same refusal to *commands* — the things a person
//! runs — which until now were declared once in clap and then spelled out again by hand in
//! the justfile, and not offered by any completion at all.
//!
//! What is canonical, and where it lives:
//!
//! | fact | canonical source |
//! |---|---|
//! | command path, arguments, defaults, accepted values | the clap declaration, `cli.rs` |
//! | one-line and long description | the clap declaration |
//! | examples | `cli::EXAMPLES`, beside the declaration |
//! | what running it changes | `Semantics`, declared with those examples |
//! | identity, HTTP and MCP exposure of a capability | the capability registry |
//! | external workflows | discovered from `just`'s own structured dump |
//! | `just` recipe name, MCP tool name, Cockpit action, docs route | computed here |
//!
//! Nothing in this module is a list of commands, and nothing downstream of it keeps one.
//!
//! The distinction from [`crate::commands`] is worth stating once: that module *implements*
//! the executable's own commands; this one *describes* every command there is.

pub mod completion;
pub mod effect;
pub mod graph;
pub mod just;
pub mod projection;
pub mod workflow;

pub use effect::{EffectClass, Interactivity, Semantics};
pub use graph::{CommandGraph, CommandNode};
pub use projection::Surface;

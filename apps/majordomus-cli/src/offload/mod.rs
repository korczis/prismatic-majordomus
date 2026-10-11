//! The derive offload: a branch's `scripts/derive` run on another machine the repository
//! declares, in that machine's turn, and the derived tree brought back staged.
//!
//! A derive costs from ten minutes on an idle machine to an hour on the shared one, and
//! every session needs one before it can commit. Until this module the offload was a script
//! in a home directory with its hosts typed into it and a lock that was a race. Now:
//!
//! - [`declaration`]: the hosts and their limits, read from the layer (kind
//!   `derive-offload`, `.ai/repo/offload/*.yaml`) and nowhere else;
//! - [`host`]: which host a derive goes to — free space, then the fewest derives running or
//!   waiting, then declaration order;
//! - [`queue`]: the FIFO queue on the host, decided here over what the host reports;
//! - [`remote`]: how a script reaches a host, over ssh or directly;
//! - [`run`]: the round trip — bundle in, derive, bundle back, stage.
//!
//! `docs/OFFLOAD.md` is the reference; `majordomus offload` is the command.

pub mod declaration;
pub mod host;
pub mod queue;
pub mod remote;
pub mod run;

use declaration::{OffloadDeclaration, KIND};

/// The repository's declaration, read from its index: `Ok(None)` when it declares none,
/// an error naming the object when one is declared and invalid, or when two are.
pub fn from_objects(
    objects: &[crate::model::Object],
) -> Result<Option<OffloadDeclaration>, String> {
    let mut found: Vec<&crate::model::Object> = objects.iter().filter(|o| o.kind == KIND).collect();
    match found.len() {
        0 => Ok(None),
        1 => OffloadDeclaration::parse(found.remove(0)).map(Some),
        _ => Err(format!(
            "{} derive-offload declarations ({}); a repository declares one",
            found.len(),
            found
                .iter()
                .map(|o| o.uri.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

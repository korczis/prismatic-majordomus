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
//! - [`run`]: the round trip — bundle in, derive, bundle back, stage;
//! - [`owner`]: who a run belongs to, exported into it and recorded on its host (I3956);
//! - [`stop`]: a run stopped as a process group, on its host, by its owner (I3956).
//!
//! `docs/OFFLOAD.md` is the reference; `majordomus offload` is the command.

pub mod declaration;
pub mod host;
pub mod owner;
pub mod queue;
pub mod remote;
pub mod run;
pub mod stop;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Object, Provenance};

    fn object(kind: &str, identity: &str, metadata: serde_json::Value) -> Object {
        Object {
            kind: kind.into(),
            identity: identity.into(),
            uri: format!("majordomus://{kind}/{identity}"),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: Provenance {
                path: format!(".ai/repo/offload/{identity}.yaml"),
                directory: ".ai/repo/offload".into(),
                source_class: "offload".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    fn declared(id: &str) -> Object {
        object(
            KIND,
            id,
            serde_json::json!({
                "schema": declaration::SCHEMA_VERSION, "kind": KIND, "id": id,
                "hosts": [{ "id": "lundra", "address": "u@lundra" }]
            }),
        )
    }

    #[test]
    fn the_one_declaration_of_the_index_is_found_among_other_objects() {
        let rule = object("rule", "project.alpha", serde_json::json!({}));
        assert_eq!(from_objects(std::slice::from_ref(&rule)).unwrap(), None);

        let d = from_objects(&[rule.clone(), declared("derive")])
            .unwrap()
            .expect("one declaration is declared");
        assert_eq!(
            (d.id.as_str(), d.hosts[0].id.as_str()),
            ("derive", "lundra")
        );

        let invalid = object(KIND, "broken", serde_json::json!({ "kind": KIND }));
        assert!(from_objects(&[invalid])
            .unwrap_err()
            .starts_with("majordomus://derive-offload/broken: "));
    }

    #[test]
    fn two_declarations_are_refused_naming_both() {
        assert_eq!(
            from_objects(&[declared("one"), declared("two")]).unwrap_err(),
            "2 derive-offload declarations (majordomus://derive-offload/one, \
             majordomus://derive-offload/two); a repository declares one"
        );
    }
}

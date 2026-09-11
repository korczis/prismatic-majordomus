//! Schema versions of the files the knowledge system reads and writes, and the
//! migrations between them. A file of a newer schema than this executable knows is
//! refused with the version that would read it; a file of an older one is migrated in
//! memory, step by step, and the steps are reported so that a person can see what
//! changed before writing it back.

use serde_json::Value;

/// The families of documents with a schema line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// The knowledge model itself.
    Model,
    /// The committed baseline.
    Baseline,
    /// The canonicality exceptions.
    Exceptions,
    /// The semantic cache.
    SemanticCache,
}

impl Family {
    /// The schema prefix, without the version.
    pub fn prefix(self) -> &'static str {
        match self {
            Family::Model => "majordomus/knowledge",
            Family::Baseline => "majordomus/knowledge-baseline",
            Family::Exceptions => "majordomus/knowledge-exceptions",
            Family::SemanticCache => "majordomus/semantic-cache",
        }
    }

    /// The version this executable reads and writes.
    pub fn current(self) -> u32 {
        1
    }

    /// The schema line this executable writes.
    pub fn schema(self) -> String {
        format!("{}/v{}", self.prefix(), self.current())
    }
}

/// One migration step.
pub struct Migration {
    /// The family it applies to.
    pub family: Family,
    /// The version it reads.
    pub from: u32,
    /// The version it writes.
    pub to: u32,
    /// What it does, one line.
    pub description: &'static str,
    /// The step.
    pub apply: fn(Value) -> Result<Value, String>,
}

/// Every migration this executable knows, in the order they chain.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        family: Family::Baseline,
        from: 0,
        to: 1,
        description: "a baseline written before the schema line existed: add it, and read `tolerated:` as `debt:`",
        apply: baseline_v0_to_v1,
    },
    Migration {
        family: Family::Exceptions,
        from: 0,
        to: 1,
        description: "an exceptions file without a schema line: add it",
        apply: add_schema_line(Family::Exceptions),
    },
    Migration {
        family: Family::SemanticCache,
        from: 0,
        to: 1,
        description: "a cache without a schema line: add it",
        apply: add_schema_line(Family::SemanticCache),
    },
];

const fn add_schema_line(family: Family) -> fn(Value) -> Result<Value, String> {
    match family {
        Family::Exceptions => |v| set_schema(v, Family::Exceptions),
        Family::SemanticCache => |v| set_schema(v, Family::SemanticCache),
        Family::Baseline => |v| set_schema(v, Family::Baseline),
        Family::Model => |v| set_schema(v, Family::Model),
    }
}

fn set_schema(mut v: Value, family: Family) -> Result<Value, String> {
    let Some(m) = v.as_object_mut() else {
        return Err("the document is not a mapping".into());
    };
    m.insert("schema".into(), Value::String(family.schema()));
    Ok(v)
}

fn baseline_v0_to_v1(mut v: Value) -> Result<Value, String> {
    let Some(m) = v.as_object_mut() else {
        return Err("the document is not a mapping".into());
    };
    if let Some(t) = m.remove("tolerated") {
        m.entry("debt").or_insert(t);
    }
    m.insert("schema".into(), Value::String(Family::Baseline.schema()));
    Ok(v)
}

/// The version a schema line names for a family; `0` for a document with no schema line;
/// `None` for a line of another family or one that does not parse.
pub fn version_of(family: Family, value: &Value) -> Option<u32> {
    match value.get("schema") {
        None => Some(0),
        Some(Value::String(s)) => {
            let rest = s.strip_prefix(family.prefix())?.strip_prefix("/v")?;
            rest.parse().ok()
        }
        Some(_) => None,
    }
}

/// Migrate a document to the current version of its family. Answers the document and the
/// descriptions of the steps taken; an empty list means it was current already.
pub fn migrate(family: Family, value: Value) -> Result<(Value, Vec<String>), String> {
    let Some(mut version) = version_of(family, &value) else {
        return Err(format!(
            "the schema line is not one of `{}/v<n>`: {}",
            family.prefix(),
            value.get("schema").map(|v| v.to_string()).unwrap_or_default()
        ));
    };
    let current = family.current();
    if version > current {
        return Err(format!(
            "the document carries schema version {version} and this executable reads {}; a newer Majordomus wrote it",
            family.schema()
        ));
    }
    let mut value = value;
    let mut steps = Vec::new();
    while version < current {
        let Some(m) = MIGRATIONS
            .iter()
            .find(|m| m.family == family && m.from == version)
        else {
            return Err(format!(
                "no migration from {}/v{version} to v{}",
                family.prefix(),
                version + 1
            ));
        };
        value = (m.apply)(value)?;
        steps.push(format!("v{} -> v{}: {}", m.from, m.to, m.description));
        version = m.to;
    }
    Ok((value, steps))
}

/// The migration table, for a listing.
pub fn table() -> Vec<serde_json::Value> {
    MIGRATIONS
        .iter()
        .map(|m| {
            serde_json::json!({
                "family": m.family.prefix(),
                "from": m.from,
                "to": m.to,
                "description": m.description,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_baseline_is_migrated_step_by_step_and_a_newer_one_refused() {
        let v = serde_json::json!({"tolerated": [{"node": "x", "freshness": "stale"}]});
        let (out, steps) = migrate(Family::Baseline, v).unwrap();
        assert_eq!(out["schema"], "majordomus/knowledge-baseline/v1");
        assert!(out["debt"].is_array());
        assert!(out.get("tolerated").is_none());
        assert_eq!(steps.len(), 1);
        let (_, steps) = migrate(
            Family::Baseline,
            serde_json::json!({"schema": "majordomus/knowledge-baseline/v1"}),
        )
        .unwrap();
        assert!(steps.is_empty());
        let err = migrate(
            Family::Baseline,
            serde_json::json!({"schema": "majordomus/knowledge-baseline/v2"}),
        )
        .unwrap_err();
        assert!(err.contains("newer"), "{err}");
        let err = migrate(Family::Baseline, serde_json::json!({"schema": "other/v1"})).unwrap_err();
        assert!(err.contains("not one of"), "{err}");
    }

    #[test]
    fn every_family_chains_from_zero_to_current() {
        for family in [Family::Baseline, Family::Exceptions, Family::SemanticCache] {
            let (out, _) = migrate(family, serde_json::json!({})).unwrap();
            assert_eq!(out["schema"], family.schema());
        }
    }
}

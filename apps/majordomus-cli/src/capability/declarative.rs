//! The declarative source: every object of the index is a resource capability. Its id is
//! `<kind>.<identity>`, its MCP resource is the object's URI, it has no HTTP or CLI
//! exposure of its own (it is read through `objects.get`), and its provenance is the
//! object's. Nothing here reads a file: the index already did, and reported what it
//! could not read.

use super::model::{
    BenchmarkPolicy, CachePolicy, Capability, CapabilityId, CapabilityKind, Exposure, McpExposure,
    McpResource, ModuleId, Provenance, Stability, WaiverReason,
};
use super::schema::CanonicalSchema;
use crate::capability::builtin::ObjectView;
use crate::model::Object;

/// The resource capability of one object of the index.
pub fn capability_of(object: &Object) -> Capability {
    Capability {
        id: CapabilityId::unchecked(&format!("{}.{}", object.kind, object.identity)),
        module: ModuleId::unchecked(&object.kind),
        kind: CapabilityKind::Resource,
        title: object
            .title
            .clone()
            .unwrap_or_else(|| object.identity.clone()),
        description: object.description.clone().unwrap_or_default(),
        input: CanonicalSchema::empty(),
        output: CanonicalSchema::of::<ObjectView>(),
        provenance: Provenance::Declarative {
            path: object.provenance.path.clone(),
            directory: object.provenance.directory.clone(),
            source_class: object.provenance.source_class.clone(),
            section: object.provenance.section.clone(),
            media_type: object.media_type.to_string(),
            member: object.provenance.member.clone(),
        },
        exposure: Exposure {
            mcp: Some(McpExposure {
                tool: None,
                resource: Some(McpResource {
                    uri: object.uri.clone(),
                    name: object.identity.clone(),
                }),
            }),
            http: None,
            cli: None,
        },
        stability: Stability::Implemented,
        tags: object.tags().iter().map(|t| t.to_string()).collect(),
        benchmark: BenchmarkPolicy::Waived {
            reason: WaiverReason::NotExecutable,
        },
        cache: CachePolicy::Disabled,
    }
}

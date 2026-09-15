# Prompt 07: Documentation, GH Pages, Landing and Deployment

Use MAXIMUM AVAILABLE CONTEXT and current deployment/versioning state.

Make documentation and public surfaces match the now-implemented provider/session/handover architecture exactly.

Update canonical docs for:

- provider architecture
- provider kind vs instance vs model vs transport
- configuration and precedence
- explicit switching and scopes
- routing and explainability
- retry vs fallback
- credentials/security/redaction
- session lifecycle
- automatic session resolution/creation
- handover lifecycle
- provider-independent continuity
- peer handover
- restart/recovery
- context discovery/compiler/freshness
- governance integration
- extension guide for adding providers
- troubleshooting
- validation/doctor commands
- test/evidence linkage

Update generated GH Pages architecture/features/reference sections. Prefer deriving inventories/schema/evidence from canonical data.

Update landing/features only with claims that are now implemented and verified. A valid core claim is conceptually:

"Switch providers without losing project context. Majordomus keeps canonical session state, handovers, governance and task provenance independent of the model runtime."

Do not publish it unless E2E tests prove it.

Check all existing public provider/session/handover claims for drift and correct them.

Follow canonical versioning/changelog mechanism. Do not hand-invent versions.

Follow repository deployment procedure:

- run docs/generation drift checks
- commit/push/land according to project workflow
- deploy GH Pages and relevant runtime/site surfaces
- verify the deployed URL/content, version and links using project tooling

A green CI job is not proof that humans can see the result. Verify the deployed artifact.

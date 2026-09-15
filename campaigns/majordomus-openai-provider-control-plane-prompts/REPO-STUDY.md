# Repository study notes

These notes are context for the implementation model. Re-verify all details against the live checkout before acting.

## Confirmed repository structure

The repository has a provider-neutral `.ai/` layer with `.ai/manifest.yaml` as its section registry. `.ai/README.md` explicitly says discovery begins from the manifest and that discoverability is not eager loading. `.ai/local/` is checkout-local operational state and must not become policy or a public projection.

`AGENTS.md` is generated from repository policy and instructs workers to use:
- `majordomus context`;
- effective `.ai/repo/rules`;
- task lifecycle;
- canonical worktree topology;
- canonical `capability!` declarations;
- `majordomus generate`;
- executable use cases.

Do not edit `AGENTS.md` manually.

## Existing architectural machinery

Observed:
- `apps/majordomus-cli/src/capability/`
- `apps/majordomus-cli/src/capability/builtin/`
- `apps/majordomus-cli/src/http/`
- `apps/majordomus-cli/src/mcp/`
- `apps/majordomus-cli/src/cockpit/`
- generated OpenAPI / capability docs
- shared MCP + HTTP server
- server-rendered Cockpit
- graph subsystem
- use-case coverage machinery
- benchmark/generation gates

ADR 0002 establishes a canonical typed capability registry with projections to MCP, HTTP, OpenAPI, Swagger UI, CLI and generated reference material.

ADR 0012 establishes the Cockpit as a projection. It must invoke the same capabilities/application semantics and must not invent an independent domain model or manually maintained navigation/catalog.

ADR 0020 further treats the capability/object graph as composed from existing registries instead of introducing a central duplicated manifest.

## Existing rules that matter heavily

The live project rules directory includes at least:
- `project.interfaces-are-projections`
- `project.derived-once`
- `project.derived-files-regenerated`
- `project.generated-artifacts-are-typed`
- `project.hot-path-reads-once`
- `project.never-store-transcripts`
- `project.no-claim-without-test`
- `project.native-cli-documented`
- `project.benchmarkable-commands`
- `project.blocking-checks-cheap`
- `project.cache-is-invisible`
- `project.context-locality`
- `project.clean-room`
- `project.distribution-canonical`

Read the exact live rules and dependencies before implementing.

## MCP

The repository already runs one shared server per repository and supports stdio clients plus Streamable HTTP `/mcp`. Client bootstrap already covers Claude Code, Gemini CLI and Codex. Peers and announcements already exist.

Do not create a second provider coordination server.

## Important interpretation

The requested work is NOT:
> add a set of OpenAI commands.

It is:
> add a provider/host/run subsystem that composes with the current capability and projection architecture, with OpenAI as its first real adapter.

Any implementation that adds separate CLI handlers, separate HTTP routing tables, separate MCP tool tables or separate Cockpit state is structurally wrong even if it "works".

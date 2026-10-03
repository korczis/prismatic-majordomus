# Majordomus OpenAI / Provider Control Plane Prompt Pack

Repository studied: `korczis/prismatic-majordomus`
Reference branch: `master`
Reference tree SHA observed during preparation: `227377773f92fed491d5efebf86aca3bd580ad0b`

This pack is deliberately a sequence, not one enormous prompt. Feed each prompt to a fresh Claude Code / Fable session with the repository available and a large context window. Each prompt requires the model to re-read the live repository before making changes, because the repository is moving quickly and the SHA above is context, not authority.

## Intended outcome

Implement a provider/host/run control-plane architecture in Majordomus where OpenAI is the first production adapter, while preserving the repository's central invariants:

- one canonical semantic definition;
- CLI, HTTP API, OpenAPI/Swagger, MCP, Cockpit and docs are projections;
- no manually synchronized registry;
- schemas are typed and derived;
- `.ai/manifest.yaml` drives discovery; do not replace it with directory walking;
- repository context remains provider-neutral;
- `.ai/local/` remains local operational state and never becomes public/normative;
- capabilities are composed through the existing Rust capability model;
- skills remain data, not registrations;
- use cases execute and prove claims;
- generated artifacts are deterministic and checked;
- no secret material reaches logs, generated files, MCP, HTTP, Cockpit, docs or tests;
- no provider-specific semantics leak above the adapter boundary;
- do not store raw transcripts merely because importing them is easy.

## Run order

1. `00-orient-and-plan.md`
2. `01-governance-and-architecture.md`
3. `02-provider-kernel-and-model-discovery.md`
4. `03-openai-platform-adapter.md`
5. `04-run-events-streaming-and-audit.md`
6. `05-hosts-chatgpt-codex-and-skills.md`
7. `06-ideas-intake-and-chatgpt-import.md`
8. `07-projections-cli-http-openapi-mcp.md`
9. `08-cockpit-and-observability.md`
10. `09-e2e-hardening-documentation-and-finish.md`

Do not skip prompt 00. It is designed to prevent the implementation model from creating a second architecture beside the one that already exists.

## Execution discipline

For each prompt:

1. work from the live repository, not this pack's snapshot;
2. follow `AGENTS.md`, `.ai/README.md`, the effective rules and the task lifecycle;
3. use the repository's canonical worktree topology;
4. create/claim the proper issue/milestone using existing project mechanisms;
5. inspect related ADRs before design;
6. update or create ADR/rule/skill/use-case only when the repository's own semantics require it;
7. implement through canonical Rust declarations and data-driven sources;
8. run impact/coverage/generation checks;
9. run focused tests first, then repository-wide gates;
10. finish only with evidence.

Never manually edit generated `AGENTS.md`, generated capability documentation, OpenAPI output, benchmark inventories or any other projection whose producer already exists.

## Expected architectural result

The final system should resemble:

```text
                     Majordomus Core
                           |
              +------------+------------+
              |                         |
        Provider Registry           Host Registry
              |                         |
       ProviderAdapter              HostAdapter
              |                         |
     +--------+---------+       +-------+--------+
     |                  |       |                |
 OpenAI Platform   OpenAI-compatible  Codex   ChatGPT/Workspace
     |
 Model discovery
 Capability normalization
 Credential resolution
 Request execution
 Stream normalization
     |
 Canonical Run + RunEvent
     |
 Capability/application services
     |
 CLI / HTTP / OpenAPI / Swagger / MCP / Cockpit / generated docs
```

Do not treat `OpenAI`, `ChatGPT`, `Codex`, `Responses API` and `Workspace Agents` as synonyms.

## Non-goals unless live repository evidence changes them

- scraping ChatGPT web UI;
- storing full raw ChatGPT transcripts as canonical knowledge;
- inventing a daemon when the existing shared-server lifetime model is sufficient;
- separate Cockpit business logic;
- OpenAPI as source of truth;
- MCP as source of truth;
- hard-coded model catalog in Rust;
- hard-coded provider/model lists in JS;
- duplicative provider-specific CLI/HTTP/MCP implementations;
- generic "agent framework" abstractions unrelated to concrete Majordomus use cases.

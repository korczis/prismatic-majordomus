# `.ai/` protocol

## Purpose

`.ai/` is a portable repository AI context and governance layer.

It is deliberately not tied to Claude, Codex, Gemini, OpenAI, Anthropic,
Majordomus, GitHub, or a specific editor.

Majordomus implements validation/enforcement around it, but the directory must
remain human-readable and LLM-readable without Majordomus.

## Root entrypoint

`.ai/README.md` is normative protocol documentation for agents and humans.

It must explain:

- purpose and boundaries,
- tracked vs local ownership,
- section discovery,
- rule loading,
- dependency resolution,
- precedence,
- versioning,
- local-data exclusion,
- skills/knowledge loading,
- failure behavior,
- Majordomus relationship.

## Root manifest

Recommended first schema:

```yaml
schema: ai-repository/v1

repo:
  path: repo

local:
  path: local
  tracked: false
  implicit_context: false

sections:
  policy: repo/policy.yaml
  profiles: repo/profiles
  rules: repo/rules
  prompts: repo/prompts
  skills: repo/skills
  workflows: repo/workflows
  knowledge: repo/knowledge
  adrs: repo/adrs
  project: repo/project
```

Do not invent inheritance, remote imports, plugin ecosystems, or organization
federation in v1.

## Discovery protocol

Agents are instructed to:

1. read `.ai/README.md`,
2. read `.ai/manifest.yaml`,
3. load mandatory protocol-defined metadata,
4. discover registered tracked sections,
5. load baseline rules,
6. resolve rule dependencies deterministically,
7. load applicable project rules,
8. load only task-relevant skills/knowledge,
9. never implicitly load `.ai/local/**`,
10. perform substantive work only after mandatory rules resolve.

Majordomus itself must use deterministic registered paths, not `find .ai -type f`
as semantic discovery.

## Context-budget principle

Discoverability does not mean eager loading.

The protocol must preserve:

```text
discover everything that may apply
resolve applicability
load minimum sufficient effective context
```

not:

```text
recursively paste every Markdown file into the prompt
```

## Tracked/local boundary

Everything under `.ai/repo/**` is intended to be Git tracked unless a narrower
format explicitly says otherwise.

Everything under `.ai/local/**` is Git ignored.

`.gitignore` should normally contain:

```gitignore
.ai/local/
```

Do not scatter per-subdirectory ignore rules unless there is a proven need.

## Local prompt history

`.ai/local/prompts/` stores timestamped raw local USER prompts when a capable
integration can observe them.

Majordomus MUST NOT claim it captures prompts from providers it cannot observe.

Raw prompt history is:

```text
archive/evidence
NOT knowledge
NOT automatic session context
NOT automatic retrieval
```

Do not store model responses/transcripts unless a later explicit contract
changes this. The current transformation only approves raw user prompt history.

## Session contexts

`.ai/local/session-contexts/` contains bounded local session context artifacts.

They are not canonical repository knowledge and are never automatically
published.

## Knowledge

`.ai/repo/knowledge/` should primarily declare/curate knowledge, not duplicate
the repository.

Prefer:

```text
.ai/repo/knowledge/sources.yaml
```

pointing at canonical tracked sources such as docs, claims, responsibilities,
project records, and selected code/configuration.

Compiled indexes/caches belong under `.ai/local/cache/` unless a specific
tracked derived artifact is justified by a behavioral guarantee.

Knowledge discovery MUST preserve the current Git-index-driven principle.

## ADRs

`.ai/repo/adrs/` contains accepted durable repository architecture decisions.

Local execution decisions remain local until explicitly promoted.

No automatic "every decision becomes ADR" behavior.

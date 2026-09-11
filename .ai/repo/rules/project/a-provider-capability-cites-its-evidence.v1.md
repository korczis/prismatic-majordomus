---
id: project.a-provider-capability-cites-its-evidence
version: 1
kind: rule
title: A provider capability is declared once, with the citation it was verified from
description: Every provider this distribution declares says what it can do about the session lifecycle and about prompt capture, in one declaration every surface projects; every cell of it carries the evidence it was verified from; and no surface states a capability the declaration does not carry or withholds a provider the declaration names.
statement: Every declared provider carries an episode capability and a prompt capability with a non-empty citation for each; every surface that reports provider capability is a projection of that one declaration; and a provider the declaration names is reported by every such surface, never omitted.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.empty-is-not-failure@1]
tags: [providers, session, capability, evidence]
---

# Rationale

A tool that says what somebody else's product can do is making a claim about a third party,
and this repository made two of them wrong at once.

`majordomus capture status` looped over the providers with a line in an internal shell
table. That table had one line. `share/providers.yaml` declares six providers, so five of
them were not reported as unsupported — they were not reported at all, and the vocabulary's
own word for them, `unsupported`, was unreachable for any provider a person could name. A
reader asking whether Codex could capture prompts here got silence, and silence reads as
no.

The silence was also out of date. The comment above that table stated that a provider
absent from it "has no documented event that hands the person's prompt to a command before
the model runs". By 2026-09-11 both Codex CLI and Gemini CLI had shipped complete hook
systems — `SessionStart`, `SessionEnd`, a pre-compaction event and a prompt hook that fires
before the model — and this tool went on implying, by omission, that they had none.

Both failures have one cause: the declaration had no place to write down *where the answer
came from*. A capability asserted with no citation cannot be checked against the world, so
nothing checks it, so it rots quietly in whichever direction the world moved. A capability
that carries its citation can be re-verified by anybody in a minute, and a cell nobody can
cite is a cell nobody should have written.

The second half of the rule is the ordinary projection argument this repository applies
everywhere else. One declaration, every surface a reader of it: the command line, the HTTP
route, the MCP tool and resource, the generated documents and the site. A provider added by
editing six surfaces is a provider six surfaces will eventually disagree about.

# Required behaviour

`share/providers.yaml` is the one declaration. Each provider entry carries a `lifecycle:`
block with four keys:

- `episode` — `hooks` (the provider fires its own session events), `connection` (it fires
  none, and the MCP connection is the boundary), or `none` (nothing can draw one);
- `prompts` — `hook` (it hands the raw prompt to a command before the model runs) or `none`;
- `episode_evidence` and `prompts_evidence` — where each of the two was verified: a vendor
  documentation URL, or the statement that no such documentation exists together with what
  was searched for it and when.

Both evidence strings are **required and non-empty**, for every value including `none`. An
absence verified is a finding; an absence assumed is a guess wearing a finding's clothes,
and the two must not look alike.

A word outside either vocabulary fails the read of the whole file, naming it. It is never
treated as `none`.

Every surface that reports what a provider can do is a projection of this declaration and
holds no second account of it. Every surface that enumerates providers enumerates **all** of
them: a provider the declaration names is reported with its state, never omitted for being
in a state the reporter finds uninteresting.

The capability is not the wiring and is never reported as if it were. What a provider can
do comes from this declaration; what this repository has done about it is computed from the
tree; and a provider that documents an event this distribution ships no adapter for is
`unadapted` — a gap in the tool, named as one — never `unsupported`, which is a statement
about the provider.

# Failure behaviour

`scripts/provider-evidence-check` exits non-zero naming each provider, each aspect and each
reason: a missing `lifecycle` block, an empty citation, a value outside the vocabulary, or a
provider the declaration names that `majordomus capture status` does not report. The
repository's CI runs it as a gate; it is not advisory.

# Verification

`scripts/provider-evidence-check` over the committed tree, run by CI.
`test/cases/200_generic_mcp_episode.sh` proves that `capture status` reports every declared
provider with its capability and its evidence, that a provider with no adapter is
`unadapted` rather than `unsupported`, and that the whole connection lifecycle the
`connection` capability claims actually runs over the real transport.
`apps/majordomus-cli/src/share.rs`'s own suite proves that a word outside the vocabulary
fails the read rather than degrading.

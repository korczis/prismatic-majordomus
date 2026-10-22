+++
title = "A provider capability is declared once, with the citation it was verified from"
description = "A provider capability is declared once, with the citation it was verified from"
weight = 74
[extra]
kind = "rule"
slug = "project-a-provider-capability-cites-its-evidence-2"
identity = "project.a-provider-capability-cites-its-evidence@2"
status = "active"
source = ".ai/repo/rules/project/a-provider-capability-cites-its-evidence.v2.md"
+++
{% raw %}

## What changed in version 2, and why

Version 1 declared two capabilities. On 2026-10-21 the owner asked for the Claude, browser
extension and browser integration to work by itself, be described and be enforced, and a
session found that the one path that drives a page and the live mesh in one session — Claude
Code with the Claude in Chrome bridge — was declared nowhere and noticed by nothing: the
bridge was unconnected and the only way to learn it was to call a browser tool and read the
error. Whether a provider's client drives a browser beside this server is a claim about the
vendor's product of exactly the kind this rule governs, so it is a third axis with its own
citation, not a paragraph in a document. A bridge also names its native messaging host,
which is how preflight sees that a machine has it, and the step a person takes to install
it, which is what preflight says when it does not: a project cannot turn the bridge on, so
naming the step is the whole of what the repository can do (I3829).

## Rationale

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

## Required behaviour

`share/providers.yaml` is the one declaration. Each provider entry carries an `offers:`
block with six keys, and a bridge two more:

- `episode` — `hooks` (the provider fires its own session events), `connection` (it fires
  none, and the MCP connection is the boundary), or `none` (nothing can draw one);
- `prompts` — `hook` (it hands the raw prompt to a command before the model runs) or `none`;
- `browser` — `bridge` (its client drives a browser through a vendor extension whose tools
  arrive in the same session as this repository's MCP server) or `none`; a vendor's own
  browser agent that reaches only the vendor's cloud is not a bridge;
- `episode_evidence`, `prompts_evidence` and `browser_evidence` — where each was verified: a vendor
  documentation URL, or the statement that no such documentation exists together with what
  was searched for it and when.

All three evidence strings are **required and non-empty**, for every value including `none`. An
absence verified is a finding; an absence assumed is a guess wearing a finding's clothes,
and the two must not look alike.

A provider that declares `browser: bridge` also declares `browser_host`, the native messaging
host the bridge registers with a Chromium browser, and `browser_setup`, what a person does
once to install it. Preflight reports, for a provider whose client configuration is in the
checkout, whether this machine has that host, and names the step when it does not; it is a
finding, never a failure, because the step is the person's and not the repository's.

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

## Failure behaviour

`scripts/provider-evidence-check` exits non-zero naming each provider, each aspect and each
reason: a missing `offers` block, an empty citation, a value outside the vocabulary, a bridge
without its host or its setup step, or a
provider the declaration names that `majordomus capture status` does not report. The
repository's CI runs it as a gate; it is not advisory.

## Verification

`scripts/provider-evidence-check` over the committed tree, run by CI.
`test/cases/200_generic_mcp_episode.sh` proves that `capture status` reports every declared
provider with its capability and its evidence, that a provider with no adapter is
`unadapted` rather than `unsupported`, and that the whole connection lifecycle the
`connection` capability claims actually runs over the real transport.
`apps/majordomus-cli/src/share.rs`'s own suite proves that a word outside the vocabulary
fails the read rather than degrading. `apps/majordomus-cli/tests/provider_browser.rs` proves the
browser axis end to end in-process: the declaration, the refusal of an unknown word, the
preflight finding with and without the host on a laid-out home, and the providers document's
column; `test/cases/3829_a_provider_declares_its_browser_bridge.sh` holds the gate to its own
negative.
{% endraw %}

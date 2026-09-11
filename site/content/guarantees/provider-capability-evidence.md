+++
title = "What each provider can do about the session lifecycle and prompt capture is one declaration with a citation per cell, every surface projects it, and no provider the declaration names goes unreported"
description = "A tool that says what somebody else's product can do is making a claim about a third party. This repository was making two of them wrong at once."
weight = 152
[extra]
claim_id = "provider-capability-evidence"
status = "guaranteed"
source = "docs/claims/provider-capability-evidence.md"
+++
{% raw %}

## What it means

A tool that says what somebody else's product can do is making a claim about a third party. This repository was making two of them wrong at once.

`majordomus capture status` reported one of the six providers `share/providers.yaml` declares. The loop was over the providers with a line in an internal shell table, that table had one line, and the five others were not reported as unsupported — they were not reported at all. The vocabulary's own word for them, `unsupported`, was therefore unreachable for any provider a person could name, and a reader asking whether Codex could capture prompts here got silence, which reads as no.

The silence had also gone stale. By 2026-09-11 Codex CLI and Gemini CLI had both shipped complete hook systems — `SessionStart`, `SessionEnd`, a pre-compaction event and a prompt hook that fires before the model — and this tool went on implying, by omitting them, that they had none.

Now every provider carries a `lifecycle:` block: what it can do about the episode boundary (`hooks`, `connection`, `none`), what it can do about prompt capture (`hook`, `none`), and **the citation each of those two answers was verified from** — a vendor documentation URL, or the statement that no such documentation exists together with what was searched for it and when. Every surface that reports provider capability is a projection of that one declaration, and every surface that enumerates providers enumerates all of them.

## How it works

`share/providers.yaml` is the declaration. `share::ProviderDeclaration` parses it, with the two capability words deserialising straight into enums so that a word outside the vocabulary fails the read of the whole file with its path named rather than degrading to `none`. `product::ProductProvider` carries it, and from there the command line (`majordomus product providers`), `GET /api/v1/product/providers`, the tool `majordomus_providers`, the resource `majordomus://product/providers`, `docs/generated/providers.{json,yaml}` and the site's product dataset all follow with no line of their own.

`majordomus capture status` reads the same declaration and joins it with the tree, over the union of the declared providers and the adapter tables. It reports two facts per aspect and never folds them into one: the **capability**, which is the vendor's, with its citation as the reason; and the **state**, which is this checkout's. The state vocabulary grew a sixth word for the join. `unsupported` is now reserved for a provider that cannot do the thing at all; `unadapted` is the honest name for a provider that documents the event while this distribution ships no adapter for it — a gap in the tool, named as one.

## How to see it

```bash
majordomus capture status
# claude-code         hook        verified     .claude/hooks/majordomus-capture is wired, and …
# codex               hook        unadapted    the provider documents a prompt event and this
#                                              distribution ships no adapter for it —
#                                              https://learn.chatgpt.com/docs/hooks — UserPromptSubmit …
# generic:session     connection  verified     bin/majordomus-mcp is in place and a synthetic attach …
majordomus product providers                   # the EPISODE and PROMPTS columns, from the same declaration
majordomus --json capture status | jq '.providers[] | {provider, aspect, capability, state, evidence}'
scripts/provider-evidence-check                # the gate
```

## What it does not cover

**Whether a citation is true.** No gate can follow a URL and read a vendor's page, and one that tried would fail on every offline machine and every CI runner without network — the wrong trade for a blocking check (`project.blocking-checks-cheap`). What is enforced is that somebody had to write something down, which is the whole of the difference between a verified answer and a guess. Re-verification is a person's job, and the date in each citation is what tells them when it was last done.

**Adapters for Codex and Gemini.** The capability is declared and cited; the adapter is not written. Each needs a writer for a configuration format this tool does not write today — a TOML `[hooks]` table, a JSON `hooks` block — and each carries caveats an adapter must encode: Gemini does not await `SessionEnd`, its `PreCompress` cannot block, Codex's `SessionEnd.reason` is a constant `other`. `unadapted` is the standing, visible record of that gap, and it is deliberately not a silence.

## Why it exists

Both original failures had one cause: the declaration had nowhere to write down *where the answer came from*. A capability asserted with no citation cannot be checked against the world, so nothing checks it, so it rots quietly in whichever direction the world moved — and it rotted in both directions here at once, understating five providers by omission and overstating the tool's own knowledge by implication. `scripts/provider-evidence-check` refuses a declaration with a missing capability, a word outside the vocabulary, an empty citation, or a provider `capture status` does not report; `test/cases/200_generic_mcp_episode.sh` holds the gate to its own negative, because a check nobody has ever seen fail is a check nobody knows the meaning of. The decision is ADR 0043 and the rule is `project.a-provider-capability-cites-its-evidence`.
{% endraw %}

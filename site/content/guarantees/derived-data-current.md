+++
title = "A commit whose derived data is behind its canonical inputs is refused before it exists, and a repository that unwires that gate is a doctor failure"
description = "The site's data is a projection of the tree it was generated from. A commit that moves one of the generator's canonical inputs and does not regenerate leaves the projection describing a tree that no longer exists — and the publication path, which refuses exactly that, then refuses everything downstream of it. The gate moves that refusal to the moment the commit is made, in the checkout of the person who caused it."
weight = 136
[extra]
claim_id = "derived-data-current"
status = "guaranteed"
source = "docs/claims/derived-data-current.md"
+++
{% raw %}

## What it means

The site's data is a projection of the tree it was generated from. A commit that moves one of the generator's canonical inputs and does not regenerate leaves the projection describing a tree that no longer exists — and the publication path, which refuses exactly that, then refuses everything downstream of it. The gate moves that refusal to the moment the commit is made, in the checkout of the person who caused it.

## How it works

`scripts/generate-site-data` writes the hash of its canonical inputs into `site/data/generated/source.json` as `source_hash`. `scripts/pages current` hashes the same list again — the list comes from the generator itself, with `--inputs`, so there is no second list to go stale — and compares. That costs about a fifth of a second, because nothing is generated: the question is only whether generating is necessary. `.githooks/pre-commit` runs it after `doctor`, and a stale tree exits 10 with both hashes and the command that repairs it.

The gate is declared in the policy's `enforcement` list as `derived-current`, wired by `git-hook:pre-commit`, so `doctor` holds the wiring the way it holds `doctor-on-commit` and `finish-on-push`: the hook must exist, be executable, invoke the gate, and not swallow its exit code. The rule is therefore enforced twice over — the gate refuses the stale commit, and the doctrine refuses the repository that unwires the gate.

Declaring it rather than compiling it in is deliberate. Which files are canonical inputs, and where their hash is recorded, are facts about this repository and not about every installation of the tool; a validator in `lib/` would teach the shipped executable one repository's layout. The enforcement list already exists to say "this repository holds itself to this, wired here", and `doctor` already proves such entries. What was missing was that its verifier assumed the program was `majordomus`; it now reads the program from the entry, so a repository can declare gates of its own.

## How to see it

```bash
scripts/pages current                  # pages: site/data/generated is current for this tree (input hash …)
printf '\n' >> docs/DOCTRINE.md        # move a canonical input
scripts/pages current                  # exit 10: is stale — it carries …, this tree hashes to …
git commit -am "…"                     # refused by .githooks/pre-commit
majordomus doctor                      # OK wiring derived-current — wired via .githooks/pre-commit
```

## What it does not cover

It answers one question — is the committed site data current for this tree — and answers it from a hash. It does not say which artifact is stale or what changed inside it; `scripts/derive-check` does that, by running both generators and diffing, and CI and the Pages workflow still run it. The fingerprint is the cheap check that can afford to run on every commit; the exhaustive one runs where minutes are available.

It covers both halves now. `scripts/generate-site-data` owns `site/data/generated`, which the input hash above answers for; `majordomus generate` owns `site/data/registry/registry.json` and `docs/generated/artifacts.*`, which record every source file the index holds — and a source file is not a site input, so the hash does not move when one changes. The gate asks the second question with `majordomus generate --check`, the generator's own definition of staleness rather than a comparison written in the gate, and reports both. A repository without the built executable is told the registry half went unchecked rather than passed silently.

A rebase does not pass through it, and this is the gap most likely to bite. Rebasing a branch that touches a source file the registry measures leaves the tree stale, because the `merge=derived` driver resolves the derived files to ours and git then drops the branch's own regeneration commit as "patch contents already upstream" — true of its content when the trunk has regenerated the same bytes, false of the rebased tree. The rebase reports success and `generate --check` is red immediately afterwards. Nothing runs the hook during a rebase, so re-run `scripts/derive` after one and commit the result.

Checking that by hand needs the branch's own executable. `generate --check` run with a binary built from another worktree reports artifacts stale that are not, because the generator producing the comparison comes from a different tree, and the answer reads exactly like a real finding.

It is a local gate, so it binds whoever has the hook installed (`git config core.hooksPath .githooks`). A merge made through the forge does not pass through it; the CI `site` job and the Pages workflow remain the backstop there.

## Why it exists

On 2026-09-06 the publication path refused master three times in one hour. Each refusal was caused by a merge that moved canonical inputs without regenerating, and each was discovered by the next person to land, who then had to run a generation they had not caused and could not shorten — twice being overtaken by another landing while it ran. The check was never the difficulty; `pages current` already existed and takes a fifth of a second. What was missing was the moment it ran. A gate that only speaks after the merge bills the wrong person, and a rule whose own text says a reviewer decides it is a rule that nothing decides.
{% endraw %}

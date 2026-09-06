# A commit whose derived data is behind its canonical inputs is refused before it exists

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

It covers the site data. `majordomus generate`'s own projections — the CLI reference, the registry, the provider bootstraps — are held by `generate --check`, which `derive-check` and the Rust gate run.

It is a local gate, so it binds whoever has the hook installed (`git config core.hooksPath .githooks`). A merge made through the forge does not pass through it; the CI `site` job and the Pages workflow remain the backstop there.

## Why it exists

On 2026-09-06 the publication path refused master three times in one hour. Each refusal was caused by a merge that moved canonical inputs without regenerating, and each was discovered by the next person to land, who then had to run a generation they had not caused and could not shorten — twice being overtaken by another landing while it ran. The check was never the difficulty; `pages current` already existed and takes a fifth of a second. What was missing was the moment it ran. A gate that only speaks after the merge bills the wrong person, and a rule whose own text says a reviewer decides it is a rule that nothing decides.

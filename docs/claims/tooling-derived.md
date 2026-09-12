# The definition of done reaches every generated bootstrap as a fragment both renderers produce byte for byte

## What it means

The instruction file each AI client reads — `AGENTS.md`, `CLAUDE.md`, every projection the
policy declares — is generated from the policy and the shipped declarations, stamped with
the hash of what produced it, and refused when edited by hand or when its source has
moved. What those files say about the lifecycle is not prose somebody typed: the
definition of done is `COMPLETION_CONTRACT`, one plain list item per stage of
`share/completion.yaml` with the questions that belong to it, rendered by the shell tool
(`mj_completion_fragment`) and by the executable (`CompletionPolicy::bootstrap_fragment`)
to the same bytes.

A stage or a question added to the policy appears in every bootstrap on the next render. A
model that edits `AGENTS.md` has not changed policy; a policy changes in the source, under
review, and the projections follow.

## How it works

`majordomus update` and `majordomus generate providers` render the same bytes from the same
files, and neither reads the other. `majordomus doctor` refuses a projection whose content
no longer matches its stamp (a hand edit); `majordomus generate --check` refuses one whose
source has moved (a stale render); regeneration is the remedy in both cases. The fragment
is plain list items on purpose — `doctor` refuses a bootstrap carrying rule bullets of its
own, and this is a projection of the policy, not a rule corpus.

## Why it is derived from the policy

A definition of done that lives in a prompt is unversioned, unenforceable and true for
exactly one tool. Rendering it from `share/completion.yaml` means the file every validator
reads is the file every agent reads, and a change to it is a reviewed change to a source,
never an edit to a projection.

## What it does not cover

The prose around the fragment is still authored, in the templates under
`share/providers/` and `.ai/repo/providers/`; only the definition of done is derived. A
client whose instruction file the policy declares no projection for reads nothing generated
at all, and `scripts/ci/providers-check` is what says which clients those are.

## How to see it

`test/cases/282_tooling_is_derived.sh` proves the fragment reaches the rendered bootstrap,
that the shell renderer and the executable agree byte for byte, that a change to the
policy's stages makes the committed render stale until regenerated, and that a hand edit is
refused. `scripts/ci/providers-check` holds the declarations and the templates together. The
rule is `project.tooling-is-derived`; the decision is ADR 0057.
